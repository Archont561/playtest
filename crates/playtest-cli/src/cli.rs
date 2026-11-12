//! Private `clifacet`-backed command parser, input bridge, pin check, and dispatcher
//! (`Spec 0008 §§3, 7`, `Spec 0012 §1`, `Spec 0013 §§1, 2, 6`).

use std::ffi::OsString;
use std::io::Write;
use std::str::FromStr;

use clifacet::clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand, CommandFactory};
use clifacet::policy::{Capabilities, Policy, Request};
use clifacet::prompt::{Collect, Scripted, Session};
use clifacet::{Facet, Failure, Handler};
use playtest_core::{
    DiscoveredWorkspace, ExitStatus, VersionPinMismatch, discover_workspace, verify_workspace_pin,
};

use crate::RunEnvironment;
use crate::commands::init::{execute_init, normalize_mirror_language};
use crate::commands::version::build_version_report;
use crate::output::{CommandOutput, OutputFormat, WorkspacePinStatus};

const CONTRACT_AFTER_HELP: &str = "\
Machine contract (Spec 0008 §7, Spec 0013 §§1-6):
  Exit codes: 0 ok | 1 test failure | 2 usage error | 3 environment or version-pin failure
  Pass `--format json` for versioned machine output (`schema_version = 1`) on stdout.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum RawSubcommand {
    Version,
    Init { languages: Vec<String> },
}

/// Root CLI grammar bridged into [`ValidatedCli`] through [`clifacet::Facet`].
#[derive(Debug, Clone, Facet)]
#[facet(input = ValidatedCli, bridge = bridge_cli)]
struct Cli {
    format: String,
    subcommand: RawSubcommand,
}

impl CommandFactory for Cli {
    fn command() -> ClapCommand {
        ClapCommand::new("playtest")
            .version(crate::VERSION)
            .about("Unified Playtest CLI for polyglot workspace setup, SDK mirrors, and test orchestration")
            .after_help(CONTRACT_AFTER_HELP)
            .subcommand_required(true)
            .arg_required_else_help(true)
            .arg(
                Arg::new("format")
                    .long("format")
                    .value_name("FMT")
                    .global(true)
                    .default_value("pretty")
                    .help("Output format: pretty (default) or json"),
            )
            .subcommand(
                ClapCommand::new("version").about(
                    "Print the CLI version, the binary's path, and target package versions",
                ),
            )
            .subcommand(
                ClapCommand::new("init")
                    .about("Create playtest.toml and .playtest/ in a workspace")
                    .arg(
                        Arg::new("lang")
                            .long("lang")
                            .value_name("TARGET")
                            .action(ArgAction::Append)
                            .help(
                                "Target SDK language(s) for [mirrors].languages (default: python, typescript)",
                            ),
                    ),
            )
    }

    fn command_for_update() -> ClapCommand {
        <Self as CommandFactory>::command()
    }
}

impl Cli {
    fn from_matches(matches: &ArgMatches) -> Self {
        let format = matches
            .get_one::<String>("format")
            .cloned()
            .unwrap_or_else(|| "pretty".to_owned());

        let subcommand = match matches.subcommand() {
            Some(("init", sub_matches)) => {
                let languages = sub_matches
                    .get_many::<String>("lang")
                    .map(|vals| vals.cloned().collect())
                    .unwrap_or_default();
                RawSubcommand::Init { languages }
            }
            _ => RawSubcommand::Version,
        };

        Self { format, subcommand }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValidatedAction {
    Version,
    Init {
        requested_languages: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedCli {
    format: OutputFormat,
    action: ValidatedAction,
}

fn bridge_cli(cli: Cli) -> Result<ValidatedCli, Failure> {
    let format = OutputFormat::from_str(&cli.format).map_err(Failure::validation)?;
    let action = match cli.subcommand {
        RawSubcommand::Version => ValidatedAction::Version,
        RawSubcommand::Init { languages } => {
            if languages.is_empty() {
                ValidatedAction::Init {
                    requested_languages: None,
                }
            } else {
                let mut normalized = Vec::new();
                for raw in &languages {
                    let canonical = normalize_mirror_language(raw).map_err(Failure::validation)?;
                    if !normalized.iter().any(|existing| existing == canonical) {
                        normalized.push(canonical.to_owned());
                    }
                }
                ValidatedAction::Init {
                    requested_languages: Some(normalized),
                }
            }
        }
    };
    Ok(ValidatedCli { format, action })
}

struct ExecContext<'a> {
    env: &'a RunEnvironment,
    existing_workspace: Option<DiscoveredWorkspace>,
    version_pin_status: Option<WorkspacePinStatus>,
}

struct CliDispatcher;

impl Handler<ExecContext<'_>> for CliDispatcher {
    type Input = ValidatedCli;
    type Output = CommandOutput;

    fn handle(&self, input: Self::Input, cx: &ExecContext<'_>) -> Result<Self::Output, Failure> {
        match input.action {
            ValidatedAction::Version => Ok(CommandOutput::Version(build_version_report(
                cx.env,
                cx.version_pin_status.clone(),
            ))),
            ValidatedAction::Init {
                requested_languages,
            } => execute_init(
                cx.env,
                cx.existing_workspace.as_ref(),
                requested_languages.as_deref(),
            )
            .map(CommandOutput::Init)
            .map_err(|err| Failure::execution(format!("failed to initialize workspace: {err}"))),
        }
    }
}

fn inspect_workspace_for_version(
    env: &RunEnvironment,
    err: &mut dyn Write,
) -> Option<WorkspacePinStatus> {
    match discover_workspace(&env.cwd) {
        Ok(Some(ws)) => {
            let matches_cli = ws.config.version == env.cli_version;
            if !matches_cli {
                let mismatch = VersionPinMismatch {
                    manifest_path: ws.manifest_path.clone(),
                    pinned_version: ws.config.version.clone(),
                    running_version: env.cli_version.clone(),
                    binary_path: env.binary_path.clone(),
                };
                let _ = writeln!(err, "playtest: warning: {mismatch}");
            }
            Some(WorkspacePinStatus {
                manifest_path: ws.manifest_path,
                pinned_version: ws.config.version,
                matches_cli,
            })
        }
        Ok(None) => None,
        Err(ws_err) => {
            let _ = writeln!(err, "playtest: warning: {ws_err}");
            None
        }
    }
}

pub(crate) fn execute_argv<I, T>(
    argv: I,
    env: &RunEnvironment,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitStatus
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = match <Cli as Facet>::command().try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(clap_err) if clap_err.use_stderr() => {
            let _ = write!(err, "{clap_err}");
            return ExitStatus::UsageError;
        }
        Err(clap_err) => {
            if write!(out, "{clap_err}").is_err() || out.flush().is_err() {
                return ExitStatus::EnvironmentFailure;
            }
            return ExitStatus::Success;
        }
    };

    let parsed = Cli::from_matches(&matches);
    let Ok(plan) = Policy::non_interactive().resolve(Request::none(), Capabilities::headless())
    else {
        return ExitStatus::UsageError;
    };
    let mut session = Session::start(plan, || Box::new(Scripted::default()));
    let validated = match parsed.collect(&mut session).and_then(Facet::validate) {
        Ok(validated) => validated,
        Err(failure) => {
            let _ = writeln!(err, "playtest: {}", failure.message());
            return ExitStatus::UsageError;
        }
    };

    let output_format = validated.format;
    let context = if matches!(validated.action, ValidatedAction::Version) {
        let version_pin_status = inspect_workspace_for_version(env, err);
        ExecContext {
            env,
            existing_workspace: None,
            version_pin_status,
        }
    } else {
        match verify_workspace_pin(&env.cwd, &env.cli_version, &env.binary_path) {
            Ok(existing_workspace) => ExecContext {
                env,
                existing_workspace,
                version_pin_status: None,
            },
            Err(ws_err) => {
                let _ = writeln!(err, "playtest: {ws_err}");
                return ExitStatus::EnvironmentFailure;
            }
        }
    };

    let command_output = match CliDispatcher.handle(validated, &context) {
        Ok(output) => output,
        Err(failure) => {
            let _ = writeln!(err, "playtest: {}", failure.message());
            return ExitStatus::EnvironmentFailure;
        }
    };

    let rendered = match command_output.render(output_format) {
        Ok(text) => text,
        Err(render_err) => {
            let _ = writeln!(err, "playtest: failed to render output: {render_err}");
            return ExitStatus::EnvironmentFailure;
        }
    };

    if write!(out, "{rendered}").is_err() || out.flush().is_err() {
        let _ = writeln!(err, "playtest: failed to write command output to stdout");
        return ExitStatus::EnvironmentFailure;
    }

    ExitStatus::Success
}
