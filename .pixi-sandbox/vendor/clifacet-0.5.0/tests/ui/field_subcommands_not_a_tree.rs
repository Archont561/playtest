//! A `subcommands` field whose type is clap's subcommand enum, not a declared tree.

#[clifacet::command(input = Input)]
struct Cli {
    #[clifacet(subcommands)]
    command: Commands,
}

struct Input {
    command: Request,
}

#[derive(clap::Subcommand)]
enum Commands {
    Done { id: u32 },
}

enum Request {
    Done { id: u32 },
}

fn main() {}
