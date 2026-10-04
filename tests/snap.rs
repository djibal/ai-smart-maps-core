use ai_smart_maps_core::graph::{Graph, Node};
use ai_smart_maps_core::snap::{resolve, Destination};

fn graph() -> Graph {
    Graph {
        nodes: vec![
            Node {
                id: "near".to_string(),
                lat: 48.0,
                lon: 2.0,
            },
            Node {
                id: "far".to_string(),
                lat: 49.0,
                lon: 2.0,
            },
        ],
        edges: Vec::new(),
    }
}

#[test]
fn a_coordinate_snaps_to_the_nearest_node() {
    let graph = graph();
    let id = resolve(
        &graph,
        Destination::Coordinate {
            lat: 48.01,
            lon: 2.0,
        },
    )
    .unwrap();
    assert_eq!(id, "near");
}

#[test]
fn an_existing_node_id_is_kept() {
    let graph = graph();
    assert_eq!(resolve(&graph, Destination::NodeId("far")).unwrap(), "far");
}

#[test]
fn equal_distance_keeps_the_smaller_node_id() {
    let graph = Graph {
        nodes: vec![
            Node {
                id: "b".to_string(),
                lat: 0.0,
                lon: 0.0,
            },
            Node {
                id: "a".to_string(),
                lat: 0.0,
                lon: 0.0,
            },
        ],
        edges: Vec::new(),
    };
    let id = resolve(&graph, Destination::Coordinate { lat: 0.0, lon: 0.0 }).unwrap();
    assert_eq!(id, "a");
}

#[test]
fn a_value_that_is_not_a_coordinate_or_a_node_id_is_rejected() {
    let graph = graph();
    assert!(resolve(&graph, Destination::NodeId("")).is_err());
    assert!(resolve(&graph, Destination::NodeId("missing")).is_err());
    assert!(resolve(
        &graph,
        Destination::Coordinate {
            lat: 91.0,
            lon: 2.0,
        },
    )
    .is_err());
    assert!(resolve(
        &graph,
        Destination::Coordinate {
            lat: 48.0,
            lon: 181.0,
        },
    )
    .is_err());
    assert!(resolve(
        &graph,
        Destination::Coordinate {
            lat: f64::NAN,
            lon: 2.0,
        },
    )
    .is_err());
    assert!(resolve(
        &Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        Destination::Coordinate { lat: 0.0, lon: 0.0 },
    )
    .is_err());
}

#[test]
fn the_router_source_does_not_snap() {
    let router_source = include_str!("../src/router.rs");
    assert!(!router_source.contains("crate::snap"));
    assert!(!router_source.contains("snap::"));
}
