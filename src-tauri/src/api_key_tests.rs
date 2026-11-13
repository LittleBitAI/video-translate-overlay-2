//! The masked key on the settings screen (R19) comes from the stored key itself.

use super::{last4, masked};

#[test]
fn the_mask_keeps_four_characters_and_never_splits_one() {
    assert_eq!(masked(last4("AIzaSyD-abcdefWXYZ")), "••••WXYZ");
    assert_eq!(last4("abc"), "abc");
    assert_eq!(last4("키가a한글b"), "a한글b");
}
