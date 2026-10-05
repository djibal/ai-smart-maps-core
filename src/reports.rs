//! Local reports. One device can update its own edge. Several stored
//! reports on the same edge are reduced with reputation weighting.
//! A reporter key is a device-local identifier, not an account.

use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use crate::graph::{Constraint, Graph};
use crate::log::{DiagnosticLog, ErrorCode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Closure,
    Hazard,
    Clear,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Observation {
    pub id: String,
    pub observed_at: String,
    pub edge_id: String,
    pub kind: Kind,
    pub detail: Option<String>,
    pub reporter_key: String,
}

impl fmt::Debug for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observation")
            .field("id", &self.id)
            .field("observed_at", &self.observed_at)
            .field("edge_id", &self.edge_id)
            .field("kind", &self.kind)
            .field("detail", &self.detail)
            .field("reporter_key", &"redacted")
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Malformed;

/// Length-prefixed bytes for the sealed cache. This is a local record, not
/// a contract: the reporter key travels only inside the sealed store.
pub fn encode_observation(observation: &Observation) -> Vec<u8> {
    let mut out = Vec::new();
    for field in [
        observation.id.as_str(),
        observation.observed_at.as_str(),
        observation.edge_id.as_str(),
        observation.reporter_key.as_str(),
        observation.detail.as_deref().unwrap_or(""),
    ] {
        out.extend((field.len() as u32).to_be_bytes());
        out.extend(field.as_bytes());
    }
    out.push(match observation.kind {
        Kind::Closure => 0,
        Kind::Hazard => 1,
        Kind::Clear => 2,
    });
    out.push(u8::from(observation.detail.is_some()));
    out
}

pub fn decode_observation(bytes: &[u8]) -> Result<Observation, Malformed> {
    let mut rest = bytes;
    let mut fields = Vec::with_capacity(5);
    for _ in 0..5 {
        let (len, after) = rest.split_at_checked(4).ok_or(Malformed)?;
        let len = u32::from_be_bytes(len.try_into().map_err(|_| Malformed)?) as usize;
        let (field, after) = after.split_at_checked(len).ok_or(Malformed)?;
        fields.push(String::from_utf8(field.to_vec()).map_err(|_| Malformed)?);
        rest = after;
    }
    let [kind, has_detail] = rest else {
        return Err(Malformed);
    };
    let kind = match kind {
        0 => Kind::Closure,
        1 => Kind::Hazard,
        2 => Kind::Clear,
        _ => return Err(Malformed),
    };
    let mut fields = fields.into_iter();
    let id = fields.next().ok_or(Malformed)?;
    let observed_at = fields.next().ok_or(Malformed)?;
    let edge_id = fields.next().ok_or(Malformed)?;
    let reporter_key = fields.next().ok_or(Malformed)?;
    let detail = fields.next().ok_or(Malformed)?;
    Ok(Observation {
        id,
        observed_at,
        edge_id,
        kind,
        detail: match has_detail {
            0 => None,
            1 => Some(detail),
            _ => return Err(Malformed),
        },
        reporter_key,
    })
}

/// A stored report plus the age, in days, supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgedReport {
    pub observation: Observation,
    pub age_days: u32,
}

/// At 90 days the detail is dropped and the report stays. After 90 days
/// the report is deleted. A younger report keeps its detail.
pub fn retain(reports: Vec<AgedReport>) -> Vec<Observation> {
    reports
        .into_iter()
        .filter_map(|report| {
            if report.age_days > 90 {
                None
            } else if report.age_days == 90 {
                let mut observation = report.observation;
                observation.detail = None;
                Some(observation)
            } else {
                Some(report.observation)
            }
        })
        .collect()
}

/// Applies every observation to the matching edge.
/// Reports for an unknown edge are ignored.
pub fn apply(graph: &mut Graph, observations: &[Observation]) {
    let mut log = DiagnosticLog::new();
    apply_logged(graph, observations, &mut log);
}

/// Same as [`apply`], and writes edge ids, the duration, and `unknown_edge`
/// when an observation names an edge that is not in the graph. The reporter
/// key and the detail text are not written.
pub fn apply_logged(graph: &mut Graph, observations: &[Observation], log: &mut DiagnosticLog) {
    let started = Instant::now();
    let known_ids: Vec<String> = observations
        .iter()
        .filter(|observation| {
            graph
                .edges
                .iter()
                .any(|edge| edge.id == observation.edge_id)
        })
        .map(|observation| observation.edge_id.clone())
        .collect();
    let unknown_ids: Vec<String> = observations
        .iter()
        .filter(|observation| {
            !graph
                .edges
                .iter()
                .any(|edge| edge.id == observation.edge_id)
        })
        .map(|observation| observation.edge_id.clone())
        .collect();
    apply_groups(graph, observations);
    let elapsed = started.elapsed();
    if !known_ids.is_empty() || unknown_ids.is_empty() {
        log.record(&known_ids, elapsed, None);
    }
    if !unknown_ids.is_empty() {
        log.record(&unknown_ids, elapsed, Some(ErrorCode::UnknownEdge));
    }
}

fn apply_groups(graph: &mut Graph, observations: &[Observation]) {
    let mut by_edge: HashMap<&str, Vec<&Observation>> = HashMap::new();
    for observation in observations {
        by_edge
            .entry(observation.edge_id.as_str())
            .or_default()
            .push(observation);
    }
    for edge in &mut graph.edges {
        let Some(group) = by_edge.get(edge.id.as_str()) else {
            continue;
        };
        let (kind, reputations) = consensus(group);
        match kind {
            Kind::Closure => edge.constraint = Constraint::Closed,
            Kind::Hazard => edge.hazard = Some(share(group, &reputations, Kind::Hazard)),
            Kind::Clear => {
                edge.hazard = Some(0.0);
                edge.constraint = Constraint::Open;
            }
        }
    }
}

fn consensus(group: &[&Observation]) -> (Kind, Vec<f64>) {
    let mut reputations: Vec<f64> = vec![1.0; group.len()];
    let mut kind = Kind::Clear;
    for _ in 0..3 {
        kind = winning_kind(group, &reputations);
        for (index, observation) in group.iter().enumerate() {
            if observation.kind == kind {
                reputations[index] *= 1.5;
            } else {
                reputations[index] *= 0.5;
            }
        }
        let mean = reputations.iter().sum::<f64>() / reputations.len() as f64;
        if mean > 0.0 {
            for reputation in &mut reputations {
                *reputation /= mean;
            }
        }
    }
    (kind, reputations)
}

fn share(group: &[&Observation], reputations: &[f64], kind: Kind) -> f64 {
    let total: f64 = reputations.iter().sum();
    if total == 0.0 {
        return 0.0;
    }
    let matching: f64 = group
        .iter()
        .zip(reputations.iter())
        .filter(|(observation, _)| observation.kind == kind)
        .map(|(_, reputation)| *reputation)
        .sum();
    matching / total
}

fn winning_kind(group: &[&Observation], reputations: &[f64]) -> Kind {
    let mut best = Kind::Closure;
    let mut best_total = -1.0;
    for kind in [Kind::Closure, Kind::Hazard, Kind::Clear] {
        let total: f64 = group
            .iter()
            .zip(reputations.iter())
            .filter(|(observation, _)| observation.kind == kind)
            .map(|(_, reputation)| *reputation)
            .sum();
        if total > best_total {
            best_total = total;
            best = kind;
        }
    }
    best
}
