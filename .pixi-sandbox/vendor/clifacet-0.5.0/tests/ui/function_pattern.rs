#[clifacet::command]
fn pair((left, right): (u8, u8)) -> Result<u8, clifacet::Failure> {
    Ok(left + right)
}

fn main() {}
