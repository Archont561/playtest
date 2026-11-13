//! An unknown `#[clifacet(..)]` key is rejected, and the message lists what is supported.

#[clifacet::command(input = Greeting)]
#[command(name = "greet")]
struct Cli {
    #[clifacet(logn, value_name = "NAME")]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
