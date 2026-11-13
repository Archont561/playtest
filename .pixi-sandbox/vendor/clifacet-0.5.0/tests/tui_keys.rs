//! Terminal events mapped onto the keys the form understands.
//!
//! This is the one part of the TUI that talks to a terminal library, so it is tested
//! without one: the events are ordinary values, and what matters is that every key a
//! form answers has a mapping and nothing else is mistaken for one.

#![cfg(feature = "tui")]

use clifacet::tui::Key;
use clifacet::tui::app::translate;
use clifacet::tui::ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseEvent,
};
use rstest::rstest;

/// A key press as a terminal reports it.
fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[rstest]
#[case::character(KeyCode::Char('a'), Some(Key::Char('a')))]
#[case::space(KeyCode::Char(' '), Some(Key::Char(' ')))]
#[case::backspace(KeyCode::Backspace, Some(Key::Backspace))]
#[case::tab(KeyCode::Tab, Some(Key::Tab))]
#[case::back_tab(KeyCode::BackTab, Some(Key::BackTab))]
#[case::enter(KeyCode::Enter, Some(Key::Enter))]
#[case::escape(KeyCode::Esc, Some(Key::Escape))]
#[case::arrows(KeyCode::Left, None)]
#[case::function_key(KeyCode::F(1), None)]
fn editing_keys_have_a_mapping(#[case] code: KeyCode, #[case] expected: Option<Key>) {
    assert_eq!(translate(&press(code)), expected);
}

#[test]
fn a_control_character_is_not_an_answer() {
    // Ctrl-C and friends arrive as characters with a modifier; the form has no
    // meaning for them, and typing one would only put an unrenderable byte in a value.
    let event = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(translate(&event), None);
}

#[test]
fn a_key_release_is_not_a_second_press() {
    let event = Event::Key(KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Release,
        state: KeyEventState::NONE,
    });

    assert_eq!(
        translate(&event),
        None,
        "only a press types a character, or every keystroke would double"
    );
}

#[test]
fn events_that_are_not_keys_are_ignored() {
    assert_eq!(translate(&Event::Resize(80, 24)), None);
    assert_eq!(
        translate(&Event::Mouse(MouseEvent {
            kind: clifacet::tui::ratatui::crossterm::event::MouseEventKind::Down(
                clifacet::tui::ratatui::crossterm::event::MouseButton::Left,
            ),
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        })),
        None
    );
}

/// A form with no keyboard to read must end its input, not wait for one.
///
/// `run_with` already treats "the keys ran out" as a cancellation, so a process whose
/// stdin is not a terminal — a pipe, a test runner, a Windows CI job whose console handle
/// exists but is not interactive — cancels immediately instead of blocking on an event
/// that can never arrive.
#[test]
fn a_form_with_no_keyboard_reads_no_keys_instead_of_waiting() {
    let mut keys = clifacet::tui::keys_when_interactive(false);

    assert_eq!(keys.next(), None, "no keyboard means no keys, and no wait");
}

#[test]
fn a_repeat_is_still_a_press() {
    let event = Event::Key(KeyEvent {
        code: KeyCode::Char('x'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Repeat,
        state: KeyEventState::NONE,
    });

    assert_eq!(translate(&event), Some(Key::Char('x')));
}
