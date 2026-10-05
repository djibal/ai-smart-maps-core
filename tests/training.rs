use ai_smart_maps_core::scorer::Features;
use ai_smart_maps_core::training::{TrainingPair, TrainingRecord, TRAINING_PAIR_CAP};

fn sample() -> TrainingPair {
    TrainingPair {
        left: Features {
            edge_count: 2,
            weight_sum: 5.0,
            hazard_sum: 0.5,
            hazard_missing: 1,
        },
        right: Features {
            edge_count: 3,
            weight_sum: 9.0,
            hazard_sum: 0.0,
            hazard_missing: 0,
        },
        left_has_lower_cost: true,
    }
}

#[test]
fn a_pair_is_refused_before_opt_in() {
    let mut record = TrainingRecord::new();
    assert!(record.record(sample()).is_err());
    assert!(record.pairs().is_empty());
    assert!(record.model_version_id().is_none());
}

#[test]
fn an_opted_in_record_keeps_the_pair_and_the_model_version() {
    let mut record = TrainingRecord::new();
    record.opt_in("scorer-1");
    record.record(sample()).unwrap();
    assert!(record.opted_in());
    assert_eq!(record.model_version_id(), Some("scorer-1"));
    assert_eq!(record.pairs().len(), 1);
    assert!(record.pairs()[0].left_has_lower_cost);
    assert_eq!(record.pairs()[0].left.edge_count, 2);
}

#[test]
fn the_cap_drops_the_oldest_pair_and_a_repeated_pair_is_kept_once() {
    assert_eq!(TrainingRecord::new().cap(), TRAINING_PAIR_CAP);
    assert_eq!(TRAINING_PAIR_CAP, 10_000);

    let mut record = TrainingRecord::with_cap(2);
    record.opt_in("scorer-1");
    record.record(sample()).unwrap();
    record.record(sample()).unwrap();
    assert_eq!(record.pairs().len(), 1);

    let mut second = sample();
    second.left.edge_count = 7;
    let mut third = sample();
    third.left.edge_count = 8;
    record.record(second.clone()).unwrap();
    record.record(third.clone()).unwrap();
    assert_eq!(record.pairs(), &[second, third]);
}

#[test]
fn the_record_round_trips_and_a_record_with_pairs_but_no_opt_in_is_refused() {
    let mut record = TrainingRecord::with_cap(3);
    record.opt_in("scorer-1");
    record.record(sample()).unwrap();
    let decoded = TrainingRecord::decode(&record.encode()).unwrap();
    assert_eq!(decoded, record);
    assert_eq!(decoded.cap(), 3);

    let empty = TrainingRecord::new();
    assert_eq!(TrainingRecord::decode(&empty.encode()), Some(empty));

    let mut bytes = record.encode();
    bytes[0] = 0;
    assert_eq!(TrainingRecord::decode(&bytes), None);
    assert_eq!(TrainingRecord::decode(&record.encode()[..10]), None);
}

#[test]
fn clearing_the_opt_in_deletes_the_pairs() {
    let mut record = TrainingRecord::new();
    record.opt_in("scorer-1");
    record.record(sample()).unwrap();
    record.clear_opt_in();
    assert!(!record.opted_in());
    assert!(record.pairs().is_empty());
    assert!(record.model_version_id().is_none());
    assert!(record.record(sample()).is_err());
}
