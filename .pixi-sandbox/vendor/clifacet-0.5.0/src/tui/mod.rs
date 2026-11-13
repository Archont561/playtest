//! A full-screen frontend built from public `ratatui` components.
//!
//! decision-4 asks for components an application can reuse, and for the generated
//! form to be assembled from them rather than beside them. This module is that
//! layer:
//!
//! - [`state`] holds the form and its keystrokes, with no screen involved;
//! - [`widgets`] draws it — [`Field`] and [`Form`] are public, and the form is
//!   nothing but fields;
//! - [`app`] runs the loop, and is the only place a terminal is touched.
//!
//! Two escape hatches compose it all without a second command: [`CustomField`] draws
//! one field with the application's own widget, and [`app::run_with`] draws the whole
//! screen. Both keep the same keys, refusals, submission and handler.
//!
//! The TUI is a frontend, not a second command. Submission hands the answers back to
//! the application, which validates them through the same bridge the flags use —
//! see `clifacet_example::run_tui` for the whole path in one place.
//!
//! ```
//! # #[cfg(feature = "tui")]
//! # {
//! use clifacet::tui::{FieldState, FormState, Key, Outcome};
//!
//! let mut form = FormState::new(vec![FieldState::new("name", "Who to greet", "")]);
//! form.handle(Key::Char('a'));
//! form.handle(Key::Char('b'));
//!
//! assert_eq!(form.handle(Key::Enter), Outcome::Submit);
//! assert_eq!(form.arguments(), vec!["--name", "ab"]);
//! # }
//! ```

pub mod app;
pub mod state;
pub mod widgets;

pub use app::{
    draw_form, keys_when_interactive, live_keys, run, run_on, run_on_with, run_with, translate,
};
pub use state::{FieldState, FormState, Key, Outcome};
pub use widgets::{CustomField, Field, Form};

// The frontend library, re-exported the way `clap` is: an application drawing its
// own screen needs the same version this crate was built against.
pub use ratatui;
