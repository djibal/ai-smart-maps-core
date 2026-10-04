use ai_smart_maps_core::budget::{Budget, MAPS_AND_MODELS_BUDGET};

#[test]
fn the_default_budget_is_two_gigabytes() {
    assert_eq!(Budget::new().limit(), MAPS_AND_MODELS_BUDGET);
    assert_eq!(MAPS_AND_MODELS_BUDGET, 2 * 1024 * 1024 * 1024);
}

#[test]
fn an_insert_past_the_budget_removes_the_oldest_tile() {
    let mut budget = Budget::with_limit(25);
    budget.insert_tile("older", "2026-01-01T00:00:00Z", 10);
    budget.insert_tile("middle", "2026-06-01T00:00:00Z", 10);
    budget.insert_tile("newer", "2026-10-01T00:00:00Z", 10);
    assert_eq!(
        budget.tile_ids(),
        vec!["middle".to_string(), "newer".to_string()]
    );
    assert_eq!(budget.used(), 20);
}

#[test]
fn a_later_observation_replaces_the_same_tile_id() {
    let mut budget = Budget::with_limit(15);
    budget.insert_tile("tile-a", "2026-01-01T00:00:00Z", 10);
    budget.insert_tile("tile-a", "2025-01-01T00:00:00Z", 10);
    assert_eq!(budget.tile_ids(), vec!["tile-a".to_string()]);
    assert_eq!(budget.used(), 10);
    budget.insert_tile("tile-a", "2026-08-01T00:00:00Z", 12);
    budget.insert_tile("tile-b", "2026-09-01T00:00:00Z", 10);
    assert_eq!(budget.tile_ids(), vec!["tile-b".to_string()]);
    assert_eq!(budget.used(), 10);
}

#[test]
fn the_active_scorer_stays_while_rollback_is_required() {
    let mut budget = Budget::with_limit(15);
    budget.insert_model("scorer-0", "2026-01-01T00:00:00Z", 10, None, false);
    budget.insert_model(
        "scorer-1",
        "2026-06-01T00:00:00Z",
        10,
        Some("scorer-0"),
        true,
    );
    budget.insert_tile("old", "2026-01-01T00:00:00Z", 10);
    budget.insert_tile("new", "2026-10-01T00:00:00Z", 10);
    assert!(budget.tile_ids().is_empty());
    assert_eq!(
        budget.model_ids(),
        vec!["scorer-0".to_string(), "scorer-1".to_string()]
    );
    assert!(budget.used() > budget.limit());
}

#[test]
fn a_scorer_without_rollback_leaves_after_the_tiles() {
    let mut budget = Budget::with_limit(15);
    budget.insert_tile("old", "2026-01-01T00:00:00Z", 10);
    budget.insert_model("scorer-1", "2026-06-01T00:00:00Z", 20, None, true);
    assert!(budget.tile_ids().is_empty());
    assert!(budget.model_ids().is_empty());
    assert_eq!(budget.used(), 0);
}
