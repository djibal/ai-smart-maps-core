use std::time::Duration;

use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::contracts::{ModelVersionRecord, TileRecord};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::scorer::Features;
use ai_smart_maps_core::store::StoreError;
use ai_smart_maps_core::tiles::Origin;
use ai_smart_maps_core::training::TrainingPair;
use ai_smart_maps_shell::{
    clear_local, open, restore, save, show, Device, DeviceError, Place, ScreenError, Timing, Trip,
    TILE_RENDER_LIMIT, VOICE_LIMIT,
};

const SUM: &[u8] = include_bytes!("../../../fixtures/sum_scorer.onnx");

fn node(id: &str, lat: f64) -> Node {
    Node {
        id: id.to_string(),
        lat,
        lon: 0.0,
    }
}

fn edge(id: &str, src: &str, dst: &str) -> Edge {
    Edge {
        id: id.to_string(),
        src: src.to_string(),
        dst: dst.to_string(),
        weight: 1.0,
        constraint: Constraint::Open,
        source: Source::Osm,
        hazard: None,
        valid_from: None,
        valid_to: None,
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

fn device_with(graph: Graph) -> Device {
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
    device
}

fn line() -> Device {
    device_with(Graph {
        nodes: vec![node("a", 0.0), node("b", 1.0)],
        edges: vec![edge("ab", "a", "b")],
    })
}

#[test]
fn the_screen_draws_the_core_edge_list_and_snaps_before_routing() {
    let mut device = line();
    let screen = show(
        "ios",
        &mut device,
        "a",
        Place::Coordinate { lat: 1.0, lon: 0.0 },
        &trip(Environment::Simple),
        Timing {
            voice: VOICE_LIMIT,
            tile: TILE_RENDER_LIMIT,
        },
    )
    .unwrap();
    assert_eq!(screen.tile_id, "tile-1");
    assert_eq!(screen.edge_ids, vec!["ab".to_string()]);
    assert!(!screen.warning);
    assert!(screen.voice_issued && screen.tile_drawn);
    assert!(device.sealed_route("a", "b", "tile-1").is_some());
}

#[test]
fn a_saved_cache_reopens_with_the_same_route_scorer_and_reporter_key() {
    let mut device = line();
    let first = show(
        "android",
        &mut device,
        "a",
        Place::Node("b"),
        &trip(Environment::Simple),
        Timing::default(),
    )
    .unwrap();
    let key = device.reporter_key().unwrap();
    let blob = save(&device).unwrap();
    assert!(!String::from_utf8_lossy(&blob).contains("tile-1"));
    assert!(!String::from_utf8_lossy(&blob).contains(&key.hex()));

    let mut reopened = restore(&[9u8; 32], &[0u8; 32], &blob).unwrap();
    assert_eq!(reopened.tile_ids(), vec!["tile-1".to_string()]);
    assert_eq!(reopened.budget_used(), device.budget_used());
    assert_eq!(
        reopened.sealed_route("a", "b", "tile-1").unwrap().edge_ids,
        first.edge_ids
    );
    assert_eq!(reopened.reporter_key().unwrap(), key);
    let again = show(
        "android",
        &mut reopened,
        "a",
        Place::Node("b"),
        &trip(Environment::Simple),
        Timing::default(),
    )
    .unwrap();
    assert_eq!(again.edge_ids, first.edge_ids);
    assert_eq!(again.tile_id, "tile-1");

    let mut empty = open(&[9u8; 32], &[0u8; 32]).unwrap();
    assert_eq!(
        show(
            "android",
            &mut empty,
            "a",
            Place::Node("b"),
            &trip(Environment::Simple),
            Timing::default(),
        ),
        Err(ScreenError::Device(DeviceError::NoScorer))
    );

    assert_eq!(
        restore(&[8u8; 32], &[0u8; 32], &blob).err(),
        Some(StoreError::Rejected)
    );
}

#[test]
fn a_bad_destination_is_rejected_and_a_low_score_warns() {
    let mut device = line();
    let rejected = show(
        "ios",
        &mut device,
        "a",
        Place::Coordinate {
            lat: 120.0,
            lon: 0.0,
        },
        &trip(Environment::Simple),
        Timing::default(),
    );
    assert_eq!(rejected, Err(ScreenError::Device(DeviceError::Rejected)));
    let warned = show(
        "android",
        &mut device,
        "a",
        Place::Node("b"),
        &trip(Environment::Complex),
        Timing::default(),
    )
    .unwrap();
    assert!(warned.warning);
}

#[test]
fn voice_and_tile_limits_are_inclusive() {
    assert_eq!(VOICE_LIMIT, Duration::from_millis(200));
    assert_eq!(TILE_RENDER_LIMIT, Duration::from_millis(500));
    let mut device = line();
    let trip = trip(Environment::Simple);
    let slow_voice = Timing {
        voice: Duration::from_millis(201),
        tile: Duration::ZERO,
    };
    assert_eq!(
        show("web", &mut device, "a", Place::Node("b"), &trip, slow_voice),
        Err(ScreenError::VoiceTooSlow)
    );
    let slow_tile = Timing {
        voice: Duration::ZERO,
        tile: Duration::from_millis(501),
    };
    assert_eq!(
        show("web", &mut device, "a", Place::Node("b"), &trip, slow_tile),
        Err(ScreenError::TileTooSlow)
    );
}

#[test]
fn an_unreachable_node_returns_no_screen() {
    let mut device = device_with(Graph {
        nodes: vec![node("a", 0.0), node("z", 2.0)],
        edges: vec![],
    });
    let result = show(
        "web",
        &mut device,
        "a",
        Place::Node("z"),
        &trip(Environment::Simple),
        Timing::default(),
    );
    assert_eq!(result, Err(ScreenError::Device(DeviceError::Unreachable)));
}

#[test]
fn clear_removes_the_sealed_routes_and_the_training_pairs() {
    let mut device = line();
    show(
        "ios",
        &mut device,
        "a",
        Place::Node("b"),
        &trip(Environment::Simple),
        Timing::default(),
    )
    .unwrap();
    device.training().opt_in("scorer-1");
    device
        .training()
        .record(TrainingPair {
            left: Features {
                edge_count: 1,
                weight_sum: 1.0,
                hazard_sum: 0.0,
                hazard_missing: 0,
            },
            right: Features {
                edge_count: 2,
                weight_sum: 2.0,
                hazard_sum: 0.0,
                hazard_missing: 0,
            },
            left_has_lower_cost: true,
        })
        .unwrap();
    clear_local(&mut device);
    assert!(device.sealed_route("a", "b", "tile-1").is_none());
    assert!(!device.training().opted_in());
    assert!(device.training().pairs().is_empty());
}

#[test]
fn adapter_sources_stay_under_ten_percent_and_name_no_person() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core_lines = rust_lines(&root.join("src"));
    for name in ["shell", "ios", "android", "web"] {
        let lines = rust_lines(&root.join("adapters").join(name).join("src"));
        assert!(
            lines * 10 < core_lines,
            "{name} has {lines} lines and the core has {core_lines}"
        );
        let source =
            std::fs::read_to_string(root.join("adapters").join(name).join("src/lib.rs")).unwrap();
        let lower = source.to_lowercase();
        for word in ["account", "password", "payment", "passkey", "contact"] {
            assert!(!lower.contains(word), "{name} contains {word}");
        }
        assert!(
            !source.contains("router::"),
            "{name} calls the router directly"
        );
        assert!(
            !source.contains("snap::resolve") && !source.contains("resolve("),
            "{name} snaps on its own"
        );
    }
    let core_lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    assert!(!core_lib.contains("adapters"));
}

fn rust_lines(dir: &std::path::Path) -> usize {
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let next = entry.path();
            if next.is_dir() {
                pending.push(next);
            } else if next.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&next).unwrap();
                total += text.lines().count();
            }
        }
    }
    total
}
