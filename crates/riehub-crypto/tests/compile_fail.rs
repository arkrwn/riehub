//! Compile-fail proofs for the crypto contracts.

#[test]
fn key32_has_no_debug_output() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/key32_no_debug.rs");
}

#[test]
fn hkdf_info_prefix_is_enforced_at_compile_time() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/hkdf_info_not_prefixed.rs");
}
