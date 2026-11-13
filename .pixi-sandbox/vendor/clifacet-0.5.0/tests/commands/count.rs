//! `count`: the same form, with a different output type, converted by `output = String`.

use clifacet::Failure;
use clifacet::command::Plan;

/// Count to a number.
#[derive(Clone)]
#[clifacet::command(input = Counted)]
#[command(about = "Count")]
pub(crate) struct Count {
    /// How far to count.
    pub times: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct Counted {
    pub times: u32,
}

impl clifacet::command::Planned for Count {
    type Input = Counted;
    type Output = String;

    fn plan(&self, input: Counted) -> Result<Plan<'_, String>, Failure> {
        let times = input.times;
        Ok(Plan::new(move || Ok(times.to_string())).step(times.to_string()))
    }
}
