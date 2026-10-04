use ai_smart_maps_core::contracts::TileRecord;
use ai_smart_maps_core::graph::{Graph, Node};
use ai_smart_maps_core::tiles::{admit, Origin, TileCache};

fn node(id: &str) -> Node {
    Node {
        id: id.to_string(),
        lat: 0.0,
        lon: 0.0,
    }
}

fn tile(id: &str, observed_at: &str, nodes: &[&str], signature: Option<&str>) -> TileRecord {
    TileRecord {
        id: id.to_string(),
        observed_at: observed_at.to_string(),
        graph: Graph {
            nodes: nodes.iter().copied().map(node).collect(),
            edges: Vec::new(),
        },
        signature: signature.map(str::to_string),
    }
}

fn accepts(tile: &TileRecord) -> bool {
    tile.signature.as_deref() == Some("source-ok")
}

#[test]
fn a_local_tile_may_omit_a_signature() {
    let tile = tile("local", "2026-10-04T00:00:00Z", &["a"], None);
    assert!(admit(&tile, Origin::Local, |_| panic!("no signature to check")).is_ok());
}

#[test]
fn a_present_signature_must_pass_the_map_source_check() {
    let good = tile("signed", "2026-10-04T00:00:00Z", &["a"], Some("source-ok"));
    let bad = tile("signed", "2026-10-04T00:00:00Z", &["a"], Some("nope"));
    assert!(admit(&good, Origin::Local, accepts).is_ok());
    assert!(admit(&bad, Origin::Local, accepts).is_err());
    assert!(admit(&bad, Origin::Fetched, accepts).is_err());
}

#[test]
fn a_fetched_tile_without_a_signature_is_dropped_and_the_cache_stays() {
    let mut cache = TileCache::new();
    let stored = tile("kept", "2026-01-01T00:00:00Z", &["a", "b"], None);
    cache.insert(stored, Origin::Local, |_| true).unwrap();

    let fetched = tile("remote", "2026-10-04T00:00:00Z", &["a", "b"], None);
    assert!(cache.insert(fetched, Origin::Fetched, |_| true).is_err());
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.newest_covering("a", "b").unwrap().id, "kept");
}

#[test]
fn the_newest_tile_whose_graph_contains_both_nodes_is_chosen() {
    let mut cache = TileCache::new();
    cache
        .insert(
            tile("old-both", "2026-01-01T00:00:00Z", &["a", "b"], None),
            Origin::Local,
            |_| true,
        )
        .unwrap();
    cache
        .insert(
            tile("new-one", "2026-10-04T00:00:00Z", &["a"], None),
            Origin::Local,
            |_| true,
        )
        .unwrap();
    cache
        .insert(
            tile(
                "new-both",
                "2026-08-01T00:00:00Z",
                &["a", "b"],
                Some("source-ok"),
            ),
            Origin::Fetched,
            accepts,
        )
        .unwrap();
    cache
        .insert(
            tile("tie-z", "2026-06-01T00:00:00Z", &["a", "b"], None),
            Origin::Local,
            |_| true,
        )
        .unwrap();

    assert_eq!(cache.newest_covering("a", "b").unwrap().id, "new-both");
    assert!(cache.newest_covering("a", "missing").is_none());
}

#[test]
fn an_equal_observation_time_keeps_the_smaller_tile_id() {
    let mut cache = TileCache::new();
    for id in ["m", "b"] {
        cache
            .insert(
                tile(id, "2026-06-01T00:00:00Z", &["a", "c"], None),
                Origin::Local,
                |_| true,
            )
            .unwrap();
    }
    assert_eq!(cache.newest_covering("a", "c").unwrap().id, "b");
}
