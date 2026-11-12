//! Proves `Secret<T>` cannot be serialized (S2).

#[test]
fn secret_does_not_serialize() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/secret_no_serialize.rs");
}
