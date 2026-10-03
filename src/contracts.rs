//! Protobuf boundary. Field names and enum strings match the published schemas.
//! The reporter key stays off this contract.

use prost::Message;

use crate::confidence::{Confidence, Environment, RouteNovelty};
use crate::graph::{Constraint, Edge, Graph, Node, Source};
use crate::reports::Kind;
use crate::router::Route;

mod wire {
    include!(concat!(env!("OUT_DIR"), "/ai_smart_maps.rs"));
}

#[derive(Debug, PartialEq, Eq)]
pub struct ContractError {
    pub field: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportRecord {
    pub id: String,
    pub observed_at: String,
    pub edge_id: String,
    pub kind: Kind,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileRecord {
    pub id: String,
    pub observed_at: String,
    pub graph: Graph,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelVersionRecord {
    pub id: String,
    pub artifact: String,
    pub created_at: String,
    pub previous_id: Option<String>,
}

pub fn encode_graph(graph: &Graph) -> Result<Vec<u8>, ContractError> {
    let message = graph_to_wire(graph)?;
    Ok(message.encode_to_vec())
}

pub fn decode_graph(bytes: &[u8]) -> Result<Graph, ContractError> {
    let message = wire::Graph::decode(bytes).map_err(|_| ContractError { field: "graph" })?;
    graph_from_wire(message)
}

pub fn encode_tile(tile: &TileRecord) -> Result<Vec<u8>, ContractError> {
    let message = wire::Tile {
        id: required(tile.id.clone(), "id")?,
        observed_at: required(tile.observed_at.clone(), "observed_at")?,
        graph: Some(graph_to_wire(&tile.graph)?),
        signature: optional_text(tile.signature.clone(), "signature")?,
    };
    Ok(message.encode_to_vec())
}

pub fn decode_tile(bytes: &[u8]) -> Result<TileRecord, ContractError> {
    let message = wire::Tile::decode(bytes).map_err(|_| ContractError { field: "tile" })?;
    Ok(TileRecord {
        id: required(message.id, "id")?,
        observed_at: required(message.observed_at, "observed_at")?,
        graph: graph_from_wire(message.graph.ok_or(ContractError { field: "graph" })?)?,
        signature: optional_text(message.signature, "signature")?,
    })
}

pub fn encode_report(report: &ReportRecord) -> Result<Vec<u8>, ContractError> {
    let message = wire::Report {
        id: required(report.id.clone(), "id")?,
        observed_at: required(report.observed_at.clone(), "observed_at")?,
        edge_id: required(report.edge_id.clone(), "edge_id")?,
        kind: kind_to_wire(report.kind).to_string(),
        detail: match &report.detail {
            None => None,
            Some(text) if text.len() <= 280 => Some(text.clone()),
            Some(_) => return Err(ContractError { field: "detail" }),
        },
    };
    Ok(message.encode_to_vec())
}

pub fn decode_report(bytes: &[u8]) -> Result<ReportRecord, ContractError> {
    let message = wire::Report::decode(bytes).map_err(|_| ContractError { field: "report" })?;
    let detail = match message.detail {
        None => None,
        Some(text) if text.len() <= 280 => Some(text),
        Some(_) => return Err(ContractError { field: "detail" }),
    };
    Ok(ReportRecord {
        id: required(message.id, "id")?,
        observed_at: required(message.observed_at, "observed_at")?,
        edge_id: required(message.edge_id, "edge_id")?,
        kind: kind_from_wire(&message.kind)?,
        detail,
    })
}

pub fn encode_confidence(confidence: &Confidence) -> Result<Vec<u8>, ContractError> {
    Ok(confidence_to_wire(confidence)?.encode_to_vec())
}

pub fn decode_confidence(bytes: &[u8]) -> Result<Confidence, ContractError> {
    let message = wire::Confidence::decode(bytes).map_err(|_| ContractError {
        field: "confidence",
    })?;
    confidence_from_wire(message)
}

pub fn encode_model_version(model: &ModelVersionRecord) -> Result<Vec<u8>, ContractError> {
    let message = wire::ModelVersion {
        id: required(model.id.clone(), "id")?,
        role: "scorer".to_string(),
        artifact: required(model.artifact.clone(), "artifact")?,
        created_at: required(model.created_at.clone(), "created_at")?,
        previous_id: optional_text(model.previous_id.clone(), "previous_id")?,
    };
    Ok(message.encode_to_vec())
}

pub fn decode_model_version(bytes: &[u8]) -> Result<ModelVersionRecord, ContractError> {
    let message = wire::ModelVersion::decode(bytes).map_err(|_| ContractError {
        field: "model_version",
    })?;
    if message.role != "scorer" {
        return Err(ContractError { field: "role" });
    }
    Ok(ModelVersionRecord {
        id: required(message.id, "id")?,
        artifact: required(message.artifact, "artifact")?,
        created_at: required(message.created_at, "created_at")?,
        previous_id: optional_text(message.previous_id, "previous_id")?,
    })
}

pub fn encode_route(route: &Route) -> Result<Vec<u8>, ContractError> {
    if route.edge_ids.is_empty() {
        return Err(ContractError { field: "edge_ids" });
    }
    let message = wire::Route {
        edge_ids: route
            .edge_ids
            .iter()
            .map(|id| required(id.clone(), "edge_ids"))
            .collect::<Result<Vec<_>, _>>()?,
        model_version_id: required(route.model_version_id.clone(), "model_version_id")?,
        confidence: Some(confidence_to_wire(&route.confidence)?),
    };
    Ok(message.encode_to_vec())
}

pub fn decode_route(bytes: &[u8]) -> Result<Route, ContractError> {
    let message = wire::Route::decode(bytes).map_err(|_| ContractError { field: "route" })?;
    if message.edge_ids.is_empty() {
        return Err(ContractError { field: "edge_ids" });
    }
    Ok(Route {
        edge_ids: message
            .edge_ids
            .into_iter()
            .map(|id| required(id, "edge_ids"))
            .collect::<Result<Vec<_>, _>>()?,
        model_version_id: required(message.model_version_id, "model_version_id")?,
        confidence: confidence_from_wire(message.confidence.ok_or(ContractError {
            field: "confidence",
        })?)?,
    })
}

fn graph_to_wire(graph: &Graph) -> Result<wire::Graph, ContractError> {
    Ok(wire::Graph {
        nodes: graph
            .nodes
            .iter()
            .map(node_to_wire)
            .collect::<Result<Vec<_>, _>>()?,
        edges: graph
            .edges
            .iter()
            .map(edge_to_wire)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn graph_from_wire(message: wire::Graph) -> Result<Graph, ContractError> {
    Ok(Graph {
        nodes: message
            .nodes
            .into_iter()
            .map(node_from_wire)
            .collect::<Result<Vec<_>, _>>()?,
        edges: message
            .edges
            .into_iter()
            .map(edge_from_wire)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn node_to_wire(node: &Node) -> Result<wire::Node, ContractError> {
    if !(-90.0..=90.0).contains(&node.lat) {
        return Err(ContractError { field: "lat" });
    }
    if !(-180.0..=180.0).contains(&node.lon) {
        return Err(ContractError { field: "lon" });
    }
    Ok(wire::Node {
        id: required(node.id.clone(), "id")?,
        lat: node.lat,
        lon: node.lon,
    })
}

fn node_from_wire(message: wire::Node) -> Result<Node, ContractError> {
    if !(-90.0..=90.0).contains(&message.lat) {
        return Err(ContractError { field: "lat" });
    }
    if !(-180.0..=180.0).contains(&message.lon) {
        return Err(ContractError { field: "lon" });
    }
    Ok(Node {
        id: required(message.id, "id")?,
        lat: message.lat,
        lon: message.lon,
    })
}

fn edge_to_wire(edge: &Edge) -> Result<wire::Edge, ContractError> {
    if edge.weight < 0.0 {
        return Err(ContractError { field: "weight" });
    }
    if let Some(hazard) = edge.hazard {
        if !(0.0..=1.0).contains(&hazard) {
            return Err(ContractError { field: "hazard" });
        }
    }
    Ok(wire::Edge {
        id: required(edge.id.clone(), "id")?,
        src: required(edge.src.clone(), "src")?,
        dst: required(edge.dst.clone(), "dst")?,
        weight: edge.weight,
        constraint: constraint_to_wire(edge.constraint).to_string(),
        source: source_to_wire(edge.source).to_string(),
        hazard: edge.hazard,
        valid_from: optional_text(edge.valid_from.clone(), "valid_from")?,
        valid_to: optional_text(edge.valid_to.clone(), "valid_to")?,
    })
}

fn edge_from_wire(message: wire::Edge) -> Result<Edge, ContractError> {
    if message.weight < 0.0 {
        return Err(ContractError { field: "weight" });
    }
    if let Some(hazard) = message.hazard {
        if !(0.0..=1.0).contains(&hazard) {
            return Err(ContractError { field: "hazard" });
        }
    }
    Ok(Edge {
        id: required(message.id, "id")?,
        src: required(message.src, "src")?,
        dst: required(message.dst, "dst")?,
        weight: message.weight,
        constraint: constraint_from_wire(&message.constraint)?,
        source: source_from_wire(&message.source)?,
        hazard: message.hazard,
        valid_from: optional_text(message.valid_from, "valid_from")?,
        valid_to: optional_text(message.valid_to, "valid_to")?,
    })
}

fn confidence_to_wire(confidence: &Confidence) -> Result<wire::Confidence, ContractError> {
    if !(0.0..=1.0).contains(&confidence.score) {
        return Err(ContractError { field: "score" });
    }
    if confidence.map_age_days < 0.0 {
        return Err(ContractError {
            field: "map_age_days",
        });
    }
    Ok(wire::Confidence {
        score: confidence.score,
        map_age_days: confidence.map_age_days,
        report_count: confidence.report_count,
        model_version_id: required(confidence.model_version_id.clone(), "model_version_id")?,
        route_novelty: novelty_to_wire(confidence.route_novelty).to_string(),
        environment: environment_to_wire(confidence.environment).to_string(),
    })
}

fn confidence_from_wire(message: wire::Confidence) -> Result<Confidence, ContractError> {
    if !(0.0..=1.0).contains(&message.score) {
        return Err(ContractError { field: "score" });
    }
    if message.map_age_days < 0.0 {
        return Err(ContractError {
            field: "map_age_days",
        });
    }
    Ok(Confidence {
        score: message.score,
        map_age_days: message.map_age_days,
        report_count: message.report_count,
        model_version_id: required(message.model_version_id, "model_version_id")?,
        route_novelty: novelty_from_wire(&message.route_novelty)?,
        environment: environment_from_wire(&message.environment)?,
    })
}

fn required(value: String, field: &'static str) -> Result<String, ContractError> {
    if value.is_empty() {
        Err(ContractError { field })
    } else {
        Ok(value)
    }
}

fn optional_text(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, ContractError> {
    match value {
        None => Ok(None),
        Some(text) if !text.is_empty() => Ok(Some(text)),
        Some(_) => Err(ContractError { field }),
    }
}

fn constraint_to_wire(value: Constraint) -> &'static str {
    match value {
        Constraint::Open => "open",
        Constraint::Closed => "closed",
    }
}

fn constraint_from_wire(value: &str) -> Result<Constraint, ContractError> {
    match value {
        "open" => Ok(Constraint::Open),
        "closed" => Ok(Constraint::Closed),
        _ => Err(ContractError {
            field: "constraint",
        }),
    }
}

fn source_to_wire(value: Source) -> &'static str {
    match value {
        Source::Osm => "osm",
        Source::Commercial => "commercial",
        Source::Custom => "custom",
        Source::Realtime => "realtime",
    }
}

fn source_from_wire(value: &str) -> Result<Source, ContractError> {
    match value {
        "osm" => Ok(Source::Osm),
        "commercial" => Ok(Source::Commercial),
        "custom" => Ok(Source::Custom),
        "realtime" => Ok(Source::Realtime),
        _ => Err(ContractError { field: "source" }),
    }
}

fn kind_to_wire(value: Kind) -> &'static str {
    match value {
        Kind::Hazard => "hazard",
        Kind::Closure => "closure",
        Kind::Clear => "clear",
    }
}

fn kind_from_wire(value: &str) -> Result<Kind, ContractError> {
    match value {
        "hazard" => Ok(Kind::Hazard),
        "closure" => Ok(Kind::Closure),
        "clear" => Ok(Kind::Clear),
        _ => Err(ContractError { field: "kind" }),
    }
}

fn novelty_to_wire(value: RouteNovelty) -> &'static str {
    match value {
        RouteNovelty::Known => "known",
        RouteNovelty::Untraveled => "untraveled",
    }
}

fn novelty_from_wire(value: &str) -> Result<RouteNovelty, ContractError> {
    match value {
        "known" => Ok(RouteNovelty::Known),
        "untraveled" => Ok(RouteNovelty::Untraveled),
        _ => Err(ContractError {
            field: "route_novelty",
        }),
    }
}

fn environment_to_wire(value: Environment) -> &'static str {
    match value {
        Environment::Simple => "simple",
        Environment::Complex => "complex",
    }
}

fn environment_from_wire(value: &str) -> Result<Environment, ContractError> {
    match value {
        "simple" => Ok(Environment::Simple),
        "complex" => Ok(Environment::Complex),
        _ => Err(ContractError {
            field: "environment",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_kind_and_a_non_scorer_role_are_rejected() {
        let report = wire::Report {
            id: "r".to_string(),
            observed_at: "2026-10-03T12:00:00Z".to_string(),
            edge_id: "e".to_string(),
            kind: "blocked".to_string(),
            detail: None,
        };
        let error = decode_report(&report.encode_to_vec()).expect_err("kind");
        assert_eq!(error.field, "kind");

        let model = wire::ModelVersion {
            id: "m".to_string(),
            role: "router".to_string(),
            artifact: "file.onnx".to_string(),
            created_at: "2026-10-03T12:00:00Z".to_string(),
            previous_id: None,
        };
        let error = decode_model_version(&model.encode_to_vec()).expect_err("role");
        assert_eq!(error.field, "role");
    }
}
