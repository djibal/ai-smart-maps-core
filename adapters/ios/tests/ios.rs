use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::contracts::{ModelVersionRecord, TileRecord};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::tiles::Origin;
use ai_smart_maps_ios::{open, show, Place, Timing, Trip, PLATFORM};

const SUM: &[u8] = include_bytes!("../../../fixtures/sum_scorer.onnx");

#[test]
fn the_ios_screen_comes_from_the_device() {
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
    let mut device = open(&[9u8; 32], &[0u8; 32]).unwrap();
    device.install_scorer(
        &ModelVersionRecord {
            id: "scorer-1".to_string(),
            artifact: "scorer-1.onnx".to_string(),
            created_at: "2026-10-01T00:00:00Z".to_string(),
            previous_id: None,
        },
        SUM,
    );
    device
        .load_tile(
            TileRecord {
                id: "tile-1".to_string(),
                observed_at: "2026-09-01T00:00:00Z".to_string(),
                graph,
                signature: None,
            },
            Origin::Local,
        )
        .unwrap();
    let screen = show(
        PLATFORM,
        &mut device,
        "a",
        Place::Node("b"),
        &Trip {
            now: "2026-10-05T00:00:00Z",
            map_age_days: 1.0,
            report_count: 1,
            route_novelty: RouteNovelty::Known,
            environment: Environment::Simple,
        },
        Timing::default(),
    )
    .unwrap();
    assert_eq!(screen.platform, "ios");
    assert_eq!(screen.tile_id, "tile-1");
    assert_eq!(screen.edge_ids, vec!["ab".to_string()]);
}
