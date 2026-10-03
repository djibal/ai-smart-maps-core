//! Map adapters. The router does not use this module.

use crate::graph::{Constraint, Edge, Graph, Node, Source};

/// One road record before it becomes a graph edge.
/// `travel_cost` is copied into `weight`. The adapter does not invent a
/// second cost formula.
#[derive(Clone, Debug)]
pub struct RoadRecord {
    pub edge_id: String,
    pub src_id: String,
    pub src_lat: f64,
    pub src_lon: f64,
    pub dst_id: String,
    pub dst_lat: f64,
    pub dst_lon: f64,
    pub travel_cost: f64,
    pub closed: bool,
}

pub mod commercial;
pub mod osm;

pub(crate) fn emit(records: &[RoadRecord], source: Source) -> Graph {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges = Vec::with_capacity(records.len());
    for record in records {
        push_node(&mut nodes, &record.src_id, record.src_lat, record.src_lon);
        push_node(&mut nodes, &record.dst_id, record.dst_lat, record.dst_lon);
        edges.push(Edge {
            id: record.edge_id.clone(),
            src: record.src_id.clone(),
            dst: record.dst_id.clone(),
            weight: record.travel_cost,
            constraint: if record.closed {
                Constraint::Closed
            } else {
                Constraint::Open
            },
            source,
            hazard: None,
            valid_from: None,
            valid_to: None,
        });
    }
    Graph { nodes, edges }
}

fn push_node(nodes: &mut Vec<Node>, id: &str, lat: f64, lon: f64) {
    if nodes.iter().any(|node| node.id == id) {
        return;
    }
    nodes.push(Node {
        id: id.to_string(),
        lat,
        lon,
    });
}
