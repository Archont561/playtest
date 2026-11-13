#[test]
fn macro_expands_to_the_macros_package_version() {
    assert_eq!(clifacet_macros::version!(), env!("CARGO_PKG_VERSION"));
}
