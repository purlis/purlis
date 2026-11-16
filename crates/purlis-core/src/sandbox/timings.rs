// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every in-process duration the sandbox depends on, in one value** (T1.1-01, #6848).
//!
//! A chat's confinement is built with one [`Timings`], and every part of it that waits reads its
//! wait from that value alone: the board of live asks ([`super::asks`]), the proxy
//! ([`super::egress`], [`super::browser_reach`]), its tally, the block throttle
//! ([`crate::sandboxblock::Throttle`]) and the window's end of a hold (`asking::hold_ends` in the
//! app). The values themselves are private to this module: nothing outside it can read or build
//! one but through [`Timings::production`], and, in a test build alone, [`Timings::scaled`].
//!
//! **Production is not configurable.** No environment variable and no settings key reaches this
//! module; the app and the session host pass [`Timings::production`].
//!
//! **Scaling keeps the interactions that matter.** Some pairs of durations change behaviour by
//! how they compare: a tie that becomes an order, or an order that flips. Each such pair is
//! declared in [`PAIRS`], and every field says which pairs it belongs to in [`FIELDS`].
//! [`Timings::scaled`] refuses (panics) where a declared pair would compare differently from
//! production. A pair marked `production_only` hinges on a duration that runs out of process at
//! real time and so never scales: it is exempt from the check, and every cell that hinges on it
//! also runs at production timings.
//!
//! **Out-of-process durations** are not here, and run at real time in the suite: the hook's
//! [`crate::harness::hooked::HOOK_TIMEOUT`] (60 s) and `QUESTION_HOOK_TIMEOUT` (10 s), the hook
//! sender's receipt (250 ms) and its wait for the app to let go (500 ms), the Linux supervisor's
//! `WINDOW` (60 s) and `PER_WINDOW` (10), the plugin's hook timeouts, and the harness's own Bash
//! timeout (120 s).

use std::time::Duration;

/// How long a held connection waits for the person: about a minute (N-6).
const HOLD: Duration = Duration::from_secs(60);

/// How close together new hosts from one chat must come to be one ask (N-10).
const GROUP: Duration = Duration::from_secs(2);

/// The most a connection is held while held open past its hold (ADR 0121 decision 13, #6007):
/// what keeps a Quiet that lasts all afternoon from holding a connection all afternoon.
const QUIET_HOLD: Duration = Duration::from_secs(15 * 60);

/// How long one chat's same block is heard once, and the window the per-chat count counts in
/// (#1338).
const THROTTLE_WINDOW: Duration = Duration::from_secs(60);

/// The most blocks one chat is heard in a [`THROTTLE_WINDOW`] (#1338).
const THROTTLE_PER_CHAT: usize = 10;

/// How long a client has to send its request, and a host to answer the connection.
const PATIENCE: Duration = Duration::from_secs(30);

/// How long a stopping proxy waits for its teller to tell what it was handed (#1353).
const TELLER_PATIENCE: Duration = Duration::from_secs(2);

/// How long a client has to send its request's whole head.
const HEAD: Duration = Duration::from_secs(30);

/// How long a connection may carry nothing in either direction before it is closed.
const IDLE: Duration = Duration::from_secs(600);

/// How long one tally line stands for (#1699).
const TALLY_WINDOW: Duration = Duration::from_secs(60);

/// How often a proxy's timer tells what has waited a [`TALLY_WINDOW`] (#1699).
const TALLY_TICK: Duration = Duration::from_secs(5);

/// **The harness's own Bash timeout**, unscaled: Claude Code's default of two minutes. Out of
/// process, so it never scales; Quiet's hold must stay above it (Q300).
pub const HARNESS_BASH_TIMEOUT: Duration = Duration::from_secs(120);

/// **The in-process durations the sandbox depends on** (module docs). Read a field; build one
/// only with [`Self::production`] (or, in a test build, [`Self::scaled`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// How long a held connection waits for the person (N-6).
    pub hold: Duration,
    /// How close together new hosts from one chat must come to be one ask (N-10).
    pub group: Duration,
    /// The most a connection is held in all while the person is Quiet (#6007).
    pub quiet_hold: Duration,
    /// How long one chat's same block is heard once (#1338).
    pub throttle_window: Duration,
    /// The most blocks one chat is heard in a `throttle_window` (#1338).
    pub throttle_per_chat: usize,
    /// How long a client has to send its request, and a host to answer the connection.
    pub patience: Duration,
    /// How long a stopping proxy waits for its teller (#1353).
    pub teller_patience: Duration,
    /// How long a client has to send its request's whole head.
    pub head: Duration,
    /// How long a connection may carry nothing before it is closed.
    pub idle: Duration,
    /// How long one tally line stands for (#1699).
    pub tally_window: Duration,
    /// How often a proxy's timer tells what has waited a `tally_window` (#1699).
    pub tally_tick: Duration,
    /// Keeps every value built here: no struct literal or `..` update outside this module.
    sealed: Sealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sealed;

/// One side of a declared pair: a field of [`Timings`], by name, or a duration out of process
/// that never scales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// A field of [`Timings`], by its name.
    Field(&'static str),
    /// A duration out of process, by what it is, at its real value.
    Fixed(&'static str, Duration),
}

/// **A pair of durations whose relation (<, =, >) changes behaviour.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    /// What the pair is, as a refusal names it.
    pub name: &'static str,
    pub left: Side,
    pub right: Side,
    /// The pair hinges on a duration that never scales: exempt from [`Timings::scaled`]'s check,
    /// and every cell that hinges on it also runs at production timings.
    pub production_only: bool,
}

/// **What a field of [`Timings`] belongs to**: the [`PAIRS`] it is in, by name, or why it is in
/// none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declared {
    pub field: &'static str,
    pub pairs: &'static [&'static str],
    /// Why the field is in no pair, where it is in none.
    pub alone: Option<&'static str>,
}

/// **The out-of-process durations**, each by the identifier a cell's Timings expression names
/// it by, what it is, and its real value: they run at real time in the suite, never scaled.
pub const OUT_OF_PROCESS: [(&str, &str, Duration); 5] = [
    // The hook's own timeout (`harness::hooked::HOOK_TIMEOUT`).
    (
        "HOOK_TIMEOUT",
        "HOOK_TIMEOUT",
        crate::harness::hooked::HOOK_TIMEOUT,
    ),
    // How long a hook sender may spend writing its line (`hookwire`'s `A_NOTICE_TAKES_AT_MOST`).
    (
        "hook_receipt",
        "the hook sender's receipt",
        crate::hookwire::A_NOTICE_TAKES_AT_MOST,
    ),
    // The hook sender's wait for the app to let go of its line (`A_NOTICE_IS_SEEN_WITHIN`).
    (
        "hook_senders_wait",
        "the hook sender's wait",
        crate::hookwire::A_NOTICE_IS_SEEN_WITHIN,
    ),
    // The Linux supervisor's window (`purlis_linux_sandbox::refusal::WINDOW`).
    (
        "supervisor_window",
        "the Linux supervisor's WINDOW",
        purlis_linux_sandbox::refusal::WINDOW,
    ),
    // The harness's own Bash timeout ([`HARNESS_BASH_TIMEOUT`]).
    (
        "bash_timeout",
        "the harness's Bash timeout",
        HARNESS_BASH_TIMEOUT,
    ),
];

/// The side of a pair that is out-of-process duration `n` of [`OUT_OF_PROCESS`].
const fn fixed(n: usize) -> Side {
    Side::Fixed(OUT_OF_PROCESS[n].1, OUT_OF_PROCESS[n].2)
}

const HOOK_TIMEOUT: Side = fixed(0);
const HOOK_SENDERS_WAIT: Side = fixed(2);
const SUPERVISOR_WINDOW: Side = fixed(3);
const BASH_TIMEOUT: Side = fixed(4);

/// **The identifier a cell names `side` by**: a field's own name, or an out-of-process
/// duration's identifier in [`OUT_OF_PROCESS`].
pub fn id_of(side: Side) -> &'static str {
    match side {
        Side::Field(field) => field,
        Side::Fixed(what, _) => OUT_OF_PROCESS
            .iter()
            .find(|(_, named, _)| *named == what)
            .map_or(what, |(id, _, _)| id),
    }
}

/// **Every declared pair** (module docs).
pub const PAIRS: [Pair; 9] = [
    // An ask is told once its group has gathered: a hook sender that waits on the app longer
    // than the group would see the ask first.
    Pair {
        name: "group > the hook sender's wait",
        left: Side::Field("group"),
        right: HOOK_SENDERS_WAIT,
        production_only: false,
    },
    // A stopping proxy's teller outwaits a hook sender's line.
    Pair {
        name: "teller_patience > the hook sender's wait",
        left: Side::Field("teller_patience"),
        right: HOOK_SENDERS_WAIT,
        production_only: false,
    },
    // A block kept back for a minute and an ask held a minute end together (Q289).
    Pair {
        name: "throttle_window = hold",
        left: Side::Field("throttle_window"),
        right: Side::Field("hold"),
        production_only: false,
    },
    // The tally's timer looks several times within a line's window.
    Pair {
        name: "tally_tick < tally_window",
        left: Side::Field("tally_tick"),
        right: Side::Field("tally_window"),
        production_only: false,
    },
    // Held open while Quiet outlasts an ordinary hold (Q300).
    Pair {
        name: "quiet_hold > hold",
        left: Side::Field("quiet_hold"),
        right: Side::Field("hold"),
        production_only: false,
    },
    // And outlasts the harness's own Bash timeout, which ends the command first (Q300).
    Pair {
        name: "quiet_hold > the harness's Bash timeout",
        left: Side::Field("quiet_hold"),
        right: BASH_TIMEOUT,
        production_only: false,
    },
    // A held connection's minute and the hook's own timeout end together.
    Pair {
        name: "hold = HOOK_TIMEOUT",
        left: Side::Field("hold"),
        right: HOOK_TIMEOUT,
        production_only: true,
    },
    // The app's throttle and the Linux supervisor's window are both a minute (Q299).
    Pair {
        name: "throttle_window = the Linux supervisor's WINDOW",
        left: Side::Field("throttle_window"),
        right: SUPERVISOR_WINDOW,
        production_only: true,
    },
    // A held connection's minute and the Linux supervisor's window.
    Pair {
        name: "hold = the Linux supervisor's WINDOW",
        left: Side::Field("hold"),
        right: SUPERVISOR_WINDOW,
        production_only: true,
    },
];

/// **Every field of [`Timings`], with the pairs it belongs to.** A new field says which.
pub const FIELDS: [Declared; 11] = [
    Declared {
        field: "hold",
        pairs: &[
            "throttle_window = hold",
            "quiet_hold > hold",
            "hold = HOOK_TIMEOUT",
            "hold = the Linux supervisor's WINDOW",
        ],
        alone: None,
    },
    Declared {
        field: "group",
        pairs: &["group > the hook sender's wait"],
        alone: None,
    },
    Declared {
        field: "quiet_hold",
        pairs: &[
            "quiet_hold > hold",
            "quiet_hold > the harness's Bash timeout",
        ],
        alone: None,
    },
    Declared {
        field: "throttle_window",
        pairs: &[
            "throttle_window = hold",
            "throttle_window = the Linux supervisor's WINDOW",
        ],
        alone: None,
    },
    Declared {
        field: "throttle_per_chat",
        pairs: &[],
        alone: Some("a count, never scaled: no duration's order depends on it"),
    },
    Declared {
        field: "patience",
        pairs: &[],
        alone: Some("bounds one connection's own wait: no other timer is ordered against it"),
    },
    Declared {
        field: "teller_patience",
        pairs: &["teller_patience > the hook sender's wait"],
        alone: None,
    },
    Declared {
        field: "head",
        pairs: &[],
        alone: Some("bounds one request's head: no other timer is ordered against it"),
    },
    Declared {
        field: "idle",
        pairs: &[],
        alone: Some("bounds one connection's silence: no other timer is ordered against it"),
    },
    Declared {
        field: "tally_window",
        pairs: &["tally_tick < tally_window"],
        alone: None,
    },
    Declared {
        field: "tally_tick",
        pairs: &["tally_tick < tally_window"],
        alone: None,
    },
];

impl Timings {
    /// **Today's values**, as the rulings and rows set them: the only timings a release build
    /// has.
    #[must_use]
    pub const fn production() -> Self {
        Self {
            hold: HOLD,
            group: GROUP,
            quiet_hold: QUIET_HOLD,
            throttle_window: THROTTLE_WINDOW,
            throttle_per_chat: THROTTLE_PER_CHAT,
            patience: PATIENCE,
            teller_patience: TELLER_PATIENCE,
            head: HEAD,
            idle: IDLE,
            tally_window: TALLY_WINDOW,
            tally_tick: TALLY_TICK,
            sealed: Sealed,
        }
    }

    /// The field named `field`, as a duration; none for a count or a name no field has.
    #[must_use]
    pub fn duration(&self, field: &str) -> Option<Duration> {
        Some(match field {
            "hold" => self.hold,
            "group" => self.group,
            "quiet_hold" => self.quiet_hold,
            "throttle_window" => self.throttle_window,
            "patience" => self.patience,
            "teller_patience" => self.teller_patience,
            "head" => self.head,
            "idle" => self.idle,
            "tally_window" => self.tally_window,
            "tally_tick" => self.tally_tick,
            _ => return None,
        })
    }

    /// The first declared pair, not `production_only`, that compares here otherwise than at
    /// production.
    #[cfg(any(test, feature = "test-support"))]
    fn broken_pair(&self) -> Option<&'static Pair> {
        let production = Self::production();
        PAIRS
            .iter()
            .filter(|pair| !pair.production_only)
            .find(|pair| {
                let at = |t: &Self| t.side(pair.left).cmp(&t.side(pair.right));
                at(self) != at(&production)
            })
    }

    /// The value one side of a pair has here.
    #[must_use]
    pub fn side(&self, side: Side) -> Duration {
        match side {
            Side::Field(field) => self.duration(field).unwrap_or_default(),
            Side::Fixed(_, at) => at,
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Timings {
    /// **Production's timings, each duration times `factor`**, for a test: `factor` is at least
    /// 0.5 and at most 1. Refuses (panics) where a declared pair, not `production_only`, would
    /// compare differently from production; a tie stays a tie.
    #[must_use]
    pub fn scaled(factor: f64) -> Self {
        let p = Self::production();
        let scaled = Self {
            hold: p.hold.mul_f64(factor),
            group: p.group.mul_f64(factor),
            quiet_hold: p.quiet_hold.mul_f64(factor),
            throttle_window: p.throttle_window.mul_f64(factor),
            throttle_per_chat: p.throttle_per_chat,
            patience: p.patience.mul_f64(factor),
            teller_patience: p.teller_patience.mul_f64(factor),
            head: p.head.mul_f64(factor),
            idle: p.idle.mul_f64(factor),
            tally_window: p.tally_window.mul_f64(factor),
            tally_tick: p.tally_tick.mul_f64(factor),
            sealed: Sealed,
        };
        if let Some(pair) = scaled.broken_pair() {
            panic!(
                "Timings::scaled({factor}) would change how `{}` compares from production",
                pair.name
            );
        }
        assert!(
            (0.5..=1.0).contains(&factor),
            "Timings::scaled({factor}): a factor is at least 0.5 and at most 1"
        );
        scaled
    }

    /// **These timings, with Quiet's hold scaled by its own `factor`** (Q300), within its
    /// declared pairs: above the hold, and above the harness's Bash timeout.
    #[must_use]
    pub fn with_quiet_hold_scaled(self, factor: f64) -> Self {
        let scaled = Self {
            quiet_hold: QUIET_HOLD.mul_f64(factor),
            ..self
        };
        if let Some(pair) = scaled.broken_pair() {
            panic!(
                "Timings::with_quiet_hold_scaled({factor}) would change how `{}` compares from \
                 production",
                pair.name
            );
        }
        scaled
    }
}

#[cfg(test)]
mod tests;
