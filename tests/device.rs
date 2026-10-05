use ai_smart_maps_core::budget::Budget;
use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::contracts::{ModelVersionRecord, TileRecord};
use ai_smart_maps_core::device::{Device, DeviceError, Trip};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::reports::Kind;
use ai_smart_maps_core::snap::Destination;
use ai_smart_maps_core::tiles::{signed_bytes, Origin};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};

const CACHE_KEY: [u8; 32] = [3u8; 32];
const SOURCE_SECRET: [u8; 32] = [5u8; 32];
const SUM: &[u8] = include_bytes!("../fixtures/sum_scorer.onnx");
const ZERO: &[u8] = include_bytes!("../fixtures/zero_scorer.onnx");

fn source_public_key() -> [u8; 32] {
    SigningKey::from_bytes(&SOURCE_SECRET)
        .verifying_key()
        .to_bytes()
}

fn node(id: &str, lat: f64) -> Node {
    Node {
        id: id.to_string(),
        lat,
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

fn graph() -> Graph {
    Graph {
        nodes: vec![node("a", 0.0), node("b", 1.0), node("c", 2.0)],
        edges: vec![
            edge("long", "a", "c", 4.0, None),
            edge("via", "a", "b", 1.0, Some(1.0)),
            edge("rest", "b", "c", 1.0, None),
        ],
    }
}

fn local_tile(id: &str, observed_at: &str) -> TileRecord {
    TileRecord {
        id: id.to_string(),
        observed_at: observed_at.to_string(),
        graph: graph(),
        signature: None,
    }
}

fn fetched_tile(id: &str, observed_at: &str) -> TileRecord {
    let mut tile = local_tile(id, observed_at);
    let message = signed_bytes(&tile).unwrap();
    let signature = SigningKey::from_bytes(&SOURCE_SECRET).sign(&message);
    tile.signature = Some(STANDARD.encode(signature.to_bytes()));
    tile
}

fn scorer(id: &str, previous_id: Option<&str>) -> ModelVersionRecord {
    ModelVersionRecord {
        id: id.to_string(),
        artifact: format!("{id}.onnx"),
        created_at: "2026-10-01T00:00:00Z".to_string(),
        previous_id: previous_id.map(str::to_string),
    }
}

fn trip(environment: Environment) -> Trip<'static> {
    Trip {
        now: "2026-10-05T00:00:00Z",
        map_age_days: 1.0,
        report_count: 1,
        route_novelty: RouteNovelty::Known,
        environment,
    }
}

fn device() -> Device {
    let mut device = Device::open(&CACHE_KEY, &source_public_key()).unwrap();
    device.install_scorer(&scorer("scorer-0", None), ZERO);
    device.install_scorer(&scorer("scorer-1", Some("scorer-0")), SUM);
    device
}

#[test]
fn a_coordinate_becomes_a_sealed_scored_route_on_the_newest_tile() {
    let mut device = device();
    device
        .load_tile(local_tile("old", "2026-01-01T00:00:00Z"), Origin::Local)
        .unwrap();
    device
        .load_tile(fetched_tile("new", "2026-09-01T00:00:00Z"), Origin::Fetched)
        .unwrap();

    let outcome = device
        .route(
            "a",
            Destination::Coordinate { lat: 2.0, lon: 0.0 },
            &trip(Environment::Simple),
        )
        .unwrap();

    assert_eq!(outcome.tile_id, "new");
    assert_eq!(outcome.route.edge_ids, vec!["long".to_string()]);
    assert_eq!(outcome.route.model_version_id, "scorer-1");
    assert!(!outcome.rolled_back);
    assert!(!outcome.route.confidence.warns());
    let prediction = outcome.prediction.unwrap();
    assert_eq!(prediction.version_id, "scorer-1");
    assert_eq!(prediction.value, 1.0 + 4.0 + 0.0 + 1.0);
    assert!(outcome.sealed);
    assert_eq!(
        device.sealed_route("a", "c", "new").unwrap().edge_ids,
        vec!["long".to_string()]
    );

    let log = device.diagnostics();
    assert!(log.contains("edges=long"));
    assert!(!log.contains("2.0"));
    assert!(!log.contains("error="));
}

#[test]
fn a_tampered_artifact_rolls_back_and_lowers_confidence() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    device.present_artifact(b"tampered").unwrap();

    let outcome = device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();

    assert!(outcome.rolled_back);
    assert_eq!(outcome.route.model_version_id, "scorer-0");
    assert_eq!(outcome.prediction.unwrap().version_id, "scorer-0");
    assert!(outcome.route.confidence.warns());
    assert_eq!(outcome.route.confidence.score, 0.5);
    assert!(!device.diagnostics().contains("tampered"));
}

#[test]
fn a_fetched_tile_without_a_valid_signature_never_enters_the_flow() {
    let mut device = device();
    let unsigned = local_tile("remote", "2026-09-01T00:00:00Z");
    assert_eq!(
        device.load_tile(unsigned, Origin::Fetched),
        Err(DeviceError::TileRejected)
    );
    let mut forged = fetched_tile("forged", "2026-09-01T00:00:00Z");
    forged.observed_at = "2026-09-02T00:00:00Z".to_string();
    assert_eq!(
        device.load_tile(forged, Origin::Fetched),
        Err(DeviceError::TileRejected)
    );
    assert!(device.tile_ids().is_empty());
    assert_eq!(
        device.route("a", Destination::NodeId("c"), &trip(Environment::Simple)),
        Err(DeviceError::NoTile)
    );
}

#[test]
fn bad_input_and_an_unreachable_node_are_refused_before_anything_is_sealed() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    assert_eq!(
        device.route(
            "a",
            Destination::Coordinate {
                lat: f64::NAN,
                lon: 0.0
            },
            &trip(Environment::Simple)
        ),
        Err(DeviceError::Rejected)
    );
    assert_eq!(
        device.route("c", Destination::NodeId("a"), &trip(Environment::Simple)),
        Err(DeviceError::Unreachable)
    );
    assert!(device.sealed_route("c", "a", "t").is_none());
    assert!(device.diagnostics().contains("error=unreachable"));
}

#[test]
fn the_budget_evicts_the_oldest_tile_from_the_flow() {
    let tile_bytes =
        ai_smart_maps_core::contracts::encode_tile(&local_tile("old", "2026-01-01T00:00:00Z"))
            .unwrap()
            .len() as u64;
    let scorer_bytes = (SUM.len() + ZERO.len()) as u64;
    let mut device = Device::open(&CACHE_KEY, &source_public_key())
        .unwrap()
        .with_budget(Budget::with_limit(scorer_bytes + tile_bytes * 2));
    device.install_scorer(&scorer("scorer-0", None), ZERO);
    device.install_scorer(&scorer("scorer-1", Some("scorer-0")), SUM);
    device
        .load_tile(local_tile("old", "2026-01-01T00:00:00Z"), Origin::Local)
        .unwrap();
    device
        .load_tile(local_tile("mid", "2026-05-01T00:00:00Z"), Origin::Local)
        .unwrap();
    device
        .load_tile(local_tile("new", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    assert_eq!(
        device.tile_ids(),
        vec!["mid".to_string(), "new".to_string()]
    );
    let outcome = device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    assert_eq!(outcome.tile_id, "new");
}

#[test]
fn clearing_local_data_removes_sealed_routes_and_the_opt_in() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    device.training().opt_in("scorer-1");
    device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    assert!(device.sealed_route("a", "c", "t").is_some());
    device.clear_local_data();
    assert!(device.sealed_route("a", "c", "t").is_none());
    assert!(!device.training().opted_in());
}

#[test]
fn a_local_report_carries_the_key_changes_the_route_and_stays_out_of_the_log() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    let before = device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    assert_eq!(before.route.edge_ids, vec!["long".to_string()]);

    let key = device.reporter_key().unwrap();
    let observation = device
        .report(
            "long",
            Kind::Closure,
            Some("tree across the road"),
            &trip(Environment::Simple),
        )
        .unwrap();
    assert_eq!(observation.reporter_key, key.hex());
    assert_eq!(observation.edge_id, "long");
    assert_eq!(observation.observed_at, "2026-10-05T00:00:00Z");

    let sealed = device.sealed_reports();
    assert_eq!(sealed, vec![observation.clone()]);
    assert_eq!(sealed[0].detail.as_deref(), Some("tree across the road"));
    let record = device.export_record(&observation.id).unwrap();
    assert!(!String::from_utf8_lossy(record).contains(&key.hex()));
    assert!(!String::from_utf8_lossy(record).contains("tree across"));

    let after = device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    assert_eq!(
        after.route.edge_ids,
        vec!["via".to_string(), "rest".to_string()]
    );

    let log = device.diagnostics();
    assert!(log.contains("long"));
    assert!(!log.contains(&key.hex()));
    assert!(!log.contains("tree"));
}

#[test]
fn a_report_on_an_unknown_edge_is_logged_and_not_sealed() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    assert_eq!(
        device.report("nowhere", Kind::Hazard, None, &trip(Environment::Simple)),
        Err(DeviceError::NoTile)
    );
    assert_eq!(
        device.report("", Kind::Hazard, None, &trip(Environment::Simple)),
        Err(DeviceError::Rejected)
    );
    assert!(device.sealed_reports().is_empty());
    assert!(device.diagnostics().contains("unknown_edge"));
    assert!(device.diagnostics().contains("nowhere"));
}

#[test]
fn sealed_reports_follow_the_ninety_day_rule_and_leave_with_a_clear() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    let young = Trip {
        now: "2026-10-05T00:00:00Z",
        ..trip(Environment::Simple)
    };
    let at_limit = Trip {
        now: "2026-07-07T00:00:00Z",
        ..trip(Environment::Simple)
    };
    let past = Trip {
        now: "2026-01-01T00:00:00Z",
        ..trip(Environment::Simple)
    };
    device
        .report("via", Kind::Hazard, Some("young"), &young)
        .unwrap();
    device
        .report("via", Kind::Hazard, Some("at limit"), &at_limit)
        .unwrap();
    device
        .report("via", Kind::Hazard, Some("past"), &past)
        .unwrap();
    assert_eq!(device.sealed_reports().len(), 3);

    device.retain_reports(|observation| match observation.observed_at.as_str() {
        "2026-10-05T00:00:00Z" => 1,
        "2026-07-07T00:00:00Z" => 90,
        _ => 91,
    });
    let kept = device.sealed_reports();
    assert_eq!(kept.len(), 2);
    let details: Vec<Option<&str>> = kept.iter().map(|o| o.detail.as_deref()).collect();
    assert!(details.contains(&Some("young")));
    assert!(details.contains(&None));
    assert!(!details.contains(&Some("past")));

    device.clear_local_data();
    assert!(device.sealed_reports().is_empty());
}

#[test]
fn routes_become_training_pairs_only_after_opt_in() {
    let mut device = device();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();

    device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    assert!(device.training().pairs().is_empty());

    device.training().opt_in("scorer-1");
    device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    let pairs = device.training().pairs().to_vec();
    assert_eq!(pairs.len(), 1, "the same decision is kept once");
    let pair = &pairs[0];
    assert_eq!(pair.left.edge_count, 1);
    assert_eq!(pair.left.weight_sum, 4.0);
    assert_eq!(pair.left.hazard_missing, 1);
    assert_eq!(pair.right.edge_count, 2);
    assert_eq!(pair.right.weight_sum, 2.0);
    assert_eq!(pair.right.hazard_sum, 1.0);
    assert_eq!(pair.right.hazard_missing, 1);
    assert!(pair.left_has_lower_cost, "4.0 against 1.0 + 3.0 + 1.0");

    device
        .route("a", Destination::NodeId("b"), &trip(Environment::Simple))
        .unwrap();
    assert_eq!(
        device.training().pairs().len(),
        1,
        "a to b has one path, so no alternative and no pair"
    );

    assert!(
        !device.diagnostics().contains("via,rest"),
        "the alternative search is not logged: {}",
        device.diagnostics()
    );

    device.clear_local_data();
    assert!(device.training().pairs().is_empty());
    device
        .route("a", Destination::NodeId("c"), &trip(Environment::Simple))
        .unwrap();
    assert!(device.training().pairs().is_empty());
}

#[test]
fn a_device_without_a_scorer_does_not_route() {
    let mut device = Device::open(&CACHE_KEY, &source_public_key()).unwrap();
    device
        .load_tile(local_tile("t", "2026-09-01T00:00:00Z"), Origin::Local)
        .unwrap();
    assert_eq!(
        device.route("a", Destination::NodeId("c"), &trip(Environment::Simple)),
        Err(DeviceError::NoScorer)
    );
}
