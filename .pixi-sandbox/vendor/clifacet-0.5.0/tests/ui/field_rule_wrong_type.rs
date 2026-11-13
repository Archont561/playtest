//! A rule on a type it cannot apply to says which rule and which type.

#[clifacet::command(input = Greeting)]
struct Cli {
    #[clifacet(long, trim)]
    repeat: Option<u8>,

    #[clifacet(long, secret)]
    token: Option<String>,
}

struct Greeting {
    repeat: Option<u8>,
    token: Option<String>,
}

fn main() {}
