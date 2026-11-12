#[rstest::rstest]
fn test_version_accessible() {
    assert_eq!(playtest::VERSION, "0.1.0");
    assert_eq!(playtest::VERSION, playtest_core::VERSION);
    assert_eq!(playtest::VERSION, playtest_macros::VERSION);
}
