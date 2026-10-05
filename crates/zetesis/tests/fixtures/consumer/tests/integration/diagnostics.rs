//! A forwarded macro error must name the consumer's offending token.

#[test]
fn a_macro_error_names_the_consumer_token() {
    trybuild::TestCases::new().compile_fail("tests/fixtures/invalid-macro.rs");
}
