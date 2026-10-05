//! The reporter key lifecycle: created on first use, stored only in the
//! sealed cache, deleted with the clear and the 90-day rotation, never in
//! a log or a contract.

use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::contracts::{encode_route, ModelVersionRecord, TileRecord};
use ai_smart_maps_core::device::{Device, Trip};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::log::DiagnosticLog;
use ai_smart_maps_core::reporter::{ReporterKey, ReporterKeyError, RECORD};
use ai_smart_maps_core::reports::{apply_logged, Kind, Observation};
use ai_smart_maps_core::snap::Destination;
use ai_smart_maps_core::store::StoreError;
use ai_smart_maps_core::tiles::Origin;

const CACHE_KEY: [u8; 32] = [3u8; 32];
const NEXT_CACHE_KEY: [u8; 32] = [4u8; 32];
const MAP_SOURCE_KEY: [u8; 32] = [0u8; 32];
const SUM: &[u8] = include_bytes!("../fixtures/sum_scorer.onnx");

fn open_device() -> Device {
    Device::open(&CACHE_KEY, &MAP_SOURCE_KEY).unwrap()
}

fn graph() -> Graph {
    Graph {
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
    }
}

#[test]
fn a_key_is_sixteen_random_bytes_written_as_thirty_two_hex_characters() {
    let first = ReporterKey::generate();
    let second = ReporterKey::generate();
    assert_ne!(first, second);
    assert_eq!(first.as_bytes().len(), 16);

    let hex = first.hex();
    assert_eq!(hex.len(), 32);
    assert!(hex
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(ReporterKey::from_hex(&hex).unwrap(), first);
    assert_eq!(
        ReporterKey::from_hex(&hex.to_ascii_uppercase()).unwrap(),
        first
    );

    assert_eq!(
        ReporterKey::from_hex(&hex[..31]),
        Err(ReporterKeyError::Malformed)
    );
    assert_eq!(
        ReporterKey::from_hex(&format!("{}zz", &hex[..30])),
        Err(ReporterKeyError::Malformed)
    );
    assert_eq!(
        ReporterKey::from_bytes(&[0u8; 15]),
        Err(ReporterKeyError::Malformed)
    );
}

#[test]
fn the_debug_form_is_a_placeholder() {
    let key = ReporterKey::generate();
    let shown = format!("{key:?}");
    assert_eq!(shown, "ReporterKey(redacted)");
    assert!(!shown.contains(&key.hex()));
}

#[test]
fn the_device_creates_the_key_once_and_a_reloaded_device_reads_the_same_key() {
    let mut device = open_device();
    assert!(device.export_record(RECORD).is_none());

    let key = device.reporter_key().unwrap();
    assert_eq!(device.reporter_key().unwrap(), key);

    let sealed = device.export_record(RECORD).unwrap().to_vec();
    assert_ne!(sealed, key.as_bytes());
    assert!(!sealed.windows(16).any(|w| w == key.as_bytes()));

    let mut reloaded = open_device();
    reloaded.import_record(RECORD, sealed);
    assert_eq!(reloaded.reporter_key().unwrap(), key);

    let mut other_cache_key = Device::open(&NEXT_CACHE_KEY, &MAP_SOURCE_KEY).unwrap();
    other_cache_key.import_record(RECORD, device.export_record(RECORD).unwrap().to_vec());
    assert_eq!(other_cache_key.reporter_key(), Err(StoreError::Rejected));
}

#[test]
fn clear_and_the_ninety_day_rotation_delete_the_key_and_an_early_rotation_keeps_it() {
    let mut device = open_device();
    let first = device.reporter_key().unwrap();

    assert_eq!(
        device.rotate_cache(&NEXT_CACHE_KEY, 89),
        Err(StoreError::NotDue)
    );
    assert_eq!(device.reporter_key().unwrap(), first);

    device.clear_local_data();
    assert!(device.export_record(RECORD).is_none());
    let second = device.reporter_key().unwrap();
    assert_ne!(second, first);

    device.rotate_cache(&NEXT_CACHE_KEY, 90).unwrap();
    assert!(device.export_record(RECORD).is_none());
    let third = device.reporter_key().unwrap();
    assert_ne!(third, second);
    assert_ne!(third, first);
}

#[test]
fn the_key_is_never_in_the_log_or_a_contract() {
    let mut device = open_device();
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
                observed_at: "2026-10-01T00:00:00Z".to_string(),
                graph: graph(),
                signature: None,
            },
            Origin::Local,
        )
        .unwrap();
    let key = device.reporter_key().unwrap();
    let hex = key.hex();

    let outcome = device
        .route(
            "a",
            Destination::NodeId("b"),
            &Trip {
                now: "2026-10-05T00:00:00Z",
                map_age_days: 1.0,
                report_count: 1,
                route_novelty: RouteNovelty::Known,
                environment: Environment::Simple,
            },
        )
        .unwrap();

    let route_bytes = encode_route(&outcome.route).unwrap();
    assert!(!route_bytes.windows(16).any(|w| w == key.as_bytes()));
    assert!(!String::from_utf8_lossy(&route_bytes).contains(&hex));
    assert!(!format!("{outcome:?}").contains(&hex));
    assert!(!device.diagnostics().contains(&hex));

    let mut graph = graph();
    let mut log = DiagnosticLog::new();
    apply_logged(
        &mut graph,
        &[
            Observation {
                id: "obs-1".to_string(),
                observed_at: "2026-10-05T00:00:00Z".to_string(),
                edge_id: "ab".to_string(),
                kind: Kind::Hazard,
                detail: None,
                reporter_key: hex.clone(),
            },
            Observation {
                id: "obs-2".to_string(),
                observed_at: "2026-10-05T00:00:00Z".to_string(),
                edge_id: "missing".to_string(),
                kind: Kind::Closure,
                detail: None,
                reporter_key: hex.clone(),
            },
        ],
        &mut log,
    );
    let rendered = log.render();
    assert!(rendered.contains("unknown_edge"));
    assert!(!rendered.contains(&hex));
}

#[test]
fn the_contracts_have_no_reporter_field_and_the_module_writes_no_log() {
    let proto = include_str!("../proto/contracts.proto");
    assert!(!proto.to_ascii_lowercase().contains("reporter"));
    let source = include_str!("../src/reporter.rs");
    assert!(!source.contains("DiagnosticLog"));
    assert!(!source.contains("println!"));
    assert!(!source.contains("eprintln!"));
}

#[test]
fn an_observation_debug_form_never_contains_the_key() {
    let key = ReporterKey::generate();
    let hex = key.hex();
    let observation = Observation {
        id: "report:ab:2026-10-05T00:00:00Z".to_string(),
        observed_at: "2026-10-05T00:00:00Z".to_string(),
        edge_id: "ab".to_string(),
        kind: Kind::Hazard,
        detail: Some("flood".to_string()),
        reporter_key: hex.clone(),
    };
    let shown = format!("{observation:?}");
    assert!(shown.contains("reporter_key: \"redacted\""));
    assert!(!shown.contains(&hex));
    assert_eq!(observation.reporter_key, hex, "the field is still readable");
}
