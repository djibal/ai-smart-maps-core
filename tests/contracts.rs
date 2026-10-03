use ai_smart_maps_core::confidence::{Confidence, Environment, RouteNovelty};
use ai_smart_maps_core::contracts::{
    decode_confidence, decode_graph, decode_model_version, decode_report, decode_route, decode_tile,
    encode_confidence, encode_graph, encode_model_version, encode_report, encode_route, encode_tile,
    ModelVersionRecord, ReportRecord, TileRecord,
};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::reports::Kind;
use ai_smart_maps_core::router::Route;

fn sample_graph() -> Graph {
    Graph {
        nodes: vec![Node {
            id: "a".to_string(),
            lat: 51.5,
            lon: -0.1,
        }],
        edges: vec![Edge {
            id: "e".to_string(),
            src: "a".to_string(),
            dst: "b".to_string(),
            weight: 2.0,
            constraint: Constraint::Open,
            source: Source::Commercial,
            hazard: Some(0.25),
            valid_from: Some("2026-10-03T00:00:00Z".to_string()),
            valid_to: None,
        }],
    }
}

fn sample_confidence() -> Confidence {
    Confidence {
        score: 0.5,
        map_age_days: 120.0,
        report_count: 3,
        model_version_id: "scorer-1".to_string(),
        route_novelty: RouteNovelty::Untraveled,
        environment: Environment::Complex,
    }
}

#[test]
fn graph_tile_report_confidence_model_and_route_round_trip() {
    let graph = sample_graph();
    assert_eq!(decode_graph(&encode_graph(&graph).unwrap()).unwrap(), graph);

    let tile = TileRecord {
        id: "tile-1".to_string(),
        observed_at: "2026-10-03T12:00:00Z".to_string(),
        graph: graph.clone(),
        signature: None,
    };
    assert_eq!(decode_tile(&encode_tile(&tile).unwrap()).unwrap(), tile);

    let report = ReportRecord {
        id: "r".to_string(),
        observed_at: "2026-10-03T12:00:00Z".to_string(),
        edge_id: "e".to_string(),
        kind: Kind::Closure,
        detail: Some("lane closed".to_string()),
    };
    assert_eq!(decode_report(&encode_report(&report).unwrap()).unwrap(), report);

    let confidence = sample_confidence();
    assert_eq!(
        decode_confidence(&encode_confidence(&confidence).unwrap()).unwrap(),
        confidence
    );

    let model = ModelVersionRecord {
        id: "scorer-1".to_string(),
        artifact: "scorer.onnx".to_string(),
        created_at: "2026-10-03T12:00:00Z".to_string(),
        previous_id: Some("scorer-0".to_string()),
    };
    let decoded = decode_model_version(&encode_model_version(&model).unwrap()).unwrap();
    assert_eq!(decoded, model);

    let route = Route {
        edge_ids: vec!["e".to_string()],
        model_version_id: "scorer-1".to_string(),
        confidence,
    };
    assert_eq!(decode_route(&encode_route(&route).unwrap()).unwrap(), route);
}

#[test]
fn the_contract_has_no_personal_fields_and_matches_schema_names() {
    let proto = include_str!("../proto/contracts.proto");
    for name in [
        "id",
        "lat",
        "lon",
        "src",
        "dst",
        "weight",
        "constraint",
        "source",
        "hazard",
        "valid_from",
        "valid_to",
        "nodes",
        "edges",
        "observed_at",
        "graph",
        "signature",
        "edge_id",
        "kind",
        "detail",
        "score",
        "map_age_days",
        "report_count",
        "model_version_id",
        "route_novelty",
        "environment",
        "role",
        "artifact",
        "created_at",
        "previous_id",
        "edge_ids",
        "confidence",
    ] {
        assert!(proto.contains(&format!(" {name} =")), "missing {name}");
    }
    for forbidden in ["account", "reporter_key", "payment", "contact", "password"] {
        assert!(!proto.contains(forbidden), "found {forbidden}");
    }
}
