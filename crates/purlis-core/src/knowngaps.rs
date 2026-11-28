// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Known gaps, declared per harness and OS** (M75 slice 37, #7046; spec §6, §14.8; Q319 C1).
//!
//! Two tables and one rule.
//!
//! - **The capability table** ([`cover`]): for each action of the policy's schema, on each harness
//!   and each OS, the enforcement point that fails closed without the hook ([`Point`]), or the
//!   gap that says there is none ([`Cover::Gap`]). A hook alone is a gap: a harness may treat a
//!   hook that dies or times out as non-blocking, so "unreachable means refused" holds only where
//!   something else also refuses (spec §6, review round 1, S-B4). The first draft is slice 03's
//!   spike (`work/who-judges/tickets/m75-policy/spike-03.md`); `every_action_has_a_fail_closed_point_or_a_gap`
//!   holds it to the schema, so a new action with no row fails the build.
//! - **The gap entries** ([`GAPS`]): each of #6972 §4's 24 gaps, Claude Code's reads (spec
//!   §11.5), Claude Code's minimum version (§11.7), the plane root's rename where the sandbox
//!   cannot hold it (§8.10), `context.machine` unset on Windows (§9.4), and one entry for each
//!   kind of action with no fail-closed point.
//! - **The rule** ([`Declared::at`]): a family whose mode is not `off` and that a gap affects is a
//!   known gap of the chat's start. The chat still starts. The families that are `enforce` and
//!   have a gap are `context.gaps` on `start` ([`Declared::context_gaps`]), so a project that wants
//!   a refusal writes a forbid on `start`; the shipped default has none ([`refusal`]).
//!
//! Nothing here reads a chat's word: a start's facts are the host's own ([`Facts`]).

use crate::enforcement::{Family, Mode, Resolved};
use crate::harness::Harness;
use crate::sandbox::Os;

/// The point that fails closed without the hook (spec §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Point {
    /// A host-brokered decision or credential: the app decides, or alone holds what the action
    /// needs.
    Host,
    /// The sandbox: Seatbelt on macOS, purlis's wrap on Linux.
    Sandbox,
    /// The egress proxy, which only a sandboxed chat can reach out through.
    Proxy,
    /// The harness itself refuses where its hook fails: OpenCode's shim, or a Claude Code whose
    /// hook is set to block on failure.
    Harness,
}

impl Point {
    /// The word Settings, doctor and the capability table's docs spell it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Sandbox => "sandbox",
            Self::Proxy => "proxy",
            Self::Harness => "harness",
        }
    }
}

/// What holds an action when its hook does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    /// This point refuses.
    Closed(Point),
    /// Nothing does: the gap entry with this id says so.
    Gap(&'static str),
}

/// The gap that stands where a hook alone enforces an action and its harness lets a failed hook
/// run the tool.
pub const HOOK_FAILS_OPEN: &str = "hook-fails-open";
/// The gap that stands on a system with no sandbox backend (Windows).
pub const NO_SANDBOX_BACKEND: &str = "no-sandbox-backend";
/// The gap that stands for a chat that starts with its sandbox lifted (the person's opt-out, or a
/// floor that could not be applied): what the sandbox, the proxy or an OpenCode shim's own files
/// held is not held.
pub const SANDBOX_LIFTED: &str = "sandbox-lifted";
/// The gap that stands for a declared harness with no adapter (spec §6, ADR 0073): no hook of its
/// own is armed and no sandbox is compiled for it, so every family is a gap there.
pub const UNADAPTED_HARNESS: &str = "harness-unadapted";
/// The gaps whose families [`cover`] derives, besides the ones their entry lists.
const COVER_GAPS: [&str; 5] = [
    HOOK_FAILS_OPEN,
    NO_SANDBOX_BACKEND,
    POLICY_POINT_NOT_WIRED,
    SANDBOX_LIFTED,
    UNADAPTED_HARNESS,
];
/// The gap that stands where a rule on `read`, `write`, `connect` or `dial` has no point yet:
/// #7222's asks-only floor must close before a later slice wires them.
pub const POLICY_POINT_NOT_WIRED: &str = "policy-point-not-wired";

/// **The enforcement point of `action` for a start with `facts`**, or the gap that stands in its
/// place; `None` for a word the schema has no action of (the test that holds this table to the
/// schema names it). What the sandbox holds is held only where the start is sandboxed.
pub fn cover(action: &str, facts: &Facts) -> Option<Cover> {
    use Cover::{Closed, Gap};
    use Point::{Harness as ByTheHarness, Host, Sandbox};
    let hook_only = match facts.harness {
        Some(Harness::Opencode) => Closed(ByTheHarness),
        Some(Harness::ClaudeCode | Harness::Codex) => Gap(HOOK_FAILS_OPEN),
        None => Gap(UNADAPTED_HARNESS),
    };
    let cover = match action {
        // #7222 (an asks-only floor) closes before these have a point of the policy's own.
        "read" | "write" | "connect" | "dial" => Gap(POLICY_POINT_NOT_WIRED),
        "reach" | "listen" | "exec" | "install" | "rewrite" | "create" | "remove" => {
            Closed(Sandbox)
        }
        // A hook alone: git's own hooks run locally and `--no-verify` skips them; the scan of a
        // credential and the shape of a call are the tool gate's.
        "commit" | "update" | "cut" | "discard" | "sign" | "expose" | "interpolate" | "chain" => {
            hook_only
        }
        "push" | "clone" | "merge" | "land" | "release" | "tag" | "save" | "authenticate"
        | "mint" | "record" | "propose" | "dispatch-to" | "use" | "request" | "store"
        | "reveal" | "inherit" | "browse" | "close" | "fill" | "log-in" | "upload" | "download"
        | "load" | "enter" | "run" | "mount" | "start" | "offload" | "forward" | "report"
        | "promote" => Closed(Host),
        // Every forge operation is the forge route's, which alone holds the person's sign-in
        // (ADR 0128, D3).
        operation if crate::policy::family_of(operation) == Some("guards.forge") => Closed(Host),
        _ => return None,
    };
    // Where purlis has no sandbox backend, what only the sandbox held is not held; nor where the
    // chat starts with its sandbox lifted, which also leaves an OpenCode shim's files unheld.
    Some(match cover {
        Closed(Sandbox | Point::Proxy | ByTheHarness) if facts.os.backend_word().is_none() => {
            Gap(NO_SANDBOX_BACKEND)
        }
        Closed(Sandbox | Point::Proxy | ByTheHarness) if !facts.sandboxed => Gap(SANDBOX_LIFTED),
        other => other,
    })
}

/// What the host knows of a start, which decides what applies to it (never a word of the chat's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// The harness the chat runs; `None` for a declared harness with no adapter (ADR 0073).
    pub harness: Option<Harness>,
    /// The system it runs on.
    pub os: Os,
    /// Whether it starts inside a sandbox.
    pub sandboxed: bool,
    /// Whether it is a Claude Code older than the version that takes purlis's proxy ports, or
    /// one that answered no version (spec §11.7).
    pub claude_code_older: bool,
    /// Whether the sandbox holds the plane root's own folder entry against rename (spec §8.10).
    /// Slice 11 compiles that hold; until it does, nothing does.
    pub plane_root_rename_held: bool,
}

impl Facts {
    /// The facts of a start of `harness` on `os`, as this build can hold them.
    pub fn of(harness: Option<Harness>, os: Os, sandbox: Option<&crate::sandbox::Applied>) -> Self {
        Self {
            harness,
            os,
            sandboxed: sandbox.is_some(),
            claude_code_older: harness == Some(Harness::ClaudeCode)
                && sandbox.is_some_and(|applied| !applied.through_purlis_proxy()),
            plane_root_rename_held: false,
        }
    }

    /// The facts of a start of `harness` on `os` that holds all it can: what Settings and doctor
    /// declare before any chat starts.
    pub fn best(harness: Harness, os: Os) -> Self {
        Self {
            harness: Some(harness),
            os,
            sandboxed: true,
            claude_code_older: false,
            plane_root_rename_held: false,
        }
    }
}

/// One known gap: a place a policy key cannot be enforced the way it is elsewhere.
#[derive(Debug, Clone, Copy)]
pub struct Gap {
    /// Its stable id, which a start's event, Settings and doctor name it by.
    pub id: &'static str,
    /// The number of #6972 §4's table, for the 24 the harness research listed.
    pub research: Option<u8>,
    /// The harnesses it holds on; every harness where empty.
    pub harnesses: &'static [Harness],
    /// The systems it holds on; every system where empty.
    pub systems: &'static [Os],
    /// The families whose rules it leaves unenforced here. None for a gap no family's mode
    /// switches off (`context.machine` unset).
    pub families: &'static [Family],
    /// What it is, as a sentence.
    pub said: &'static str,
    /// What purlis does there, as a sentence.
    pub instead: &'static str,
    /// Where it comes from.
    pub from: &'static str,
    /// What else must hold of the start for it to apply, where something does.
    when: Option<fn(&Facts) -> bool>,
}

impl Gap {
    /// Whether the gap holds for a start with `facts`.
    pub fn holds_for(&self, facts: &Facts) -> bool {
        (self.harnesses.is_empty()
            || facts
                .harness
                .is_some_and(|harness| self.harnesses.contains(&harness)))
            && (self.systems.is_empty() || self.systems.contains(&facts.os))
            && self.when.is_none_or(|holds| holds(facts))
    }

    /// Whether the gap holds only for a start with something more than a harness and a system:
    /// a Claude Code that is older, a chat whose sandbox is lifted.
    pub fn conditional(&self) -> bool {
        self.when.is_some()
    }

    /// Whether the gap holds for `harness` on `os`, whatever else a start's facts say.
    pub fn may_hold_for(&self, harness: Harness, os: Os) -> bool {
        // A harness purlis has an adapter for is never an unadapted one.
        self.id != UNADAPTED_HARNESS
            && (self.harnesses.is_empty() || self.harnesses.contains(&harness))
            && (self.systems.is_empty() || self.systems.contains(&os))
    }
}

fn when_lifted(facts: &Facts) -> bool {
    !facts.sandboxed
}

fn when_older(facts: &Facts) -> bool {
    facts.claude_code_older
}

fn when_unadapted(facts: &Facts) -> bool {
    facts.harness.is_none()
}

fn when_rename_unheld(facts: &Facts) -> bool {
    !facts.plane_root_rename_held
}

const CC: &[Harness] = &[Harness::ClaudeCode];
const CODEX: &[Harness] = &[Harness::Codex];
const OPENCODE: &[Harness] = &[Harness::Opencode];
const CC_CODEX: &[Harness] = &[Harness::ClaudeCode, Harness::Codex];
const CODEX_OPENCODE: &[Harness] = &[Harness::Codex, Harness::Opencode];
const EVERY_HARNESS: &[Harness] = &[];
const EVERY_SYSTEM: &[Os] = &[];

macro_rules! gap {
    ($id:expr, $research:expr, $harnesses:expr, $systems:expr, $families:expr,
     $said:literal, $instead:literal, $from:literal) => {
        gap!(
            $id, $research, $harnesses, $systems, $families, $said, $instead, $from, None
        )
    };
    ($id:expr, $research:expr, $harnesses:expr, $systems:expr, $families:expr,
     $said:literal, $instead:literal, $from:literal, $when:expr) => {
        Gap {
            id: $id,
            research: $research,
            harnesses: $harnesses,
            systems: $systems,
            families: $families,
            said: $said,
            instead: $instead,
            from: $from,
            when: $when,
        }
    };
}

/// **Every known gap** (#7046). The first 24 are #6972 §4's, in its order.
pub const GAPS: &[Gap] = &[
    gap!(
        "cross-workspace-reads",
        Some(1),
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::Sandbox],
        "No harness has an OS limit on one workspace reading another's files.",
        "Only purlis's own MCP tools are confined; a forbid on read closes it the same way in all three once it is wired.",
        "#6972 §4 gap 1"
    ),
    gap!(
        "claude-code-file-tools",
        Some(2),
        CC,
        EVERY_SYSTEM,
        &[Family::Sandbox],
        "Claude Code holds its own file tools with an in-process check, not the OS.",
        "permissions.deny and the Read guard.",
        "#6972 §4 gap 2"
    ),
    gap!(
        "claude-code-keychain",
        Some(3),
        CC,
        &[Os::MacOs],
        &[Family::Vaults],
        "Claude Code's settings cannot deny the macOS keychain service.",
        "It rests on the keyring item's own access rule.",
        "#6972 §4 gap 3"
    ),
    gap!(
        "harness-sign-in",
        Some(4),
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::Vaults],
        "A harness's own sign-in is not kept from the commands it runs.",
        "Nothing.",
        "#6972 §4 gap 4"
    ),
    gap!(
        "claude-code-hooks-and-mcp-outside",
        Some(5),
        CC,
        EVERY_SYSTEM,
        &[Family::Network],
        "Claude Code's hooks and non-purlis MCP servers run outside the sandbox and the proxy.",
        "Nothing yet (ADR 0124, phase 3).",
        "#6972 §4 gap 5"
    ),
    gap!(
        "claude-code-linux-listen",
        Some(6),
        CC,
        &[Os::Linux],
        &[Family::Ports],
        "Claude Code on Linux has no door to listen on, and ignores allowUnixSockets.",
        "Nothing. Whether a Listen grant works there is not checked.",
        "#6972 §4 gap 6"
    ),
    gap!(
        "claude-code-managed-local-binding",
        Some(7),
        CC,
        &[Os::MacOs],
        &[Family::Ports],
        "A managed allowLocalBinding setting outranks purlis's own.",
        "Nothing.",
        "#6972 §4 gap 7"
    ),
    gap!(
        "background-work-count",
        Some(8),
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::Processes],
        "No harness limits the count of background work and sub-agents but with its own caps.",
        "Claude Code's count is observed; the others have none.",
        "#6972 §4 gap 8"
    ),
    gap!(
        "codex-read-edit-guards",
        Some(9),
        CODEX,
        EVERY_SYSTEM,
        &[Family::Vaults, Family::GuardsPlane],
        "Codex has no Read, Grep or Edit hook, so the vault and state-folder guards are missing.",
        "The sandbox's denial classes alone.",
        "#6972 §4 gap 9"
    ),
    gap!(
        "codex-write-stdin",
        Some(10),
        CODEX,
        EVERY_SYSTEM,
        &[Family::GuardsShape],
        "Codex does not hook write_stdin, the typing into a running command.",
        "Nothing.",
        "#6972 §4 gap 10"
    ),
    gap!(
        "codex-hooks-untrusted",
        Some(11),
        CODEX,
        EVERY_SYSTEM,
        &[Family::GuardsShape],
        "Codex's hooks are inert until its TUI trusts them, and says nothing when they are not.",
        "Nothing.",
        "#6972 §4 gap 11"
    ),
    gap!(
        "opencode-plugin-realm",
        Some(12),
        OPENCODE,
        EVERY_SYSTEM,
        &[Family::GuardsShape],
        "Another OpenCode plugin shares the shim's JS realm and can get round it.",
        "Nothing (#1069 moves the hooks out of the wrap).",
        "#6972 §4 gap 12"
    ),
    gap!(
        "hooks-inside-the-wrap-unverified",
        Some(13),
        CODEX_OPENCODE,
        EVERY_SYSTEM,
        &[Family::GuardsShape],
        "Where Codex's and OpenCode's hooks run inside the wrap is rated unverified.",
        "ADR 0124's matrix says so.",
        "#6972 §4 gap 13"
    ),
    gap!(
        HOOK_FAILS_OPEN,
        Some(14),
        CC_CODEX,
        EVERY_SYSTEM,
        &[
            Family::GuardsGit,
            Family::GuardsCredential,
            Family::GuardsShape
        ],
        "A hook that times out or dies lets the tool run, and a hook alone enforces these guards.",
        "purlis bounds its own guard to 2 s. Claude Code's onFailure \"block\" is not set yet; Codex has no such option.",
        "#6972 §4 gap 14; spike 03"
    ),
    gap!(
        "codex-no-command-ask",
        Some(15),
        CODEX,
        EVERY_SYSTEM,
        &[Family::Asks],
        "Codex has no permissions file for a command-pattern ask.",
        "The guard applies.",
        "#6972 §4 gap 15"
    ),
    gap!(
        "codex-sandboxed-asks-rejected",
        Some(16),
        CODEX,
        EVERY_SYSTEM,
        &[Family::Asks],
        "A sandboxed Codex chat rejects where its harness would ask.",
        "Granular-reject.",
        "#6972 §4 gap 16"
    ),
    gap!(
        "opencode-ask-operator",
        Some(17),
        OPENCODE,
        EVERY_SYSTEM,
        &[Family::Asks],
        "OpenCode has no MCP form for ask_operator.",
        "Nothing.",
        "#6972 §4 gap 17"
    ),
    gap!(
        "sub-agent-dispatch",
        Some(18),
        CODEX_OPENCODE,
        EVERY_SYSTEM,
        &[Family::Dispatch],
        "A sub-agent dispatch is refused for Codex's Bash only and for OpenCode's not at all.",
        "A partial refusal.",
        "#6972 §4 gap 18"
    ),
    gap!(
        "attendance-and-mode-read-at-start",
        Some(19),
        CODEX_OPENCODE,
        EVERY_SYSTEM,
        &[Family::HarnessMode, Family::Dispatch],
        "Attendance and the permission mode are read only from how purlis started the chat.",
        "Nothing.",
        "#6972 §4 gap 19"
    ),
    gap!(
        "skill-and-config-edits",
        Some(20),
        CODEX_OPENCODE,
        EVERY_SYSTEM,
        &[Family::Skills],
        "Codex and OpenCode have no proposal or draft for a config or skill edit.",
        "The sandbox's refusal alone.",
        "#6972 §4 gap 20"
    ),
    gap!(
        "codex-skills-need-a-restart",
        Some(21),
        CODEX,
        EVERY_SYSTEM,
        &[Family::Skills],
        "A live change to a skill reaches Codex only after a restart.",
        "Nothing.",
        "#6972 §4 gap 21"
    ),
    gap!(
        "permission-mode-semantics",
        Some(22),
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::HarnessMode],
        "The harnesses' permission modes have no common meaning.",
        "Each harness's own settings.",
        "#6972 §4 gap 22"
    ),
    gap!(
        "policy-self-protection-lifted",
        Some(23),
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::GuardsPlane],
        "A chat with its sandbox lifted can rewrite the policy that governs it: there are no OS denials.",
        "Nothing.",
        "#6972 §4 gap 23",
        Some(when_lifted)
    ),
    gap!(
        "claude-code-mods",
        Some(24),
        CC,
        EVERY_SYSTEM,
        &[Family::GuardsPlane],
        "A Claude Code mod can override deny rules and hook blocks unless managed settings are present.",
        "Not checked.",
        "#6972 §4 gap 24"
    ),
    gap!(
        "claude-code-reads",
        None,
        CC,
        EVERY_SYSTEM,
        &[Family::Sandbox],
        "Claude Code's own sandbox has no read list, only deny paths: its reads are everything but the forbids.",
        "Seatbelt and Landlock compile a read list; Claude Code's does not.",
        "spec §11.5"
    ),
    gap!(
        "claude-code-minimum-version",
        None,
        CC,
        EVERY_SYSTEM,
        &[Family::Network],
        "A Claude Code older than 2.1.285 does not take purlis's proxy: it keeps its own and the allowed domains.",
        "Its connections are not judged by the policy. Update Claude Code to 2.1.285 or later.",
        "spec §11.7",
        Some(when_older)
    ),
    gap!(
        "plane-root-rename",
        None,
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::GuardsPlane],
        "The sandbox does not hold the plane root's own folder entry against a rename.",
        "A renamed plane reads as the deny state, never as a fresh install.",
        "spec §8.10",
        Some(when_rename_unheld)
    ),
    gap!(
        "machine-unset",
        None,
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[],
        "context.machine is unset on every system: it is sealed until the person confirms it (slice 07b), so a rule that reads it is denied.",
        "It holds until the device id is minted.",
        "spec §9.4"
    ),
    gap!(
        NO_SANDBOX_BACKEND,
        None,
        EVERY_HARNESS,
        &[Os::Windows, Os::Other],
        &[
            Family::Sandbox,
            Family::Network,
            Family::Ports,
            Family::Processes,
            Family::GuardsPlane,
            Family::Skills
        ],
        "purlis has no sandbox backend on Windows or on a system it does not build one for, so nothing the sandbox or the proxy would hold is held.",
        "The seal stops hand edits only.",
        "spec §6; spike 03"
    ),
    gap!(
        SANDBOX_LIFTED,
        None,
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[
            Family::Sandbox,
            Family::Network,
            Family::Ports,
            Family::Processes
        ],
        "This chat starts with its sandbox lifted, so nothing the sandbox, the proxy or an OpenCode shim's protected files would hold is held.",
        "Only what a hook alone enforces, and the seal on hand edits.",
        "spec §6; ADR 0067",
        Some(when_lifted)
    ),
    gap!(
        UNADAPTED_HARNESS,
        None,
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &Family::ALL,
        "A declared harness has no adapter: no hook is armed and no sandbox is compiled for it, so no family's rule is enforced there.",
        "Nothing, beyond what the host decides itself.",
        "spec §6; ADR 0073",
        Some(when_unadapted)
    ),
    gap!(
        POLICY_POINT_NOT_WIRED,
        None,
        EVERY_HARNESS,
        EVERY_SYSTEM,
        &[Family::Sandbox, Family::Network],
        "A rule on read, write, connect or dial is not yet compiled into the sandbox or asked of the proxy.",
        "The sandbox holds today's floor, and the proxy today's hosts, not the rule (#7222).",
        "#7222"
    ),
];

/// **The families `gap` leaves unenforced**: the ones its entry lists, and, for a gap the
/// capability table ([`cover`]) stands in for, every family of an action it answers with that
/// gap, so an entry cannot say less than the table does.
fn affected(gap: &Gap, facts: &Facts) -> Vec<Family> {
    let mut families = gap.families.to_vec();
    if COVER_GAPS.contains(&gap.id) {
        // A lifted sandbox is the gap's own fact: asked as lifted wherever it is listed.
        let asked = Facts {
            sandboxed: facts.sandboxed && gap.id != SANDBOX_LIFTED,
            ..*facts
        };
        for (action, family) in crate::policy::actions() {
            if let Some(family) = Family::of_key(family)
                && cover(action, &asked) == Some(Cover::Gap(gap.id))
                && !families.contains(&family)
            {
                families.push(family);
            }
        }
        families.sort();
    }
    families
}

/// One declared gap of a start: a gap, and the families it leaves unenforced.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The gap.
    pub gap: &'static Gap,
    /// The families it affects whose mode is not `off`.
    pub in_force: Vec<Family>,
    /// Of those, the ones whose mode is `enforce`: what `context.gaps` carries.
    pub enforced: Vec<Family>,
    /// Every family it affects, whatever its mode: what `context.gaps` would carry were every
    /// family enforced.
    pub affected: Vec<Family>,
}

/// **The known gaps of one start**: every gap that holds there and affects a family whose mode is
/// not `off`, and every gap no family's mode switches off. A rule that is `off` needs no
/// enforcement, so it declares none (Q307 Q5 as amended by Q319 C1).
#[derive(Debug, Clone, Default)]
pub struct Declared(Vec<Entry>);

impl Declared {
    /// The gaps of a start with `facts`, under the modes `floor` resolves.
    pub fn at(facts: &Facts, floor: &Resolved) -> Self {
        Self::of(GAPS.iter().filter(|gap| gap.holds_for(facts)), floor, facts)
    }

    /// The gaps that may hold for `harness` on `os`, under the modes `floor` resolves: what
    /// Settings and doctor declare, before any chat starts. A gap that holds only for some starts
    /// is still listed.
    pub fn on(harness: Harness, os: Os, floor: &Resolved) -> Self {
        Self::of(
            GAPS.iter().filter(|gap| gap.may_hold_for(harness, os)),
            floor,
            &Facts::best(harness, os),
        )
    }

    fn of(gaps: impl Iterator<Item = &'static Gap>, floor: &Resolved, facts: &Facts) -> Self {
        let entries = gaps
            .filter_map(|gap| {
                let in_force: Vec<Family> = affected(gap, facts)
                    .into_iter()
                    .filter(|family| floor.mode(*family) != Mode::Off)
                    .collect();
                // Every family it affects is off: nothing asks for what it lacks.
                if !affected(gap, facts).is_empty() && in_force.is_empty() {
                    return None;
                }
                let enforced = in_force
                    .iter()
                    .copied()
                    .filter(|family| floor.mode(*family) == Mode::Enforce)
                    .collect();
                Some(Entry {
                    gap,
                    in_force,
                    enforced,
                    affected: affected(gap, facts),
                })
            })
            .collect();
        Self(entries)
    }

    /// The declared gaps.
    pub fn entries(&self) -> &[Entry] {
        &self.0
    }

    /// Whether none is declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The declared gaps' ids, in the table's order.
    pub fn ids(&self) -> Vec<&'static str> {
        self.0.iter().map(|entry| entry.gap.id).collect()
    }

    /// **`context.gaps` on `start`** (spec §3.2): the keys of the families that are enforced and
    /// have a gap here, sorted. A family set to `log` or `off` is not in it.
    pub fn context_gaps(&self) -> Vec<String> {
        let mut families: Vec<String> = self
            .0
            .iter()
            .flat_map(|entry| entry.enforced.iter().map(|family| family.key().to_owned()))
            .collect();
        families.sort();
        families.dedup();
        families
    }

    /// **`context.gaps` as if every family were enforced**: the keys of every family a gap affects
    /// here, sorted. What a start is judged by where the sealed answers are in a deny state with
    /// nothing verified, since no accepted copy can say a family set to `log` or `off` is meant to be.
    pub fn context_gaps_if_all_enforced(&self) -> Vec<String> {
        let mut families: Vec<String> = self
            .0
            .iter()
            .flat_map(|entry| entry.affected.iter().map(|family| family.key().to_owned()))
            .collect();
        families.sort();
        families.dedup();
        families
    }
}

/// What a start is judged by, besides its gaps.
#[derive(Debug, Clone, Copy)]
pub struct Judged<'a> {
    /// The plane root.
    pub root: &'a std::path::Path,
    /// The profile the chat runs on.
    pub profile: &'a str,
    /// Its persona and workspace, where it has them.
    pub of: crate::policy::levels::Chat<'a>,
    /// The sandbox level's word: `off`, `floor` or `full`.
    pub sandbox: &'a str,
    /// Whether the person is at the chat being started: the attendance fact its start sets
    /// ([`crate::attendance::Fact::at_start`]).
    pub attended: bool,
    /// The mode the host starts the harness in ([`crate::attendance::Fact::start_mode`]); `None`
    /// where it holds none, which `context.harness_mode` reads as `unknown`.
    pub harness_mode: Option<crate::attendance::ClaudeCodeMode>,
}

/// **What a project's own rule on `start` makes of a start with known gaps.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ruling {
    /// The start is refused: nothing is started. The sentence says why and what to change.
    Refused(String),
    /// The rule would have refused it, and the family `trust` is set to `log`: the chat starts,
    /// and the sentence is the would-have-blocked line (spec §6.4).
    Logged(String),
}

/// **The ruling a project's own rule on `start` makes of this start**, or `None` where nothing
/// would refuse it. The shipped default has no such rule, so with it no known gap ever refuses a
/// start. A rule that errs denies (spec §6.3), and so does a rule that asks: asking at a start is
/// slice 36's, so an `@ask` forbid refuses until it is wired.
///
/// The rule judges the start as the host knows it: `context.gaps` is [`Declared::context_gaps`],
/// and the family `trust`'s mode and the person's floor apply as to any other decision: the
/// rule refuses only where `trust` is `enforce`, and under `log` the start goes ahead with the
/// would-have-blocked line.
pub fn refusal(
    judged: &Judged<'_>,
    facts: &Facts,
    declared: &Declared,
    floor: &Resolved,
) -> Option<Ruling> {
    use crate::policy::decide::{Asker, Outcome, in_plane_held_to};
    // A start in a deny state with nothing verified is judged by no accepted copy, so a gap of any
    // family counts, whatever mode the floor gives it (M75-14, #7305 round 4).
    let sealed = crate::policy::decide::Sealed::for_start(judged.root);
    let gaps = if sealed.standing.is_unverified_by_a_reachable_cause() {
        declared.context_gaps_if_all_enforced()
    } else {
        declared.context_gaps()
    };
    let harness = facts.harness.map_or("unknown", Harness::name);
    let asker = Asker {
        // A start is decided before the board numbers its chat; no rule reads the number.
        chat: 0,
        of: judged.of,
        sandbox: judged.sandbox,
        // The attendance and mode the host's own fact for this start holds
        // ([`crate::attendance::Fact::at_start`]), never a default.
        attended: judged.attended,
        harness_mode: crate::policy::decide::harness_mode_word(judged.harness_mode),
        depth: 0,
        via: "start",
        start: Some(crate::policy::decide::StartFacts {
            profile: judged.profile,
            gaps: &gaps,
        }),
        git: None,
    };
    // A start is judged by the rules whatever the plane's standing: the deny state only adds
    // refusals to the other actions (M75-07b, review round 5, SEC5-SF2).
    let decision = in_plane_held_to(
        judged.root,
        &asker,
        "start",
        ("Harness", harness),
        &sealed,
        Some(floor),
    )
    .held_to(floor);
    let affecting: Vec<String> = declared
        .entries()
        .iter()
        .filter(|entry| !entry.enforced.is_empty())
        .map(|entry| {
            let families = entry
                .enforced
                .iter()
                .map(|family| family.key())
                .collect::<Vec<_>>()
                .join(", ");
            format!("`{}` ({families})", entry.gap.id)
        })
        .collect();
    let because = if affecting.is_empty() {
        String::new()
    } else {
        format!(
            " while a known gap affects a family it enforces: {}",
            affecting.join(", ")
        )
    };
    let rules = decision.rules_said();
    match decision.after_mode {
        Outcome::Deny => Some(Ruling::Refused(format!(
            "Nothing was started. {rules} refuses a start of this chat{because}. Change that rule \
             in policy.cedar, or set the family trust under [policy] to log or off."
        ))),
        Outcome::Ask => Some(Ruling::Refused(format!(
            "Nothing was started. {rules} asks before a start of this chat{because}, and asking at \
             a start is not wired yet, so it is refused. Make that rule a plain forbid in \
             policy.cedar, or set the family trust under [policy] to log or off."
        ))),
        Outcome::Allow if decision.would_have_blocked() && decision.mode == Mode::Log => {
            Some(Ruling::Logged(format!(
                "Would have refused this start: {rules} {} a start of this chat{because}. The \
                 family trust is set to log, so it started. Set trust to enforce to refuse it.",
                if decision.outcome == Outcome::Ask {
                    "asks before"
                } else {
                    "refuses"
                }
            )))
        }
        Outcome::Allow => None,
    }
}

#[cfg(test)]
mod all_enforced_tests {
    use super::*;

    #[test]
    fn a_family_at_log_is_a_gap_only_where_every_family_is_taken_as_enforced() {
        let floor = Resolved::all(Mode::Log);
        let declared = Declared::on(Harness::ClaudeCode, Os::this(), &floor);
        let enforced = declared.context_gaps();
        let all = declared.context_gaps_if_all_enforced();
        assert!(
            all.len() > enforced.len(),
            "log families are left out of context.gaps: {enforced:?} against {all:?}"
        );
        assert!(enforced.iter().all(|family| all.contains(family)));
    }
}
