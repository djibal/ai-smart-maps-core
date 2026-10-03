use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::scorer::{deterministic_choice, features, Host};

fn graph() -> Graph {
    Graph {
        nodes: vec![Node {
            id: "a".to_string(),
            lat: 0.0,
            lon: 0.0,
        }],
        edges: vec![
            Edge {
                id: "e1".to_string(),
                src: "a".to_string(),
                dst: "b".to_string(),
                weight: 2.0,
                constraint: Constraint::Open,
                source: Source::Osm,
                hazard: None,
                valid_from: None,
                valid_to: None,
            },
            Edge {
                id: "e2".to_string(),
                src: "b".to_string(),
                dst: "c".to_string(),
                weight: 3.0,
                constraint: Constraint::Open,
                source: Source::Osm,
                hazard: Some(0.5),
                valid_from: None,
                valid_to: None,
            },
        ],
    }
}

#[test]
fn features_are_the_four_route_numbers() {
    let found = features(
        &graph(),
        &["e1".to_string(), "e2".to_string()],
    )
    .unwrap();
    assert_eq!(found.edge_count, 2);
    assert_eq!(found.weight_sum, 5.0);
    assert_eq!(found.hazard_sum, 0.5);
    assert_eq!(found.hazard_missing, 1);
}

#[test]
fn the_cheaper_sequence_wins_over_a_scorer_preference() {
    let cheap = vec!["a".to_string()];
    let dear = vec!["b".to_string()];
    let chosen = deterministic_choice(&cheap, 1.0, &dear, 9.0);
    assert_eq!(chosen, cheap.as_slice());
}

#[test]
fn a_byte_mismatch_uses_the_previous_version() {
    let mut host = Host::new();
    host.install("scorer-1", b"good");
    host.install("scorer-0", b"older");
    assert_eq!(
        host.accepted_version("scorer-1", b"good", Some("scorer-0"))
            .as_deref(),
        Some("scorer-1")
    );
    assert_eq!(
        host.accepted_version("scorer-1", b"tampered", Some("scorer-0"))
            .as_deref(),
        Some("scorer-0")
    );
    assert!(host
        .accepted_version("scorer-1", b"tampered", None)
        .is_none());
}

#[test]
fn the_accepted_onnx_artifact_scores_the_four_features() {
    let sum = include_bytes!("../fixtures/sum_scorer.onnx");
    let zero = include_bytes!("../fixtures/zero_scorer.onnx");
    let found = features(&graph(), &["e1".to_string(), "e2".to_string()]).unwrap();
    let mut host = Host::new();
    host.install("scorer-1", sum);
    host.install("scorer-0", zero);

    let matched = host
        .score("scorer-1", sum, Some("scorer-0"), &found)
        .unwrap();
    assert_eq!(matched.version_id, "scorer-1");
    assert_eq!(matched.value, 8.5);

    let rolled = host
        .score("scorer-1", b"tampered", Some("scorer-0"), &found)
        .unwrap();
    assert_eq!(rolled.version_id, "scorer-0");
    assert_eq!(rolled.value, 0.0);

    assert!(host.score("scorer-1", b"tampered", None, &found).is_err());
}
