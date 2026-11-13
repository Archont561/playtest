#[clifacet::command(bridge = build)]
fn list(#[clifacet(long, help = "How many")] limit: Option<u32>) -> Result<u32, clifacet::Failure> {
    Ok(limit.unwrap_or(0))
}

fn main() {}
