//! A command written as a function is the same command (CLI-52, decision-19 item 6).
//!
//! The claim being proven is equality, not resemblance: the same arguments, the same help and
//! the same parsed input as the struct form a person writes by hand. Anything less and the two
//! forms would drift, which is the one thing sugar over an existing expansion must not do.
//!
//! The leaves live in modules, as they do in an application, because that is the shape
//! `commands!` reads: the file stem names the command, and the type in it is that stem in
//! Pascal case. A function command has to produce the same type under the same name.

use clap::Parser as _;
use clifacet::{Command, Facet, Failure, Handler};

// --- one command, both ways ---------------------------------------------------------

/// `rename`: the function form.
mod rename {
    use clifacet::Failure;

    /// Rename a task.
    ///
    /// The new title replaces the old one, and the old one is gone.
    #[clifacet::command]
    pub fn rename(
        #[clifacet(long, value_name = "ID", help = "The task to rename")] id: u32,
        #[clifacet(long, help = "The new title")] title: String,
    ) -> Result<String, Failure> {
        Ok(format!("{id}: {title}"))
    }
}

// The same declaration, written as a struct: the doc comment is the `about`, exactly as it is
// for the function, which is what makes the comparison a comparison.

/// Rename a task.
///
/// The new title replaces the old one, and the old one is gone.
#[clifacet::command(input = RenameHandWritten)]
struct RenameDeclared {
    #[clifacet(long, value_name = "ID", help = "The task to rename")]
    id: u32,
    #[clifacet(long, help = "The new title")]
    title: String,
}

struct RenameHandWritten {
    id: u32,
    title: String,
}

impl Command for RenameDeclared {
    type Input = RenameHandWritten;
    type Output = String;

    fn execute(&self, input: RenameHandWritten) -> Result<String, Failure> {
        Ok(format!("{}: {}", input.id, input.title))
    }
}

/// `echo`: a positional, declared by writing it bare.
mod echo {
    use clifacet::Failure;

    /// Say what was given.
    #[clifacet::command]
    pub fn echo(#[clifacet(help = "What to say")] words: String) -> Result<String, Failure> {
        Ok(words)
    }
}

#[test]
fn a_function_command_renders_the_same_help_as_the_struct_that_declares_it() {
    let mut generated = <rename::Rename as Facet>::command();
    let mut hand_written = <RenameDeclared as Facet>::command();

    assert_eq!(generated.get_name(), hand_written.get_name());
    assert_eq!(
        generated.get_about().map(ToString::to_string),
        // clap drops a one-line about's closing period, for a struct and a function alike.
        Some(String::from("Rename a task")),
    );
    assert_eq!(
        generated.render_help().to_string(),
        hand_written.render_help().to_string(),
    );
}

#[test]
fn a_function_command_produces_the_same_input_and_the_same_output() {
    // The leaf comes from clap, which is how an application gets one: the generated struct is
    // private to its module so that `validate` stays the only way to an input.
    let leaf = rename::Rename::try_parse_from(["rename", "--id", "7", "--title", "Ship it"])
        .expect("clap parses the generated command");
    let command = leaf.clone();
    let input = leaf.validate().expect("the generated input accepts it");
    assert_eq!(input.id, 7);
    assert_eq!(input.title, "Ship it");

    let ours = command.execute(input).expect("the function runs");
    let theirs = RenameDeclared {
        id: 7,
        title: String::from("Ship it"),
    }
    .execute(RenameHandWritten {
        id: 7,
        title: String::from("Ship it"),
    })
    .expect("the struct form runs");
    assert_eq!(ours, theirs);
}

#[test]
fn a_bare_parameter_is_a_positional_as_a_bare_field_is() {
    let parsed = echo::Echo::try_parse_from(["echo", "hello"]).expect("parsed");
    let input = parsed.validate().expect("validated");
    assert_eq!(input.words, "hello");

    // A positional is not a flag: the same command refuses `--words`.
    assert!(echo::Echo::try_parse_from(["echo", "--words", "hello"]).is_err());
}

#[test]
fn the_function_is_still_a_function_the_application_can_call() {
    assert_eq!(
        echo::echo(String::from("hi")).expect("the function itself"),
        "hi"
    );
}

// --- the context reaches the function ---------------------------------------------

use std::cell::RefCell;

/// What a command may need: here, the records it writes into.
#[derive(Default)]
struct Store {
    records: RefCell<Vec<String>>,
}

/// `add`: the context is the last parameter.
mod add {
    use clifacet::Failure;

    use crate::Store;

    /// Put a record on the shelf.
    #[clifacet::command]
    pub fn add(
        #[clifacet(long, help = "The record to add")] record: String,
        cx: &Store,
    ) -> Result<usize, Failure> {
        cx.records.borrow_mut().push(record);
        Ok(cx.records.borrow().len())
    }
}

#[test]
fn a_context_parameter_is_handed_to_the_function_by_the_handler() {
    let store = Store::default();
    let leaf = add::Add::try_parse_from(["add", "--record", "CLI-52"]).expect("parsed");
    let command = leaf.clone();
    let input = leaf.validate().expect("validated");
    assert_eq!(command.handle(input, &store).expect("handled"), 1);
    assert_eq!(store.records.borrow().as_slice(), &[String::from("CLI-52")]);
}

// --- one folder, both forms -------------------------------------------------------

/// The tree: a function leaf and a struct leaf in one folder, which is what AC#3 asks for.
#[clifacet::subcommands(input = ShelfRequest, output = usize, context = Store)]
#[derive(Clone)]
enum Shelf {
    /// Put a record on the shelf.
    Add(add::Add),
    /// Count what is on the shelf: written as a struct, as it always was.
    Count(Count),
}

enum ShelfRequest {
    Add(add::AddInput),
    Count(CountInput),
}

#[clifacet::command(input = CountInput)]
#[derive(Clone)]
struct Count {
    #[clifacet(long, help = "Count at most this many")]
    limit: Option<usize>,
}

struct CountInput {
    limit: Option<usize>,
}

impl Handler<Store> for Count {
    type Input = CountInput;
    type Output = usize;

    fn handle(&self, input: CountInput, store: &Store) -> Result<usize, Failure> {
        let total = store.records.borrow().len();
        Ok(input.limit.map_or(total, |limit| total.min(limit)))
    }
}

/// The root, so a run has somewhere to start: the tree and nothing else.
#[clifacet::command(input = ShelfInput)]
#[command(name = "shelf")]
struct ShelfRoot {
    #[clifacet(subcommands)]
    command: Shelf,
}

struct ShelfInput {
    command: ShelfRequest,
}

#[test]
fn a_tree_mixes_a_function_leaf_with_a_struct_leaf() {
    let store = Store::default();
    let root = ShelfRoot::try_parse_from(["shelf", "add", "--record", "one"]).expect("parsed");
    let tree = root.command.clone();
    let request = root.validate().expect("validated").command;

    assert_eq!(
        tree.handle(request, &store).expect("handled"),
        1,
        "the function leaf ran through the tree's own Handler",
    );
    assert_eq!(store.records.borrow().as_slice(), &[String::from("one")]);

    // And the tree's help lists both forms the same way: the function's doc comment is the
    // line, exactly as the struct's is.
    let help = <ShelfRoot as Facet>::command().render_help().to_string();
    assert!(help.contains("add"), "{help}");
    assert!(help.contains("Count what is on the shelf"), "{help}");
}

// --- what a parameter can declare beyond the grammar -----------------------------

/// `greet`: a filled value, a question, and a rule across the input.
mod greet {
    use clifacet::Failure;

    /// Greet someone.
    #[clifacet::command(check = crate::check_greeting)]
    pub fn greet(
        #[clifacet(
            long,
            help = "Who to greet",
            ask = "Who should I greet?",
            fill = "world",
            trim,
            non_empty
        )]
        name: String,
        #[clifacet(long, help = "How many times", default_value_t = 1)] repeat: u8,
    ) -> Result<String, Failure> {
        Ok(format!("{name} x{repeat}"))
    }
}

/// The one rule the declarations cannot state: `--repeat` is at least one.
fn check_greeting(input: &greet::GreetInput) -> Result<(), Failure> {
    if input.repeat == 0 {
        return Err(Failure::validation("--repeat must be at least 1"));
    }
    Ok(())
}

#[test]
fn a_filled_parameter_is_the_value_and_the_flag_stays_optional() {
    let parsed = greet::Greet::try_parse_from(["greet"]).expect("no flag is needed");
    let input = Facet::validate(parsed).expect("the fallback supplied the value");

    // `fill` is the declaration the struct form makes on an `Option` field, and the body still
    // receives a `String`: the parameter's type is the value, not the flag's.
    assert_eq!(input.name, "world");
    assert_eq!(input.repeat, 1);
}

#[test]
fn a_question_declared_on_a_parameter_is_asked_before_the_fallback() {
    use clifacet::policy::{Capabilities, Mode, Policy, Request as ModeRequest};
    use clifacet::prompt::{Answer, Collect as _, Scripted, Session};

    let plan = Policy::up_to(Mode::Interactive)
        .resolve(ModeRequest::none(), Capabilities::terminal())
        .expect("a terminal may be asked");
    let mut session = Session::start(plan, || {
        Box::new(Scripted::new([Answer::Text(String::from("  ada "))]))
    });

    let input = greet::Greet::try_parse_from(["greet"])
        .expect("parsed")
        .collect(&mut session)
        .and_then(Facet::validate)
        .expect("the answer was trimmed and accepted");
    assert_eq!(input.name, "ada", "an answer goes through the same rules");
}

#[test]
fn a_check_across_the_whole_input_is_forwarded() {
    let parsed = greet::Greet::try_parse_from(["greet", "--repeat", "0"]).expect("parsed");
    let refused = Facet::validate(parsed).expect_err("zero is refused by the check");
    assert_eq!(refused.kind(), clifacet::FailureKind::Validation);
    assert!(refused.to_string().contains("at least 1"), "{refused}");
}

// --- a value the command carries, not the input -------------------------------------

/// `announce`: a skipped parameter beside an input one.
mod announce {
    use clifacet::Failure;

    /// Announce something.
    #[clifacet::command]
    pub fn announce(
        #[clifacet(long, help = "What to announce")] what: String,
        #[command(flatten)]
        #[clifacet(skip)]
        globals: clifacet::Globals,
    ) -> Result<String, Failure> {
        Ok(what)
    }
}

#[test]
fn a_skipped_parameter_is_parsed_but_is_not_input() {
    let parsed = announce::Announce::try_parse_from(["announce", "--what", "news", "--dry-run"])
        .expect("the skipped group still parses its flag");

    // The value is on the command — the application reads it there — and the input the command
    // runs on does not mention it: `skip` is the difference between the two.
    assert!(parsed.globals.dry_run);
    let input = Facet::validate(parsed).expect("a valid input");
    assert_eq!(
        input,
        announce::AnnounceInput {
            what: String::from("news")
        }
    );
}

#[test]
fn a_skipped_parameter_carries_no_rule() {
    let parsed = announce::Announce::try_parse_from(["announce", "--what", ""])
        .expect("the empty value is the check's business, not the parser's");
    let input = Facet::validate(parsed).expect("no rule was declared for it");
    assert_eq!(input.what, "");
}
