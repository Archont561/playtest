#[rstest::rstest]
fn test_version_accessible() {
    assert_eq!(playtest_core::VERSION, "0.1.0");
}
