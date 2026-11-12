#[rstest::rstest]
fn test_version_matches_workspace_crates() {
    assert_eq!(playtest_cli::VERSION, "0.1.0");
    assert_eq!(playtest_cli::VERSION, playtest_core::VERSION);
    assert_eq!(playtest_cli::VERSION, playtest_assets::VERSION);
}
