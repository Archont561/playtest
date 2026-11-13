use super::VERSION;

#[test]
fn version_matches_package_metadata() {
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    assert_eq!(super::version(), VERSION);
}

#[test]
fn reexported_macro_reports_matching_workspace_version() {
    assert_eq!(crate::version!(), VERSION);
}

mod failure {
    use crate::{Failure, FailureKind};

    #[test]
    fn the_two_kinds_are_distinguishable() {
        let validation = Failure::validation("bad input");
        let execution = Failure::execution("bad input");

        assert_eq!(validation.kind(), FailureKind::Validation);
        assert_eq!(execution.kind(), FailureKind::Execution);
        assert_ne!(validation.kind(), execution.kind());
    }

    #[test]
    fn the_message_is_recoverable_and_displayed_verbatim() {
        let failure = Failure::execution("the operation failed");

        assert_eq!(failure.message(), "the operation failed");
        assert_eq!(failure.to_string(), "the operation failed");
    }

    #[test]
    fn a_failure_is_a_std_error() {
        let failure = Failure::validation("bad input");
        let as_error: &dyn std::error::Error = &failure;

        assert_eq!(as_error.to_string(), "bad input");
    }
}

mod policy {
    use crate::policy::{Mode, Policy};

    /// A policy never reports more than the ceiling it was given, and the default
    /// is the least interactive one, so a forgotten ceiling can never prompt.
    #[test]
    fn a_policy_reports_the_ceiling_it_was_given() {
        assert_eq!(Policy::non_interactive().ceiling(), Mode::NonInteractive);
        assert_eq!(
            Policy::up_to(Mode::Interactive).ceiling(),
            Mode::Interactive
        );
        assert_eq!(Policy::up_to(Mode::Tui).ceiling(), Mode::Tui);
        assert_eq!(Policy::default().ceiling(), Mode::NonInteractive);
    }
}

mod prompt {
    use std::io::{BufRead, Read, Write};

    use crate::failure::FailureKind;
    use crate::prompt::{Prompter, Terminal};

    /// A stream that refuses everything, standing in for a closed terminal.
    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Read for Broken {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::Other))
        }
    }

    impl BufRead for Broken {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::from(std::io::ErrorKind::Other))
        }

        fn consume(&mut self, _amount: usize) {}
    }

    #[test]
    fn the_process_terminal_is_constructible_without_touching_it() {
        let terminal = Terminal::stdio();

        assert!(format!("{terminal:?}").contains("Terminal"));
    }

    #[test]
    fn a_prompt_stream_that_fails_is_an_execution_failure() {
        let mut terminal = Terminal::new(std::io::Cursor::new("ada\n"), Broken);

        let failure = terminal
            .ask("Who should I greet?")
            .expect_err("a closed prompt stream cannot show the question");

        assert_eq!(failure.kind(), FailureKind::Execution);
        assert!(failure.message().contains("cannot write the prompt"));
    }

    #[test]
    fn an_input_stream_that_fails_is_an_execution_failure() {
        let mut terminal = Terminal::new(Broken, Vec::new());

        let failure = terminal
            .ask("Who should I greet?")
            .expect_err("a broken input cannot answer");

        assert_eq!(failure.kind(), FailureKind::Execution);
        assert!(failure.message().contains("cannot read the answer"));
    }
}

mod describe_argument {
    use clap::{Arg, Command};

    use crate::facet::describe_argument;

    /// One argument of each shape a diagnostic has to name.
    fn command() -> Command {
        Command::new("probe")
            .arg(Arg::new("name").long("greet-name").short('n'))
            .arg(Arg::new("level").short('l'))
            .arg(Arg::new("target").value_name("TARGET"))
            .arg(Arg::new("rest"))
    }

    #[test]
    fn an_argument_is_named_the_way_the_user_types_it() {
        let cases = [
            // The long flag wins, and it is clap's spelling, not the field's.
            ("name", "--greet-name"),
            ("level", "-l"),
            ("target", "<TARGET>"),
            // A positional with no value name falls back to its id, as clap shows it.
            ("rest", "REST"),
            // An id the command does not have is still named, never a panic.
            ("missing", "missing"),
        ];
        for (id, expected) in cases {
            assert_eq!(describe_argument(&command(), id), expected, "{id}");
        }
    }
}
