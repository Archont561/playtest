#[rstest::rstest]
fn test_version_accessible() {
    assert_eq!(playtest_assets::VERSION, "0.1.0");
}
