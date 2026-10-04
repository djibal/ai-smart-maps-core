//! Shortest path on a graph.
//!
//! This module does not import a map adapter and does not snap a coordinate.
//! Cost is `weight + 3 * hazard`.
//! A missing hazard counts as 0. Equal costs break toward the
//! lexicographically smaller edge-id sequence.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::time::{Duration, Instant};

use crate::confidence::{Confidence, Environment, RouteNovelty};
use crate::graph::{Constraint, Edge, Graph};
use crate::log::{DiagnosticLog, ErrorCode};

pub const INITIAL_ROUTE_LIMIT: Duration = Duration::from_secs(2);
pub const REROUTE_LIMIT: Duration = Duration::from_secs(1);

/// The hard limit is inclusive. A longer duration is the failure line.
pub fn within_hard_limit(limit: Duration, elapsed: Duration) -> bool {
    elapsed <= limit
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub edge_ids: Vec<String>,
    pub model_version_id: String,
    pub confidence: Confidence,
}

#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub origin: &'a str,
    pub destination: &'a str,
    pub tile_id: &'a str,
    pub now: &'a str,
    pub model_version_id: &'a str,
    pub map_age_days: f64,
    pub report_count: u32,
    pub rolled_back: bool,
    pub route_novelty: RouteNovelty,
    pub environment: Environment,
}

#[derive(Default)]
pub struct Router {
    cache: HashMap<(String, String, String), Route>,
    log: DiagnosticLog,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn diagnostics(&self) -> &DiagnosticLog {
        &self.log
    }

    /// Returns the stored route when the origin, destination, and tile id
    /// match. Otherwise computes one. An unreachable destination returns
    /// `None` and is not cached. A result past [`INITIAL_ROUTE_LIMIT`] is
    /// not returned and is not cached. Either outcome is written to the
    /// diagnostic log as edge ids, a duration, and an error code.
    pub fn route(&mut self, graph: &Graph, request: &Request<'_>) -> Option<Route> {
        self.route_within(graph, request, INITIAL_ROUTE_LIMIT)
    }

    /// Same as [`Self::route`], with the 1 second reroute hard limit.
    pub fn reroute(&mut self, graph: &Graph, request: &Request<'_>) -> Option<Route> {
        self.route_within(graph, request, REROUTE_LIMIT)
    }

    pub fn route_within(
        &mut self,
        graph: &Graph,
        request: &Request<'_>,
        limit: Duration,
    ) -> Option<Route> {
        let started = Instant::now();
        let key = (
            request.origin.to_string(),
            request.destination.to_string(),
            request.tile_id.to_string(),
        );
        if let Some(stored) = self.cache.get(&key) {
            let found = stored.clone();
            return self.finish(limit, started.elapsed(), Some(found), None);
        }
        let Some(found) = compute(graph, request) else {
            self.finish(limit, started.elapsed(), None, Some(ErrorCode::Unreachable));
            return None;
        };
        let elapsed = started.elapsed();
        if !within_hard_limit(limit, elapsed) {
            self.log
                .record(&found.edge_ids, elapsed, Some(ErrorCode::TooSlow));
            return None;
        }
        self.cache.insert(key, found.clone());
        self.log.record(&found.edge_ids, elapsed, None);
        Some(found)
    }

    fn finish(
        &mut self,
        limit: Duration,
        elapsed: Duration,
        found: Option<Route>,
        error_code: Option<ErrorCode>,
    ) -> Option<Route> {
        if !within_hard_limit(limit, elapsed) {
            let edge_ids = found
                .as_ref()
                .map(|route| route.edge_ids.as_slice())
                .unwrap_or(&[]);
            self.log.record(edge_ids, elapsed, Some(ErrorCode::TooSlow));
            return None;
        }
        if let Some(route) = found {
            self.log.record(&route.edge_ids, elapsed, None);
            return Some(route);
        }
        self.log.record(&[], elapsed, error_code);
        None
    }
}

fn compute(graph: &Graph, request: &Request<'_>) -> Option<Route> {
    if !graph.has_node(request.origin) || !graph.has_node(request.destination) {
        return None;
    }
    let confidence = Confidence::interim(
        request.map_age_days,
        request.report_count,
        request.model_version_id,
        request.rolled_back,
        request.route_novelty,
        request.environment,
    );
    let allow = |edge: &Edge| edge.traversable_at(request.now);
    let mut edge_ids = shortest(graph, request.origin, request.destination, allow)?;
    if confidence.warns() {
        let safe = |edge: &Edge| {
            edge.constraint == Constraint::Open
                && edge.hazard_or_zero() == 0.0
                && edge.traversable_at(request.now)
        };
        if let Some(replacement) = shortest(graph, request.origin, request.destination, safe) {
            edge_ids = replacement;
        }
    }
    if request.origin != request.destination && edge_ids.is_empty() {
        return None;
    }
    Some(Route {
        edge_ids,
        model_version_id: request.model_version_id.to_string(),
        confidence,
    })
}

fn shortest<F>(graph: &Graph, origin: &str, destination: &str, allow: F) -> Option<Vec<String>>
where
    F: Fn(&Edge) -> bool,
{
    if origin == destination {
        return Some(Vec::new());
    }
    let mut outgoing: HashMap<&str, Vec<&Edge>> = HashMap::new();
    for edge in &graph.edges {
        if allow(edge) {
            outgoing.entry(edge.src.as_str()).or_default().push(edge);
        }
    }

    let mut best: HashMap<String, (f64, Vec<String>)> = HashMap::new();
    let mut heap = BinaryHeap::new();
    best.insert(origin.to_string(), (0.0, Vec::new()));
    heap.push(State {
        cost: 0.0,
        path: Vec::new(),
        node: origin.to_string(),
    });

    while let Some(state) = heap.pop() {
        match best.get(&state.node) {
            Some((cost, path))
                if state.cost > *cost || (state.cost == *cost && state.path != *path) =>
            {
                continue;
            }
            None => continue,
            Some(_) => {}
        }
        if state.node == destination {
            return Some(state.path);
        }
        let Some(edges) = outgoing.get(state.node.as_str()) else {
            continue;
        };
        for edge in edges {
            let next_cost = state.cost + edge.cost();
            let mut next_path = state.path.clone();
            next_path.push(edge.id.clone());
            let replace = match best.get(&edge.dst) {
                None => true,
                Some((cost, path)) => {
                    next_cost < *cost
                        || (next_cost == *cost && next_path.as_slice() < path.as_slice())
                }
            };
            if replace {
                best.insert(edge.dst.clone(), (next_cost, next_path.clone()));
                heap.push(State {
                    cost: next_cost,
                    path: next_path,
                    node: edge.dst.clone(),
                });
            }
        }
    }
    None
}

struct State {
    cost: f64,
    path: Vec<String>,
    node: String,
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for State {}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.cost.total_cmp(&self.cost) {
            Ordering::Equal => other.path.cmp(&self.path),
            ordering => ordering,
        }
    }
}
