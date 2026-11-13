//! `greet`, registered in a folder whose commands share a context and a global option.

use clifacet::{Failure, Handler};

use super::Store;

/// Say hello, loudly when the application is loud.
#[clifacet::command(input = Greeting)]
#[derive(Clone)]
pub struct Greet {
    /// Who to greet.
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Greeting {
    pub name: String,
}

impl Handler<Store> for Greet {
    type Input = Greeting;
    type Output = String;

    fn handle(&self, input: Greeting, store: &Store) -> Result<String, Failure> {
        let greeting = format!("hello {}", input.name);
        Ok(if store.loud {
            greeting.to_uppercase()
        } else {
            greeting
        })
    }
}
