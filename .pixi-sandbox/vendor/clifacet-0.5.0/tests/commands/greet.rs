//! `greet`: one command per file, registered by `commands!` in the folder's `mod.rs`.

use clifacet::Failure;
use clifacet::command::Plan;

/// Say hello.
#[derive(Clone)]
#[clifacet::command(input = Greeting)]
#[command(about = "Say hello")]
pub(crate) struct Greet {
    /// Who to greet.
    pub name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Greeting {
    pub name: String,
}

impl clifacet::command::Planned for Greet {
    type Input = Greeting;
    type Output = String;

    fn plan(&self, input: Greeting) -> Result<Plan<'_, String>, Failure> {
        let step = format!("hello {}", input.name);
        Ok(Plan::new(move || Ok(format!("hello {}", input.name))).step(step))
    }
}
