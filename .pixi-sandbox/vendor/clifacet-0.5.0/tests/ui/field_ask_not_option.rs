//! `ask` fills a value no flag supplied, so the field has to be able to be missing.

#[clifacet::command(input = Greeting)]
struct Cli {
    #[clifacet(long, ask = "How many?")]
    repeat: u8,
}

struct Greeting {
    repeat: u8,
}

fn main() {}
