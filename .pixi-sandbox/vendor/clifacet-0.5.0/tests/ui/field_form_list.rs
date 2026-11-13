//! A form field submits one value, so a list cannot be one: it would be cut to one item.

#[clifacet::command(input = Labels)]
struct Cli {
    #[clifacet(long, form)]
    label: Vec<String>,
}

struct Labels {
    label: Vec<String>,
}

fn main() {}
