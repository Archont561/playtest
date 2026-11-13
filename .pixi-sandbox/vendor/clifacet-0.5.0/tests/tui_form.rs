//! The TUI's public components, driven headlessly.
//!
//! decision-4 asks for ratatui components an application can reuse, with the
//! generated form built *from* them rather than beside them. These tests draw into
//! a `TestBackend` and press keys, so nothing here needs a terminal — which is what
//! makes focus, submission and cancellation assertable at all.

#![cfg(feature = "tui")]

use clifacet::clap::{CommandFactory, Parser};
use clifacet::tui::ratatui::Terminal;
use clifacet::tui::ratatui::backend::TestBackend;
use clifacet::tui::ratatui::widgets::Widget;
use clifacet::tui::{Form, FormState, Key, Outcome};
use rstest::rstest;

/// A small command to generate a form from.
#[derive(Debug, Parser)]
#[command(name = "demo")]
struct Demo {
    /// Who to greet
    #[arg(long, default_value = "world")]
    name: String,

    /// How many times to greet
    #[arg(long, default_value_t = 1)]
    repeat: u8,

    /// A value with no default
    #[arg(long)]
    extra: Option<String>,
}

/// A form generated from the demo command.
fn form() -> FormState {
    FormState::generated(&Demo::command(), &["name", "repeat", "extra"])
        .expect("these fields exist on the command")
}

/// Renders the whole form into a buffer of the given size and returns it as text.
fn rendered(state: &FormState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test backend");
    terminal
        .draw(|frame| {
            let area = frame.area();
            Form::new(state).render(area, frame.buffer_mut());
        })
        .expect("drawing into a test backend cannot fail");

    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|line| {
            (0..buffer.area.width)
                .map(|column| buffer[(column, line)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_generated_form_mirrors_the_command() {
    let state = form();

    let names: Vec<&str> = state
        .fields()
        .iter()
        .map(clifacet::tui::FieldState::name)
        .collect();
    assert_eq!(names, ["name", "repeat", "extra"]);

    let name = state.field("name").expect("the name field exists");
    assert_eq!(
        name.label(),
        "Who to greet",
        "the help text becomes the label"
    );
    assert_eq!(
        name.value(),
        "world",
        "the clap default is the initial value"
    );

    let extra = state.field("extra").expect("the extra field exists");
    assert_eq!(
        extra.value(),
        "",
        "an argument without a default starts empty"
    );
    assert!(!extra.required(), "an Option is not required");
}

#[test]
fn a_generated_form_refuses_a_field_the_command_does_not_have() {
    let error = FormState::generated(&Demo::command(), &["name", "nope"])
        .expect_err("an unknown field is a mistake, not an empty form");

    assert!(
        error.message().contains("nope"),
        "the message names the field: {}",
        error.message()
    );
}

#[rstest]
#[case::letters(vec![Key::Char('a'), Key::Char('b')], "ab")]
#[case::bytes(vec![Key::Char('\t')], "world")]
#[case::backspace(vec![Key::Backspace], "worl")]
#[case::backspace_on_empty(vec![Key::Backspace; 8], "")]
fn typing_edits_only_the_focused_field(#[case] keys: Vec<Key>, #[case] expected: &str) {
    let mut state = form();
    let mut outcomes = Vec::new();

    for key in keys {
        outcomes.push(state.handle(key));
    }

    assert!(
        outcomes.iter().all(|outcome| *outcome == Outcome::Editing),
        "editing keys do not submit or cancel: {outcomes:?}"
    );
    assert_eq!(state.field("name").expect("name exists").value(), expected);
    assert_eq!(
        state.field("repeat").expect("repeat exists").value(),
        "1",
        "the other fields are untouched"
    );
}

#[test]
fn tab_and_backtab_move_the_focus_and_wrap() {
    let mut state = form();
    assert_eq!(state.focus(), 0);

    state.handle(Key::Tab);
    assert_eq!(state.focus(), 1);
    state.handle(Key::Tab);
    state.handle(Key::Tab);
    assert_eq!(state.focus(), 0, "focus wraps to the first field");

    state.handle(Key::BackTab);
    assert_eq!(state.focus(), 2, "and backwards from the first field");
}

#[test]
fn typing_goes_to_the_focused_field_only() {
    let mut state = form();
    state.handle(Key::Tab);
    state.handle(Key::Char('7'));
    state.handle(Key::Char('2'));

    assert_eq!(
        state.field("repeat").expect("repeat exists").value(),
        "72",
        "typing replaces the untouched default in the focused field"
    );
    assert_eq!(
        state.field("name").expect("name exists").value(),
        "world",
        "the field that lost the focus is untouched"
    );
}

#[test]
fn enter_submits_and_escape_cancels() {
    let mut state = form();

    assert_eq!(state.handle(Key::Enter), Outcome::Submit);
    assert_eq!(state.handle(Key::Escape), Outcome::Cancel);
}

#[test]
fn the_arguments_name_only_the_fields_that_were_filled() {
    let mut state = form();

    assert_eq!(
        state.arguments(),
        vec!["--name", "world", "--repeat", "1"],
        "an empty optional value is omitted rather than passed as empty"
    );

    state.handle(Key::Tab);
    state.handle(Key::Tab);
    state.handle(Key::Char('x'));
    assert_eq!(
        state.arguments(),
        vec!["--name", "world", "--repeat", "1", "--extra", "x"]
    );
}

#[test]
fn the_form_renders_its_labels_values_and_the_focused_field() {
    let text = rendered(&form(), 40, 8);

    assert!(text.contains("Who to greet"), "{text}");
    assert!(text.contains("world"), "{text}");
    assert!(text.contains("How many times to greet"), "{text}");

    let first_line = text.lines().next().expect("the form draws something");
    assert!(
        first_line.contains('>') || first_line.contains('*'),
        "the focused field is marked: {first_line:?}"
    );

    let mut moved = form();
    moved.handle(Key::Tab);
    let after = rendered(&moved, 40, 8);
    assert_ne!(text, after, "moving focus changes the drawing");
}

#[test]
fn a_message_is_rendered_below_the_fields() {
    let mut state = form();
    state.set_message("--repeat must be between 1 and 8, got 9");

    let text = rendered(&state, 60, 10);

    assert!(
        text.contains("must be between 1 and 8"),
        "a rejected answer is shown where it was typed: {text}"
    );
}

#[test]
fn a_field_can_be_rendered_on_its_own() {
    // The builder path an application uses for a custom layout (decision-4).
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).expect("a test backend");
    let state = form();
    terminal
        .draw(|frame| {
            let area = frame.area();
            clifacet::tui::Field::new(state.field("name").expect("name exists"), true)
                .render(area, frame.buffer_mut());
        })
        .expect("drawing one field cannot fail");

    let buffer = terminal.backend().buffer().clone();
    let text: String = (0..buffer.area.width)
        .map(|column| buffer[(column, 0)].symbol())
        .collect();
    assert!(text.contains("world"), "{text}");
}

#[test]
fn a_form_generated_from_no_arguments_is_a_mistake() {
    let error = FormState::generated(&Demo::command(), &[])
        .expect_err("a form with nothing to ask is a bug in the application");

    assert!(
        error.message().contains("at least one argument"),
        "the message says what is wrong: {}",
        error.message()
    );
}

#[test]
fn an_argument_without_help_is_labelled_by_its_name() {
    /// A command whose second argument documents nothing.
    #[derive(Debug, Parser)]
    #[command(name = "terse")]
    struct Terse {
        /// A documented value.
        #[arg(long)]
        documented: Option<String>,
        #[arg(long, hide = true)]
        undocumented: Option<String>,
    }

    let state = FormState::generated(&Terse::command(), &["documented", "undocumented"])
        .expect("both arguments exist");

    assert_eq!(
        state
            .field("undocumented")
            .expect("undocumented exists")
            .label(),
        "undocumented",
        "a value with no help still has to be askable"
    );
}

#[test]
fn a_screen_too_short_draws_only_what_fits() {
    // A terminal can be resized mid-run; a form that panics on a short screen would
    // take the run with it.
    let state = form();

    let text = rendered(&state, 20, 1);
    let rows: Vec<&str> = text.lines().collect();

    assert_eq!(rows.len(), 1, "one row was asked for: {text}");
    assert!(rows[0].contains("Who to greet"), "{text}");
}

/// A command whose flags are not spelled like their ids.
#[derive(Debug, Parser)]
#[command(name = "spelled")]
struct Spelled {
    /// Derived from the field: `--dry-run`, id `dry_run`.
    #[arg(long, default_value = "yes")]
    dry_run: String,

    /// Renamed outright.
    #[arg(long = "greet-name", default_value = "ada")]
    name: String,

    /// Short only.
    #[arg(short = 'l', default_value = "high")]
    level: String,
}

#[test]
fn a_generated_field_is_submitted_under_the_flag_clap_knows_it_by() {
    let form = FormState::generated(&Spelled::command(), &["dry_run", "name", "level"])
        .expect("these fields exist on the command");

    let arguments = form.arguments();

    assert_eq!(
        arguments,
        ["--dry-run", "yes", "--greet-name", "ada", "-l", "high"],
        "a field's id is not its flag"
    );
    let parsed = Spelled::try_parse_from(std::iter::once(String::from("spelled")).chain(arguments))
        .expect("what the form submits, clap parses");
    assert_eq!(
        (
            parsed.dry_run.as_str(),
            parsed.name.as_str(),
            parsed.level.as_str()
        ),
        ("yes", "ada", "high")
    );
}

// --- Field kinds: what the grammar says a field is decides what a key does to it ---

use clifacet::FailureKind;
use clifacet::clap::ArgAction;
use clifacet::tui::FieldState;

/// A command with every kind of argument a form meets.
#[derive(Debug, Parser)]
#[command(name = "settings")]
struct Settings {
    /// Colour
    #[arg(long, value_parser = ["red", "green", "grey"])]
    colour: Option<String>,

    /// Size
    #[arg(long, value_parser = ["small", "large"], default_value = "small")]
    size: String,

    /// Shape
    #[arg(long, value_parser = ["round", "square"])]
    shape: String,

    /// Dry run
    #[arg(long)]
    dry_run: bool,

    /// Colour output
    #[arg(long = "no-color", action = ArgAction::SetFalse)]
    color: bool,

    /// Tags
    #[arg(long)]
    tags: Vec<String>,

    /// Verbosity
    #[arg(short, action = ArgAction::Count)]
    verbose: u8,
}

/// A form over every single-valued argument of `Settings`.
fn settings() -> FormState {
    FormState::generated(
        &Settings::command(),
        &["colour", "size", "shape", "dry_run", "color"],
    )
    .expect("these are single-valued arguments of the command")
}

/// Presses `keys` on the field at `focus`, and returns the form.
fn pressing(focus: usize, keys: &[Key]) -> FormState {
    let mut form = settings();
    for _ in 0..focus {
        form.handle(Key::Tab);
    }
    for key in keys {
        form.handle(*key);
    }
    form
}

fn field(form: &FormState, name: &str) -> FieldState {
    form.field(name).expect("a field of the form").clone()
}

#[test]
fn the_grammar_decides_each_fields_kind() {
    let form = settings();

    assert_eq!(field(&form, "colour").choices(), ["red", "green", "grey"]);
    assert_eq!(
        field(&form, "colour").value(),
        "",
        "optional, no default: (none)"
    );
    assert_eq!(
        field(&form, "size").value(),
        "small",
        "the default is chosen"
    );
    assert_eq!(field(&form, "shape").choices(), ["round", "square"]);
    assert!(field(&form, "dry_run").is_toggle());
    assert!(
        !field(&form, "dry_run").is_on(),
        "a SetTrue flag starts off"
    );
    assert!(field(&form, "color").is_on(), "a SetFalse flag starts on");
    assert_eq!(field(&form, "dry_run").choices(), [""; 0].map(String::from));
    assert!(!field(&form, "colour").is_toggle());
}

const SPACE: Key = Key::Char(' ');

#[rstest]
#[case::untouched(&[], "")]
#[case::space_picks_the_first(&[SPACE], "red")]
#[case::space_moves_on(&[SPACE, SPACE, SPACE], "grey")]
#[case::past_the_last_is_none_again(&[SPACE, SPACE, SPACE, SPACE], "")]
#[case::a_letter_jumps(&[Key::Char('g')], "green")]
#[case::the_same_letter_jumps_again(&[Key::Char('g'), Key::Char('g')], "grey")]
#[case::and_wraps(&[Key::Char('g'), Key::Char('g'), Key::Char('g')], "green")]
#[case::in_any_case(&[Key::Char('R')], "red")]
#[case::a_letter_nothing_starts_with_changes_nothing(&[SPACE, Key::Char('x')], "red")]
#[case::backspace_clears_an_optional_choice(&[SPACE, Key::Backspace], "")]
fn an_optional_choice_moves_through_its_options(#[case] keys: &[Key], #[case] expected: &str) {
    assert_eq!(field(&pressing(0, keys), "colour").value(), expected);
}

#[rstest]
#[case::space_picks_the_first(&[SPACE], "round")]
#[case::and_wraps_without_a_none(&[SPACE, SPACE, SPACE], "round")]
#[case::backspace_cannot_clear_it(&[SPACE, Key::Backspace], "round")]
fn a_required_choice_never_returns_to_none(#[case] keys: &[Key], #[case] expected: &str) {
    assert_eq!(field(&pressing(2, keys), "shape").value(), expected);
}

#[test]
fn a_choice_with_a_default_moves_on_from_it() {
    assert_eq!(field(&pressing(1, &[SPACE]), "size").value(), "large");
}

#[rstest]
#[case::space_flips_it(&[SPACE], true)]
#[case::twice_flips_it_back(&[SPACE, SPACE], false)]
#[case::a_letter_is_not_an_answer(&[Key::Char('y')], false)]
#[case::backspace_is_not_an_answer(&[Key::Backspace], false)]
fn space_flips_a_toggle(#[case] keys: &[Key], #[case] on: bool) {
    assert_eq!(field(&pressing(3, keys), "dry_run").is_on(), on);
}

/// Parses what the form submits with the command's own parser.
fn submitted(form: &FormState) -> Settings {
    Settings::try_parse_from(std::iter::once(String::from("settings")).chain(form.arguments()))
        .expect("what the form submits, clap parses")
}

#[test]
fn an_untouched_form_submits_only_what_it_must() {
    let mut form = pressing(2, &[SPACE]);
    assert_eq!(form.arguments(), ["--size", "small", "--shape", "round"]);
    form.handle(Key::Tab);

    let parsed = submitted(&form);
    assert_eq!(parsed.colour, None);
    assert!(!parsed.dry_run && parsed.color, "the flags' own defaults");
}

/// A toggle submits the bare flag, and only when it differs from what absence means —
/// `--dry-run yes` is not something clap accepts.
#[test]
fn a_toggle_submits_its_bare_flag_when_it_differs_from_the_default() {
    let mut form = pressing(0, &[Key::Char('g')]);
    for key in [Key::Tab, Key::Tab, SPACE, Key::Tab, SPACE, Key::Tab, SPACE] {
        form.handle(key);
    }
    let arguments = form.arguments();

    assert!(
        arguments.contains(&String::from("--dry-run")),
        "{arguments:?}"
    );
    assert!(
        arguments.contains(&String::from("--no-color")),
        "{arguments:?}"
    );
    let parsed = submitted(&form);
    assert_eq!(parsed.colour.as_deref(), Some("green"));
    assert_eq!(parsed.shape, "round");
    assert!(parsed.dry_run, "on");
    assert!(!parsed.color, "the SetFalse flag was switched off");
}

#[rstest]
#[case::a_repeatable_value("tags", "--tags")]
#[case::a_counter("verbose", "-v")]
fn a_list_is_refused_rather_than_cut_to_one_value(#[case] name: &str, #[case] flag: &str) {
    let failure = FormState::generated(&Settings::command(), &[name])
        .expect_err("a form field holds one value");

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(failure.message().contains(flag), "{}", failure.message());
}

#[test]
fn a_hand_built_form_can_ask_a_choice_and_a_toggle() {
    let mut form = FormState::new(vec![
        FieldState::new("level", "Level", "").with_choices(["basic", "verbose"]),
        FieldState::toggle("loud", "Loud", false),
    ]);
    for key in [Key::Char('v'), Key::Tab, SPACE] {
        form.handle(key);
    }

    assert_eq!(form.arguments(), ["--level", "verbose", "--loud"]);
}

#[test]
fn a_choice_and_a_toggle_are_drawn_as_what_they_are() {
    let untouched = rendered(&settings(), 40, 6);
    assert!(untouched.contains("Colour: ‹(none)›"), "{untouched}");
    assert!(untouched.contains("Size: ‹small›"), "{untouched}");
    assert!(untouched.contains("Dry run: [ ]"), "{untouched}");
    assert!(untouched.contains("Colour output: [x]"), "{untouched}");

    let chosen = rendered(
        &pressing(0, &[Key::Char('g'), Key::Tab, Key::Tab, Key::Tab, SPACE]),
        40,
        6,
    );
    assert!(chosen.contains("Colour: ‹green›"), "{chosen}");
    assert!(chosen.contains("Dry run: [x]"), "{chosen}");
}

#[test]
fn an_action_that_takes_no_value_is_refused_for_what_it_is() {
    let command = clifacet::clap::Command::new("manual").arg(
        clifacet::clap::Arg::new("manual")
            .long("manual")
            .action(ArgAction::Help),
    );

    let failure = FormState::generated(&command, &["manual"]).expect_err("help is not an answer");

    assert!(
        failure.message().contains("--manual"),
        "{}",
        failure.message()
    );
    assert!(
        !failure.message().contains("more than one"),
        "{}",
        failure.message()
    );
}
