//! Scorer host. It extracts the four route features and decides which
//! artifact bytes are allowed to run. It does not choose the route.
//! The deterministic cost remains the choice.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::graph::Graph;
use crate::log::{DiagnosticLog, ErrorCode};

pub const INFERENCE_LIMIT: Duration = Duration::from_millis(500);

/// The hard limit is inclusive. A longer duration is the failure line.
pub fn within_inference_limit(elapsed: Duration) -> bool {
    elapsed <= INFERENCE_LIMIT
}

#[derive(Clone, Debug, PartialEq)]
pub struct Features {
    pub edge_count: u32,
    pub weight_sum: f64,
    pub hazard_sum: f64,
    pub hazard_missing: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownEdge;

pub fn features(graph: &Graph, edge_ids: &[String]) -> Result<Features, UnknownEdge> {
    let mut weight_sum = 0.0;
    let mut hazard_sum = 0.0;
    let mut hazard_missing = 0u32;
    for id in edge_ids {
        let edge = graph
            .edges
            .iter()
            .find(|edge| edge.id == *id)
            .ok_or(UnknownEdge)?;
        weight_sum += edge.weight;
        match edge.hazard {
            Some(hazard) => hazard_sum += hazard,
            None => hazard_missing += 1,
        }
    }
    Ok(Features {
        edge_count: u32::try_from(edge_ids.len()).unwrap_or(u32::MAX),
        weight_sum,
        hazard_sum,
        hazard_missing,
    })
}

/// Keeps the cheaper edge sequence. An equal cost keeps the
/// lexicographically smaller sequence. A scorer ranking is not an input.
pub fn deterministic_choice<'a>(
    left_edges: &'a [String],
    left_cost: f64,
    right_edges: &'a [String],
    right_cost: f64,
) -> &'a [String] {
    if left_cost < right_cost || (left_cost == right_cost && left_edges < right_edges) {
        left_edges
    } else {
        right_edges
    }
}

#[derive(Default)]
pub struct Host {
    bytes_for_version: HashMap<String, Vec<u8>>,
    log: DiagnosticLog,
}

impl Host {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diagnostics(&self) -> &DiagnosticLog {
        &self.log
    }

    pub fn install(&mut self, version_id: &str, bytes: &[u8]) {
        self.bytes_for_version
            .insert(version_id.to_string(), bytes.to_vec());
    }

    /// Drops every installed artifact. The log stays.
    pub fn uninstall_all(&mut self) {
        self.bytes_for_version.clear();
    }

    /// Returns the version whose stored bytes match `presented`.
    /// A mismatch refuses the claimed version and returns `previous_id`
    /// when that version is installed. Otherwise it refuses to score.
    pub fn accepted_version(
        &self,
        claimed_id: &str,
        presented: &[u8],
        previous_id: Option<&str>,
    ) -> Option<String> {
        if self.bytes_for_version.get(claimed_id).map(Vec::as_slice) == Some(presented) {
            return Some(claimed_id.to_string());
        }
        let previous = previous_id?;
        self.bytes_for_version
            .get(previous)
            .map(|_| previous.to_string())
    }

    /// Runs the accepted artifact on the four features. A mismatched claim
    /// runs the previous version's stored bytes, never the presented bytes.
    /// Inference past [`INFERENCE_LIMIT`] returns [`ScoreError::TooSlow`].
    pub fn score(
        &mut self,
        claimed_id: &str,
        presented: &[u8],
        previous_id: Option<&str>,
        features: &Features,
    ) -> Result<Prediction, ScoreError> {
        self.score_within(
            claimed_id,
            presented,
            previous_id,
            features,
            INFERENCE_LIMIT,
        )
    }

    pub fn score_within(
        &mut self,
        claimed_id: &str,
        presented: &[u8],
        previous_id: Option<&str>,
        features: &Features,
        limit: Duration,
    ) -> Result<Prediction, ScoreError> {
        let started = Instant::now();
        let result = self.score_inner(claimed_id, presented, previous_id, features, limit);
        let code = match &result {
            Ok(_) => None,
            Err(ScoreError::Refused) => Some(ErrorCode::Refused),
            Err(ScoreError::InvalidArtifact) => Some(ErrorCode::InvalidArtifact),
            Err(ScoreError::TooSlow) => Some(ErrorCode::TooSlow),
        };
        self.log.record(&[], started.elapsed(), code);
        result
    }

    fn score_inner(
        &self,
        claimed_id: &str,
        presented: &[u8],
        previous_id: Option<&str>,
        features: &Features,
        limit: Duration,
    ) -> Result<Prediction, ScoreError> {
        let version_id = self
            .accepted_version(claimed_id, presented, previous_id)
            .ok_or(ScoreError::Refused)?;
        let bytes = self
            .bytes_for_version
            .get(&version_id)
            .ok_or(ScoreError::Refused)?;
        let started = Instant::now();
        let value = infer(bytes, features)?;
        if started.elapsed() > limit {
            return Err(ScoreError::TooSlow);
        }
        Ok(Prediction { version_id, value })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prediction {
    pub version_id: String,
    pub value: f32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScoreError {
    Refused,
    InvalidArtifact,
    TooSlow,
}

fn infer(bytes: &[u8], features: &Features) -> Result<f32, ScoreError> {
    use tract_onnx::prelude::*;

    let model = tract_onnx::onnx()
        .model_for_read(&mut std::io::Cursor::new(bytes))
        .map_err(|_| ScoreError::InvalidArtifact)?
        .into_optimized()
        .map_err(|_| ScoreError::InvalidArtifact)?
        .into_runnable()
        .map_err(|_| ScoreError::InvalidArtifact)?;
    let input = tract_ndarray::Array2::from_shape_vec(
        (1, 4),
        vec![
            features.edge_count as f32,
            features.weight_sum as f32,
            features.hazard_sum as f32,
            features.hazard_missing as f32,
        ],
    )
    .map_err(|_| ScoreError::InvalidArtifact)?;
    let outputs = model
        .run(tvec!(Tensor::from(input).into()))
        .map_err(|_| ScoreError::InvalidArtifact)?;
    let view = outputs[0]
        .to_plain_array_view::<f32>()
        .map_err(|_| ScoreError::InvalidArtifact)?;
    view.iter()
        .copied()
        .next()
        .ok_or(ScoreError::InvalidArtifact)
}
