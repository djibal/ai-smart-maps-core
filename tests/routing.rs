use ai_smart_maps_core::adapt::{commercial, osm, RoadRecord};
use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::router::{Request, Router};

fn node(id: &str) -> Node {
    Node {
        id: id.to_string(),
        lat: 0.0,
        lon: 0.0,
    }
}

fn edge(id: &str, src: &str, dst: &str, weight: f64, hazard: Option<f64>) -> Edge {
    Edge {
        id: id.to_string(),
        src: src.to_string(),
        dst: dst.to_string(),
        weight,
        constraint: Constraint::Open,
        source: Source::Osm,
        hazard,
        valid_from: None,
        valid_to: None,
    }
}

fn request<'a>(origin: &'a str, destination: &'a str) -> Request<'a> {
    Request {
        origin,
        destination,
        tile_id: "tile-1",
        now: "2026-10-03T12:00:00Z",
        model_version_id: "scorer-1",
        map_age_days: 1.0,
        report_count: 0,
        rolled_back: false,
        route_novelty: RouteNovelty::Known,
        environment: Environment::Simple,
    }
}

#[test]
fn open_edge_cost_uses_three_times_hazard_and_missing_hazard_is_zero() {
    let graph = Graph {
        nodes: vec![node("a"), node("b"), node("c")],
        edges: vec![
            edge("long", "a", "c", 4.0, None),
            edge("via", "a", "b", 1.0, Some(1.0)),
            edge("rest", "b", "c", 1.0, None),
        ],
    };
    let route = Router::new()
        .route(&graph, &request("a", "c"))
        .expect("path");
    assert_eq!(route.edge_ids, vec!["long".to_string()]);
}

#[test]
fn closed_and_out_of_window_edges_are_excluded() {
    let mut closed = edge("shut", "a", "c", 1.0, None);
    closed.constraint = Constraint::Closed;
    let mut early = edge("early", "a", "c", 1.0, None);
    early.valid_from = Some("2026-10-03T18:00:00Z".to_string());
    let graph = Graph {
        nodes: vec![node("a"), node("b"), node("c")],
        edges: vec![
            closed,
            early,
            edge("around", "a", "b", 2.0, None),
            edge("on", "b", "c", 2.0, None),
        ],
    };
    let route = Router::new()
        .route(&graph, &request("a", "c"))
        .expect("path");
    assert_eq!(route.edge_ids, vec!["around".to_string(), "on".to_string()]);
}

#[test]
fn equal_cost_picks_the_smaller_edge_id_sequence() {
    let graph = Graph {
        nodes: vec![node("a"), node("c")],
        edges: vec![edge("m", "a", "c", 5.0, None), edge("b", "a", "c", 5.0, None)],
    };
    let route = Router::new()
        .route(&graph, &request("a", "c"))
        .expect("path");
    assert_eq!(route.edge_ids, vec!["b".to_string()]);
}

#[test]
fn the_same_inputs_return_the_same_route() {
    let graph = Graph {
        nodes: vec![node("a"), node("b"), node("c")],
        edges: vec![
            edge("a1", "a", "b", 1.0, None),
            edge("b1", "b", "c", 1.0, None),
            edge("z", "a", "c", 3.0, None),
        ],
    };
    let first = Router::new().route(&graph, &request("a", "c"));
    let second = Router::new().route(&graph, &request("a", "c"));
    assert_eq!(first, second);
}

#[test]
fn unreachable_destination_is_empty() {
    let graph = Graph {
        nodes: vec![node("a"), node("b")],
        edges: vec![edge("one", "a", "b", 1.0, None)],
    };
    assert!(Router::new().route(&graph, &request("b", "a")).is_none());
}

#[test]
fn cache_returns_the_stored_route_for_the_same_tile() {
    let graph = Graph {
        nodes: vec![node("a"), node("c")],
        edges: vec![edge("only", "a", "c", 1.0, None)],
    };
    let mut router = Router::new();
    let first = router.route(&graph, &request("a", "c")).expect("path");
    let changed = Graph {
        nodes: vec![node("a"), node("c")],
        edges: vec![edge("other", "a", "c", 1.0, None)],
    };
    let second = router.route(&changed, &request("a", "c")).expect("cached");
    assert_eq!(first, second);
    assert_eq!(second.edge_ids, vec!["only".to_string()]);
}

#[test]
fn score_below_0_6_recomputes_on_open_edges_with_hazard_zero() {
    let graph = Graph {
        nodes: vec![node("a"), node("b"), node("c")],
        edges: vec![
            edge("hazardous", "a", "c", 1.0, Some(1.0)),
            edge("safe1", "a", "b", 4.0, None),
            edge("safe2", "b", "c", 4.0, Some(0.0)),
        ],
    };
    let mut ask = request("a", "c");
    ask.map_age_days = 120.0;
    let route = Router::new().route(&graph, &ask).expect("path");
    assert!(route.confidence.score < 0.6);
    assert_eq!(
        route.edge_ids,
        vec!["safe1".to_string(), "safe2".to_string()]
    );
}

#[test]
fn warning_keeps_the_first_path_when_no_safe_path_exists() {
    let graph = Graph {
        nodes: vec![node("a"), node("c")],
        edges: vec![edge("hazardous", "a", "c", 1.0, Some(0.4))],
    };
    let mut ask = request("a", "c");
    ask.route_novelty = RouteNovelty::Untraveled;
    let route = Router::new().route(&graph, &ask).expect("path");
    assert!(route.confidence.warns());
    assert_eq!(route.edge_ids, vec!["hazardous".to_string()]);
}

#[test]
fn adapters_emit_the_graph_type_and_the_router_source_does_not_name_them() {
    let record = RoadRecord {
        edge_id: "e".to_string(),
        src_id: "a".to_string(),
        src_lat: 1.0,
        src_lon: 2.0,
        dst_id: "b".to_string(),
        dst_lat: 3.0,
        dst_lon: 4.0,
        travel_cost: 8.0,
        closed: false,
    };
    let from_osm = osm::graph(&[record.clone()]);
    let from_commercial = commercial::graph(&[record]);
    assert_eq!(from_osm.edges[0].source, Source::Osm);
    assert_eq!(from_osm.edges[0].weight, 8.0);
    assert_eq!(from_osm.edges[0].constraint, Constraint::Open);
    assert_eq!(from_commercial.edges[0].source, Source::Commercial);
    assert_eq!(from_osm.edges[0].id, from_commercial.edges[0].id);

    let router_source = include_str!("../src/router.rs");
    assert!(!router_source.contains("crate::adapt"));
    assert!(!router_source.contains("adapt::"));
}

#[test]
fn deterministic_cost_is_what_the_path_uses() {
    let graph = Graph {
        nodes: vec![node("a"), node("c")],
        edges: vec![edge("cheap", "a", "c", 1.0, None), edge("dear", "a", "c", 9.0, None)],
    };
    let route = Router::new()
        .route(&graph, &request("a", "c"))
        .expect("path");
    assert_eq!(route.edge_ids, vec!["cheap".to_string()]);
    assert_eq!(route.model_version_id, "scorer-1");
}
