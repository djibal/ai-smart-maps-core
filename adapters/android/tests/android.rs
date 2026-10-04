use std::time::Duration;

use ai_smart_maps_android::{show, Place, Trip, PLATFORM};
use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::router::Router;

#[test]
fn the_android_screen_uses_the_core_edge_list() {
    let graph = Graph {
        nodes: vec![
            Node {
                id: "a".to_string(),
                lat: 0.0,
                lon: 0.0,
            },
            Node {
                id: "b".to_string(),
                lat: 1.0,
                lon: 0.0,
            },
        ],
        edges: vec![Edge {
            id: "ab".to_string(),
            src: "a".to_string(),
            dst: "b".to_string(),
            weight: 1.0,
            constraint: Constraint::Open,
            source: Source::Osm,
            hazard: None,
            valid_from: None,
            valid_to: None,
        }],
    };
    let screen = show(
        PLATFORM,
        &graph,
        &mut Router::new(),
        "a",
        Place::Node("b"),
        &Trip {
            tile_id: "tile-1",
            now: "2026-10-04T00:00:00Z",
            model_version_id: "scorer-1",
            map_age_days: 1.0,
            report_count: 1,
            rolled_back: false,
            route_novelty: RouteNovelty::Known,
            environment: Environment::Simple,
        },
        Duration::ZERO,
        Duration::ZERO,
    )
    .unwrap();
    assert_eq!(screen.platform, "android");
    assert_eq!(screen.edge_ids, vec!["ab".to_string()]);
}
