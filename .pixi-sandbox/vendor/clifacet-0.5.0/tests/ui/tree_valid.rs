//! A tree, two levels of it, converts, collects and runs the chosen leaf.

use clifacet::command::{Plan, Planned};
use clifacet::{Command, Facet, Failure};

#[clifacet::command(input = Input)]
#[command(name = "todo")]
struct Cli {
    #[clifacet(subcommands)]
    command: Commands,
}

struct Input {
    command: Request,
}

#[clifacet::subcommands(input = Request, output = String)]
#[derive(Clone)]
enum Commands {
    /// Add a task.
    #[clifacet(alias = "new", visible_alias = "a")]
    Add(Add),
    #[clifacet(name = "finish", hide)]
    Done(Done),
}

enum Request {
    Add(AddInput),
    Done(DoneInput),
}

#[clifacet::command(input = AddInput)]
#[derive(Clone)]
struct Add {
    title: String,
}

struct AddInput {
    title: String,
}

#[clifacet::command(input = DoneInput)]
#[derive(Clone)]
struct Done {
    id: u32,
}

struct DoneInput {
    id: u32,
}

impl Command for Add {
    type Input = AddInput;
    type Output = &'static str;

    fn execute(&self, _input: AddInput) -> Result<&'static str, Failure> {
        Ok("added")
    }
}

impl Planned for Done {
    type Input = DoneInput;
    type Output = String;

    fn plan(&self, input: DoneInput) -> Result<Plan<'_, String>, Failure> {
        Ok(Plan::new(move || Ok(format!("done {}", input.id))).step("finish"))
    }
}

/// A planned tree of one leaf: no fallback arm is reachable, and nothing warns.
#[clifacet::subcommands(input = Only, output = String, planned)]
enum Single {
    Done(Done),
}

enum Only {
    Done(DoneInput),
}

fn main() {
    let cli = <Cli as clifacet::clap::Parser>::try_parse_from(["todo", "finish", "7"]).unwrap();
    let tree = cli.command.clone();
    let input = cli.validate().unwrap();
    assert_eq!(tree.execute(input.command).unwrap(), "done 7");

    let plan = Single::Done(Done { id: 1 }).plan(Only::Done(DoneInput { id: 1 })).unwrap();
    assert_eq!(plan.describe(), "finish");
    let _ = Add { title: String::new() }.validate().unwrap().title;
}
