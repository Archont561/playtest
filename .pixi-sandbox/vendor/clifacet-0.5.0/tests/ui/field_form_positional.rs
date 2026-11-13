//! A form submits each answer as its flag's value, so only a named option is a form field.

#[clifacet::command(input = Greeting)]
struct Cli {
    #[clifacet(value_name = "NAME", form)]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
