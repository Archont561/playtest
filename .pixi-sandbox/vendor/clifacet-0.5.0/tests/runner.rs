//! The runner owns the pipeline: parse, ask, validate, run or preview, render, exit.
//!
//! Every test drives [`App::run_with`] — explicit arguments, capabilities, frontend
//! and streams — so nothing here reaches a terminal or the process environment.

use clifacet::policy::Capabilities;
use clifacet::prompt::{Prompter, Scripted};
use clifacet::{App, Command, Failure, Globals};

/// A one-command application whose parsed command is the command that runs.
#[clifacet::command(input = Greeting)]
#[derive(Debug, Clone)]
#[command(name = "greet", version = "1.2.3", about = "Say hello")]
struct Greet {
    /// Who to greet.
    #[clifacet(
        long,
        ask = "Who should I greet?",
        fill = "world",
        trim,
        non_empty,
        form
    )]
    name: Option<String>,

    /// Greet loudly, after confirming.
    #[clifacet(long, confirm = "Shout?")]
    shout: bool,

    #[command(flatten)]
    #[clifacet(skip)]
    globals: Globals,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Greeting {
    name: String,
    shout: bool,
}

impl Command for Greet {
    type Input = Greeting;
    type Output = String;

    fn execute(&self, input: Greeting) -> Result<String, Failure> {
        if input.name == "nobody" {
            return Err(Failure::execution("nobody is there to greet"));
        }
        let greeting = format!("hello {}", input.name);
        Ok(if input.shout {
            greeting.to_uppercase()
        } else {
            greeting
        })
    }
}

/// What one run left behind: its exit code, its data and its diagnostics.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

fn run_greet(argv: &[&str], capabilities: Capabilities, answers: Scripted) -> Ran {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let frontend = move || -> Box<dyn Prompter> { Box::new(answers) };
    let code = App::<Greet>::new().run_with(argv, capabilities, frontend, &mut out, &mut err);
    Ran {
        code,
        out: String::from_utf8(out).expect("utf-8 data"),
        err: String::from_utf8(err).expect("utf-8 diagnostics"),
    }
}

fn headless(argv: &[&str]) -> Ran {
    run_greet(argv, Capabilities::headless(), Scripted::default())
}

#[test]
fn a_run_writes_the_output_on_the_data_stream_and_exits_with_success() {
    let ran = headless(&["greet", "--name", "ada"]);

    assert_eq!(ran.code, 0);
    assert_eq!(ran.out, "hello ada\n");
    assert_eq!(ran.err, "");
}

fn on_a_terminal(argv: &[&str], answers: Scripted) -> Ran {
    run_greet(argv, Capabilities::terminal(), answers)
}

/// Each way a run can stop, the code it costs, and which stream says so.
#[rstest::rstest]
#[case::help(&["greet", "--help"], 0, "Usage: greet", "")]
#[case::version(&["greet", "--version"], 0, "greet 1.2.3", "")]
#[case::usage_error(&["greet", "--nmae", "ada"], 2, "", "unexpected argument '--nmae'")]
#[case::validation(&["greet", "--name", "  "], 2, "", "greet: ")]
#[case::execution(&["greet", "--name", "nobody"], 1, "", "greet: nobody is there to greet")]
fn every_outcome_costs_its_code_on_its_stream(
    #[case] argv: &[&str],
    #[case] code: i32,
    #[case] out: &str,
    #[case] err: &str,
) {
    let ran = headless(argv);

    assert_eq!(ran.code, code, "out: {}\nerr: {}", ran.out, ran.err);
    assert!(ran.out.contains(out), "data stream: {:?}", ran.out);
    assert!(ran.err.contains(err), "diagnostic stream: {:?}", ran.err);
    if out.is_empty() {
        assert_eq!(ran.out, "", "nothing reaches the data stream");
    }
}

#[test]
fn a_missing_value_is_asked_for_when_a_terminal_can_answer() {
    let ran = on_a_terminal(
        &["greet"],
        Scripted::new([clifacet::prompt::Answer::Text(String::from("ada"))]),
    );

    assert_eq!((ran.code, ran.out.as_str()), (0, "hello ada\n"));
}

#[test]
fn a_declined_confirmation_cancels_with_the_cancellation_code_and_runs_nothing() {
    let ran = on_a_terminal(
        &["greet", "--name", "ada", "--shout"],
        Scripted::new([clifacet::prompt::Answer::No]),
    );

    assert_eq!(ran.code, 130);
    assert_eq!(ran.out, "");
}

#[test]
fn a_mode_the_policy_forbids_is_refused_before_anything_is_asked() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = App::<Greet>::new()
        .policy(clifacet::policy::Policy::non_interactive())
        .mode_from(|_| clifacet::policy::Request::mode(clifacet::policy::Mode::Interactive))
        .run_with(
            ["greet"],
            Capabilities::terminal(),
            || -> Box<dyn Prompter> { panic!("a refused run builds no frontend") },
            &mut out,
            &mut err,
        );

    assert_eq!(code, 2);
    assert_eq!(out, b"");
    assert!(String::from_utf8_lossy(&err).starts_with("greet: interactive use is not available"));
}

#[test]
fn yes_answers_the_confirmation_instead_of_asking() {
    // An empty script fails any question it is asked, so passing means none was.
    let ran = on_a_terminal(
        &["greet", "--name", "ada", "--shout", "--yes"],
        Scripted::default(),
    );

    assert_eq!(
        (ran.code, ran.out.as_str(), ran.err.as_str()),
        (0, "HELLO ADA\n", "")
    );
}

#[test]
fn a_dry_run_of_a_command_that_cannot_preview_is_refused_and_runs_nothing() {
    let ran = headless(&["greet", "--name", "nobody", "--dry-run"]);

    assert_eq!(ran.code, 2, "refused like any invocation that ran nothing");
    assert_eq!(ran.out, "");
    assert!(
        ran.err.contains("cannot show what it would do"),
        "the refusal says why: {}",
        ran.err
    );
}

// --- a planned tree: a dry run prints the plan and runs nothing ----------------------

thread_local! {
    /// What the chores actually did, in order.
    static SWEPT: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn swept() -> Vec<String> {
    SWEPT.with(|swept| swept.borrow_mut().drain(..).collect())
}

#[clifacet::command(input = Chores)]
#[derive(Debug, Clone)]
#[command(name = "chores")]
struct ChoresCli {
    #[clifacet(subcommands)]
    command: ChoreCommands,

    #[command(flatten)]
    #[clifacet(skip)]
    globals: Globals,
}

struct Chores {
    command: Chore,
}

#[clifacet::subcommands(input = Chore, output = String, planned)]
#[derive(Debug, Clone)]
enum ChoreCommands {
    /// Sweep one room.
    Sweep(Sweep),
}

enum Chore {
    Sweep(SweepInput),
}

#[clifacet::command(input = SweepInput)]
#[derive(Debug, Clone)]
struct Sweep {
    room: String,
}

struct SweepInput {
    room: String,
}

impl clifacet::command::Planned for Sweep {
    type Input = SweepInput;
    type Output = String;

    fn plan(&self, input: SweepInput) -> Result<clifacet::command::Plan<'_, String>, Failure> {
        let step = format!("sweep the {}", input.room);
        let work = move || {
            SWEPT.with(|swept| swept.borrow_mut().push(input.room.clone()));
            Ok(format!("swept the {}", input.room))
        };
        Ok(clifacet::command::Plan::new(work).step(step))
    }
}

/// The root runs the chosen leaf's plan: the tree is the command.
impl clifacet::command::Planned for ChoresCli {
    type Input = Chores;
    type Output = String;

    fn plan(&self, input: Chores) -> Result<clifacet::command::Plan<'_, String>, Failure> {
        self.command.plan(input.command)
    }
}

fn run_chores(argv: &[&str]) -> Ran {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = App::<ChoresCli>::new().run_with(
        argv,
        Capabilities::headless(),
        || -> Box<dyn Prompter> { Box::new(Scripted::default()) },
        &mut out,
        &mut err,
    );
    Ran {
        code,
        out: String::from_utf8(out).expect("utf-8 data"),
        err: String::from_utf8(err).expect("utf-8 diagnostics"),
    }
}

#[test]
fn a_dry_run_prints_the_plan_on_the_data_stream_and_changes_nothing() {
    swept();
    // `--dry-run` after the subcommand: the global options are accepted at any depth.
    let ran = run_chores(&["chores", "sweep", "kitchen", "--dry-run"]);

    assert_eq!(
        (ran.code, ran.out.as_str(), ran.err.as_str()),
        (0, "sweep the kitchen\n", "")
    );
    assert_eq!(swept(), Vec::<String>::new(), "nothing was swept");
}

#[test]
fn a_real_run_executes_the_plan_the_dry_run_printed() {
    swept();
    let ran = run_chores(&["chores", "sweep", "kitchen"]);

    assert_eq!((ran.code, ran.out.as_str()), (0, "swept the kitchen\n"));
    assert_eq!(swept(), ["kitchen"]);
}

// --- what runs, and how it is written, are the application's to swap ----------------

/// Another command with the greeting's input.
struct Whisper;

impl Command for Whisper {
    type Input = Greeting;
    type Output = String;

    fn execute(&self, input: Greeting) -> Result<String, Failure> {
        Ok(format!("psst, {}", input.name))
    }
}

#[test]
fn another_command_runs_with_the_same_parse_questions_and_validation() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = App::<Greet>::new().command(&Whisper).run_with(
        ["greet", "--name", " ada "],
        Capabilities::headless(),
        || -> Box<dyn Prompter> { Box::new(Scripted::default()) },
        &mut out,
        &mut err,
    );

    assert_eq!((code, out.as_slice()), (0, b"psst, ada\n".as_slice()));
}

#[test]
fn a_renderer_decides_how_the_output_is_written() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = App::<Greet>::new()
        .render(|greeting: String, out: &mut dyn std::io::Write| writeln!(out, "<{greeting}>"))
        .run_with(
            ["greet", "--name", "ada"],
            Capabilities::headless(),
            || -> Box<dyn Prompter> { Box::new(Scripted::default()) },
            &mut out,
            &mut err,
        );

    assert_eq!((code, out.as_slice()), (0, b"<hello ada>\n".as_slice()));
}

#[test]
fn a_data_stream_that_cannot_be_written_is_an_execution_failure() {
    struct Closed;

    impl std::io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut err = Vec::new();
    let code = App::<Greet>::new().run_with(
        ["greet"],
        Capabilities::headless(),
        || -> Box<dyn Prompter> { Box::new(Scripted::default()) },
        &mut Closed,
        &mut err,
    );

    assert_eq!(code, 1);
    assert!(String::from_utf8_lossy(&err).contains("cannot write the output"));
}

// --- the full-screen form is hosted by the runner -----------------------------------

fn asking_for_the_form(out: &mut Vec<u8>, err: &mut Vec<u8>) -> i32 {
    use clifacet::policy::{Mode, Policy, Request};

    App::<Greet>::new()
        .policy(Policy::up_to(Mode::Tui))
        .mode_from(|_| Request::mode(Mode::Tui))
        .run_with(
            ["greet"],
            Capabilities::terminal_with_tui(),
            || -> Box<dyn Prompter> { panic!("the form asks no prompt questions") },
            out,
            err,
        )
}

#[cfg(feature = "tui")]
#[test]
fn the_form_is_drawn_on_the_diagnostic_stream_and_a_silent_terminal_cancels() {
    use std::io::IsTerminal;

    // A real terminal on stdin would wait for a key; without one the form reads no
    // events and cancels, which is the path a test can take.
    if std::io::stdin().is_terminal() {
        return;
    }
    let (mut out, mut err) = (Vec::new(), Vec::new());

    let code = asking_for_the_form(&mut out, &mut err);

    let err = String::from_utf8_lossy(&err);
    assert_eq!(code, 130, "a form nobody can type into is cancelled: {err}");
    assert!(out.is_empty(), "a screen is not data");
    assert!(
        err.contains("\u{1b}[?1049h"),
        "the alternate screen is entered on stderr"
    );
    assert!(err.contains("\u{1b}[?1049l"), "and left again");
}

#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_the_form_is_refused_and_nothing_runs() {
    let (mut out, mut err) = (Vec::new(), Vec::new());

    let code = asking_for_the_form(&mut out, &mut err);

    assert_eq!(code, 2);
    assert_eq!(out, b"");
    assert!(String::from_utf8_lossy(&err).contains("`tui` feature"));
}
