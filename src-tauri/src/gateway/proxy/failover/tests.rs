use super::select_next_provider_id_from_order;
use std::collections::HashSet;

fn set(ids: &[i64]) -> HashSet<i64> {
    ids.iter().copied().collect()
}

#[test]
fn select_next_provider_id_wraps_and_skips_missing() {
    let order = vec![1, 2, 3, 4];
    let current = set(&[2, 4]);

    assert_eq!(
        select_next_provider_id_from_order(4, &order, &current),
        Some(2)
    );
    assert_eq!(
        select_next_provider_id_from_order(2, &order, &current),
        Some(4)
    );
}

#[test]
fn select_next_provider_id_returns_none_when_no_candidate() {
    let order = vec![1, 2, 3];
    assert_eq!(
        select_next_provider_id_from_order(2, &order, &set(&[])),
        None
    );
    assert_eq!(
        select_next_provider_id_from_order(2, &order, &set(&[99])),
        None
    );
}

#[test]
fn select_next_provider_id_starts_from_head_when_bound_missing() {
    let order = vec![10, 20, 30];
    let current = set(&[30]);
    assert_eq!(
        select_next_provider_id_from_order(999, &order, &current),
        Some(30)
    );
}
