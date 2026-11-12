//! Constant-time comparison (security spec, section 1).

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    aws_lc_rs::constant_time::verify_slices_are_equal(a, b).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn ct_eq_matches_eq_for_random_inputs(
            a in proptest::collection::vec(any::<u8>(), 0..64),
            b in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            assert_eq!(ct_eq(&a, &b), a == b);
        }
    }

    #[test]
    fn ct_eq_is_exact_for_fixed_inputs() {
        assert!(ct_eq(b"", b""));
        assert!(ct_eq(b"same", b"same"));
        assert!(!ct_eq(b"same", b"sameX"));
        assert!(!ct_eq(b"one", b"two"));
    }
}
