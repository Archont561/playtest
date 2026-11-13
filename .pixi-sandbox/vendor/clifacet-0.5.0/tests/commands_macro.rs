//! `clifacet::commands!`: a folder registered in one line (CLI-33).
//!
//! The folder is `tests/commands/`, one command per file, and the inline module's first
//! statement is the whole registry. Everything else here is what a reader
//! would check: that the tree parses as clap declares it, that the request is validated,
//! and that the generated root carries the globals every application gets.

use clap::Parser as _;
use clifacet::command::Planned;
use clifacet::policy::Capabilities;
use clifacet::prompt::{Prompter, Scripted};
use clifacet::{App, Facet};

mod commands {
    // The registry: one line, and every file in this folder is in it.
    clifacet::commands!(greet, count, output = String, planned);
}

use commands::{Cli, Commands, Request};

#[test]
fn one_line_registers_the_modules_the_tree_and_the_request() {
    let cli = Cli::try_parse_from(["app", "count", "3"]).expect("parses");
    assert!(matches!(cli.command, Some(Commands::Count(_))));
    // Every application gets these, and they stay out of the input.
    assert!(
        Cli::try_parse_from(["app", "--dry-run", "count", "3"])
            .expect("global after the subcommand")
            .globals
            .dry_run
    );

    let request = Request::try_from(cli.command.expect("chosen")).expect("validated");
    assert!(matches!(request, Request::Count(_)));
}

/// What one parsed command line would do, as the generated tree describes it.
fn planned(cli: &Cli) -> String {
    let tree = cli.command.clone().expect("a subcommand");
    let request = Request::try_from(tree.clone()).expect("validated");
    tree.plan(request).expect("it plans").describe()
}

#[test]
fn the_chosen_leaf_runs_and_its_output_is_converted() {
    let cli = Cli::try_parse_from(["app", "count", "3"]).expect("parses");
    let input = cli.clone().validate().expect("valid");
    // A dry run shows the leaf's own plan: the tree delegates `plan` when `planned` is
    // declared, and the leaf is the one that knows what it would do.
    assert_eq!(
        planned(&cli),
        "3",
        "the generated tree plans through the chosen leaf"
    );
    // The tree is parsed by clap and validated by the generated request: one variant per
    // command, so a leaf that is in the list and not in the request does not compile.
    let tree = cli.command.expect("a subcommand");
    let request = Request::try_from(tree).expect("validated");
    assert!(matches!(request, Request::Count(_)));
    assert!(matches!(input.command, Some(Request::Count(_))));
}

#[test]
fn the_runner_can_host_the_generated_root() {
    // `Cli` is a `Facet` like any other: the trait bounds the runner needs on the root
    // are the ones it needs on a hand-written one.
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = App::<Cli>::new()
        .command(&Cli::try_parse_from(["app", "greet", "ada"]).expect("parses"))
        .run_with(
            ["app", "greet", "ada"],
            Capabilities::headless(),
            || -> Box<dyn Prompter> { Box::new(Scripted::new([])) },
            &mut out,
            &mut err,
        );
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
}

#[test]
fn clap_names_each_command_after_its_module() {
    let help = Cli::command().render_help().to_string();
    assert!(help.contains("count") && help.contains("greet"), "{help}");
    let count = Cli::try_parse_from(["app", "count", "1"]).expect("parses");
    assert!(matches!(count.command, Some(Commands::Count(_))));
    // A name clap would not derive is a clap error, not a silent alias.
    assert!(Cli::try_parse_from(["app", "Count", "1"]).is_err());
}

// --- the keys a folder's commands share ----------------------------------------------

/// A second folder, whose one command runs with a context and beside the application's
/// own global option.
mod contextual {

    /// What the application depends on: how loud to be.
    pub struct Store {
        pub loud: bool,
    }

    /// An application's own global option, beside `--yes` and `--dry-run`.
    #[derive(Debug, Clone, clap::Args)]
    pub struct Options {
        /// Shout.
        #[arg(long, global = true)]
        pub loud: bool,
    }

    clifacet::commands!(
        greet,
        globals = Options,
        output = String,
        context = Store,
        name = "shout"
    );
}

#[test]
fn a_context_and_the_applications_own_globals_reach_the_root_and_the_leaf() {
    use clifacet::policy::Capabilities;
    use clifacet::prompt::{Prompter, Scripted};

    let cli = contextual::Cli::try_parse_from(["app", "--loud", "greet", "ada"]).expect("parses");
    assert!(
        cli.globals_of_the_application.loud,
        "the application's own global"
    );
    // `Globals` is there too, and neither reaches the input.
    assert!(!cli.globals.dry_run);

    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = App::<contextual::Cli>::new()
        .command(&cli)
        .context(|| Ok::<_, std::convert::Infallible>(contextual::Store { loud: true }))
        .run_with(
            ["app", "--loud", "greet", "ada"],
            Capabilities::headless(),
            || -> Box<dyn Prompter> { Box::new(Scripted::new([])) },
            &mut out,
            &mut err,
        );
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(out, b"HELLO ADA\n");
}
