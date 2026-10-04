use ai_smart_maps_core::contracts::TileRecord;
use ai_smart_maps_core::graph::{Graph, Node};
use ai_smart_maps_core::tiles::{admit, signed_bytes, Origin, TileCache};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};

const SECRET: [u8; 32] = [7u8; 32];

fn public_key() -> [u8; 32] {
    SigningKey::from_bytes(&SECRET).verifying_key().to_bytes()
}

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

fn signed(id: &str, observed_at: &str, nodes: &[&str]) -> TileRecord {
    let mut record = tile(id, observed_at, nodes, None);
    let message = signed_bytes(&record).unwrap();
    let signature = SigningKey::from_bytes(&SECRET).sign(&message);
    record.signature = Some(STANDARD.encode(signature.to_bytes()));
    record
}

#[test]
fn a_local_tile_may_omit_a_signature() {
    let tile = tile("local", "2026-10-04T00:00:00Z", &["a"], None);
    assert!(admit(&tile, Origin::Local, &public_key()).is_ok());
}

#[test]
fn a_present_signature_must_verify_as_ed25519() {
    let good = signed("signed", "2026-10-04T00:00:00Z", &["a"]);
    let mut bad = good.clone();
    bad.observed_at = "2026-10-05T00:00:00Z".to_string();
    let key = public_key();
    assert!(admit(&good, Origin::Local, &key).is_ok());
    assert!(admit(&good, Origin::Fetched, &key).is_ok());
    assert!(admit(&bad, Origin::Fetched, &key).is_err());
    assert!(admit(&good, Origin::Fetched, &[1u8; 32]).is_err());
    assert!(admit(&good, Origin::Fetched, &[0u8; 31]).is_err());
}

#[test]
fn a_fetched_tile_without_a_signature_is_dropped_and_the_cache_stays() {
    let mut cache = TileCache::new();
    let stored = tile("kept", "2026-01-01T00:00:00Z", &["a", "b"], None);
    cache.insert(stored, Origin::Local, &public_key()).unwrap();

    let fetched = tile("remote", "2026-10-04T00:00:00Z", &["a", "b"], None);
    assert!(cache
        .insert(fetched, Origin::Fetched, &public_key())
        .is_err());
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.newest_covering("a", "b").unwrap().id, "kept");
}

#[test]
fn the_newest_tile_whose_graph_contains_both_nodes_is_chosen() {
    let mut cache = TileCache::new();
    let key = public_key();
    cache
        .insert(
            tile("old-both", "2026-01-01T00:00:00Z", &["a", "b"], None),
            Origin::Local,
            &key,
        )
        .unwrap();
    cache
        .insert(
            tile("new-one", "2026-10-04T00:00:00Z", &["a"], None),
            Origin::Local,
            &key,
        )
        .unwrap();
    cache
        .insert(
            signed("new-both", "2026-08-01T00:00:00Z", &["a", "b"]),
            Origin::Fetched,
            &key,
        )
        .unwrap();
    cache
        .insert(
            tile("tie-z", "2026-06-01T00:00:00Z", &["a", "b"], None),
            Origin::Local,
            &key,
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
                &public_key(),
            )
            .unwrap();
    }
    assert_eq!(cache.newest_covering("a", "c").unwrap().id, "b");
}
