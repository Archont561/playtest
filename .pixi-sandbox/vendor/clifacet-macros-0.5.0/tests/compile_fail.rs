#[test]
fn version_macro_rejects_arguments() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/version_with_arguments.rs");
}
