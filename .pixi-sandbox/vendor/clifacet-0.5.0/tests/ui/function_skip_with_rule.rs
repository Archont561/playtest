#[clifacet::command]
fn announce(
    #[clifacet(skip, trim)]
    globals: clifacet::Globals,
) -> Result<(), clifacet::Failure> {
    Ok(())
}

fn main() {}
