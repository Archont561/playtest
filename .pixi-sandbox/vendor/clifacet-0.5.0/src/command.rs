//! The UI-free command seam.
//!
//! A [`Command`] receives validated input and answers with data or a
//! [`Failure`](crate::Failure). It takes no stream, reads no terminal and holds
//! no presentation state, which is what lets the same command serve flag-based,
//! interactive and TUI frontends (decision-1) without knowing any of them exist.
//!
//! Parsing, validation and rendering stay outside this trait. The application
//! turns its `clap` arguments into `Input`, rejects bad values as
//! [`Failure::validation`](crate::Failure::validation), and writes whatever the
//! command returns. See the `clifacet-example` crate for a complete, tested example of that
//! division.
//!
//! A command that changes something can be [`Planned`] instead: it answers its input
//! with a [`Plan`] — the steps it would take, and the work that takes them — and the
//! library implements [`Command`] for it as "plan, then execute". A `--dry-run` prints
//! the plan's steps; a real run executes the plan; and because the library's
//! `execute` is the only way to run a planned command, the two cannot take different
//! paths (decision-18 item 5).

use std::fmt;

use crate::failure::Failure;

/// A command that runs without a terminal, a prompt or a user interface.
///
/// # Example
///
/// ```
/// use clifacet::{Command, Failure};
///
/// struct Echo;
///
/// impl Command for Echo {
///     type Input = String;
///     type Output = String;
///
///     fn execute(&self, input: Self::Input) -> Result<Self::Output, Failure> {
///         if input.is_empty() {
///             return Err(Failure::execution("nothing to echo"));
///         }
///         Ok(input)
///     }
/// }
///
/// assert_eq!(Echo.execute("hi".to_owned()).unwrap(), "hi");
/// assert_eq!(
///     Echo.execute(String::new()).unwrap_err().kind(),
///     clifacet::FailureKind::Execution,
/// );
/// ```
pub trait Command {
    /// Input the command accepts. Construct it after parsing and validation, so
    /// this type can be total: every value of it is runnable.
    type Input;

    /// Data the application may render or print.
    ///
    /// This is the machine data a caller asked for. It is not a message: the
    /// application decides which stream it belongs on and how it is formatted.
    type Output;

    /// Runs the command against validated input.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::execution`] when the operation cannot complete. Input
    /// problems are rejected before this call, as [`Failure::validation`], by
    /// whoever built the input.
    fn execute(&self, input: Self::Input) -> Result<Self::Output, Failure>;

    /// What the command would do with `input`, without doing it — what `--dry-run` prints.
    ///
    /// A plain command cannot say, so by default this refuses with a validation failure:
    /// nothing ran, and the invocation asked for something this command does not offer.
    /// A [`Planned`] command answers with its [`Plan`], through the implementation the
    /// library provides for it, so a planned command — or a tree declared `planned` —
    /// previews without any code of its own.
    ///
    /// ```
    /// use clifacet::{Command, Failure, FailureKind};
    ///
    /// struct Echo;
    ///
    /// impl Command for Echo {
    ///     type Input = String;
    ///     type Output = String;
    ///
    ///     fn execute(&self, input: String) -> Result<String, Failure> {
    ///         Ok(input)
    ///     }
    /// }
    ///
    /// let refusal = Echo.preview("hi".to_owned()).unwrap_err();
    /// assert_eq!(refusal.kind(), FailureKind::Validation);
    /// ```
    ///
    /// # Errors
    ///
    /// The default returns [`Failure::validation`]; a [`Planned`] command returns what
    /// [`Planned::plan`] does.
    fn preview(&self, input: Self::Input) -> Result<Plan<'_, Self::Output>, Failure> {
        drop(input);
        Err(Failure::validation(
            "this command cannot show what it would do without doing it, so nothing ran",
        ))
    }
}

/// What a planned command would do, as a value: described first, executed at most once.
///
/// A plan is the steps a person reads — one line each, for a `--dry-run` to print —
/// and the work that performs them. Building one does nothing; [`Plan::execute`] is
/// what runs, and it consumes the plan, so the same plan cannot run twice.
///
/// The steps are written by the command next to the work, from the same input, and
/// the [parity harness](crate::harness) is how a test checks that they describe what
/// the work did: compare a described plan's steps with an executed plan's effects.
///
/// ```
/// use clifacet::command::Plan;
///
/// let plan = Plan::new(|| Ok(2)).step("copy a.txt").step("copy b.txt");
///
/// assert_eq!(plan.describe(), "copy a.txt\ncopy b.txt");
/// assert_eq!(plan.execute().unwrap(), 2);
/// ```
pub struct Plan<'a, T> {
    steps: Vec<String>,
    work: Box<dyn FnOnce() -> Result<T, Failure> + 'a>,
}

impl<'a, T> Plan<'a, T> {
    /// A plan that performs `work`, with no steps described yet.
    pub fn new(work: impl FnOnce() -> Result<T, Failure> + 'a) -> Self {
        Self {
            steps: Vec::new(),
            work: Box::new(work),
        }
    }

    /// Adds a step to the description, after the ones already there.
    #[must_use]
    pub fn step(mut self, step: impl Into<String>) -> Self {
        self.steps.push(step.into());
        self
    }

    /// The steps, in order.
    #[must_use]
    pub fn steps(&self) -> &[String] {
        &self.steps
    }

    /// The steps as text, one per line — what a `--dry-run` prints.
    #[must_use]
    pub fn describe(&self) -> String {
        self.steps.join("\n")
    }

    /// Performs the work.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Failure`] the work reports, as [`Command::execute`] would.
    pub fn execute(self) -> Result<T, Failure> {
        (self.work)()
    }

    /// The same plan, its output converted by `convert` once the work has run.
    ///
    /// The steps are kept as they are: converting what the work returns does not change
    /// what it does. This is how a command tree hands a leaf's plan up as its own, with
    /// the leaf's output turned into the tree's.
    ///
    /// ```
    /// use clifacet::command::Plan;
    ///
    /// let plan = Plan::new(|| Ok(2)).step("count").map(|count| format!("{count} done"));
    ///
    /// assert_eq!(plan.steps(), ["count"]);
    /// assert_eq!(plan.execute().unwrap(), "2 done");
    /// ```
    pub fn map<U>(self, convert: impl FnOnce(T) -> U + 'a) -> Plan<'a, U>
    where
        T: 'a,
    {
        let work = self.work;
        Plan {
            steps: self.steps,
            work: Box::new(move || work().map(convert)),
        }
    }
}

impl<T> fmt::Debug for Plan<'_, T> {
    /// Shows the steps; the work is a closure, and has nothing to show.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Plan")
            .field("steps", &self.steps)
            .finish_non_exhaustive()
    }
}

/// A command that answers its input with a [`Plan`] instead of acting on it.
///
/// Implement this instead of [`Command`]: the library implements [`Command`] for every
/// planned command, and its [`execute`](Command::execute) is `self.plan(input)?.execute()`.
/// That is what makes a dry run trustworthy — there is no second execution path for
/// the plan's description to drift from.
///
/// ```
/// use clifacet::command::{Plan, Planned};
/// use clifacet::{Command, Failure};
///
/// struct Remove;
///
/// impl Planned for Remove {
///     type Input = Vec<String>;
///     type Output = usize;
///
///     fn plan(&self, files: Vec<String>) -> Result<Plan<'_, usize>, Failure> {
///         let steps: Vec<String> = files.iter().map(|file| format!("remove {file}")).collect();
///         let plan = Plan::new(move || Ok(files.len())); // the real removal goes here
///         Ok(steps.into_iter().fold(plan, Plan::step))
///     }
/// }
///
/// let files = vec![String::from("a.txt")];
/// assert_eq!(Remove.plan(files.clone()).unwrap().describe(), "remove a.txt"); // --dry-run
/// assert_eq!(Remove.execute(files).unwrap(), 1); // the real run, through the plan
/// ```
pub trait Planned {
    /// Input the command accepts, validated as for [`Command::Input`].
    type Input;

    /// Data the executed plan produces, as for [`Command::Output`].
    type Output;

    /// What the command would do with `input`, without doing it.
    ///
    /// # Errors
    ///
    /// Returns a [`Failure`] when there is nothing that could be run — the same
    /// failure a real run reports, because a real run is this plan executed.
    fn plan(&self, input: Self::Input) -> Result<Plan<'_, Self::Output>, Failure>;
}

impl<P: Planned> Command for P {
    type Input = P::Input;
    type Output = P::Output;

    /// Plans, then executes the plan: the only way a planned command runs.
    fn execute(&self, input: Self::Input) -> Result<Self::Output, Failure> {
        self.plan(input)?.execute()
    }

    /// The plan itself, unexecuted: the same value a real run executes.
    fn preview(&self, input: Self::Input) -> Result<Plan<'_, Self::Output>, Failure> {
        self.plan(input)
    }
}

/// A command that runs with the application's context beside its input.
///
/// The context is what a command *depends on* — a store, a client, a clock — built once
/// by the application; the input is what the user supplied, validated. Keeping them apart
/// is what keeps a handle out of `Input` and out of a global. Shared, because a command
/// borrows its dependencies; [`App::context`](crate::App::context) says how it is built.
///
/// Every [`Command`] is a `Handler` under any context, through the blanket impl below, so
/// no command written before contexts existed changes. An application implements
/// `Handler` for a command that needs the context instead of `Command`. Implementing both
/// for one type is a compile error (E0119, conflicting implementations): the blanket
/// already makes that type a handler, and two would disagree about which one runs.
///
/// ```
/// use std::cell::RefCell;
/// use clifacet::{Failure, Handler};
///
/// struct Store {
///     records: RefCell<Vec<String>>,
/// }
///
/// struct Add;
///
/// impl Handler<Store> for Add {
///     type Input = String;
///     type Output = usize;
///
///     fn handle(&self, record: String, store: &Store) -> Result<usize, Failure> {
///         store.records.borrow_mut().push(record);
///         Ok(store.records.borrow().len())
///     }
/// }
///
/// let store = Store { records: RefCell::new(Vec::new()) };
/// assert_eq!(Add.handle(String::from("CLI-7"), &store).unwrap(), 1);
/// ```
pub trait Handler<Cx: ?Sized> {
    /// Input the command accepts, validated as for [`Command::Input`].
    type Input;

    /// Data the command produces, as for [`Command::Output`].
    type Output;

    /// Performs the command with its input and the application's context.
    ///
    /// # Errors
    ///
    /// Returns a [`Failure`] as [`Command::execute`] does.
    fn handle(&self, input: Self::Input, cx: &Cx) -> Result<Self::Output, Failure>;

    /// What the command would do with `input`, without doing it: what `--dry-run` prints.
    ///
    /// The plan may borrow the context. As for [`Command::preview`], the default refuses
    /// with a validation failure; a planned handler — `#[clifacet::planned]` on a
    /// [`PlannedWith`] impl — answers with the plan its real run executes.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::validation`] by default, and the plan's failure otherwise.
    fn preview<'a>(
        &'a self,
        input: Self::Input,
        cx: &'a Cx,
    ) -> Result<Plan<'a, Self::Output>, Failure> {
        let _ = (input, cx);
        Err(Failure::validation(
            "this command cannot describe what it would do, so it cannot be run with --dry-run",
        ))
    }
}

impl<C: Command, Cx: ?Sized> Handler<Cx> for C {
    type Input = C::Input;
    type Output = C::Output;

    /// The command, ignoring the context it does not need.
    fn handle(&self, input: Self::Input, _cx: &Cx) -> Result<Self::Output, Failure> {
        self.execute(input)
    }

    /// The command's own preview.
    fn preview<'a>(
        &'a self,
        input: Self::Input,
        _cx: &'a Cx,
    ) -> Result<Plan<'a, Self::Output>, Failure> {
        Command::preview(self, input)
    }
}

/// [`Planned`], with the application's context: the plan, and its work, may borrow it.
///
/// A second blanket impl of [`Handler`] for these would overlap the one for every
/// [`Command`], so the bridge is written by `#[clifacet::planned]` on the impl instead: it
/// adds the `Handler` whose `handle` is `self.plan(input, cx)?.execute()` and whose
/// `preview` is the plan — one execution path, as [`Planned`] guarantees.
///
/// ```
/// use std::cell::RefCell;
/// use clifacet::command::{Plan, PlannedWith};
/// use clifacet::{Failure, Handler};
///
/// struct Store {
///     records: RefCell<Vec<String>>,
/// }
///
/// struct Clear;
///
/// #[clifacet::planned]
/// impl PlannedWith<Store> for Clear {
///     type Input = ();
///     type Output = usize;
///
///     fn plan<'a>(&'a self, (): (), store: &'a Store) -> Result<Plan<'a, usize>, Failure> {
///         let count = store.records.borrow().len();
///         let work = Plan::new(move || Ok(store.records.borrow_mut().drain(..).count()));
///         Ok(work.step(format!("remove {count} records")))
///     }
/// }
///
/// let store = Store { records: RefCell::new(vec![String::from("CLI-7")]) };
/// assert_eq!(Clear.preview((), &store).unwrap().describe(), "remove 1 records"); // --dry-run
/// assert_eq!(Clear.handle((), &store).unwrap(), 1); // the real run, through the plan
/// ```
pub trait PlannedWith<Cx: ?Sized> {
    /// Input the command accepts, validated as for [`Command::Input`].
    type Input;

    /// Data the executed plan produces, as for [`Command::Output`].
    type Output;

    /// What the command would do with `input` and the context, without doing it.
    ///
    /// # Errors
    ///
    /// Returns a [`Failure`] when there is nothing that could be run — the same failure
    /// a real run reports, because a real run is this plan executed.
    fn plan<'a>(
        &'a self,
        input: Self::Input,
        cx: &'a Cx,
    ) -> Result<Plan<'a, Self::Output>, Failure>;
}
