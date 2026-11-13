#[rstest::rstest]
fn exposed_version_matches_the_published_package_metadata() {
    assert_eq!(clifacet::VERSION, env!("CARGO_PKG_VERSION"));
    assert_eq!(clifacet::version(), clifacet::VERSION);
}
