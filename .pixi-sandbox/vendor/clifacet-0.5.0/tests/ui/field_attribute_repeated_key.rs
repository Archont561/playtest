//! Declaring the same key twice is rejected rather than resolved by order.

#[clifacet::command(input = Greeting)]
#[command(name = "greet")]
struct Cli {
    #[clifacet(long, long = "other")]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
