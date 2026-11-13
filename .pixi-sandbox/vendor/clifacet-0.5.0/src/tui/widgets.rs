//! The public `ratatui` components, and the generated form built from them.
//!
//! decision-4 is explicit: the generated form must be assembled from the same
//! public widgets an application can use. [`Form`] therefore renders [`Field`]s and
//! nothing else, so anything that renders correctly inside a generated form also
//! renders correctly in a hand-written screen — there is no private drawing code
//! that could drift away from the public one.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::state::{FieldState, FormState};

/// The marker shown in front of the field with the focus.
const FOCUS_MARKER: &str = "> ";

/// The marker shown in front of every other field.
const IDLE_MARKER: &str = "  ";

/// One labelled value, with an optional cursor.
///
/// It is public because an application that wants a different layout still wants
/// the same field: label, value, and a marker showing where typing goes.
#[derive(Debug, Clone, Copy)]
pub struct Field<'a> {
    field: &'a FieldState,
    focused: bool,
}

impl<'a> Field<'a> {
    /// Draws this field, marking it as focused or not.
    #[must_use]
    pub const fn new(field: &'a FieldState, focused: bool) -> Self {
        Self { field, focused }
    }

    /// The line this field renders to.
    fn line(&self) -> Line<'a> {
        let marker = if self.focused {
            FOCUS_MARKER
        } else {
            IDLE_MARKER
        };
        let value_style = if self.field.is_empty() {
            // An empty field is a prompt, not an empty answer: show what is missing.
            Style::default().add_modifier(Modifier::DIM)
        } else {
            Style::default()
        };
        // A secret is drawn as the marker every other redacted thing uses, and its
        // length is not reported either: a mask of the right width would be a fact
        // about the value. A choice is drawn between ‹ › so it reads as something to
        // move through rather than to type into, and a toggle as a box.
        let shown = if self.field.is_toggle() {
            String::from(if self.field.is_on() { "[x]" } else { "[ ]" })
        } else if self.field.is_secret() && !self.field.is_empty() {
            String::from(crate::secret::REDACTED)
        } else if !self.field.choices().is_empty() {
            let chosen = if self.field.is_empty() {
                "(none)"
            } else {
                self.field.value()
            };
            format!("‹{chosen}›")
        } else if self.field.is_empty() {
            String::from("…")
        } else {
            self.field.value().to_owned()
        };

        Line::from(vec![
            Span::styled(marker, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(self.field.label()),
            Span::raw(": "),
            Span::styled(shown, value_style),
        ])
    }
}

impl Widget for Field<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Paragraph::new(self.line()).render(area, buffer);
    }
}

/// One field, drawn by the application instead of by [`Field`].
///
/// This is the first of decision-4's two escape hatches. It is a trait rather than a
/// widget value because a field is drawn again on every frame: the custom drawing has
/// to see the current value and focus, or it would show a stale answer. Nothing is
/// registered anywhere — a [`Form`] is handed the overrides it should use, so the same
/// application can draw the same field two ways on two screens.
pub trait CustomField {
    /// Draws `field` into `area` on this frame.
    ///
    /// Draw with any `ratatui` widget — `Paragraph::new(..).render(area, buffer)` is
    /// the usual one — or directly onto the buffer. `focused` is passed rather than
    /// read from the form, because a custom field is often the only thing that knows
    /// how to show the focus in its own layout.
    fn draw(&self, field: &FieldState, focused: bool, area: Rect, buffer: &mut Buffer);
}

/// Every field of a form, with the last refusal underneath.
///
/// The hint line is part of the component because it is part of the promise: a
/// rejected answer is shown where it was typed, not printed and forgotten.
pub struct Form<'a> {
    state: &'a FormState,
    overrides: Vec<(&'a str, &'a dyn CustomField)>,
}

impl<'a> Form<'a> {
    /// Draws this form.
    #[must_use]
    pub fn new(state: &'a FormState) -> Self {
        Self {
            state,
            overrides: Vec::new(),
        }
    }

    /// Draws the field named `name` with the application's own widget.
    ///
    /// The name is the argument id, the same one [`FieldState::name`] reports. An
    /// override that names a field the form does not have is ignored: the form is
    /// still drawable, and the field list stays the single source of truth for what
    /// exists.
    #[must_use]
    pub fn with_field(mut self, name: &'a str, field: &'a dyn CustomField) -> Self {
        self.overrides.push((name, field));
        self
    }

    /// The custom drawing for a field, if the application supplied one.
    fn custom(&self, name: &str) -> Option<&'a dyn CustomField> {
        self.overrides
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, custom)| *custom)
    }
}

/// The rectangle of one row, or `None` when the screen is not tall enough for it.
fn row(area: Rect, index: u16) -> Option<Rect> {
    (index < area.height).then(|| Rect {
        height: 1,
        y: area.y.saturating_add(index),
        ..area
    })
}

impl Widget for Form<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        // One row per field, then the refusal, then the keys: a custom field needs a
        // rectangle of its own, so the form is a column of rows rather than a
        // paragraph that happens to have several lines.
        // The row index is a `u16` from the start, so a form with more fields than a
        // screen has rows simply stops at the last one instead of needing a checked
        // conversion it could never fail.
        for (index, field) in (0_u16..).zip(self.state.fields()) {
            let Some(cell) = row(area, index) else {
                return;
            };
            let focused = usize::from(index) == self.state.focus();

            match self.custom(field.name()) {
                Some(custom) => custom.draw(field, focused, cell, buffer),
                None => Field::new(field, focused).render(cell, buffer),
            }
        }

        let mut next = u16::try_from(self.state.fields().len()).unwrap_or(u16::MAX);

        if let Some(message) = self.state.message() {
            if let Some(cell) = row(area, next) {
                Paragraph::new(Span::styled(
                    message.to_owned(),
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .render(cell, buffer);
            }
            next = next.saturating_add(1);
        }

        if let Some(cell) = row(area, next) {
            Paragraph::new(Span::styled(
                "Tab: next field   Enter: submit   Esc: cancel",
                Style::default().add_modifier(Modifier::DIM),
            ))
            .render(cell, buffer);
        }
    }
}
