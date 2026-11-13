#[clifacet::command]
fn ping(#[clifacet(long, help = "Say more")] loud: bool) -> String {
    String::from("pong")
}

fn main() {}
