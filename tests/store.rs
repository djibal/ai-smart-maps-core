use ai_smart_maps_core::store::LocalStore;

fn key() -> [u8; 32] {
    [7u8; 32]
}

#[test]
fn a_sealed_record_opens_with_the_same_key() {
    let mut store = LocalStore::open(&key()).unwrap();
    store.put("cache", b"route-bytes").unwrap();
    assert_eq!(store.get("cache").unwrap(), b"route-bytes");
}

#[test]
fn a_different_key_cannot_open_the_record() {
    let mut writer = LocalStore::open(&key()).unwrap();
    writer.put("cache", b"route-bytes").unwrap();
    let sealed = writer.export("cache").unwrap().to_vec();

    let mut reader = LocalStore::open(&[9u8; 32]).unwrap();
    reader.import("cache", sealed);
    assert!(reader.get("cache").is_err());
}

#[test]
fn a_short_key_is_rejected() {
    assert!(LocalStore::open(&[1u8; 16]).is_err());
}

#[test]
fn clear_removes_cache_reports_training_pairs_and_preferences() {
    let mut store = LocalStore::open(&key()).unwrap();
    for name in ["cache", "reports", "training_pairs", "preferences"] {
        store.put(name, b"secret").unwrap();
    }
    store.clear();
    assert!(store.is_empty());
    for name in ["cache", "reports", "training_pairs", "preferences"] {
        assert!(store.get(name).is_err());
    }
}

#[test]
fn rotation_before_90_days_keeps_the_record_and_the_key() {
    let mut store = LocalStore::open(&key()).unwrap();
    store.put("cache", b"route-bytes").unwrap();
    assert_eq!(
        store.rotate(&[8u8; 32], 89),
        Err(ai_smart_maps_core::store::StoreError::NotDue)
    );
    assert_eq!(store.get("cache").unwrap(), b"route-bytes");
}

#[test]
fn rotation_at_90_days_deletes_records_and_installs_the_new_key() {
    let mut store = LocalStore::open(&key()).unwrap();
    for name in ["cache", "reports", "training_pairs", "preferences"] {
        store.put(name, b"secret").unwrap();
    }
    let previous = store.export("cache").unwrap().to_vec();
    store.rotate(&[8u8; 32], 90).unwrap();
    assert!(store.is_empty());
    store.import("cache", previous);
    assert!(store.get("cache").is_err());

    store.put("cache", b"fresh").unwrap();
    let sealed = store.export("cache").unwrap().to_vec();
    let mut opened = LocalStore::open(&[8u8; 32]).unwrap();
    opened.import("cache", sealed);
    assert_eq!(opened.get("cache").unwrap(), b"fresh");
}

#[test]
fn a_short_rotation_key_is_rejected_and_the_records_stay() {
    let mut store = LocalStore::open(&key()).unwrap();
    store.put("cache", b"route-bytes").unwrap();
    assert!(store.rotate(&[1u8; 16], 90).is_err());
    assert_eq!(store.get("cache").unwrap(), b"route-bytes");
}
