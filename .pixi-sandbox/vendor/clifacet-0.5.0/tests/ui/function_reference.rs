#[clifacet::command]
fn greet(name: &str) -> Result<String, clifacet::Failure> {
    Ok(name.to_owned())
}

fn main() {}
