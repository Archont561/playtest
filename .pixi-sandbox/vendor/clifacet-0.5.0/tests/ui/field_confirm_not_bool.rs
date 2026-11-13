//! `confirm` puts a set flag to the user, so it is declared on a `bool`.

#[clifacet::command(input = Greeting)]
struct Cli {
    #[clifacet(long, confirm = "Repeat that many times?")]
    repeat: u8,
}

struct Greeting {
    repeat: u8,
}

fn main() {}
