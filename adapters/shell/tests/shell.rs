use std::time::Duration;

use ai_smart_maps_core::confidence::{Environment, RouteNovelty};
use ai_smart_maps_core::graph::{Constraint, Edge, Graph, Node, Source};
use ai_smart_maps_core::router::Router;
use ai_smart_maps_core::training::{TrainingPair, TrainingRecord};
use ai_smart_maps_shell::{
    clear_local, open_cache, show, Place, ScreenError, Trip, TILE_RENDER_LIMIT, VOICE_LIMIT,
};

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
        tile_id: "tile-1",
        now: "2026-10-04T00:00:00Z",
        model_version_id: "scorer-1",
        map_age_days: 1.0,
        report_count: 1,
        rolled_back: false,
        route_novelty: RouteNovelty::Known,
        environment,
    }
}

fn line(id: &str, src: &str, dst: &str) -> Graph {
    Graph {
        nodes: vec![node(src, 0.0), node(dst, 1.0)],
        edges: vec![edge(id, src, dst)],
    }
}

#[test]
fn the_screen_draws_the_core_edge_list_and_snaps_before_routing() {
    let graph = Graph {
        nodes: vec![node("a", 0.0), node("b", 1.0)],
        edges: vec![edge("ab", "a", "b")],
    };
    let screen = show(
        "ios",
        &graph,
        &mut Router::new(),
        "a",
        Place::Coordinate { lat: 1.0, lon: 0.0 },
        &trip(Environment::Simple),
        Duration::from_millis(200),
        TILE_RENDER_LIMIT,
    )
    .unwrap();
    assert_eq!(screen.edge_ids, vec!["ab".to_string()]);
    assert!(!screen.warning);
    assert!(screen.voice_issued && screen.tile_drawn);
}

#[test]
fn a_bad_destination_is_rejected_and_a_low_score_warns() {
    let graph = line("ab", "a", "b");
    let rejected = show(
        "ios",
        &graph,
        &mut Router::new(),
        "a",
        Place::Coordinate {
            lat: 120.0,
            lon: 0.0,
        },
        &trip(Environment::Simple),
        Duration::ZERO,
        Duration::ZERO,
    );
    assert_eq!(rejected, Err(ScreenError::Rejected));
    let warned = show(
        "android",
        &graph,
        &mut Router::new(),
        "a",
        Place::Node("b"),
        &trip(Environment::Complex),
        Duration::ZERO,
        Duration::ZERO,
    )
    .unwrap();
    assert!(warned.warning);
}

#[test]
fn voice_and_tile_limits_are_inclusive() {
    assert_eq!(VOICE_LIMIT, Duration::from_millis(200));
    assert_eq!(TILE_RENDER_LIMIT, Duration::from_millis(500));
    let graph = line("ab", "a", "b");
    let trip = trip(Environment::Simple);
    assert_eq!(
        show(
            "web",
            &graph,
            &mut Router::new(),
            "a",
            Place::Node("b"),
            &trip,
            Duration::from_millis(201),
            Duration::ZERO,
        ),
        Err(ScreenError::VoiceTooSlow)
    );
    assert_eq!(
        show(
            "web",
            &graph,
            &mut Router::new(),
            "a",
            Place::Node("b"),
            &trip,
            Duration::ZERO,
            Duration::from_millis(501),
        ),
        Err(ScreenError::TileTooSlow)
    );
}

#[test]
fn an_unreachable_node_returns_no_screen() {
    let graph = Graph {
        nodes: vec![node("a", 0.0), node("z", 2.0)],
        edges: vec![],
    };
    let result = show(
        "web",
        &graph,
        &mut Router::new(),
        "a",
        Place::Node("z"),
        &trip(Environment::Simple),
        Duration::ZERO,
        Duration::ZERO,
    );
    assert_eq!(result, Err(ScreenError::Unreachable));
}

#[test]
fn clear_removes_the_cache_and_the_training_pairs() {
    let mut store = open_cache(&[9u8; 32]).unwrap();
    store.put("route", b"edges").unwrap();
    let mut training = TrainingRecord::new();
    training.opt_in("scorer-1");
    training
        .record(TrainingPair {
            left: ai_smart_maps_core::scorer::Features {
                edge_count: 1,
                weight_sum: 1.0,
                hazard_sum: 0.0,
                hazard_missing: 0,
            },
            right: ai_smart_maps_core::scorer::Features {
                edge_count: 2,
                weight_sum: 2.0,
                hazard_sum: 0.0,
                hazard_missing: 0,
            },
            left_has_lower_cost: true,
        })
        .unwrap();
    clear_local(&mut store, &mut training);
    assert!(store.is_empty());
    assert!(!training.opted_in());
    assert!(training.pairs().is_empty());
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
