//! Commands receive the application's context beside their input (decision-19 item 2, CLI-47).
//!
//! The context is what a command depends on — a store, a client, a clock — and the input
//! is what the user supplied. This file is its own crate, outside clifacet, which is the
//! position an application is in: it implements `Handler` for its own commands beside the
//! library's blanket impl for every `Command`.

use std::cell::{Cell, RefCell};

use clifacet::command::{Plan, PlannedWith};
use clifacet::{Command, Failure, Handler};

/// The application's dependencies: a list of records, and how often it was opened.
#[derive(Default)]
struct Store {
    records: RefCell<Vec<String>>,
}

/// A command written before contexts existed: it must not change.
struct Count;

impl Command for Count {
    type Input = Vec<String>;
    type Output = usize;

    fn execute(&self, input: Vec<String>) -> Result<usize, Failure> {
        Ok(input.len())
    }
}

/// A command that needs the store.
struct Add;

impl Handler<Store> for Add {
    type Input = String;
    type Output = usize;

    fn handle(&self, input: String, store: &Store) -> Result<usize, Failure> {
        store.records.borrow_mut().push(input);
        Ok(store.records.borrow().len())
    }
}

#[test]
fn an_existing_command_is_a_handler_under_any_context() {
    let input = vec![String::from("a"), String::from("b")];
    assert_eq!(
        Handler::<Store>::handle(&Count, input.clone(), &Store::default()).unwrap(),
        2
    );
    assert_eq!(Handler::<()>::handle(&Count, input, &()).unwrap(), 2);
}

#[test]
fn an_application_handler_receives_the_context_beside_its_input() {
    let store = Store::default();
    assert_eq!(Add.handle(String::from("first"), &store).unwrap(), 1);
    assert_eq!(Add.handle(String::from("second"), &store).unwrap(), 2);
    assert_eq!(*store.records.borrow(), ["first", "second"]);
}

#[test]
fn a_plain_handler_refuses_to_preview_and_runs_nothing() {
    let store = Store::default();
    let refused = Add.preview(String::from("first"), &store).unwrap_err();
    assert_eq!(refused.kind(), clifacet::FailureKind::Validation);
    assert!(store.records.borrow().is_empty());
}

// --- the planned twin ------------------------------------------------------------

/// Removes a record by name: the plan describes it, and its work borrows the store.
struct Remove {
    opened: Cell<u8>,
}

#[clifacet::planned]
impl PlannedWith<Store> for Remove {
    type Input = String;
    type Output = usize;

    fn plan<'a>(&'a self, name: String, store: &'a Store) -> Result<Plan<'a, usize>, Failure> {
        self.opened.set(self.opened.get() + 1);
        if !store.records.borrow().contains(&name) {
            return Err(Failure::validation(format!(
                "there is no record named {name}"
            )));
        }
        let step = format!("remove {name}");
        Ok(Plan::new(move || {
            store.records.borrow_mut().retain(|record| *record != name);
            Ok(store.records.borrow().len())
        })
        .step(step))
    }
}

fn stocked() -> Store {
    let store = Store::default();
    store
        .records
        .borrow_mut()
        .extend(["a", "b"].map(String::from));
    store
}

#[test]
fn a_planned_handler_previews_its_plan_without_running_it() {
    let store = stocked();
    let remove = Remove {
        opened: Cell::new(0),
    };
    let plan = remove.preview(String::from("a"), &store).unwrap();
    assert_eq!(plan.describe(), "remove a");
    drop(plan);
    assert_eq!(*store.records.borrow(), ["a", "b"]);
}

#[test]
fn a_planned_handler_runs_through_the_plan_it_previews() {
    let store = stocked();
    let remove = Remove {
        opened: Cell::new(0),
    };
    assert_eq!(remove.handle(String::from("a"), &store).unwrap(), 1);
    assert_eq!(*store.records.borrow(), ["b"]);
    // One plan built for one run: there is no second path to drift from the preview.
    assert_eq!(remove.opened.get(), 1);
    let refused = remove.handle(String::from("zzz"), &store).unwrap_err();
    assert_eq!(refused.message(), "there is no record named zzz");
}

// --- the runner builds the context ------------------------------------------------

use std::rc::Rc;

use clifacet::policy::Capabilities;
use clifacet::prompt::{Answer, Confirmation, Prompter, Scripted, Session};
use clifacet::{App, Globals};

/// The application's store: the open tasks, and a log of what was marked done that the
/// test reads back after the runner has dropped the store itself.
#[derive(Debug)]
struct Tasks {
    open: Vec<String>,
    done: Rc<RefCell<Vec<String>>>,
}

/// `done [--task NAME]`: marks an open task done, asking which one when none was named.
#[clifacet::command(input = DoneInput, collect = pick_task, check = one_word)]
#[derive(Clone)]
#[command(name = "done", version = "1.0.0")]
struct Done {
    /// The task to mark done.
    #[clifacet(long, trim, non_empty)]
    task: Option<String>,

    #[command(flatten)]
    #[clifacet(skip)]
    globals: Globals,
}

struct DoneInput {
    task: Option<String>,
}

/// Validation: a task name is one word. A rejected value must not open the store.
fn one_word(input: &DoneInput) -> Result<(), Failure> {
    match &input.task {
        Some(task) if task.contains(' ') => Err(Failure::validation("a task name is one word")),
        _ => Ok(()),
    }
}

/// The question reads the context: the choices are the store's open tasks. It reaches for
/// the store only when it is going to ask, so a run that cannot ask never builds it early.
fn pick_task(mut done: Done, session: &mut Session) -> Result<Done, Failure> {
    if done.task.is_none() && session.plan().asks_the_user() {
        let tasks = session.context::<Tasks>()?;
        done.task = session.choose(None, "Which task?", tasks.open.clone())?;
    }
    if let Some(task) = &done.task
        && session.confirm(&format!("Mark {task} done?"))? == Confirmation::Declined
    {
        return Err(Failure::cancelled(format!("{task} stays open")));
    }
    Ok(done)
}

#[clifacet::planned]
impl PlannedWith<Tasks> for Done {
    type Input = DoneInput;
    type Output = String;

    fn plan<'a>(&'a self, input: DoneInput, tasks: &'a Tasks) -> Result<Plan<'a, String>, Failure> {
        let Some(task) = input.task else {
            return Err(Failure::validation("name a task with --task"));
        };
        if !tasks.open.contains(&task) {
            return Err(Failure::validation(format!(
                "there is no open task named {task}"
            )));
        }
        let step = format!("mark {task} done");
        Ok(Plan::new(move || {
            tasks.done.borrow_mut().push(task.clone());
            Ok(format!("{task} is done"))
        })
        .step(step))
    }
}

/// One run, and what it cost: the exit code, both streams, how many times the store was
/// opened, and what the run marked done.
struct Ran {
    code: i32,
    out: String,
    err: String,
    opened: u32,
    done: Vec<String>,
    asked: Vec<String>,
}

fn run_done(
    argv: &[&str],
    capabilities: Capabilities,
    answers: &[Answer],
    store: Result<(), &'static str>,
) -> Ran {
    let opened = Rc::new(Cell::new(0));
    let done = Rc::new(RefCell::new(Vec::new()));
    let (counter, log) = (Rc::clone(&opened), Rc::clone(&done));
    let app = App::<Done>::new().context(move || {
        counter.set(counter.get() + 1);
        store.map(|()| Tasks {
            open: vec![String::from("a"), String::from("b")],
            done: Rc::clone(&log),
        })
    });
    let script = Scripted::new(answers.iter().cloned());
    let frontend = {
        let script = script.clone();
        move || -> Box<dyn Prompter> { Box::new(script) }
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = app.run_with(argv, capabilities, frontend, &mut out, &mut err);
    Ran {
        code,
        out: String::from_utf8(out).expect("utf-8 data"),
        err: String::from_utf8(err).expect("utf-8 diagnostics"),
        opened: opened.get(),
        done: done.borrow().clone(),
        asked: script.asked(),
    }
}

fn headless(argv: &[&str]) -> Ran {
    run_done(argv, Capabilities::headless(), &[], Ok(()))
}

#[test]
fn a_run_builds_the_context_once_after_validation_and_hands_it_to_the_command() {
    let ran = headless(&["done", "--task", "a"]);
    assert_eq!(
        (ran.code, ran.out.as_str(), ran.err.as_str()),
        (0, "a is done\n", "")
    );
    assert_eq!((ran.opened, ran.done), (1, vec![String::from("a")]));
}

#[test]
fn a_dry_run_previews_with_the_context_and_changes_nothing() {
    let ran = headless(&["done", "--task", "b", "--dry-run"]);
    assert_eq!((ran.code, ran.out.as_str()), (0, "mark b done\n"));
    assert_eq!(ran.opened, 1);
    assert_eq!(
        ran.done,
        Vec::<String>::new(),
        "a dry run marked something done"
    );
}

#[rstest::rstest]
#[case::help(&["done", "--help"], 0)]
#[case::version(&["done", "--version"], 0)]
#[case::usage_error(&["done", "--bogus"], 2)]
#[case::validation_failure(&["done", "--task", "a b"], 2)]
fn nothing_that_stops_before_the_command_builds_the_context(
    #[case] argv: &[&str],
    #[case] code: i32,
) {
    let ran = headless(argv);
    assert_eq!(ran.code, code, "{argv:?}: {}", ran.err);
    assert_eq!(ran.opened, 0, "{argv:?} opened the store");
}

#[test]
fn a_declined_question_builds_no_context_and_exits_with_the_cancellation_code() {
    let ran = run_done(
        &["done", "--task", "a"],
        Capabilities::terminal(),
        &[Answer::No],
        Ok(()),
    );
    assert_eq!(ran.code, 130, "{}", ran.err);
    assert_eq!((ran.opened, ran.done.len()), (0, 0));
}

#[test]
fn a_context_that_cannot_be_built_is_an_execution_failure_and_nothing_runs() {
    let ran = run_done(
        &["done", "--task", "a"],
        Capabilities::headless(),
        &[],
        Err("the store is locked"),
    );
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    assert!(
        ran.err
            .contains("cannot prepare the application: the store is locked"),
        "{}",
        ran.err
    );
    assert_eq!(ran.done, Vec::<String>::new());
}

#[test]
fn a_question_offers_the_contexts_records_and_the_command_reuses_the_same_context() {
    let answers = [Answer::Text(String::from("b")), Answer::Yes];
    let ran = run_done(&["done"], Capabilities::terminal(), &answers, Ok(()));
    assert_eq!(
        (ran.code, ran.out.as_str()),
        (0, "b is done\n"),
        "{}",
        ran.err
    );
    assert_eq!(ran.asked, ["Which task?", "Mark b done?"]);
    // Built on first use, by the question, and the command ran with that one.
    assert_eq!(ran.opened, 1);
}

#[test]
fn a_session_without_a_context_says_so_instead_of_guessing() {
    let session = Session::start(
        clifacet::policy::Policy::up_to(clifacet::policy::Mode::Interactive)
            .resolve(clifacet::policy::Request::none(), Capabilities::headless())
            .expect("a headless plan"),
        || -> Box<dyn Prompter> { Box::new(Scripted::default()) },
    );
    let missing = session.context::<Tasks>().unwrap_err();
    assert_eq!(missing.kind(), clifacet::FailureKind::Execution);
    assert!(missing.message().contains("Tasks"), "{}", missing.message());
}

// --- a tree threads the context to the chosen leaf ----------------------------------

use clap::Parser as _;

/// `shelf <add NAME | count NAME... | clear>`: the root holds the tree and the globals.
#[clifacet::command(input = ShelfInput)]
#[derive(Clone)]
#[command(name = "shelf")]
struct Shelf {
    #[clifacet(subcommands)]
    command: Shelved,

    #[command(flatten)]
    #[clifacet(skip)]
    globals: Globals,
}

struct ShelfInput {
    command: ShelfRequest,
}

#[clifacet::subcommands(input = ShelfRequest, output = Said, context = Store)]
#[derive(Clone)]
enum Shelved {
    /// Put a record on the shelf: needs the store.
    Put(Put),
    /// Count names: a plain command, written before contexts existed.
    Tally(Tally),
    /// Empty the shelf: a planned handler, so a dry run describes it.
    Clear(Clear),
}

enum ShelfRequest {
    Put(PutInput),
    Tally(TallyInput),
    Clear(ClearInput),
}

/// What every leaf's output converts into.
#[derive(Debug, PartialEq)]
struct Said(String);

impl std::fmt::Display for Said {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<usize> for Said {
    fn from(count: usize) -> Self {
        Self(format!("{count} on the shelf"))
    }
}

#[clifacet::command(input = PutInput)]
#[derive(Clone)]
struct Put {
    name: String,
}

struct PutInput {
    name: String,
}

impl Handler<Store> for Put {
    type Input = PutInput;
    type Output = usize;

    fn handle(&self, input: PutInput, store: &Store) -> Result<usize, Failure> {
        store.records.borrow_mut().push(input.name);
        Ok(store.records.borrow().len())
    }
}

#[clifacet::command(input = TallyInput)]
#[derive(Clone)]
struct Tally {
    names: Vec<String>,
}

struct TallyInput {
    names: Vec<String>,
}

impl Command for Tally {
    type Input = TallyInput;
    type Output = Said;

    fn execute(&self, input: TallyInput) -> Result<Said, Failure> {
        Ok(Said(format!("{} names", input.names.len())))
    }
}

#[clifacet::command(input = ClearInput)]
#[derive(Clone)]
struct Clear {}

struct ClearInput {}

#[clifacet::planned]
impl PlannedWith<Store> for Clear {
    type Input = ClearInput;
    type Output = usize;

    fn plan<'a>(&'a self, _: ClearInput, store: &'a Store) -> Result<Plan<'a, usize>, Failure> {
        let count = store.records.borrow().len();
        Ok(Plan::new(move || {
            store.records.borrow_mut().clear();
            Ok(0)
        })
        .step(format!("remove {count} records")))
    }
}

/// The root runs its tree, handing the context down — what `commands!` will generate.
impl Handler<Store> for Shelf {
    type Input = ShelfInput;
    type Output = Said;

    fn handle(&self, input: ShelfInput, store: &Store) -> Result<Said, Failure> {
        self.command.handle(input.command, store)
    }

    fn preview<'a>(
        &'a self,
        input: ShelfInput,
        store: &'a Store,
    ) -> Result<Plan<'a, Said>, Failure> {
        self.command.preview(input.command, store)
    }
}

/// Runs `shelf` headless against a store holding `a`, and returns the exit code, the
/// data stream and the records left in the store.
fn run_shelf(argv: &[&str]) -> (i32, String, Vec<String>) {
    let records = Rc::new(RefCell::new(vec![String::from("a")]));
    let shared = Rc::clone(&records);
    let app = App::<Shelf>::new().context(move || {
        // The store is rebuilt from the shared records, so the test reads what a run left.
        Ok::<_, std::convert::Infallible>(Store {
            records: RefCell::new(shared.borrow().clone()),
        })
    });
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let frontend = || -> Box<dyn Prompter> { Box::new(Scripted::default()) };
    let code = app.run_with(argv, Capabilities::headless(), frontend, &mut out, &mut err);
    let out = String::from_utf8(out).expect("utf-8 data");
    let err = String::from_utf8(err).expect("utf-8 diagnostics");
    assert!(code == 0 || !err.is_empty(), "a failure says why");
    let left = records.borrow().clone();
    (code, if code == 0 { out } else { err }, left)
}

#[test]
fn a_tree_hands_the_context_to_the_leaf_that_needs_it() {
    let (code, out, _) = run_shelf(&["shelf", "put", "b"]);
    assert_eq!((code, out.as_str()), (0, "2 on the shelf\n"));
}

#[test]
fn a_leaf_without_context_still_runs_inside_a_tree_that_has_one() {
    let (code, out, _) = run_shelf(&["shelf", "tally", "x", "y", "z"]);
    assert_eq!((code, out.as_str()), (0, "3 names\n"));
}

#[test]
fn a_tree_previews_through_the_chosen_leaf() {
    let (code, out, _) = run_shelf(&["shelf", "clear", "--dry-run"]);
    assert_eq!((code, out.as_str()), (0, "remove 1 records\n"));
    // A leaf that cannot describe itself refuses, and runs nothing.
    let (code, err, _) = run_shelf(&["shelf", "put", "b", "--dry-run"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--dry-run"), "{err}");
}

#[test]
fn the_tree_still_parses_and_converts_as_before() {
    let shelf = Shelf::try_parse_from(["shelf", "tally", "q"]).expect("parses");
    assert!(matches!(shelf.command, Shelved::Tally(_)));
}
