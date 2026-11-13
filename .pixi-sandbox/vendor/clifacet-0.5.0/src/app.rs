//! The runner: one call that owns an application's pipeline (decision-19 item 1).
//!
//! Every application used to write the same `main` by hand — parse, settle the mode,
//! ask, validate, run or preview, write the output, map the exit code. [`App`] is that
//! pipeline once, so `main.rs` is one line:
//!
//! ```no_run
//! # use clifacet::{Command, Failure, Globals};
//! #[clifacet::command(input = Greeting)]
//! #[derive(Clone)]
//! #[command(name = "greet")]
//! struct Greet {
//!     #[clifacet(long, fill = "world")]
//!     name: Option<String>,
//!     #[command(flatten)]
//!     #[clifacet(skip)]
//!     globals: Globals,
//! }
//!
//! struct Greeting {
//!     name: String,
//! }
//!
//! impl Command for Greet {
//!     type Input = Greeting;
//!     type Output = String;
//!
//!     fn execute(&self, input: Greeting) -> Result<String, Failure> {
//!         Ok(format!("hello {}", input.name))
//!     }
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     clifacet::run::<Greet>()
//! }
//! ```
//!
//! The pipeline, in order, and what each step costs when it stops the run:
//!
//! | Step | Stops with | Exit code |
//! | --- | --- | --- |
//! | parse | `clap`'s help or version, on the data stream | success |
//! | parse | a usage error, on the diagnostic stream | [`ExitPolicy::usage`] |
//! | settle the mode | a [`Refusal`](crate::policy::Refusal), as a validation failure | [`ExitPolicy::validation`] |
//! | ask | a declined question | [`ExitPolicy::cancelled`] |
//! | validate | the bridge's refusal | [`ExitPolicy::validation`] |
//! | run, or preview with `--dry-run` | the command's failure | its kind's code |
//! | render | a data stream that cannot be written | [`ExitPolicy::execution`] |
//!
//! Each step is the library's existing seam — [`Policy::resolve`], [`Session`],
//! [`Collect`], [`Facet::validate`], [`Command`] and [`report::render`] — so the runner
//! adds an order, not a second implementation of any of them.
//!
//! [`run_with`](App::run_with) is the same pipeline with the process taken out: the
//! arguments, the capabilities, the frontend and both streams are parameters, which is
//! how a test runs an application without a terminal or the process environment.

use std::ffi::OsString;
use std::fmt::Display;
use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;

use clap::{ArgMatches, Parser};

use crate::command::{Command, Handler};
use crate::contract::ExitPolicy;
use crate::facet::Facet;
use crate::failure::Failure;
use crate::policy::{Capabilities, Mode, Policy, Request};
use crate::prompt::{Collect, Lazy, Prompter, Session};
use crate::report;
use crate::settings;

/// The options every application gets: `--yes` and `--dry-run`.
///
/// Flatten it into the root command, beside the application's own options:
///
/// ```
/// #[derive(clap::Parser)]
/// struct Cli {
///     /// The application's own global option.
///     #[arg(long, global = true)]
///     json: bool,
///
///     #[command(flatten)]
///     globals: clifacet::Globals,
/// }
///
/// let cli = <Cli as clap::Parser>::try_parse_from(["app", "--dry-run", "--json"]).unwrap();
/// assert!(cli.globals.dry_run && cli.json);
/// ```
///
/// Both are `global`, so they are accepted after a subcommand too. They decide how a run
/// goes, not what it asks for, which is why an application keeps them out of its input
/// with `#[clifacet(skip)]`. The runner reads them from `clap`'s matches by their ids,
/// `yes` and `dry_run`, so an application that declared its own flags with those ids
/// before this type existed keeps working unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, clap::Args)]
pub struct Globals {
    /// Answer every confirmation with yes instead of asking.
    #[arg(long, global = true)]
    pub yes: bool,

    /// Show what would run, and run nothing.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Read settings from this file instead of the default location.
    ///
    /// Recognised because [`App::settings`] reads it before the parse; declared here so
    /// `clap` accepts it too, and so an application that never calls `settings` gets a
    /// flag that parses rather than one that fails as unknown.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,

    /// Read no settings file at all, ignoring `--config`, the environment and the default
    /// location.
    #[arg(long, global = true, conflicts_with = "config")]
    pub no_config: bool,
}

impl Globals {
    /// The global options as `clap` matched them; an id the command lacks reads as off.
    fn matched(matches: &ArgMatches) -> Self {
        let on = |id: &str| matches!(matches.try_get_one::<bool>(id), Ok(Some(true)));
        Self {
            yes: on("yes"),
            dry_run: on("dry_run"),
            config: matches
                .try_get_one::<std::path::PathBuf>("config")
                .ok()
                .flatten()
                .cloned(),
            no_config: on("no_config"),
        }
    }
}

/// How a command's output is written on the data stream.
///
/// The runner calls it once, with the output of a successful run. [`Plain`] is the
/// default; a closure taking the output and the stream is a renderer too, which is how
/// one output type is written two ways — say, for people and as JSON:
///
/// ```
/// use std::io::Write;
/// use clifacet::Render;
///
/// let json = |count: usize, out: &mut dyn Write| writeln!(out, "{{\"count\":{count}}}");
/// let mut out = Vec::new();
/// json.render(3, &mut out).unwrap();
///
/// assert_eq!(out, b"{\"count\":3}\n");
/// ```
///
/// A failure is never rendered here: it goes through [`report::render`], on the
/// diagnostic stream, whatever the renderer.
pub trait Render<T> {
    /// Writes `output` on `out`.
    ///
    /// # Errors
    ///
    /// Returns the stream's error. The runner reports it as an execution failure: a run
    /// that cannot write its data has nowhere to report success.
    fn render(&self, output: T, out: &mut dyn Write) -> std::io::Result<()>;
}

/// The default renderer: an output's [`Display`], then a newline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Plain;

impl<T: Display> Render<T> for Plain {
    fn render(&self, output: T, out: &mut dyn Write) -> std::io::Result<()> {
        writeln!(out, "{output}")
    }
}

impl<T, F> Render<T> for F
where
    F: Fn(T, &mut dyn Write) -> std::io::Result<()>,
{
    fn render(&self, output: T, out: &mut dyn Write) -> std::io::Result<()> {
        self(output, out)
    }
}

/// Which command a run executes.
enum Chosen<'c, Cli, C> {
    /// Built from the parsed command line — the command line *is* the command.
    Parsed(fn(&Cli) -> C),
    /// Supplied by the application, and the same for every run.
    Given(&'c C),
}

/// An application's pipeline, configured once and run against a command line.
///
/// `Cli` is the root `clap` command. By default the parsed `Cli` is also the command
/// that runs, as a declared tree runs a clone of itself (decision-14); [`command`]
/// runs another command with the same input instead. [`policy`], [`exit`],
/// [`mode_from`] and [`render`] replace the defaults, which are prompts when a terminal
/// can host them ([`Policy::up_to`]`(Mode::Interactive)`), [`ExitPolicy::standard`],
/// no mode requested, and [`Plain`].
///
/// [`command`]: App::command
/// [`policy`]: App::policy
/// [`exit`]: App::exit
/// [`mode_from`]: App::mode_from
/// [`render`]: App::render
pub struct App<'c, Cli, C = Cli, R = Plain, Cx = ()> {
    policy: Policy,
    exit: ExitPolicy,
    mode_from: fn(&Cli) -> Request,
    command: Chosen<'c, Cli, C>,
    render: R,
    context: Rc<dyn Fn() -> Result<Cx, Failure>>,
    settings: Option<Settings>,
}

/// Reads the settings file for a given argument list.
///
/// A function of the arguments rather than a value, because `--config` is in them: the file
/// cannot be read until the run says which file it means.
type Settings = Rc<dyn Fn(&[OsString]) -> Result<settings::Settings, crate::failure::Failure>>;

impl<Cli: Clone> App<'_, Cli> {
    /// The pipeline for `Cli`, with the parsed command line as the command that runs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: Policy::up_to(Mode::Interactive),
            exit: ExitPolicy::standard(),
            mode_from: |_| Request::none(),
            command: Chosen::Parsed(Cli::clone),
            render: Plain,
            context: Rc::new(|| Ok(())),
            settings: None,
        }
    }
}

impl<Cli: Clone> Default for App<'_, Cli> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'c, Cli, C, R, Cx> App<'c, Cli, C, R, Cx> {
    /// How interactive a run may be, and how interactive it is unasked.
    #[must_use]
    pub fn policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }

    /// The exit code each outcome costs.
    #[must_use]
    pub fn exit(mut self, exit: ExitPolicy) -> Self {
        self.exit = exit;
        self
    }

    /// The mode the user asked for, read from the parsed command line.
    ///
    /// For an application with its own `--interactive` or `--tui` flags. `--yes` is
    /// added to whatever this returns, so the function decides the mode only.
    #[must_use]
    pub fn mode_from(mut self, requested: fn(&Cli) -> Request) -> Self {
        self.mode_from = requested;
        self
    }

    /// Runs `command` instead of the parsed command line.
    ///
    /// The input is still `Cli`'s, so the parse, the questions and the validation are
    /// the same; only what runs changes. That is how a test observes a run — see
    /// [`harness::Recording`](crate::harness::Recording).
    #[must_use]
    pub fn command<D>(self, command: &D) -> App<'_, Cli, D, R, Cx> {
        App {
            policy: self.policy,
            exit: self.exit,
            mode_from: self.mode_from,
            command: Chosen::Given(command),
            render: self.render,
            context: self.context,
            settings: self.settings,
        }
    }

    /// Writes the output with `render` instead of [`Plain`].
    #[must_use]
    pub fn render<S>(self, render: S) -> App<'c, Cli, C, S, Cx> {
        App {
            policy: self.policy,
            exit: self.exit,
            mode_from: self.mode_from,
            command: self.command,
            render,
            context: self.context,
            settings: self.settings,
        }
    }

    /// How to build the application's context: what its commands depend on.
    ///
    /// The command runs as a [`Handler`] of that context, which every [`Command`] is. The
    /// context is built after validation, on first use — by a question that reads it
    /// ([`Session::context`]) or by the command — and once per run; so `--help`,
    /// `--version`, a usage error, a rejected value or a declined question never build
    /// it. A context that cannot be built is an execution failure, reported as
    /// `cannot prepare the application: {error}`, and nothing runs.
    #[must_use]
    pub fn context<D, E, F>(self, build: F) -> App<'c, Cli, C, R, D>
    where
        F: Fn() -> Result<D, E> + 'static,
        E: Display,
    {
        App {
            policy: self.policy,
            exit: self.exit,
            mode_from: self.mode_from,
            command: self.command,
            render: self.render,
            context: Rc::new(move || {
                build().map_err(|error| {
                    Failure::execution(format!("cannot prepare the application: {error}"))
                })
            }),
            settings: self.settings,
        }
    }
}

impl<Cli, C, R, Cx> App<'_, Cli, C, R, Cx>
where
    Cli: Parser + Facet + Collect,
    C: Handler<Cx, Input = <Cli as Facet>::Input>,
    R: Render<<C as Handler<Cx>>::Output>,
    Cx: 'static,
{
    /// Runs the process's own invocation and answers with its exit status.
    ///
    /// The arguments, the terminal's capabilities and the frontend are the process's:
    /// the `inquire` dialog with the `interactive` feature, plain lines on the terminal
    /// without it. A data stream that cannot be flushed downgrades success to
    /// [`ExitPolicy::execution`], because the data may not have arrived.
    #[must_use]
    pub fn run(&self) -> ExitCode {
        let mut out = std::io::stdout().lock();
        let mut err = std::io::stderr().lock();
        let code = self.run_with(
            std::env::args_os(),
            Capabilities::detected(),
            process_frontend,
            &mut out,
            &mut err,
        );
        let code = match out.flush().and_then(|()| err.flush()) {
            Ok(()) => code,
            Err(_) => self.exit.execution(),
        };
        ExitCode::from(u8::try_from(code).unwrap_or(1))
    }

    /// Layers a settings file under the flags and the environment.
    ///
    /// The file is chosen the way a user expects — `--config PATH`, then `<APP>_CONFIG`,
    /// then the platform's config directory — and every entry it holds becomes a `clap`
    /// default, so the precedence is flag > environment > file > built-in default and each
    /// value is parsed by its argument's own parser (see [`settings`](crate::settings)).
    ///
    /// `loader` decides the format, so the library does not choose an application's
    /// configuration language; `crate::settings::toml::Toml` is the one it ships with the
    /// `settings-toml` feature.
    ///
    /// `--help` and `--version` never read the file: they are answered before any parse, so
    /// the defaults they document are the built-in ones and one machine's settings never
    /// leak into another's documentation. A file that cannot be read or applied fails the
    /// run as a rejected invocation, before anything executes.
    #[must_use]
    pub fn settings<L>(mut self, application: &'static str, loader: L) -> Self
    where
        L: crate::settings::Loader + 'static,
    {
        self.settings = Some(Rc::new(move |argv: &[std::ffi::OsString]| {
            let asked = settings::source::Asked::in_argv(argv.iter().cloned());
            asked
                .read(application, &settings::Process, &loader)
                .map_err(|error| Failure::validation(error.message()))
        }));
        self
    }

    /// Runs `argv` against explicit capabilities, frontend and streams.
    ///
    /// The same pipeline as [`run`](Self::run), with nothing taken from the process: a
    /// test states what the terminal can do, scripts the answers, and reads both
    /// streams. `build_frontend` is called only when the run asks the user something.
    pub fn run_with<I, T>(
        &self,
        argv: I,
        capabilities: Capabilities,
        build_frontend: impl FnOnce() -> Box<dyn Prompter>,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> i32
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        // The settings file has to be read before the parse, because a file's value is a
        // default and `clap` needs the default before it reads the argument list. The
        // argument list is collected once so it can be looked at twice: `--config` is in
        // it, and the parse still receives it (clap ignores what it does not declare, and
        // the two flags below are declared by this module's own readers).
        let argv: Vec<OsString> = argv.into_iter().map(Into::into).collect();

        // `--help` and `--version` are answered from the command as the application built
        // it, before the settings file is read. That is AC#7: help documents the built-in
        // defaults and never one machine's settings file. It is also the one place where
        // precedence does not matter, because neither flag is a value.
        if self.settings.is_some() && asks_for_help_or_version(&argv) {
            match <Cli as Facet>::command().try_get_matches_from(argv.iter().cloned()) {
                // `DisplayHelp` and `DisplayVersion` arrive here as an `Err` clap has
                // already rendered, which `report::render` writes on the data stream with
                // the policy's success code — the same rendering a run without settings
                // produces, because it is the same command.
                Ok(_) => {}
                Err(error) => {
                    return report::render(&program::<Cli>(), &error, &self.exit, out, err);
                }
            }
        }

        match self.parse_with_settings(&argv) {
            Ok((cli, globals)) => {
                self.run_parsed(cli, globals, capabilities, build_frontend, out, err)
            }
            Err(error) => report::render(&program::<Cli>(), &error, &self.exit, out, err),
        }
    }

    /// Parses `argv`, with the settings file applied to the command when one is configured.
    ///
    /// Without settings this is [`parse`] and nothing else: an application that never calls
    /// [`settings`](Self::settings) pays for one `Option` check.
    fn parse_with_settings(&self, argv: &[OsString]) -> Result<(Cli, Globals), clap::Error> {
        let Some(read) = &self.settings else {
            return parse::<Cli, _, _>(argv.iter().cloned());
        };
        let settings = match read(argv) {
            Ok(settings) => settings,
            // A settings file that cannot be read is a rejected invocation: nothing ran,
            // and the argument list the user typed was not the problem. Reporting it as a
            // `clap` error keeps one path to the diagnostic stream and one exit code.
            Err(failure) => {
                return Err(clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    failure.message(),
                ));
            }
        };
        let command = settings::source::declare(<Cli as Facet>::command());
        let applied = match settings.apply(command, <Cli as Facet>::secret_fields()) {
            Ok(applied) => applied,
            Err(error) => {
                return Err(clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    error.message(),
                ));
            }
        };
        let mut matches = applied
            .into_command()
            .try_get_matches_from(argv.iter().cloned())?;
        let globals = Globals::matched(&matches);
        let cli = Cli::from_arg_matches_mut(&mut matches)
            .map_err(|error| error.format(&mut <Cli as Facet>::command()))?;
        Ok((cli, globals))
    }

    /// Runs an already-parsed command line: every step after parsing.
    ///
    /// For an application that holds a `Cli` it parsed itself. `globals` are passed
    /// rather than read, because a parsed `Cli` no longer has `clap`'s matches to read
    /// them from.
    ///
    /// `globals` is taken by value because this is a published signature: an application
    /// calls it, and a future `Globals` may well be consumed here. Changing it to a
    /// reference to satisfy a lint would be a breaking change for no gain.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "a published signature, kept stable for callers"
    )]
    pub fn run_parsed(
        &self,
        cli: Cli,
        globals: Globals,
        capabilities: Capabilities,
        build_frontend: impl FnOnce() -> Box<dyn Prompter>,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> i32 {
        let requested = (self.mode_from)(&cli);
        let requested = requested.assuming_yes(requested.assumes_yes() || globals.yes);
        // One context per run, shared by the questions and the command, built on first use.
        let context = Rc::new(Lazy::new(Rc::clone(&self.context)));
        let outcome = match self.policy.resolve(requested, capabilities) {
            Ok(plan) if plan.mode() == Mode::Tui => self.full_screen(&globals, &context, out, err),
            Ok(plan) => {
                let mut session = Session::start(plan, build_frontend).sharing(Rc::clone(&context));
                cli.collect(&mut session)
                    .and_then(|cli| self.settle(cli, &globals, &context, out))
            }
            Err(refusal) => Err(Failure::validation(refusal.message())),
        };
        self.conclude(outcome, out, err)
    }

    /// Hosts the form generated from `Cli`'s `form` fields, drawn on the diagnostic stream.
    ///
    /// The form produces arguments and `clap` parses them, so a submission reaches the
    /// same validation, command and renderer as a flag run. A refused submission is
    /// shown on the form rather than ending the run, because a form that closes on a
    /// typo makes the user start again. The rendered output is held until the screen
    /// is left, then written on the data stream.
    #[cfg(feature = "tui")]
    fn full_screen(
        &self,
        globals: &Globals,
        context: &Lazy<Cx>,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<(), Failure> {
        use crate::tui;

        let mut form = tui::FormState::of::<Cli>()?;
        let mut rendered = Vec::new();
        // The globals are borrowed rather than copied: the settings flags mean `Globals` is
        // no longer `Copy`, and a form submission is the same run as the one that opened it.
        let submit = |state: &tui::FormState| -> Result<Vec<String>, String> {
            let argv = std::iter::once(program::<Cli>()).chain(state.arguments());
            let cli = Cli::try_parse_from(argv).map_err(|error| error.to_string())?;
            rendered.clear();
            self.settle(cli, globals, context, &mut rendered)
                .map_err(|failure| failure.message().to_owned())?;
            Ok(Vec::new())
        };
        tui::run_on(&mut *err, &mut form, tui::live_keys(), submit)?;
        out.write_all(&rendered)
            .map_err(|error| Failure::execution(format!("cannot write the output: {error}")))
    }

    /// Without the `tui` feature there is no form to host, so the mode is refused.
    #[cfg(not(feature = "tui"))]
    #[allow(
        clippy::unused_self,
        reason = "the same signature as the `tui` build's method, so the call site is one"
    )]
    fn full_screen(
        &self,
        _globals: &Globals,
        _context: &Lazy<Cx>,
        _out: &mut dyn Write,
        _err: &mut dyn Write,
    ) -> Result<(), Failure> {
        Err(Failure::validation(
            "a TUI is not available: this program was built without clifacet's `tui` feature",
        ))
    }

    /// Validates, then runs or previews, then renders: everything after the questions.
    fn settle(
        &self,
        cli: Cli,
        globals: &Globals,
        context: &Lazy<Cx>,
        out: &mut dyn Write,
    ) -> Result<(), Failure> {
        // The command is taken before validation consumes the command line it is
        // built from — the clone a declared tree keeps of itself.
        let parsed;
        let command = match &self.command {
            Chosen::Parsed(build) => {
                parsed = build(&cli);
                &parsed
            }
            Chosen::Given(command) => *command,
        };
        let input = cli.validate()?;
        // Built only now that the input is valid — unless a question already needed it.
        let cx = context.get()?;
        // A dry run previews through the same validated input a real run executes, so
        // what it prints is what the real run would do — or the reason it cannot say.
        let written = if globals.dry_run {
            let steps = Handler::<Cx>::preview(command, input, &cx)?.describe();
            if steps.is_empty() {
                Ok(())
            } else {
                writeln!(out, "{steps}")
            }
        } else {
            self.render
                .render(Handler::<Cx>::handle(command, input, &cx)?, out)
        };
        written.map_err(|error| Failure::execution(format!("cannot write the output: {error}")))
    }

    /// The exit code a finished run answers with, its failure reported if it had one.
    fn conclude(
        &self,
        outcome: Result<(), Failure>,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> i32 {
        match outcome {
            Ok(()) => self.exit.success(),
            Err(failure) => report::render(&program::<Cli>(), &failure, &self.exit, out, err),
        }
    }
}

/// Runs `Cli` as the whole application: the one line a `main` needs.
///
/// The parsed command line is the command, its output is written with [`Plain`], and
/// every other choice is [`App`]'s default; build an [`App`] to change any of them.
#[must_use]
pub fn run<Cli>() -> ExitCode
where
    Cli: Parser + Facet + Collect + Clone + Command<Input = <Cli as Facet>::Input>,
    <Cli as Command>::Output: Display,
{
    App::<Cli>::new().run()
}

/// Whether the argument list asks for help or a version, which is answered without a file.
///
/// `--` ends the flags, so anything after it names a value rather than asking a question.
fn asks_for_help_or_version(argv: &[OsString]) -> bool {
    for argument in argv {
        let Some(text) = argument.to_str() else {
            continue;
        };
        match text {
            "--" => return false,
            "--help" | "-h" | "--version" | "-V" => return true,
            _ => {}
        }
    }
    false
}

/// Parses `argv` into the command line and the global options it carried.
///
/// `clap`'s own two steps, as [`Parser::try_parse_from`] takes them, with the globals
/// read from the matches in between.
fn parse<Cli, I, T>(argv: I) -> Result<(Cli, Globals), clap::Error>
where
    Cli: Parser,
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut matches = Cli::command().try_get_matches_from(argv)?;
    let globals = Globals::matched(&matches);
    let cli = Cli::from_arg_matches_mut(&mut matches)
        .map_err(|error| error.format(&mut Cli::command()))?;
    Ok((cli, globals))
}

/// How a diagnostic names the program: the root command's name.
fn program<Cli: Facet>() -> String {
    <Cli as Facet>::command().get_name().to_owned()
}

/// The frontend a real process asks with.
fn process_frontend() -> Box<dyn Prompter> {
    #[cfg(feature = "interactive")]
    let frontend = Box::new(crate::prompt::Dialog::new());
    #[cfg(not(feature = "interactive"))]
    let frontend = Box::new(crate::prompt::Terminal::stdio());
    frontend
}
