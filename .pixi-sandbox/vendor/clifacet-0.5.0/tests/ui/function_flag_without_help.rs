#[clifacet::command]
fn list(#[clifacet(long)] limit: Option<u32>) -> Result<u32, clifacet::Failure> {
    Ok(limit.unwrap_or(0))
}

fn main() {}
