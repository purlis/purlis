// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether a chat may start on a profile, and whether charter's guard runs in it.
//!
//! **There is nothing left to install.** This module used to be a port of `charter/wiring.py`
//! (ADR 0022): it asked `claude plugin list --json` under the profile's environment whether
//! the Python charter's `charter@charter` plugin was installed and enabled for the chat's
//! directory, installed it from its GitHub marketplace when it was not, and refused every
//! chat it could not wire — which, offline, was every Claude chat. Codex needed the same
//! marketplace's plugin, a policy line and a trusted guard hook in `~/.codex/config.toml`.
//!
//! The app now ships its own plugin and arms every chat it starts with it, for that session
//! alone ([`crate::plugin`], [`crate::harness::Harness::state_hooks`]): Claude Code loads the
//! bundled plugin with `--plugin-dir`, Codex is armed with `-c` flags from the same
//! registry, and opencode loads the bundle's opencode shim through `OPENCODE_CONFIG_CONTENT`
//! ([`crate::opencode`]). So the guard runs in a chat because the app started it, whatever the profile's
//! config folder holds, and there is no folder to ask about and nothing to put in one.
//!
//! What is left is the gate a launch still needs — the kind is one this app starts, the file
//! the profile came from is not one git would carry, and the operator approved its command —
//! and, for `charter doctor`, the one fact about a profile that can still be wrong: whether
//! its harness can be found at all.

use std::path::{Path, PathBuf};

use crate::harness_declaration::Declarations;
use crate::profiles::{self, Profile, Source};
use crate::shown;

/// What charter found when it asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// charter's guard runs in a chat started on this profile.
    Wired,
    /// charter could not look, or cannot start this profile. **Not a pass.**
    Unknown,
}

/// What was asked, what it answered, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wiring {
    pub state: State,
    /// What was asked and what it answered — escaped, never clipped.
    pub detail: String,
    /// The command that would fix it; empty when wired.
    pub fix: String,
}

/// A value escaped for a terminal and not clipped.
fn whole(value: &str) -> String {
    shown::readable(value, usize::MAX)
}

/// Why this app will not start `p` at all — `None` when it may.
///
/// **Parsing is not launching.** [`crate::profiles`] reads every kind the Python charter
/// accepted, so both implementations give one answer about one file; what this app starts is
/// asked here. Since #371 that is all three kinds.
///
/// Decided from the DECLARED `kind`, which is a known value
/// ([`crate::harness::Harness::of_kind`]), never from the program name, which is not.
///
/// **And a profile that would start its harness past charter's guard is not started.** An
/// opencode profile that passes `--pure`, or sets the variables charter arms opencode through,
/// would run a chat that looks guarded and loads no charter plugin at all (measured, opencode
/// 1.18.23 — [`crate::opencode::disarmed_by`]).
///
/// **A harness a project declares starts too, at level 1** (ADR 0073, FD-14): its terminal
/// alone, with nothing to arm and so nothing that could start it disarmed.
fn not_startable(p: &Profile, declared: &Declarations) -> Option<String> {
    let Some(harness) = crate::harness::Harness::of_kind(&p.kind) else {
        if project_declaration(p, declared).is_some() {
            return None;
        }
        return Some(format!(
            "profile '{}' runs {}, which this app does not start.",
            shown::short(&p.name),
            shown::short(&p.kind),
        ));
    };
    harness.disarmed_by(&p.command, &p.env).map(|why| {
        format!(
            "profile '{}' would start {} without purlis's guard: {why}. Nothing was started \
             — take it out of the profile in charter.local.toml.",
            shown::short(&p.name),
            harness.name(),
        )
    })
}

/// Why charter may not run `p`'s command — `None` when it may.
///
/// The ignore check first: an approved profile in a file git would commit is still a
/// declaration charter has refused, and a record saying the operator once approved it says
/// nothing about the file it now sits in. Built-ins skip both — their command is charter's
/// own, out of the registry.
fn not_asked(p: &Profile, root: &Path, declared: &Declarations) -> Option<(String, String)> {
    if p.source == Source::BuiltIn {
        return None;
    }
    let name = shown::short(&p.name);
    let check = if p.source == Source::Local {
        profiles::ignore_check(root)
    } else {
        profiles::IgnoreCheck::default()
    };
    if !check.passes() {
        return Some((
            format!(
                "profile '{name}' is declared in a file purlis has refused ({}) — nothing \
                 was started.",
                check.reason
            ),
            check.fix,
        ));
    }
    let fix = format!(
        "start a chat on '{}' from the app's new-chat picker, which shows its command and \
         asks once",
        whole(&p.name)
    );
    // **Whether the approval is asked at all is `[policy] trust`'s** (Q308b): under `off` a
    // command nobody approved runs, and under `log` it runs and one line says it would have
    // been refused. A file purlis refused, above, is refused in every mode.
    let waived = |rule: &str| {
        crate::enforcement::judged(
            root,
            None,
            crate::enforcement::Family::Trust,
            crate::enforcement::mode(root, crate::enforcement::Family::Trust),
            rule,
            None,
            (),
        )
        .is_none()
    };
    if let Some(state) = crate::profiletrust::profile_approval_needed(root, p)
        && !waived(crate::enforcement::RULE_PROFILE_APPROVAL)
    {
        return Some((
            format!(
                "profile '{name}' is {}, and nobody has approved it — nothing was started. \
                 Approve it once so purlis can run it.",
                state.as_str()
            ),
            fix,
        ));
    }
    crate::profiletrust::declaration_approval_needed_in(root, p, declared)
        .filter(|_| !waived(crate::enforcement::RULE_DECLARATION_APPROVAL))
        .map(|(state, d)| {
            (
                format!(
                    "harness '{}', declared in {}, is {} on this machine, and nobody has approved \
                 it — nothing was started. Approve it once so purlis can run it.",
                    shown::short(&d.name),
                    d.file,
                    state.as_str()
                ),
                fix,
            )
        })
}

/// The harness declaration `p` runs on, where its kind is one the project or this machine
/// (#968) adds to the ones purlis ships.
fn project_declaration<'a>(
    p: &Profile,
    declared: &'a Declarations,
) -> Option<&'a crate::harness_declaration::Declaration> {
    declared.added().find(|d| d.name == p.kind)
}

/// Whether charter's guard runs in a chat the app would start on `p`.
///
/// It does wherever the app can start the harness at all, because the app arms the chat
/// itself. So the one question left is whether the harness can be FOUND — the same lookup a
/// launch makes ([`crate::programs::resolve_argv`], charter-app#134) — and a harness that
/// cannot be is an unknown, not a pass, with the directories that were searched.
///
/// Nothing is run: a profile charter may not run a command for is told why before anything
/// else, and a harness that is found is not asked anything.
pub fn detect(p: &Profile, root: &Path) -> Wiring {
    let declared = crate::harness_declaration::read(root);
    if let Some((why, fix)) = not_asked(p, root, &declared) {
        return Wiring {
            state: State::Unknown,
            detail: why,
            fix,
        };
    }
    if let Some(why) = not_startable(p, &declared) {
        return Wiring {
            state: State::Unknown,
            detail: why,
            fix: String::new(),
        };
    }
    let home = profiles::home().unwrap_or_else(|| PathBuf::from("~"));
    let argv = profiles::expanded_command(p, &home);
    match crate::programs::resolve_argv(&argv) {
        // A command the operator wrote as a path is handed to the spawn as written and never
        // searched for, so the one thing to ask of it is whether it is there.
        Ok(found)
            if found
                .first()
                .is_some_and(|program| names_a_place(program) && !Path::new(program).is_file()) =>
        {
            let program = whole(found.first().map(String::as_str).unwrap_or_default());
            Wiring {
                state: State::Unknown,
                detail: format!("{program} is not a program on this machine"),
                fix: format!(
                    "point profile {} at a program that exists, in charter.local.toml",
                    whole(&p.name)
                ),
            }
        }
        Ok(found) => Wiring {
            state: State::Wired,
            // What the app does first and the path after it: a doctor row is clipped, and the
            // sentence is the part a reader acts on.
            detail: format!(
                "{} — {}",
                crate::harness::Harness::of_kind(&p.kind)
                    .map(crate::harness::Harness::armed_with)
                    .unwrap_or_else(|| {
                        format!(
                            "a {} chat runs in its terminal alone, with none of purlis's hooks \
                             armed (it is declared in {})",
                            shown::short(&p.kind),
                            project_declaration(p, &declared).map_or_else(
                                || format!("{}/", crate::harness_declaration::DIR),
                                |d| d.file.clone()
                            )
                        )
                    }),
                whole(found.first().map(String::as_str).unwrap_or_default()),
            ),
            fix: String::new(),
        },
        Err(gone) => Wiring {
            state: State::Unknown,
            detail: gone.said(),
            fix: format!(
                "give profile {} an absolute command in charter.local.toml: \
                 command = [\"/full/path/to/{}\"]",
                whole(&p.name),
                whole(&gone.program)
            ),
        },
    }
}

/// Whether `program` names a place rather than a word to search for. Off unix a bare word is
/// handed back unresolved ([`crate::programs::resolve`]), and it is not a missing file.
///
/// **A question only off unix**, and the mutation run excludes its two mutants on that
/// ground (`.cargo/mutants.toml`). On unix `MAIN_SEPARATOR` is `/`, so `||` and `&&` between
/// the two halves ask one question twice; and every program `resolve_argv` hands back on unix
/// is absolute, so `true` is what this already answers there.
fn names_a_place(program: &str) -> bool {
    program.contains('/') || program.contains(std::path::MAIN_SEPARATOR)
}

/// Why `p` may not start — `None` when it may.
///
/// **The one home for every launch path**: the picker's start, a reopen, a handoff. First the
/// kind, because a profile that can never start is not worth approving and the sentence an
/// operator wants is the one about v1 rather than one about consent; then the gate that says
/// whether charter may run the profile's command at all.
pub fn refusal(p: &Profile, root: &Path) -> Option<String> {
    refusal_in(p, root, &crate::harness_declaration::read(root))
}

/// [`refusal`] against the project's harness declarations as already read: the ones the
/// launch then runs ([`crate::start::ready_in`]), so the bytes approved are the bytes run.
/// **What the person is told when a start runs a command nobody approved** (Q329): where
/// `[policy] trust` does not enforce the approval (only `open`, or the person's own pressed
/// `trust = "off"` or `"log"`), the profile or harness declaration `p` runs on starts
/// unapproved, and its tab says so as it starts ([`crate::start::Ready::notices`]). `None`
/// where nothing unapproved runs, or where the approval was asked.
pub fn unapproved_notice(p: &Profile, root: &Path, declared: &Declarations) -> Option<String> {
    if p.source == Source::BuiltIn
        || crate::enforcement::mode(root, crate::enforcement::Family::Trust)
            == crate::enforcement::Mode::Enforce
    {
        return None;
    }
    // Each that nobody approved is named: the profile, the declaration its kind names, or both
    // (the re-review of #7104).
    let profile = crate::profiletrust::profile_approval_needed(root, p)
        .map(|state| format!("Profile '{}' is {}", shown::short(&p.name), state.as_str()));
    let harness =
        crate::profiletrust::declaration_approval_needed_in(root, p, declared).map(|(state, d)| {
            format!(
                "Harness '{}', declared in {}, is {} on this machine",
                shown::short(&d.name),
                d.file,
                state.as_str()
            )
        });
    let (what, which) = match (profile, harness) {
        (Some(profile), Some(harness)) => (format!("{profile}, {harness}"), "either"),
        (Some(one), None) | (None, Some(one)) => (one, "it"),
        (None, None) => return None,
    };
    Some(format!(
        "{what}, and nobody has approved {which}. It started anyway, because this project's \
         policy does not ask for approval. To be asked again, set trust to enforce in Settings › \
         Project › Policy."
    ))
}

pub fn refusal_in(p: &Profile, root: &Path, declared: &Declarations) -> Option<String> {
    if let Some(why) = not_startable(p, declared) {
        return Some(why);
    }
    not_asked(p, root, declared).map(|(why, _fix)| why)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_nobody_approved_is_refused_as_policy_trust_runs() {
        // Q308b: the approval is `trust`'s. Enforced, nothing starts until the person
        // approves it; logged, it starts and one line says it would have been refused.
        for (policy, refused, lines) in [("", true, 0), ("trust = \"log\"\n", false, 1)] {
            let root = tempfile::tempdir().expect("a plane");
            std::fs::write(
                crate::names::manifest(root.path()),
                format!("[policy]\ntemplate = \"strict\"\n{policy}"),
            )
            .expect("the manifest");
            // A loosening below `watch`, which enforces `trust` (Q329), is the person's press.
            let _home = crate::sandbox::local::OwnConfigHome::new();
            pressed(root.path());
            let mut p = built_in("claude", "claude");
            p.source = Source::Local;
            p.name = "mine".to_owned();
            let said = not_asked(&p, root.path(), &Declarations::default());
            assert_eq!(said.is_some(), refused, "{policy:?}");
            if refused {
                assert!(said.unwrap().0.contains("nobody has approved it"));
            }
            let logged =
                crate::enforcement::read_log(root.path(), crate::enforcement::UNKNOWN_CHAT);
            assert_eq!(logged.len(), lines, "{policy:?}");
            assert!(
                logged
                    .iter()
                    .all(|line| line.family == "trust" && line.rule == "profile.approval")
            );
        }
    }

    #[test]
    fn an_unapproved_declared_harness_is_refused_under_watch_and_told_where_trust_is_lifted() {
        // Q329: `trust` is enforced under every template but `open`. Under `watch` (no
        // template named) a declared harness nobody approved is refused, and the picker's
        // offer asks for the approval; under `open` it starts, and its tab says so.
        crate::unsteered!();
        for (policy, refused) in [
            ("", true),
            ("[policy]\ntemplate = \"watch\"\n", true),
            ("[policy]\ntemplate = \"hold\"\n", true),
            ("[policy]\ntemplate = \"open\"\n", false),
            ("[policy]\ntemplate = \"strict\"\ntrust = \"off\"\n", false),
        ] {
            let root = tempfile::tempdir().expect("a plane");
            std::fs::write(crate::names::manifest(root.path()), policy).expect("the manifest");
            // A loosening below `watch` is the person's press (Q308a).
            let _home = crate::sandbox::local::OwnConfigHome::new();
            pressed(root.path());
            let dir = root.path().join(crate::harness_declaration::DIR);
            std::fs::create_dir_all(&dir).expect("the declarations' folder");
            std::fs::write(
                dir.join("shelly.toml"),
                "name = \"shelly\"\ntitle = \"Shelly\"\nprogram = \"true\"\n",
            )
            .expect("a declaration");
            let declared = crate::harness_declaration::read(root.path());
            let set = profiles::current(root.path());
            let shelly = set
                .get("shelly")
                .unwrap_or_else(|| panic!("{:?}", set.refused));
            let why = refusal_in(shelly, root.path(), &declared);
            assert_eq!(why.is_some(), refused, "{policy:?}: {why:?}");
            if let Some(why) = why {
                assert!(why.contains("nobody has approved it"), "{why}");
            }
            let offers = crate::personaprofile::offers_of(root.path(), &set, &declared);
            let offer = offers
                .iter()
                .find(|offer| offer.name == "shelly")
                .expect("offered");
            assert_eq!(offer.needs_approval, refused, "{policy:?}");
            let told = unapproved_notice(shelly, root.path(), &declared);
            assert_eq!(told.is_some(), !refused, "{policy:?}: {told:?}");
            if let Some(told) = told {
                assert!(
                    told.contains("harnesses/shelly.toml") && told.contains("started anyway"),
                    "{told}"
                );
            }
        }
    }

    #[test]
    fn a_start_on_an_unapproved_profile_of_an_unapproved_harness_names_both() {
        // The re-review of #7104: where neither the profile nor the declaration its kind names
        // was approved, the tab names each, not the profile alone.
        crate::unsteered!();
        let root = tempfile::tempdir().expect("a plane");
        std::fs::write(
            crate::names::manifest(root.path()),
            "[policy]\ntemplate = \"open\"\n",
        )
        .expect("the manifest");
        let _home = crate::sandbox::local::OwnConfigHome::new();
        pressed(root.path());
        let dir = root.path().join(crate::harness_declaration::DIR);
        std::fs::create_dir_all(&dir).expect("the declarations' folder");
        std::fs::write(
            dir.join("shelly.toml"),
            "name = \"shelly\"\ntitle = \"Shelly\"\nprogram = \"true\"\n",
        )
        .expect("a declaration");
        std::fs::write(
            root.path().join(profiles::LOCAL_FILE),
            "[harness.shelly-work]\nkind = \"shelly\"\ncommand = [\"true\", \"-e\"]\n",
        )
        .expect("a local profile");
        let declared = crate::harness_declaration::read(root.path());
        let set = profiles::current(root.path());
        let work = set
            .get("shelly-work")
            .unwrap_or_else(|| panic!("{:?}", set.refused));
        let told = unapproved_notice(work, root.path(), &declared).expect("told");
        assert!(
            told.contains("Profile 'shelly-work'") && told.contains("harnesses/shelly.toml"),
            "{told}"
        );
        assert!(told.contains("nobody has approved either"), "{told}");
    }

    /// The person's press in the window, on what the project at `root` says now.
    fn pressed(root: &Path) {
        crate::enforcement::press(root, &crate::enforcement::digest_of(root)).expect("pressed");
    }

    fn built_in(kind: &str, command: &str) -> Profile {
        let mut p = profiles::builtins()
            .into_iter()
            .find(|p| p.kind == "claude")
            .expect("a built-in claude profile");
        p.kind = kind.to_owned();
        p.command = vec![command.to_owned()];
        p
    }

    #[test]
    fn a_built_in_claude_profile_may_start_with_nothing_installed() {
        // The whole point: no marketplace, no install record, no config folder asked.
        let root = tempfile::tempdir().expect("a plane");
        assert_eq!(refusal(&built_in("claude", "claude"), root.path()), None);
    }

    #[test]
    fn an_opencode_profile_may_start_with_nothing_installed() {
        // #371: the app arms an opencode chat with its own shim, as it does the other two.
        let root = tempfile::tempdir().expect("a plane");
        assert_eq!(
            refusal(&built_in("opencode", "opencode"), root.path()),
            None
        );
    }

    #[test]
    fn an_opencode_profile_that_would_load_no_plugin_is_not_started() {
        // Measured on 1.18.23: `--pure` and `OPENCODE_PURE=1` load no external plugin, and the
        // vault's content then reached the model.
        let root = tempfile::tempdir().expect("a plane");
        let mut pure = built_in("opencode", "opencode");
        pure.command.push("--pure".to_owned());
        let why = refusal(&pure, root.path()).expect("refused");
        assert!(
            why.contains("without purlis's guard") && why.contains("--pure"),
            "{why}"
        );

        let mut env = built_in("opencode", "opencode");
        env.env = vec![("OPENCODE_CONFIG_CONTENT".to_owned(), "{}".to_owned())];
        assert!(refusal(&env, root.path()).is_some());

        // The same flag on another harness is that harness's business.
        let mut claude = built_in("claude", "claude");
        claude.command.push("--pure".to_owned());
        assert_eq!(refusal(&claude, root.path()), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_harness_that_is_found_is_wired_and_says_the_app_arms_it() {
        let root = tempfile::tempdir().expect("a plane");
        let w = detect(&built_in("claude", "/bin/sh"), root.path());
        assert_eq!(w.state, State::Wired, "{w:?}");
        assert!(
            w.detail
                .starts_with("the app arms each chat with its own plugin, purlis@inline — "),
            "{}",
            w.detail
        );
        assert!(!w.detail.contains("charter-app"), "{}", w.detail);
        assert!(!w.detail.contains("charter@charter"), "{}", w.detail);
    }

    #[test]
    fn a_harness_that_cannot_be_found_is_an_unknown_and_never_a_pass() {
        let root = tempfile::tempdir().expect("a plane");
        let w = detect(
            &built_in("claude", "charter-no-such-harness-anywhere"),
            root.path(),
        );
        assert_eq!(w.state, State::Unknown, "{w:?}");
        assert!(w.fix.contains("absolute command"), "{}", w.fix);
    }
}
