use std::time::Duration;

use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::scorer::{
    deterministic_choice, features, within_inference_limit, Features, Host, ScoreError,
    INFERENCE_LIMIT,
};

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
    let found = features(&graph(), &["e1".to_string(), "e2".to_string()]).unwrap();
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

/// The A1-v5 device scorer: 32 hidden units, 2 layers, trained on the
/// linear variant with torch seed 0 from the experiment repository and
/// exported to ONNX. `fixtures/a1v5_device_32x2.json` records the
/// reference scores. A higher score is the preferred route.
#[test]
fn the_a1_v5_device_scorer_runs_on_device_and_ranks_the_cheaper_route_higher() {
    let artifact = include_bytes!("../fixtures/a1v5_device_32x2.onnx");
    assert!(artifact.len() < 8 * 1024, "{} bytes", artifact.len());
    let cheap = Features {
        edge_count: 2,
        weight_sum: 5.0,
        hazard_sum: 0.2,
        hazard_missing: 0,
    };
    let dear = Features {
        edge_count: 4,
        weight_sum: 30.0,
        hazard_sum: 2.5,
        hazard_missing: 1,
    };
    let mut host = Host::new();
    host.install("a1v5-32x2", artifact);

    let started = std::time::Instant::now();
    let cheap_score = host.score("a1v5-32x2", artifact, None, &cheap).unwrap();
    let dear_score = host.score("a1v5-32x2", artifact, None, &dear).unwrap();
    let elapsed = started.elapsed();

    assert!(within_inference_limit(elapsed), "{elapsed:?}");
    assert!(
        (cheap_score.value - -0.355_85).abs() < 1e-4,
        "{cheap_score:?}"
    );
    assert!((dear_score.value - -1.540_3).abs() < 1e-4, "{dear_score:?}");
    assert!(cheap_score.value > dear_score.value);
    assert!(!host.diagnostics().render().contains("error="));
}

#[test]
fn the_inference_hard_limit_is_500_milliseconds() {
    assert_eq!(INFERENCE_LIMIT, Duration::from_millis(500));
    assert!(within_inference_limit(Duration::from_millis(500)));
    assert!(!within_inference_limit(
        Duration::from_millis(500) + Duration::from_nanos(1)
    ));
}

#[test]
fn inference_past_the_limit_is_refused() {
    let sum = include_bytes!("../fixtures/sum_scorer.onnx");
    let found = features(&graph(), &["e1".to_string(), "e2".to_string()]).unwrap();
    let mut host = Host::new();
    host.install("scorer-1", sum);
    let error = host
        .score_within("scorer-1", sum, None, &found, Duration::ZERO)
        .unwrap_err();
    assert_eq!(error, ScoreError::TooSlow);
    assert!(host.diagnostics().render().contains("error=too_slow"));
}
