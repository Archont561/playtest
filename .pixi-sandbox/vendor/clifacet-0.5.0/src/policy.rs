//! Which interaction mode a run may use, decided before anything happens.
//!
//! decision-3 fixes the precedence: an **explicit request**, then the
//! **application's policy**, then what the **environment can offer**. This module
//! is that rule and nothing else — no terminal is opened, no prompt is built, no
//! stream is written. Resolution is a pure function of its three inputs, which is
//! what makes it a decision table instead of a guess.
//!
//! ```
//! use clifacet::policy::{Capabilities, Mode, Policy, Request};
//!
//! // Permission and intent are separate: prompts are what this application wants by
//! // default, a full-screen form is what it will do when asked.
//! let policy = Policy::up_to(Mode::Tui).prefer(Mode::Interactive);
//!
//! // An explicit request wins, if the policy permits it.
//! let plan = policy
//!     .resolve(Request::mode(Mode::Tui), Capabilities::terminal_with_tui())
//!     .expect("a TUI is permitted here");
//! assert_eq!(plan.mode(), Mode::Tui);
//!
//! // Without a request, the preference decides — not the ceiling.
//! let plan = policy
//!     .resolve(Request::none(), Capabilities::terminal_with_tui())
//!     .expect("a terminal can host prompts");
//! assert_eq!(plan.mode(), Mode::Interactive);
//!
//! // Capability can only take interaction away from that default.
//! let plan = policy
//!     .resolve(Request::none(), Capabilities::headless())
//!     .expect("a headless run still resolves");
//! assert_eq!(plan.mode(), Mode::NonInteractive);
//!
//! // A terminal on its own is never consent to prompt: the policy says no.
//! assert!(
//!     Policy::non_interactive()
//!         .resolve(Request::mode(Mode::Tui), Capabilities::terminal_with_tui())
//!         .is_err()
//! );
//! ```

use std::io::IsTerminal;

/// How a run talks to the user.
///
/// The ordering is meaningful: interaction only ever *decreases* when a mode is
/// unavailable, so `NonInteractive < Interactive < Tui` is the scale a [`Policy`]
/// ceiling and preference are compared against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Flags in, data out. Nothing is ever asked.
    #[default]
    NonInteractive,
    /// Prompts on a terminal.
    Interactive,
    /// A full-screen terminal interface.
    Tui,
}

impl Mode {
    /// The position of this mode on the interaction scale.
    ///
    /// The comparison operators are derived from this one number rather than beside
    /// it, so `Mode` is ordered in exactly one place and a `Policy` can compare a
    /// preference with a ceiling in a `const fn` — which is what lets a policy that
    /// asks for more than it permits fail to compile.
    const fn rank(self) -> u8 {
        match self {
            Self::NonInteractive => 0,
            Self::Interactive => 1,
            Self::Tui => 2,
        }
    }

    /// Whether this mode may ask the user anything.
    ///
    /// This is the single source of truth for "may a frontend be constructed?",
    /// so a forbidden mode cannot reach a prompter by taking a different path.
    #[must_use]
    pub const fn asks_the_user(self) -> bool {
        !matches!(self, Self::NonInteractive)
    }
}

impl Ord for Mode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for Mode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// What a mode needs that the environment may not provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// A terminal on the input and output side.
    Terminal,
    /// Terminal support for a full-screen interface.
    Tui,
}

/// What the environment can offer a run.
///
/// Construct these explicitly — from a test, from the application, or with
/// [`Capabilities::detected`]. Detection is never treated as consent on its own;
/// see [`Policy`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    terminal: bool,
    tui: bool,
}

impl Capabilities {
    /// No terminal at all: CI, a pipe, a machine consumer.
    #[must_use]
    pub const fn headless() -> Self {
        Self {
            terminal: false,
            tui: false,
        }
    }

    /// A terminal, but no full-screen interface.
    #[must_use]
    pub const fn terminal() -> Self {
        Self {
            terminal: true,
            tui: false,
        }
    }

    /// A terminal that can host a full-screen interface.
    #[must_use]
    pub const fn terminal_with_tui() -> Self {
        Self {
            terminal: true,
            tui: true,
        }
    }

    /// What this process actually has.
    ///
    /// A full-screen interface needs three things: the `tui` feature, so there is code
    /// to draw with; a terminal for the screen, which the form draws on the diagnostic
    /// stream; and a terminal for the data, because a piped data stream asks for data
    /// rather than for a screen. Claiming any of them without the others would let a
    /// policy resolve to a mode that cannot render or cannot be consumed, so the
    /// capability is the conjunction and nothing else is probed.
    #[must_use]
    pub fn detected() -> Self {
        let terminal = std::io::stdout().is_terminal();
        Self {
            terminal,
            tui: terminal && std::io::stderr().is_terminal() && cfg!(feature = "tui"),
        }
    }

    /// Whether these capabilities can host the mode at all.
    #[must_use]
    pub const fn supports(self, mode: Mode) -> bool {
        match mode {
            Mode::NonInteractive => true,
            Mode::Interactive => self.terminal,
            Mode::Tui => self.terminal && self.tui,
        }
    }

    /// What these capabilities are missing for the given mode, if anything.
    ///
    /// Ordered by what a mode needs first: a terminal before TUI support, so the
    /// explanation names the gap the user can act on.
    #[must_use]
    pub const fn missing_for(self, mode: Mode) -> Option<Capability> {
        if self.supports(mode) {
            None
        } else if !self.terminal {
            Some(Capability::Terminal)
        } else {
            Some(Capability::Tui)
        }
    }
}

/// What one invocation asked for, before policy has a say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Request {
    mode: Option<Mode>,
    assume_yes: bool,
}

impl Request {
    /// The user asked for no particular mode.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            mode: None,
            assume_yes: false,
        }
    }

    /// The user asked for this mode explicitly.
    #[must_use]
    pub const fn mode(mode: Mode) -> Self {
        Self {
            mode: Some(mode),
            assume_yes: false,
        }
    }

    /// Records `--yes`: it answers confirmations, and decides nothing else.
    #[must_use]
    pub const fn assuming_yes(mut self, assume_yes: bool) -> Self {
        self.assume_yes = assume_yes;
        self
    }

    /// The mode the user asked for, if any.
    #[must_use]
    pub const fn requested(self) -> Option<Mode> {
        self.mode
    }

    /// Whether confirmations are answered without asking.
    #[must_use]
    pub const fn assumes_yes(self) -> bool {
        self.assume_yes
    }
}

/// How much interaction an application permits, and how much it wants by default.
///
/// A policy answers two different questions, and keeping them apart is the whole
/// point of the type:
///
/// - **permission** — the ceiling. `Policy::up_to(Mode::Interactive)` means prompts
///   are allowed and a TUI is not. Fewer modes are always allowed, because less
///   interaction never exceeds a permission.
/// - **intent** — the preference. It is the mode a run uses when the user asked for
///   nothing, and it may be *below* the ceiling: a destructive command can permit a
///   prompt for the user who asks for one while never offering it unasked.
///
/// With no preference stated, the ceiling doubles as the default, which is what a
/// policy meant before there was a way to say otherwise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Policy {
    ceiling: Mode,
    preference: Option<Mode>,
}

impl Policy {
    /// This application never asks anything.
    #[must_use]
    pub const fn non_interactive() -> Self {
        Self {
            ceiling: Mode::NonInteractive,
            preference: None,
        }
    }

    /// The most interactive mode this application is willing to enter.
    #[must_use]
    pub const fn up_to(ceiling: Mode) -> Self {
        Self {
            ceiling,
            preference: None,
        }
    }

    /// The mode this application uses when the user asks for nothing.
    ///
    /// A preference is not a permission: it can lower the default below the ceiling,
    /// never raise it above the ceiling. Asking for more interaction than the policy
    /// permits has no sensible resolution, so it panics here rather than being
    /// silently clamped — and because this is a `const fn`, a policy declared with
    /// `const` fails to compile instead of failing at run time.
    ///
    /// # Panics
    ///
    /// Panics when `preference` is above this policy's ceiling.
    #[must_use]
    pub const fn prefer(self, preference: Mode) -> Self {
        assert!(
            preference.rank() <= self.ceiling.rank(),
            "a policy cannot prefer a mode above its ceiling"
        );
        Self {
            preference: Some(preference),
            ..self
        }
    }

    /// The most interactive mode this policy permits.
    #[must_use]
    pub const fn ceiling(self) -> Mode {
        self.ceiling
    }

    /// The mode this policy prefers when the user asks for nothing, if it states one.
    #[must_use]
    pub const fn preference(self) -> Option<Mode> {
        self.preference
    }

    /// The mode a run uses when the user asks for nothing, before the environment
    /// has a say.
    ///
    /// It is the stated preference, or the ceiling when none is stated: an
    /// application that never says which mode it wants still wants the most it
    /// permits.
    #[must_use]
    pub const fn default_mode(self) -> Mode {
        match self.preference {
            Some(preference) => preference,
            None => self.ceiling,
        }
    }

    /// Applies the precedence and answers with a plan, or with the reason it cannot.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Forbidden`] when the request exceeds this policy's
    /// ceiling, and [`Refusal::Unavailable`] when the environment cannot host the
    /// permitted mode. Neither outcome runs anything.
    pub fn resolve(self, request: Request, capabilities: Capabilities) -> Result<Plan, Refusal> {
        let mode = if let Some(requested) = request.requested() {
            if requested > self.ceiling {
                return Err(Refusal::Forbidden {
                    requested,
                    ceiling: self.ceiling,
                });
            }
            if let Some(missing) = capabilities.missing_for(requested) {
                return Err(Refusal::Unavailable { requested, missing });
            }
            requested
        } else {
            // No request: the policy's default, or the most interactive mode below it
            // that the environment supports when the default cannot be hosted.
            // Interaction only ever decreases from the default, so a terminal can
            // never add consent the policy withheld nor interest it did not state.
            let default = self.default_mode();
            [Mode::Tui, Mode::Interactive, Mode::NonInteractive]
                .into_iter()
                .find(|candidate| *candidate <= default && capabilities.supports(*candidate))
                .unwrap_or(Mode::NonInteractive)
        };

        Ok(Plan {
            mode,
            assume_yes: request.assumes_yes(),
        })
    }
}

/// A permitted way to run: the mode, and whether confirmations are answered.
///
/// Only [`Policy::resolve`] produces one, so "may this run ask the user anything?"
/// has exactly one answer, and it comes from the same place every time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    mode: Mode,
    assume_yes: bool,
}

impl Plan {
    /// The mode this run uses.
    #[must_use]
    pub const fn mode(self) -> Mode {
        self.mode
    }

    /// Whether a frontend may ask the user anything.
    #[must_use]
    pub const fn asks_the_user(self) -> bool {
        self.mode.asks_the_user()
    }

    /// Whether confirmations are answered with yes instead of asked.
    #[must_use]
    pub const fn assumes_yes(self) -> bool {
        self.assume_yes
    }

    /// Builds a frontend, or nothing at all when this plan asks nothing.
    ///
    /// This is the seam a presentation layer goes through. Because the closure is
    /// the only place a prompter or terminal frontend can be constructed, a
    /// non-interactive run cannot build one by accident — there is no other door.
    pub fn with_frontend<T>(self, build: impl FnOnce() -> T) -> Option<T> {
        self.asks_the_user().then(build)
    }
}

/// Why a request could not be honored.
///
/// A refusal is not a [`Failure`]: nothing was asked of the command, and the
/// application decides what the explanation costs. It is also not a
/// cancellation — a cancel is a decision by the user, a refusal is one by the
/// policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The application's policy does not permit this mode.
    Forbidden {
        /// What the user asked for.
        requested: Mode,
        /// The most interactive mode the policy permits.
        ceiling: Mode,
    },
    /// The environment cannot host the mode.
    Unavailable {
        /// What the user asked for.
        requested: Mode,
        /// What the environment is missing.
        missing: Capability,
    },
}

impl Refusal {
    /// The caller-facing explanation, for a diagnostic stream.
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::Forbidden { requested, ceiling } => format!(
                "{} is not available: this command allows at most {}",
                describe(requested),
                describe(ceiling)
            ),
            Self::Unavailable { requested, missing } => format!(
                "{} is not available here: no {}",
                describe(requested),
                match missing {
                    Capability::Terminal => "terminal",
                    Capability::Tui => "TUI support",
                }
            ),
        }
    }
}

/// How a mode is named in a diagnostic.
const fn describe(mode: Mode) -> &'static str {
    match mode {
        Mode::NonInteractive => "non-interactive",
        Mode::Interactive => "interactive use",
        Mode::Tui => "a TUI",
    }
}
