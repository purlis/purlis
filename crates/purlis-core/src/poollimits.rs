// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pool's limits** (spec #4275, the person's rulings Q1 to Q16 of 2026-10-31): the numbers
//! that bound a pool's runners whose right value depends on the person's setup.
//!
//! This is the one list of them. Each [`Limit`] says its name, its unit, its default and the
//! measurement behind it, what it affects, how it takes effect and how a value is checked
//! ([`Limit::check`]), so the window, the command, the file and the errors read one answer and
//! cannot disagree. A new limit is one row of [`all`], and needs nothing else to be read and
//! written with its pool ([`Chosen`]).
//!
//! **A value is the person's or purlis's** ([`Chosen`]): one left unset follows purlis's default
//! as it improves, and one set stays across versions (Q11).
//!
//! **A limit that is hit is a [`LimitHit`]**, not only a sentence: which limit, the value in
//! force, what was needed, and a value to suggest ([`Limit::hit`]), from which the window draws
//! its offer and [`LimitHit::said`] its words.

use serde_json::{Map, Value};

/// What a limit counts, as the window and the file say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Mebibytes, as a pool's `memory` and `tmp` are written.
    Mib,
    Files,
    Minutes,
    Seconds,
    /// CPUs, held in thousandths, as a pool's `cpus` is read.
    Cpus,
    Processes,
}

impl Unit {
    /// A value of it, in plain words: `128 MiB`, `8192 files`, `10 minutes`.
    #[must_use]
    pub fn said(self, value: u64) -> String {
        match self {
            Unit::Mib => format!("{value} MiB"),
            Unit::Files => format!("{value} files"),
            Unit::Minutes if value == 1 => "1 minute".to_owned(),
            Unit::Minutes => format!("{value} minutes"),
            Unit::Seconds if value == 1 => "1 second".to_owned(),
            Unit::Seconds => format!("{value} seconds"),
            Unit::Cpus if value == 1000 => "1 CPU".to_owned(),
            Unit::Cpus => format!("{} CPUs", cpus_said(value)),
            Unit::Processes => format!("{value} processes"),
        }
    }

    /// The word the window puts beside a number of it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Unit::Mib => "MiB",
            Unit::Files => "files",
            Unit::Minutes => "minutes",
            Unit::Seconds => "seconds",
            Unit::Cpus => "CPUs",
            Unit::Processes => "processes",
        }
    }

    /// What a suggestion of a size is rounded up to, and the least margin it leaves; `None` for
    /// a time, which is doubled (Q9).
    fn step(self) -> Option<u64> {
        match self {
            Unit::Mib => Some(16),
            Unit::Files => Some(256),
            Unit::Minutes | Unit::Seconds => None,
            Unit::Cpus => Some(1000),
            Unit::Processes => Some(64),
        }
    }

    /// `value` of it as a length of time; `None` for a size.
    #[must_use]
    pub fn as_time(self, value: u64) -> Option<std::time::Duration> {
        match self {
            Unit::Minutes => Some(std::time::Duration::from_secs(value.saturating_mul(60))),
            Unit::Seconds => Some(std::time::Duration::from_secs(value)),
            Unit::Mib | Unit::Files | Unit::Cpus | Unit::Processes => None,
        }
    }
}

/// CPUs in thousandths, as a person writes them: `1.5`.
fn cpus_said(thousandths: u64) -> String {
    let (whole, part) = (thousandths / 1000, thousandths % 1000);
    if part == 0 {
        return whole.to_string();
    }
    let part = format!("{part:03}");
    format!("{whole}.{}", part.trim_end_matches('0'))
}

/// `1.5` as thousandths of a CPU; `None` for anything but digits with at most three after a point.
fn cpus_read(typed: &str) -> Option<u64> {
    let (whole, part) = typed.split_once('.').unwrap_or((typed, ""));
    let digits = |text: &str| text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(whole) || !digits(part) || part.len() > 3 || (whole.is_empty() && part.is_empty()) {
        return None;
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<u64>().ok()?
    };
    let part = format!("{part:0<3}").parse::<u64>().ok()?;
    whole.checked_mul(1000)?.checked_add(part)
}

/// What a limit affects: the groups of the pool's page (variant B of the prototype).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Code,
    SignIn,
    Runner,
}

impl Group {
    /// Every group, in the order the pool's page draws them.
    pub const ALL: [Group; 3] = [Group::Code, Group::SignIn, Group::Runner];

    /// Its heading on the pool's page.
    #[must_use]
    pub fn plain(self) -> &'static str {
        match self {
            Group::Code => "Code",
            Group::SignIn => "Sign-in",
            Group::Runner => "Runner",
        }
    }

    /// What its limits affect, in a line under the heading.
    #[must_use]
    pub fn help(self) -> &'static str {
        match self {
            Group::Code => "What purlis sends to the runner.",
            Group::SignIn => "Profiles going to the runner and signing in there.",
            Group::Runner => "What the runner is made with.",
        }
    }
}

/// When a changed value takes effect (Q13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakesEffect {
    /// From the next attempt, with nothing restarted.
    NextAttempt,
    /// When the runner is next replaced.
    NextReplacement,
}

impl TakesEffect {
    /// In the words the person's ruling gave them.
    #[must_use]
    pub fn said(self) -> &'static str {
        match self {
            TakesEffect::NextAttempt => "Applies from the next attempt.",
            TakesEffect::NextReplacement => "Applies when this runner is next replaced.",
        }
    }
}

/// **Why a default is what it is** (Q10, user story 35): what was measured, on what, and what it
/// came to. A row of [`all`] with no measurement fails its test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measurement {
    pub on: &'static str,
    pub found: &'static str,
    pub chosen_because: &'static str,
}

/// Which limit: the name code holds one by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitId {
    ProfileSize = 0,
    ProfileFiles = 1,
    CodeSize = 2,
    CodeSendTime = 3,
    SignInWait = 4,
    VouchWait = 5,
    AnswerWait = 6,
    Cpus = 7,
    Memory = 8,
    Processes = 9,
    Tmp = 10,
    SuspendAfter = 11,
}

/// One of a pool's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub id: LimitId,
    /// Its key in the pool's row.
    pub key: &'static str,
    /// Its name in plain words, for the window and the errors.
    pub plain: &'static str,
    pub unit: Unit,
    pub default: u64,
    /// The least and the most any value can be: what purlis can carry at all, on any pool. A
    /// team's highest allowed is another thing, and is given to [`Limit::check`].
    pub least: u64,
    pub most: u64,
    pub group: Group,
    /// Under Advanced on the pool's page.
    pub advanced: bool,
    /// Only a cluster pool has it: a Docker pool's page leaves it out, and refuses a value.
    pub clusters_only: bool,
    pub takes_effect: TakesEffect,
    pub measured: Measurement,
}

const ALL: [Limit; 12] = [
    Limit {
        id: LimitId::ProfileSize,
        key: "profileSize",
        plain: "Most a profile may send: size",
        unit: Unit::Mib,
        default: 128,
        least: 1,
        // What a runner stages in memory (`MOST_PROFILE_SETTINGS_STAGED_BYTES`). Near it a default
        // runner peaked at 3.26 of its 4 GiB (999 MiB sent, #4314): more than three times the size.
        most: 1024,
        group: Group::SignIn,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "a real Claude Code profile in daily use, read as a runner is sent it, on \
                 2026-10-31; and the largest measured for #4081",
            found: "50.3 MiB (2026-10-31); 58 MiB (#4081)",
            chosen_because: "over twice the largest real profile, and still what a default \
                             runner holds in memory with room to spare",
        },
    },
    Limit {
        id: LimitId::ProfileFiles,
        key: "profileFiles",
        plain: "Most a profile may send: files",
        unit: Unit::Files,
        default: 8192,
        least: 1,
        // What a runner stages (`MOST_PROFILE_SETTINGS_STAGED_FILES`).
        most: 65_536,
        group: Group::SignIn,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "a real Claude Code profile in daily use, read as a runner is sent it, on \
                 2026-10-31",
            found: "2,901 files, over the 2,048 a single message once carried",
            chosen_because: "over twice the largest real profile",
        },
    },
    Limit {
        id: LimitId::CodeSize,
        key: "codeSize",
        plain: "Largest code sent to a runner",
        unit: Unit::Mib,
        default: 1024,
        least: 1,
        // What a runner's end takes (`code::HIGHEST_BUNDLE_BYTES`): a bundle is streamed to a
        // file, so what bounds it is the runner's disk, never its memory.
        most: 65_536,
        group: Group::Code,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "a whole bundle of purlis itself, on 2026-11-01",
            found: "235 MiB",
            chosen_because: "over four times a large real repository, and what purlis has always carried",
        },
    },
    Limit {
        id: LimitId::CodeSendTime,
        key: "codeSendTime",
        plain: "Longest code send",
        unit: Unit::Minutes,
        default: 10,
        least: 1,
        // Half a day: the largest code above, at a little over 1 MiB/s.
        most: 720,
        group: Group::Code,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "a start sending bytes that do not compress, debug builds through docker exec on a CI runner, 2026-11-01",
            found: "64 MiB in 20 s, 256 MiB in 101 s, 512 MiB in 273 s: about half a second a MiB",
            chosen_because: "holds the default largest code even on that slow build, about 9 minutes",
        },
    },
    Limit {
        id: LimitId::SignInWait,
        key: "signInWait",
        plain: "Longest wait for a sign-in",
        unit: Unit::Minutes,
        default: 15,
        least: 1,
        // Two hours: past any page a harness keeps open, and a login stays a process on the
        // runner and a port on this machine for as long as it is waited on.
        most: 120,
        group: Group::SignIn,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "not measurable by a machine: a person is at the page. GitHub's device code, \
                 the longest-lived code a harness's sign-in shows (`forge::device`)",
            found: "15 minutes (GitHub's stated lifetime of a device code)",
            chosen_because: "as long as the longest code stays good; a wait past it has a person \
                             typing a code that no longer works",
        },
    },
    Limit {
        id: LimitId::VouchWait,
        key: "vouchWait",
        plain: "Longest wait for a cluster runner to be vouched for",
        unit: Unit::Minutes,
        default: 5,
        least: 1,
        // An hour: past a cluster that has to add a node and pull an image.
        most: 60,
        group: Group::Runner,
        advanced: false,
        clusters_only: true,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "a whole chat test on a kind cluster on CI with its image already loaded; no real cluster, 2026-11-01",
            found: "under 15 seconds, claim to chat",
            chosen_because: "stated, not measured: room for a real cluster to pull an image and schedule a pod",
        },
    },
    Limit {
        id: LimitId::AnswerWait,
        key: "answerWait",
        plain: "Longest wait for a runner's answer",
        unit: Unit::Seconds,
        default: 30,
        least: 5,
        // Ten minutes: a window that waits longer for one answer looks broken, however slow the
        // link.
        most: 600,
        group: Group::Runner,
        advanced: true,
        clusters_only: false,
        takes_effect: TakesEffect::NextAttempt,
        measured: Measurement {
            on: "no answer on its own was timed: the pool journey and parity behaviours on a busy CI runner, 2026-11-01",
            found: "none ran out at 30 seconds",
            chosen_because: "stated: what a window waited before this was a limit, which real work has not outrun",
        },
    },
    Limit {
        id: LimitId::Cpus,
        key: "cpus",
        plain: "CPUs",
        unit: Unit::Cpus,
        default: 2000,
        // Docker's own least; `dockerpool::Limits::new` holds the same.
        least: 10,
        most: 1_000_000,
        group: Group::Runner,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextReplacement,
        measured: Measurement {
            on: "not timed on its own: the pool journey and parity behaviours on a busy CI runner, 2026-11-01",
            found: "none failed for want of CPU at two",
            chosen_because: "stated: what a pool has had since #2506, which real work has not outrun",
        },
    },
    Limit {
        id: LimitId::Memory,
        key: "memory",
        plain: "Memory",
        unit: Unit::Mib,
        default: 4096,
        least: 512,
        most: 16 * 1024 * 1024,
        group: Group::Runner,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextReplacement,
        measured: Measurement {
            on: "a default runner taking a 999 MiB profile on CI, 2026-11-01 (#4314)",
            found: "a peak of 3.26 of its 4 GiB",
            chosen_because: "holds the largest profile a runner can be sent",
        },
    },
    Limit {
        id: LimitId::Processes,
        key: "processes",
        plain: "Processes",
        unit: Unit::Processes,
        default: 1024,
        // A runner's host, a chat, its harness and its sandbox's supervisor, with room for a build.
        least: 64,
        // Linux's own highest process id.
        most: 4_194_304,
        group: Group::Runner,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextReplacement,
        measured: Measurement {
            on: "not counted on its own: the pool journey and parity behaviours on CI, 2026-11-01",
            found: "none failed for want of a process at 1024",
            chosen_because: "stated: what a pool has had since #2506, which real work has not outrun",
        },
    },
    Limit {
        id: LimitId::Tmp,
        key: "tmp",
        plain: "Size of /tmp",
        unit: Unit::Mib,
        default: 1024,
        least: 1,
        // No larger than the memory it counts against, which a pool checks with its other values.
        most: 16 * 1024 * 1024,
        group: Group::Runner,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextReplacement,
        measured: Measurement {
            on: "not measured on its own: the pool journey and parity behaviours on CI, 2026-11-01",
            found: "none filled it at 1024 MiB",
            chosen_because: "stated: a quarter of the default memory it counts against, since #2506",
        },
    },
    Limit {
        id: LimitId::SuspendAfter,
        key: "suspendAfter",
        plain: "Suspends after this long with no link",
        unit: Unit::Minutes,
        default: 1440,
        least: 1,
        // `dockerpool::MOST_SUSPEND_AFTER`: 30 days.
        most: 43_200,
        group: Group::Runner,
        advanced: false,
        clusters_only: false,
        takes_effect: TakesEffect::NextReplacement,
        measured: Measurement {
            on: "not measurable by a machine: it is how long a person stays away",
            found: "a day, ruled for #2051",
            chosen_because: "stated: a night away keeps the runner up, and a runner left for longer stops costing",
        },
    },
];

/// **Every limit a pool has.**
#[must_use]
pub fn all() -> &'static [Limit] {
    &ALL
}

/// **Every limit that can't change** (#4281): kept apart from [`all`], so nothing that reads a
/// value, draws a row or offers a raise can reach one.
#[must_use]
pub fn fixed() -> &'static [purlis_session_protocol::fixedlimits::FixedLimit] {
    &purlis_session_protocol::fixedlimits::ALL
}

/// The limit whose row key is `key`.
#[must_use]
pub fn of_key(key: &str) -> Option<&'static Limit> {
    ALL.iter().find(|limit| limit.key == key)
}

impl LimitId {
    #[must_use]
    pub fn limit(self) -> &'static Limit {
        &ALL[self as usize]
    }
}

impl Limit {
    /// **Whether it shapes the runner itself** (Q13): written in a pool's row as its own key, on
    /// a Docker pool alone, whose runner this machine makes. A cluster's team makes its runners.
    #[must_use]
    pub fn shapes_runner(&self) -> bool {
        self.takes_effect == TakesEffect::NextReplacement
    }

    /// **Whether `value` can be this limit's**, under the `highest` allowed where a team set one:
    /// the one check the file, the command and the window share.
    pub fn check(&self, value: u64, highest: Option<u64>) -> Result<(), String> {
        if value < self.least || value > self.most {
            return Err(format!(
                "{} is at least {} and at most {}",
                self.plain,
                self.unit.said(self.least),
                self.unit.said(self.most)
            ));
        }
        match highest {
            Some(highest) if value > highest => Err(format!(
                "{} is at most {} on this pool: {TEAM_SET}",
                self.plain,
                self.unit.said(highest)
            )),
            _ => Ok(()),
        }
    }

    /// **Why pool `name`, a Docker pool, can't hold this limit**, where only a cluster pool has it:
    /// the words the file, the command and the window all give.
    #[must_use]
    pub fn not_on_docker(&self, name: &str) -> Option<String> {
        self.clusters_only.then(|| {
            format!(
                "{} is a cluster pool's, and {name} is a Docker pool",
                self.plain
            )
        })
    }

    /// **The value in the words `typed`**, or why they aren't one this limit can be: [`Self::check`]
    /// for what a person types, so a window never reads a number by rules of its own.
    pub fn check_typed(&self, typed: &str, highest: Option<u64>) -> Result<u64, String> {
        let typed = typed.trim();
        if self.unit == Unit::Cpus {
            let value = cpus_read(typed)
                .ok_or_else(|| format!("{} is a number such as 1.5", self.plain))?;
            return self.check(value, highest).map(|()| value);
        }
        let value = typed
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| typed.parse::<u64>().ok())
            .flatten()
            .ok_or_else(|| format!("{} is a whole number", self.plain))?;
        self.check(value, highest).map(|()| value)
    }

    /// **This limit hit**: `needed` against the value `in_force`, under the `highest` allowed.
    #[must_use]
    pub fn hit(&self, in_force: u64, needed: u64, highest: Option<u64>) -> LimitHit {
        LimitHit {
            limit: self.id,
            in_force,
            needed,
            suggested: self.suggested(in_force, needed, highest),
            highest,
            by_team: false,
            team_suggested: false,
        }
    }

    /// **This wait run out** at the value `in_force`: what it took is not known past that.
    #[must_use]
    pub fn waited(&self, in_force: u64, highest: Option<u64>) -> LimitHit {
        self.hit(in_force, in_force, highest)
    }

    /// For a time, double the wait; for a size, what is needed plus a margin (a quarter, or a
    /// step at least), rounded up to a step. Never above what this limit can be. `None` where
    /// that would not hold what is needed, or would not raise a time at all.
    fn suggested(&self, in_force: u64, needed: u64, highest: Option<u64>) -> Option<u64> {
        let ceiling = highest.map_or(self.most, |highest| highest.min(self.most));
        let Some(step) = self.unit.step() else {
            let doubled = in_force.saturating_mul(2).min(ceiling);
            return (doubled > in_force).then_some(doubled);
        };
        let with_margin = needed.saturating_add((needed / 4).max(step));
        let rounded = with_margin.div_ceil(step).saturating_mul(step);
        let value = rounded.min(ceiling);
        (value >= needed).then_some(value)
    }
}

/// What a person at a team's highest is told: who set it, and where to go (#4287).
const TEAM_SET: &str = "the team that defines this pool set that: ask whoever looks after its \
                        Pool in the cluster to raise it";

/// **A limit that was hit** (spec #4275's typed refusal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitHit {
    pub limit: LimitId,
    pub in_force: u64,
    pub needed: u64,
    pub suggested: Option<u64>,
    pub highest: Option<u64>,
    /// That highest is what the team defining the pool set (#4287).
    pub by_team: bool,
    /// The value suggested is what the team defining the pool suggests (#4287).
    pub team_suggested: bool,
}

impl LimitHit {
    /// In plain words: how much is needed, the limit, and what to raise it to.
    #[must_use]
    pub fn said(&self) -> String {
        let limit = self.limit.limit();
        if limit.unit.step().is_none() {
            let in_force = limit.unit.said(self.in_force);
            return match self.suggested {
                Some(value) => format!(
                    "it took longer than this pool's limit of {in_force} ({}): raise it to {}",
                    limit.plain,
                    limit.unit.said(value)
                ),
                None if self.by_team => format!(
                    "it took longer than this pool's limit of {in_force} ({}), the most it can be: \
                     {TEAM_SET}",
                    limit.plain
                ),
                None => format!(
                    "it took longer than this pool's limit of {in_force} ({}), the most it can \
                     be: try again when the runner and the link to it are less busy",
                    limit.plain
                ),
            };
        }
        let offer = match self.suggested {
            Some(value) => format!(": raise it to {}", limit.unit.said(value)),
            None if self.by_team => format!(
                ", and this pool allows at most {}: {TEAM_SET}, or send less",
                limit.unit.said(self.highest.unwrap_or(limit.most))
            ),
            None => format!(", more than this limit can be: send less than {}", {
                limit.unit.said(self.highest.unwrap_or(limit.most))
            }),
        };
        format!(
            "it needs {}, over this pool's limit of {} ({}){offer}",
            limit.unit.said(self.needed),
            limit.unit.said(self.in_force),
            limit.plain,
        )
    }
}

/// **A limit hit on a pool, kept for the window to ask of** ([`hits_said_in`]): its words, and
/// the hit they came from. One a limit, on a pool: the map stays as small as they are.
static HITS: std::sync::Mutex<std::collections::BTreeMap<(String, usize), (LimitHit, String)>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// **`hit`, on pool `pool`**, in words the window can ask of ([`hits_said_in`]). Only purlis's own
/// measure of a limit comes here, never a number a runner said: a runner cannot choose what the
/// person is offered.
#[must_use]
pub fn hit_on(pool: &str, hit: LimitHit) -> String {
    let said = hit.said();
    HITS.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert((pool.to_owned(), hit.limit as usize), (hit, said.clone()));
    said
}

/// **The limits of pool `pool` that the refusal in the words `refused` said**: asked of what was
/// kept, never read from the words. Only the pool acted on is asked of, so words a
/// runner echoes reach no other pool's value, and only what purlis itself measured there. Empty
/// for any other refusal.
#[must_use]
pub fn hits_said_in(pool: &str, refused: &str) -> Vec<LimitHit> {
    HITS.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .filter(|((of, _), (_, words))| of == pool && refused.contains(words.as_str()))
        .map(|(_, (hit, _))| *hit)
        .collect()
}

/// **The values the person chose for a pool's limits**, each unset where the default holds; and,
/// on a cluster pool, **what the team defining it set** (#4287): each limit's default and highest
/// allowed, read from the cluster's Pool and never chosen here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Chosen {
    set: [Option<u64>; ALL.len()],
    team_default: [Option<u32>; ALL.len()],
    team_highest: [Option<u32>; ALL.len()],
}

impl Chosen {
    /// What the person set, if they did: the mark of a value as theirs.
    #[must_use]
    pub fn get(&self, id: LimitId) -> Option<u64> {
        self.set[id as usize]
    }

    /// The time that holds for a wait, as [`Self::in_force`] says it; none for a size.
    #[must_use]
    pub fn wait(&self, id: LimitId) -> Option<std::time::Duration> {
        id.limit().unit.as_time(self.in_force(id))
    }

    /// **The default this pool's limit follows**: purlis's, or the team's where it is lower, and
    /// never above the highest the team allows. A team's default never raises a limit unasked
    /// (the person's ruling of 2026-11-01): a higher one is [`Self::team_suggests`].
    #[must_use]
    pub fn default_of(&self, id: LimitId) -> u64 {
        let ours = id.limit().default;
        let default = self.team_default[id as usize].map_or(ours, |team| u64::from(team).min(ours));
        self.team_highest(id)
            .map_or(default, |top| top.min(default))
    }

    /// **The value the team suggests**: its default, where that is above what holds now. Only the
    /// person's press sets it.
    #[must_use]
    pub fn team_suggests(&self, id: LimitId) -> Option<u64> {
        self.team_default[id as usize]
            .map(u64::from)
            .filter(|team| *team > self.in_force(id) && *team > id.limit().default)
    }

    /// Whether the default this limit follows is the team's doing.
    #[must_use]
    pub fn default_by_team(&self, id: LimitId) -> bool {
        self.default_of(id) != id.limit().default
    }

    /// **What is said of a value the person set above what the team allows**, which holds to it.
    #[must_use]
    pub fn over_team_said(&self, id: LimitId) -> Option<String> {
        let top = self.team_highest(id)?;
        (self.get(id)? > top).then(|| {
            format!(
                "The team that defines this pool allows at most {}, so that holds. Set a value \
                 within it or reset this.",
                id.limit().unit.said(top)
            )
        })
    }

    /// The highest the team defining this pool allows, where it set one.
    #[must_use]
    pub fn team_highest(&self, id: LimitId) -> Option<u64> {
        self.team_highest[id as usize].map(u64::from)
    }

    /// The value that holds: what the person set, or the default, and never above the highest the
    /// team allows, so a value kept from before the team lowered it holds to it too.
    #[must_use]
    pub fn in_force(&self, id: LimitId) -> u64 {
        let value = self.get(id).unwrap_or(self.default_of(id));
        self.team_highest(id).map_or(value, |top| top.min(value))
    }

    /// These values, with `id` set to `value`, or back to the default.
    #[must_use]
    pub fn with(mut self, id: LimitId, value: Option<u64>) -> Chosen {
        self.set[id as usize] = value;
        self
    }

    /// **These values, under what the team set**, from the shape a Pool's `spec.limits` and a
    /// pool's row hold: `{key: {default, highest}}`. An entry that is no limit, or a value its
    /// limit can't be, says nothing; a default above the highest is dropped. A highest below what
    /// the limit can be holds at the least it can be, so what asked for strictness still gets it.
    /// A limit only a cluster pool has isn't a Pool's to set: it's the wait to be vouched for, and
    /// the wait for a thing shouldn't be set by that thing. So a Pool can't make a limit more than
    /// purlis can carry.
    #[must_use]
    pub fn under_team(mut self, team: Option<&Value>) -> Chosen {
        self.team_default = [None; ALL.len()];
        self.team_highest = [None; ALL.len()];
        for limit in all().iter().filter(|limit| !limit.clusters_only) {
            let Some(entry) = team.and_then(|team| team.get(limit.key)) else {
                continue;
            };
            let value = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_u64)
                    .filter(|value| limit.check(*value, None).is_ok())
                    .and_then(|value| u32::try_from(value).ok())
            };
            let highest = entry
                .get("highest")
                .and_then(Value::as_u64)
                .map(|highest| highest.max(limit.least))
                .filter(|highest| limit.check(*highest, None).is_ok())
                .and_then(|highest| u32::try_from(highest).ok());
            self.team_highest[limit.id as usize] = highest;
            self.team_default[limit.id as usize] = value("default")
                .filter(|default| highest.is_none_or(|highest| *default <= highest));
        }
        self
    }

    /// Writes what the team set into a pool's `row`, as [`Self::under_team`] reads it.
    pub fn team_to_row(&self, row: &mut Value) {
        let mut team = Map::new();
        for limit in all() {
            let mut entry = Map::new();
            if let Some(default) = self.team_default[limit.id as usize] {
                entry.insert("default".to_owned(), default.into());
            }
            if let Some(highest) = self.team_highest[limit.id as usize] {
                entry.insert("highest".to_owned(), highest.into());
            }
            if !entry.is_empty() {
                team.insert(limit.key.to_owned(), entry.into());
            }
        }
        if !team.is_empty() {
            row["teamLimits"] = team.into();
        }
    }

    /// **Limit `id` hit** at `in_force`, needing `needed`: under the team's highest, and any
    /// `older` runner's, whichever is lower.
    #[must_use]
    pub fn hit(&self, id: LimitId, in_force: u64, needed: u64, older: Option<u64>) -> LimitHit {
        let team = self.team_highest(id);
        let highest = match (team, older) {
            (Some(team), Some(older)) => Some(team.min(older)),
            (team, older) => team.or(older),
        };
        let hit = id.limit().hit(in_force, needed, highest);
        // What the team suggests is offered where it holds what is needed.
        let team_says = self.team_default[id as usize]
            .map(u64::from)
            .filter(|team| *team > in_force && *team >= needed)
            .filter(|team| highest.is_none_or(|top| *team <= top));
        LimitHit {
            by_team: team.is_some() && team == highest,
            team_suggested: team_says.is_some(),
            suggested: team_says.or(hit.suggested),
            ..hit
        }
    }

    /// **Wait `id` run out** at `in_force`, as [`Self::hit`].
    #[must_use]
    pub fn waited(&self, id: LimitId, in_force: u64, older: Option<u64>) -> LimitHit {
        self.hit(id, in_force, in_force, older)
    }

    /// Writes the values set into a pool's `row`; one unset is left out.
    pub fn to_row(&self, row: &mut Value) {
        for limit in all().iter().filter(|limit| !limit.shapes_runner()) {
            if let Some(value) = self.get(limit.id) {
                row[limit.key] = value.into();
            }
        }
    }

    /// **The values a pool's row holds, and those it can't** (#4381): a value [`Limit::check`]
    /// refuses costs only that limit, which follows purlis's default and says why, so a typo
    /// never stops the file being read. `docker` names a Docker pool, which holds no limit only
    /// a cluster pool has.
    #[must_use]
    pub fn read_row(
        fields: &Map<String, Value>,
        docker: Option<&str>,
    ) -> (Chosen, Vec<RefusedValue>) {
        let mut chosen = Chosen::default();
        let mut refused = Vec::new();
        // Those shaping a runner have their own spelling in the row (`runners::Set`).
        for limit in all().iter().filter(|limit| !limit.shapes_runner()) {
            let Some(held) = fields.get(limit.key) else {
                continue;
            };
            let read = held
                .as_u64()
                .ok_or_else(|| format!("{} is a whole number", limit.plain))
                .and_then(|value| limit.check(value, None).map(|()| value))
                .and_then(
                    |value| match docker.and_then(|name| limit.not_on_docker(name)) {
                        Some(why) => Err(why),
                        None => Ok(value),
                    },
                );
            match read {
                Ok(value) => chosen = chosen.with(limit.id, Some(value)),
                Err(why) => refused.push(RefusedValue {
                    limit: limit.id,
                    held: held.clone(),
                    why,
                }),
            }
        }
        (chosen, refused)
    }
}

/// **A value in a pool's file its limit can't be** (#4381). The limit follows purlis's default,
/// and the value stays in the file as it was typed, for the person to put right, until they set
/// or reset that limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedValue {
    pub limit: LimitId,
    /// What the file holds, written back as it was. Never said: only [`Self::why`] is.
    pub held: Value,
    /// The check's own words.
    pub why: String,
}

impl RefusedValue {
    /// In plain words, for the pool's page and the command.
    #[must_use]
    pub fn said(&self) -> String {
        format!(
            "The value in this pool's file can't be used, so this follows the default. {}. \
             Set a value or reset it.",
            self.why
        )
    }

    /// Writes the value back into a pool's `row`.
    pub fn to_row(&self, row: &mut Value) {
        row[self.limit.limit().key] = self.held.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_limit_has_a_default_within_its_checks_and_a_recorded_measurement() {
        for limit in all() {
            assert_eq!(limit.id.limit().key, limit.key, "{limit:?} is out of order");
            assert_eq!(of_key(limit.key), Some(limit));
            assert_eq!(limit.check(limit.default, None), Ok(()), "{}", limit.key);
            let Measurement {
                on,
                found,
                chosen_because,
            } = limit.measured;
            for said in [on, found, chosen_because] {
                assert!(!said.trim().is_empty(), "{} has no measurement", limit.key);
            }
        }
    }

    /// The list says what a pool holds when nothing is set, so neither can drift from the other.
    #[test]
    fn the_limits_shaping_a_runner_default_to_what_a_runner_is_made_with() {
        let made = crate::dockerpool::Limits::default();
        assert_eq!(LimitId::Cpus.limit().default, u64::from(made.millicpus()));
        assert_eq!(LimitId::Memory.limit().default, made.memory_mib());
        assert_eq!(LimitId::Processes.limit().default, u64::from(made.pids()));
        assert_eq!(LimitId::Tmp.limit().default, made.tmp_mib());
        assert_eq!(
            LimitId::SuspendAfter.limit().default * 60,
            crate::dockerpool::SUSPEND_AFTER.as_secs()
        );
        assert_eq!(
            LimitId::SuspendAfter.limit().most * 60,
            crate::dockerpool::MOST_SUSPEND_AFTER.as_secs()
        );
    }

    #[test]
    fn cpus_are_typed_as_a_person_writes_them_and_held_in_thousandths() {
        let cpus = LimitId::Cpus.limit();
        assert_eq!(cpus.check_typed("1.5", None), Ok(1500));
        assert_eq!(cpus.check_typed(" 2 ", None), Ok(2000));
        assert_eq!(cpus.check_typed(".25", None), Ok(250));
        for bad in ["", ".", "1.2345", "1,5", "-1", "1e3", "0"] {
            assert!(cpus.check_typed(bad, None).is_err(), "{bad:?}");
        }
        assert_eq!(cpus.unit.said(1000), "1 CPU");
        assert_eq!(cpus.unit.said(1500), "1.5 CPUs");
        assert_eq!(cpus.unit.said(2000), "2 CPUs");
    }

    /// The defaults hold what was measured, so an ordinary profile starts with no change.
    #[test]
    fn the_defaults_hold_the_largest_real_profile_measured() {
        assert!(LimitId::ProfileSize.limit().default >= 58);
        assert!(LimitId::ProfileFiles.limit().default >= 2901);
    }

    #[test]
    fn a_value_is_refused_outside_what_a_limit_can_be_and_above_the_highest_allowed() {
        let size = LimitId::ProfileSize.limit();
        assert_eq!(size.check(1, None), Ok(()));
        assert_eq!(size.check(1024, None), Ok(()));
        let low = size.check(0, None).unwrap_err();
        assert!(low.contains("at least 1 MiB"), "{low}");
        assert!(size.check(1025, None).is_err());
        assert_eq!(size.check(512, Some(512)), Ok(()));
        let high = size.check(513, Some(512)).unwrap_err();
        assert!(high.contains("at most 512 MiB on this pool"), "{high}");
    }

    #[test]
    fn what_a_person_types_is_a_value_or_the_same_refusal_as_the_file_gets() {
        let size = LimitId::ProfileSize.limit();
        assert_eq!(size.check_typed(" 256 ", None), Ok(256));
        for not_a_number in [
            "",
            "  ",
            "1.5",
            "-1",
            "+1",
            "1e3",
            "12a",
            "\u{663}",
            "99999999999999999999",
        ] {
            let why = size.check_typed(not_a_number, None).unwrap_err();
            assert!(why.contains("whole number"), "{not_a_number:?}: {why}");
        }
        assert_eq!(size.check_typed("0", None), size.check(0, None).map(|()| 0));
        assert_eq!(
            size.check_typed("513", Some(512)),
            size.check(513, Some(512)).map(|()| 513)
        );
    }

    /// Q12: the window's word is "limit", so nothing the list says reaches it as another.
    #[test]
    fn nothing_the_list_says_calls_a_limit_a_timeout_a_quota_or_config() {
        let mut said: Vec<String> = Group::ALL
            .iter()
            .flat_map(|group| [group.plain().to_owned(), group.help().to_owned()])
            .collect();
        said.extend(
            [TakesEffect::NextAttempt, TakesEffect::NextReplacement]
                .map(|one| one.said().to_owned()),
        );
        for limit in all() {
            said.push(limit.plain.to_owned());
            said.push(limit.check(0, None).unwrap_err());
            said.push(limit.check(limit.most, Some(1)).unwrap_err());
            said.push(limit.check_typed("x", None).unwrap_err());
            said.push(limit.hit(limit.default, limit.default + 1, None).said());
        }
        for line in said {
            let lower = line.to_lowercase();
            for word in ["timeout", "time out", "quota", "config"] {
                assert!(!lower.contains(word), "{line}");
            }
        }
    }

    /// The page draws these, so a new group can't reach it without a heading and a line.
    #[test]
    fn every_group_and_every_way_of_taking_effect_says_itself() {
        for group in Group::ALL {
            assert!(!group.plain().is_empty() && !group.help().is_empty());
        }
        for limit in all() {
            assert!(Group::ALL.contains(&limit.group), "{}", limit.key);
            assert!(limit.takes_effect.said().starts_with("Applies "));
        }
    }

    #[test]
    fn a_suggestion_is_what_is_needed_with_a_margin_rounded_up() {
        let table = [
            // A quarter over, then the next 16 MiB.
            (LimitId::ProfileSize, 150, None, Some(192)),
            // The least margin is one step.
            (LimitId::ProfileSize, 20, None, Some(48)),
            (LimitId::ProfileFiles, 9000, None, Some(11_264)),
            // Stops at the highest allowed, which still holds what is needed.
            (LimitId::ProfileSize, 150, Some(160), Some(160)),
            (LimitId::ProfileFiles, 60_000, None, Some(65_536)),
            // No value holds it: nothing to suggest.
            (LimitId::ProfileSize, 170, Some(160), None),
            (LimitId::ProfileSize, 2000, None, None),
        ];
        for (id, needed, highest, wanted) in table {
            assert_eq!(
                id.limit().hit(128, needed, highest).suggested,
                wanted,
                "{id:?} {needed} {highest:?}"
            );
        }
    }

    /// A wait is doubled, up to what it can be; at the top nothing raises it (Q9, Q8).
    #[test]
    fn a_suggestion_for_a_wait_is_double_it_and_never_past_the_most() {
        let table = [
            (LimitId::CodeSendTime, 10, None, Some(20)),
            (LimitId::SignInWait, 15, None, Some(30)),
            (LimitId::VouchWait, 5, None, Some(10)),
            (LimitId::AnswerWait, 30, None, Some(60)),
            (LimitId::AnswerWait, 400, None, Some(600)),
            (LimitId::AnswerWait, 600, None, None),
            // Stops at the highest allowed, and at it offers nothing.
            (LimitId::VouchWait, 5, Some(8), Some(8)),
            (LimitId::VouchWait, 8, Some(8), None),
        ];
        for (id, in_force, highest, wanted) in table {
            assert_eq!(
                id.limit().waited(in_force, highest).suggested,
                wanted,
                "{id:?} {in_force} {highest:?}"
            );
        }
    }

    #[test]
    fn a_wait_run_out_says_its_limit_and_what_to_raise_it_to() {
        let said = LimitId::CodeSendTime.limit().waited(10, None).said();
        assert_eq!(
            said,
            "it took longer than this pool's limit of 10 minutes (Longest code send): raise it \
             to 20 minutes"
        );
        let top = LimitId::AnswerWait.limit().waited(600, None).said();
        assert!(top.contains("the most it can be"), "{top}");
        assert!(!top.contains("raise it"), "{top}");
    }

    #[test]
    fn the_refused_words_find_the_hit_they_said_and_no_other_words_do() {
        let hit = LimitId::CodeSize.limit().hit(1024, 1500, None);
        let said = hit_on("hits-of-a-test", hit);
        let refused = format!("the code did not reach the runner: {said}");
        assert_eq!(hits_said_in("hits-of-a-test", "other words"), Vec::new());
        // The same words from another pool's runner reach nothing of this pool's.
        assert_eq!(hits_said_in("another-pool", &refused), Vec::new());
        assert_eq!(hits_said_in("hits-of-a-test", &refused), vec![hit]);
        assert_eq!(
            hits_said_in("hits-of-a-test", &refused),
            vec![hit],
            "a window drawn again is offered it again"
        );
    }

    /// #4281: a fixed limit says why and what to do instead, and has no way to be changed.
    #[test]
    fn a_fixed_limit_says_why_and_what_instead_and_is_no_limit_that_can_change() {
        for limit in fixed() {
            assert!(of_key(limit.key).is_none(), "{}", limit.key);
            assert!(!limit.why.is_empty() && !limit.instead.is_empty());
            let said = limit.said();
            assert!(said.contains("fixed") && said.contains("Instead"), "{said}");
            let lower = format!("{} {said}", limit.plain).to_lowercase();
            for word in ["timeout", "time out", "quota", "config", "raise"] {
                assert!(!lower.contains(word), "{lower}");
            }
        }
    }

    /// A wait set and left unset, as what it is held to.
    #[test]
    fn only_a_time_is_a_length_of_time() {
        use std::time::Duration;
        assert_eq!(Unit::Minutes.as_time(2), Some(Duration::from_secs(120)));
        assert_eq!(Unit::Seconds.as_time(2), Some(Duration::from_secs(2)));
        assert_eq!(Unit::Mib.as_time(2), None);
        let set = Chosen::default().with(LimitId::AnswerWait, Some(45));
        assert_eq!(set.wait(LimitId::AnswerWait), Some(Duration::from_secs(45)));
        assert_eq!(set.wait(LimitId::VouchWait), Some(Duration::from_secs(300)));
        assert_eq!(set.wait(LimitId::CodeSize), None);
    }

    /// Only a cluster pool waits to be vouched for, and the answer wait is under Advanced.
    #[test]
    fn the_vouch_wait_is_a_cluster_pools_and_only_the_answer_wait_is_advanced() {
        for limit in all() {
            assert_eq!(
                limit.clusters_only,
                limit.id as usize == LimitId::VouchWait as usize
            );
            assert_eq!(
                limit.advanced,
                limit.id as usize == LimitId::AnswerWait as usize
            );
        }
    }

    /// The waits still kept beside what they hold are what the list says.
    #[test]
    fn the_defaults_of_the_waits_are_the_waits_in_the_code() {
        use crate::pools::VOUCHED_WITHIN;
        use crate::profilestore::sign_in::driven::LONGEST;
        let default = |id: LimitId| id.limit().unit.as_time(id.limit().default);
        assert_eq!(default(LimitId::SignInWait), Some(LONGEST));
        assert_eq!(default(LimitId::VouchWait), Some(VOUCHED_WITHIN));
    }

    /// What the list says a runner's end takes is what it takes.
    #[test]
    fn the_largest_code_is_what_a_runners_end_takes_and_the_defaults_are_todays() {
        use purlis_session_protocol::code;
        let size = LimitId::CodeSize.limit();
        assert_eq!(size.most << 20, code::HIGHEST_BUNDLE_BYTES);
        assert_eq!(size.default << 20, code::MOST_BUNDLE_BYTES);
        let time = LimitId::CodeSendTime.limit();
        assert_eq!(time.most * 60, code::LONGEST_TRANSFER_SECS);
    }

    #[test]
    fn a_hit_says_what_is_needed_the_limit_and_the_value_to_raise_it_to() {
        let said = LimitId::ProfileSize.limit().hit(128, 150, None).said();
        assert_eq!(
            said,
            "it needs 150 MiB, over this pool's limit of 128 MiB (Most a profile may send: \
             size): raise it to 192 MiB"
        );
        let none = LimitId::ProfileFiles.limit().hit(8192, 70_000, None).said();
        assert!(none.contains("send less than 65536 files"), "{none}");
        assert!(!none.contains("raise it"), "{none}");
    }

    #[test]
    fn an_unset_value_follows_the_default_and_a_set_one_is_kept_and_marked() {
        let unset = Chosen::default();
        assert_eq!(unset.get(LimitId::ProfileSize), None);
        assert_eq!(unset.in_force(LimitId::ProfileSize), 128);

        let set = unset.with(LimitId::ProfileSize, Some(128));
        assert_eq!(set.get(LimitId::ProfileSize), Some(128), "the person's own");
        assert_eq!(set.with(LimitId::ProfileSize, None), unset, "a reset");
    }

    #[test]
    fn a_row_holds_only_the_values_set() {
        let chosen = Chosen::default().with(LimitId::ProfileFiles, Some(9000));
        let mut row = json!({"name": "builds"});
        chosen.to_row(&mut row);
        assert_eq!(row, json!({"name": "builds", "profileFiles": 9000}));
        assert_eq!(
            Chosen::read_row(row.as_object().unwrap(), None),
            (chosen, Vec::new())
        );
    }

    /// #4287: what the team set is the default, the top, and what a suggestion stops at.
    #[test]
    fn a_teams_default_and_highest_hold_and_a_suggestion_stops_at_the_highest() {
        let team = json!({
            "signInWait": {"default": 10, "highest": 40},
            "profileSize": {"highest": 160},
        });
        let chosen = Chosen::default().under_team(Some(&team));
        assert_eq!(chosen.in_force(LimitId::SignInWait), 10);
        assert_eq!(chosen.team_highest(LimitId::SignInWait), Some(40));
        // A value kept from before the team lowered it holds to it.
        let high = chosen.with(LimitId::SignInWait, Some(100));
        assert_eq!(high.in_force(LimitId::SignInWait), 40);
        assert_eq!(
            high.get(LimitId::SignInWait),
            Some(100),
            "still the person's"
        );

        let hit = chosen.waited(LimitId::SignInWait, 30, None);
        assert_eq!(hit.suggested, Some(40));
        let top = chosen.waited(LimitId::SignInWait, 40, None);
        assert_eq!(top.suggested, None);
        assert!(
            top.said().contains("team that defines this pool"),
            "{}",
            top.said()
        );
        assert!(
            top.said().contains("its Pool in the cluster"),
            "{}",
            top.said()
        );

        let over = chosen.hit(LimitId::ProfileSize, 128, 170, None);
        assert_eq!(over.suggested, None);
        assert!(
            over.said().contains("team that defines this pool"),
            "{}",
            over.said()
        );

        assert!(over.said().contains("at most 160 MiB"), "{}", over.said());

        // Only a highest set: the default holds to it.
        let low = Chosen::default().under_team(Some(&json!({"profileSize": {"highest": 64}})));
        assert_eq!(low.default_of(LimitId::ProfileSize), 64);
        assert!(low.default_by_team(LimitId::ProfileSize));

        // A highest below what the limit can be holds at the least; the vouch wait is no Pool's.
        let odd = Chosen::default().under_team(Some(&json!({
            "answerWait": {"highest": 3},
            "vouchWait": {"default": 60, "highest": 60},
        })));
        assert_eq!(odd.team_highest(LimitId::AnswerWait), Some(5));
        assert_eq!(odd.in_force(LimitId::VouchWait), 5);
        assert_eq!(odd.team_highest(LimitId::VouchWait), None);

        // An older runner's lower top is no team's.
        let older = chosen.waited(LimitId::SignInWait, 10, Some(10));
        assert!(!older.by_team);

        let why = LimitId::SignInWait.limit().check(41, Some(40)).unwrap_err();
        assert!(why.contains("team that defines this pool"), "{why}");
    }

    /// The person's ruling of 2026-11-01: a team's default only lowers unasked. A higher one is an
    /// offer, on the page and on a hit, and holds nothing until the person sets it.
    #[test]
    fn a_teams_higher_default_is_an_offer_and_raises_nothing() {
        let team = json!({"profileSize": {"default": 256, "highest": 512}});
        let chosen = Chosen::default().under_team(Some(&team));
        assert_eq!(
            chosen.in_force(LimitId::ProfileSize),
            128,
            "purlis's default holds"
        );
        assert_eq!(chosen.default_of(LimitId::ProfileSize), 128);
        assert!(!chosen.default_by_team(LimitId::ProfileSize));
        assert_eq!(chosen.team_suggests(LimitId::ProfileSize), Some(256));

        // Offered where it holds what is needed; the usual suggestion where it doesn't.
        let hit = chosen.hit(LimitId::ProfileSize, 128, 150, None);
        assert_eq!(hit.suggested, Some(256));
        assert!(hit.team_suggested);
        assert!(hit.said().contains("raise it to 256 MiB"), "{}", hit.said());
        let more = chosen.hit(LimitId::ProfileSize, 128, 300, None);
        assert_eq!(more.suggested, Some(384));
        assert!(!more.team_suggested);

        // Once the person is there, nothing is offered.
        let set = chosen.with(LimitId::ProfileSize, Some(256));
        assert_eq!(set.in_force(LimitId::ProfileSize), 256);
        assert_eq!(set.team_suggests(LimitId::ProfileSize), None);

        // A lower one holds with no prompt, and offers nothing.
        let lower = Chosen::default().under_team(Some(&json!({"profileSize": {"default": 64}})));
        assert_eq!(lower.in_force(LimitId::ProfileSize), 64);
        assert_eq!(lower.team_suggests(LimitId::ProfileSize), None);
    }

    /// A Pool can't make a limit more than purlis carries, and what it set round-trips a row.
    #[test]
    fn what_a_team_set_is_checked_as_read_and_round_trips() {
        let team = json!({
            "signInWait": {"default": 5000, "highest": 100000},
            "codeSize": {"default": 2048, "highest": 1024},
            "answerWait": {"default": "lots", "highest": 90},
            "nothing": {"default": 1},
        });
        let chosen = Chosen::default().under_team(Some(&team));
        assert_eq!(chosen.in_force(LimitId::SignInWait), 15, "purlis's default");
        assert_eq!(chosen.team_highest(LimitId::SignInWait), None);
        assert_eq!(
            chosen.in_force(LimitId::CodeSize),
            1024,
            "a default over the top"
        );
        assert_eq!(chosen.team_highest(LimitId::AnswerWait), Some(90));
        let mut row = json!({});
        chosen.team_to_row(&mut row);
        assert_eq!(
            Chosen::default().under_team(row.get("teamLimits")),
            chosen,
            "{row}"
        );
        let mut none = json!({});
        Chosen::default().team_to_row(&mut none);
        assert_eq!(none, json!({}), "nothing written for a pool with none");
    }

    /// #4381: a bad value costs that limit alone, and the file's refusal is the command's and the
    /// window's, word for word.
    #[test]
    fn a_bad_value_in_a_row_refuses_that_limit_alone_in_the_checks_own_words() {
        let size = LimitId::ProfileSize.limit();
        for (bad, typed) in [
            (json!(0), "0"),
            (json!(1025), "1025"),
            (json!("128"), "x"),
            (json!(1.5), "1.5"),
            (json!(-1), "-1"),
        ] {
            let row = json!({"profileSize": bad, "profileFiles": 9000});
            let (chosen, refused) = Chosen::read_row(row.as_object().unwrap(), None);
            assert_eq!(chosen.get(LimitId::ProfileSize), None, "{bad}");
            assert_eq!(chosen.in_force(LimitId::ProfileSize), size.default);
            assert_eq!(chosen.get(LimitId::ProfileFiles), Some(9000), "{bad}");
            assert_eq!(refused.len(), 1, "{bad}");
            assert_eq!(refused[0].held, bad);
            assert_eq!(Err(refused[0].why.clone()), size.check_typed(typed, None));
            assert!(refused[0].said().contains("follows the default"));
        }
    }

    #[test]
    fn a_docker_pools_row_refuses_only_the_limit_only_a_cluster_pool_has() {
        let row = json!({"vouchWait": 10, "answerWait": 45});
        let row = row.as_object().unwrap();
        let (chosen, refused) = Chosen::read_row(row, Some("builds"));
        assert_eq!(chosen.get(LimitId::VouchWait), None);
        assert_eq!(chosen.get(LimitId::AnswerWait), Some(45));
        assert_eq!(
            Some(refused[0].why.clone()),
            LimitId::VouchWait.limit().not_on_docker("builds")
        );
        let (on_a_cluster, none) = Chosen::read_row(row, None);
        assert_eq!(on_a_cluster.get(LimitId::VouchWait), Some(10));
        assert!(none.is_empty());
    }
}
