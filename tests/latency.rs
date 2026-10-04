use std::time::Duration;

use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::router::{
    within_hard_limit, Request, Router, INITIAL_ROUTE_LIMIT, REROUTE_LIMIT,
};

fn graph() -> Graph {
    Graph {
        nodes: vec![
            Node {
                id: "a".to_string(),
                lat: 0.0,
                lon: 0.0,
            },
            Node {
                id: "c".to_string(),
                lat: 0.0,
                lon: 0.0,
            },
        ],
        edges: vec![Edge {
            id: "only".to_string(),
            src: "a".to_string(),
            dst: "c".to_string(),
            weight: 1.0,
            constraint: Constraint::Open,
            source: Source::Osm,
            hazard: None,
            valid_from: None,
            valid_to: None,
        }],
    }
}

fn request<'a>() -> Request<'a> {
    Request {
        origin: "a",
        destination: "c",
        tile_id: "tile-1",
        now: "2026-10-04T00:00:00Z",
        model_version_id: "scorer-1",
        map_age_days: 1.0,
        report_count: 0,
        rolled_back: false,
        route_novelty: RouteNovelty::Known,
        environment: Environment::Simple,
    }
}

#[test]
fn the_hard_limits_are_two_seconds_and_one_second() {
    assert_eq!(INITIAL_ROUTE_LIMIT, Duration::from_secs(2));
    assert_eq!(REROUTE_LIMIT, Duration::from_secs(1));
    assert!(within_hard_limit(
        INITIAL_ROUTE_LIMIT,
        Duration::from_secs(2)
    ));
    assert!(!within_hard_limit(
        INITIAL_ROUTE_LIMIT,
        Duration::from_secs(2) + Duration::from_nanos(1)
    ));
    assert!(within_hard_limit(REROUTE_LIMIT, Duration::from_secs(1)));
    assert!(!within_hard_limit(
        REROUTE_LIMIT,
        Duration::from_millis(1001)
    ));
}

#[test]
fn an_initial_route_and_a_reroute_finish_inside_their_limits() {
    let graph = graph();
    let ask = request();
    let mut router = Router::new();
    let initial = router.route(&graph, &ask).expect("initial");
    assert_eq!(initial.edge_ids, vec!["only".to_string()]);
    assert!(!router.diagnostics().render().contains("error="));

    let mut rerouter = Router::new();
    let again = rerouter.reroute(&graph, &ask).expect("reroute");
    assert_eq!(again.edge_ids, vec!["only".to_string()]);
    assert!(!rerouter.diagnostics().render().contains("error="));
}

#[test]
fn a_result_past_the_limit_is_not_returned_or_cached() {
    let graph = graph();
    let ask = request();
    let mut router = Router::new();
    assert!(router.route_within(&graph, &ask, Duration::ZERO).is_none());
    assert!(router.diagnostics().render().contains("error=too_slow"));

    let recovered = router
        .route(&graph, &ask)
        .expect("not cached from the failure");
    assert_eq!(recovered.edge_ids, vec!["only".to_string()]);
}
