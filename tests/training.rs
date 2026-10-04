use ai_smart_maps_core::scorer::Features;
use ai_smart_maps_core::training::{TrainingPair, TrainingRecord};

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
