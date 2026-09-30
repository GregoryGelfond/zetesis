//! Checked layout arithmetic, without constructing enormous relations.

use super::*;

#[test]
fn tile_result_layout_checks_each_address_product() {
    assert_eq!(result_layout(3, 2, 2).unwrap(), (13, 104));
    assert_eq!(
        result_layout(u32::MAX - 5, 1, 1).unwrap(),
        (u32::MAX, u64::from(u32::MAX) * 4)
    );
    for (words, tiles, queries) in [
        (1, u32::MAX / 5 + 1, 1),
        (u32::MAX - 4, 1, 1),
        (1, 1, u32::MAX / 6 + 1),
    ] {
        assert_eq!(
            result_layout(words, tiles, queries).unwrap_err().kind(),
            GpuErrorKind::Capacity
        );
    }
}
