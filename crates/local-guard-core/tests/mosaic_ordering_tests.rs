//! Tests deterministic temporal ordering for 3x3 mosaics.

use local_guard_core::deterministic_tile_order;

#[test]
fn mosaic_ordering_tests_enforces_row_major_temporal_order() {
    let order = deterministic_tile_order(9).expect("ordering should be generated");
    assert_eq!(order, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
}
