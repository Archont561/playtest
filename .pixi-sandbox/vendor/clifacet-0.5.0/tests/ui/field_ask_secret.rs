//! Asking for a credential would echo it as it is typed.

use clifacet::secret::Secret;

#[clifacet::command(input = Login)]
struct Cli {
    #[clifacet(long, ask = "Token?", secret)]
    token: Option<Secret<String>>,
}

struct Login {
    token: Option<Secret<String>>,
}

fn main() {}
