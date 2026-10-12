// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `plane-root guard` and `guard seen`: whether the Bash guard is declared where a session
//! started here would load it, and when it was last seen running (#994).
//!
//! Two facts, two rows, so neither can hide behind the other: a declaration can be real while
//! nothing dispatches it, and Python's charter met exactly that — a project root switched
//! between branches and committed to while the declaration row read green throughout.
//!
//! **Declared** is asked of the files Claude Code reads for a session rooted in this
//! directory, through the reader `purlis init` uses to decide whether to write the hook
//! ([`crate::scaffold::settings::guard_in`]): this directory's `.claude/settings.json` and
//! `.claude/settings.local.json`, the user settings of the Claude Code config folder in use,
//! and an enabled plugin whose own hooks dispatch it
//! ([`crate::scaffold::settings::plugin_dispatches_guard`]). Project settings do not walk up,
//! so the project's own file says nothing for a session rooted in a workspace.
//!
//! **Seen** is the record the guard itself writes each time it runs
//! ([`crate::guardseen`]): an age and a harness, never a verdict.
//!
//! **The chats the app starts are armed by the app** (`--plugin-dir`), whatever this directory
//! declares. So a guard declared nowhere is a warning about chats started outside the app, and
//! the preflight — which runs inside a chat the app or the plugin already armed — says it as a
//! fact rather than as a warning, as the `plugin install` row does.

use std::path::{Path, PathBuf};

use super::{Doctor, Row, canonical, short_path};
use crate::guardseen::{self, Sighting};
use crate::hookstate::State;
use crate::scaffold::settings::{self, GuardIn};

const GUARD: &str = "plane-root guard";
const SEEN: &str = "guard seen";

/// What a session started here would load of the guard.
struct Declared {
    /// The settings files that declare it, as a reader would name them.
    files: Vec<String>,
    /// The ones that are there and could not be read.
    unread: Vec<String>,
    /// The enabled plugin that dispatches it, by its id.
    plugin: Option<String>,
}

/// The settings files a Claude Code session rooted in `here` reads: its own two, then the user
/// settings of `folder` when a real config folder is in use.
fn settings_files(here: &Path, folder: Option<&Path>) -> Vec<PathBuf> {
    let mut files = vec![
        here.join(".claude/settings.json"),
        here.join(".claude/settings.local.json"),
    ];
    if let Some(folder) = folder {
        files.push(folder.join("settings.json"));
    }
    files
}

/// A settings file as a reader finds it: relative to the session's directory when it is inside
/// it, and by its short path otherwise.
fn shown(file: &Path, here: &Path) -> String {
    match file.strip_prefix(here) {
        Ok(rel) => super::one_line(&rel.display().to_string(), super::PATH_DISPLAY_LIMIT),
        Err(_) => short_path(file),
    }
}

fn declared(d: &Doctor, here: &Path) -> Declared {
    // The user settings only for a doctor of this machine: a doctor a test names reads none of
    // the operator's own (`Doctor::home`).
    let folder = d
        .home
        .as_ref()
        .map(|_| super::session::claude_config_home());
    let mut out = Declared {
        files: Vec::new(),
        unread: Vec::new(),
        plugin: settings::plugin_dispatches_guard(&d.root, d.home.as_deref()),
    };
    for file in settings_files(here, folder.as_deref()) {
        match settings::guard_in(&file) {
            GuardIn::Declared => out.files.push(shown(&file, here)),
            GuardIn::Unread => out.unread.push(shown(&file, here)),
            GuardIn::Absent => {}
        }
    }
    out
}

/// `plane-root guard`: is the Bash guard declared where a session started here loads it?
pub(super) fn plane_root_guard(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(GUARD, "no control plane found");
    }
    let here = canonical(&d.cwd);
    let found = declared(d, &here);
    let seen = guardseen::last(&State::of(&d.root));
    guard_row(d, &here, &found, seen.as_ref())
}

fn guard_row(d: &Doctor, here: &Path, found: &Declared, seen: Option<&Sighting>) -> Row {
    let named = |id: &str| super::one_line(id, super::DISPLAY_LIMIT);
    match (&found.plugin, found.files.first()) {
        (Some(plugin), Some(_)) => Row::warn(
            GUARD,
            format!(
                "declared twice — the enabled plugin {} dispatches it, and so does {}",
                named(plugin),
                found.files.join(", ")
            ),
            format!(
                "Each declaration runs the guard once, so every Bash call is guarded twice. \
                 Remove the purlis `hooks` block from {} and keep the plugin, or disable the \
                 plugin and keep the block. purlis does not edit that file for you.",
                found.files.join(", ")
            ),
        ),
        (Some(plugin), None) => {
            // Enabled is not loaded: a plugin's hooks load when a session starts. A sighting the
            // plugin made is the proof.
            if seen.is_some_and(|s| s.source.as_deref() == Some("plugin")) {
                Row::ok(
                    GUARD,
                    format!(
                        "wired (enabled plugin {}) — and it has run here",
                        named(plugin)
                    ),
                )
            } else {
                Row::warn(
                    GUARD,
                    format!(
                        "enabled plugin {} declares it, but it has not run here yet — a \
                         plugin's hooks load when a session starts, so this is wired for the \
                         NEXT session",
                        named(plugin)
                    ),
                    "Start a new session, or run a Bash command through this one, and re-check.",
                )
            }
        }
        (None, Some(file)) => Row::ok(GUARD, format!("wired ({file})")),
        (None, None) if !found.unread.is_empty() => Row::warn(
            GUARD,
            format!(
                "purlis could not tell whether the guard is declared here: {} is not JSON it \
                 can read",
                found.unread.join(", ")
            ),
            "Claude Code loads nothing from a settings file it cannot parse, so whether the \
             guard runs in a session here cannot be said either way. Fix that file (purlis \
             never repairs it) and re-check.",
        ),
        (None, None) => not_wired(d, here),
    }
}

fn not_wired(d: &Doctor, here: &Path) -> Row {
    let at_root = here == d.root;
    let detail = if at_root {
        "pretooluse is not wired — a chat started outside the app runs no Bash guard here"
            .to_owned()
    } else {
        format!(
            "pretooluse is not wired — a chat started outside the app runs no Bash guard in \
             {}, and nothing declares it there",
            super::fsx::path_field(here)
        )
    };
    if d.preflight {
        // Inside a chat the app or the plugin already armed: a fact about other chats.
        return Row::ok(
            GUARD,
            format!("{detail}; the chats the app starts are armed by the app either way"),
        );
    }
    let hint = if at_root {
        "`purlis reinit` wires it into this project's .claude/settings.json, or install \
         purlis's Claude Code plugin (the `plugin install` row says how). The chats the app \
         starts are armed by the app either way."
            .to_owned()
    } else {
        format!(
            "A session rooted in {here} reads its settings from there and the host does not walk \
             up, so the project's own .claude/settings.json never reaches it, and `purlis reinit` \
             (which writes that file) would not change it. Install purlis's Claude Code plugin \
             (the `plugin install` row says how), or declare `purlis hook pretooluse` under \
             hooks.PreToolUse in {here}/.claude/settings.json. The chats the app starts are \
             armed by the app either way.",
            here = super::fsx::path_field(here)
        )
    };
    Row::warn(GUARD, detail, hint)
}

/// `guard seen`: when the guard last ran in this project, and under which harness.
pub(super) fn guard_seen(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(SEEN, "no control plane found");
    }
    let seen = guardseen::last(&State::of(&d.root));
    // The folder in use only for a doctor of this machine, as `plane-root guard` reads it.
    let folder = d
        .home
        .as_ref()
        .map(|_| super::session::claude_config_home());
    seen_row(
        seen.as_ref(),
        folder.as_deref(),
        || guardseen::worked_in(&d.root),
        chrono::Utc::now(),
    )
}

fn seen_row(
    seen: Option<&Sighting>,
    folder: Option<&Path>,
    worked_in: impl FnOnce() -> bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Row {
    let Some(seen) = seen else {
        if !worked_in() {
            return Row::ok(SEEN, "project not worked in yet");
        }
        return Row::warn(
            SEEN,
            "no guard has ever run in this project",
            "purlis's guards reach a chat through its harness, so work done outside one is \
             unguarded and says nothing either way. If you work here through a harness, the \
             guard is not reaching it: the `plane-root guard` row says what declares it.",
        );
    };
    let harness = seen.harness.as_deref().map_or_else(
        || "an unnamed harness".to_owned(),
        |h| super::one_line(h, super::DISPLAY_LIMIT),
    );
    let ran = format!(
        "last ran {} ago under {harness}",
        crate::pieces::since(seen.at, now)
    );
    // A Claude Code sighting under another config folder says nothing about the sessions on
    // the folder in use: its plugin list and its settings are another account's.
    if seen.is_claude_code()
        && let Some(folder) = folder.filter(|f| f.is_absolute())
        && seen
            .claude_config_dir
            .as_deref()
            .is_some_and(|recorded| Path::new(recorded) != folder)
    {
        return Row::warn(
            SEEN,
            format!(
                "{ran}, but under the Claude Code config folder {}, not the one in use ({})",
                super::one_line(
                    seen.claude_config_dir.as_deref().unwrap_or_default(),
                    super::PATH_DISPLAY_LIMIT
                ),
                short_path(folder)
            ),
            "A sighting under another config folder is no evidence for sessions on this one. \
             Run a Bash command in a chat on the folder in use, and re-check.",
        );
    }
    Row::ok(SEEN, ran)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::Status;

    fn at(text: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn sighting(harness: Option<&str>, source: &str, folder: Option<&str>) -> Sighting {
        Sighting {
            at: at("2026-10-13T00:00:00Z"),
            harness: harness.map(str::to_owned),
            source: Some(source.to_owned()),
            claude_config_dir: folder.map(str::to_owned),
        }
    }

    /// A doctor for a project at a temp dir, standing at `cwd` below it, reading no home.
    fn doctor(root: &Path, cwd: &Path, preflight: bool) -> Doctor {
        let mut d = Doctor::at(root, cwd, false, preflight);
        d.has_plane = true;
        d
    }

    fn found(files: &[&str], unread: &[&str], plugin: Option<&str>) -> Declared {
        Declared {
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            unread: unread.iter().map(|f| (*f).to_owned()).collect(),
            plugin: plugin.map(str::to_owned),
        }
    }

    #[test]
    fn a_settings_declaration_is_wired_and_named() {
        let d = doctor(Path::new("/p"), Path::new("/p"), false);
        let r = guard_row(
            &d,
            Path::new("/p"),
            &found(&[".claude/settings.json"], &[], None),
            None,
        );
        assert_eq!(r, Row::ok(GUARD, "wired (.claude/settings.json)"));
    }

    #[test]
    fn a_plugin_is_wired_only_once_a_sighting_it_made_says_it_runs() {
        let d = doctor(Path::new("/p"), Path::new("/p"), false);
        let plugin = found(&[], &[], Some("purlis@purlis"));
        let r = guard_row(&d, Path::new("/p"), &plugin, None);
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(r.detail.contains("NEXT session"), "{r:?}");
        let by_settings = sighting(Some("claude-code"), "settings", None);
        assert_eq!(
            guard_row(&d, Path::new("/p"), &plugin, Some(&by_settings)).status,
            Status::Warn,
            "a sighting from a settings block is no proof the plugin loads"
        );
        let by_plugin = sighting(Some("claude-code"), "plugin", None);
        let r = guard_row(&d, Path::new("/p"), &plugin, Some(&by_plugin));
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert!(r.detail.contains("purlis@purlis"), "{r:?}");
    }

    #[test]
    fn a_plugin_and_a_settings_block_is_declared_twice() {
        let d = doctor(Path::new("/p"), Path::new("/p"), false);
        let r = guard_row(
            &d,
            Path::new("/p"),
            &found(&[".claude/settings.json"], &[], Some("purlis@purlis")),
            None,
        );
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(r.detail.starts_with("declared twice"), "{r:?}");
    }

    #[test]
    fn a_file_purlis_cannot_read_is_never_called_not_wired() {
        let d = doctor(Path::new("/p"), Path::new("/p"), false);
        let r = guard_row(
            &d,
            Path::new("/p"),
            &found(&[], &[".claude/settings.json"], None),
            None,
        );
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(r.detail.contains("could not tell"), "{r:?}");
        assert!(!r.detail.contains("not wired"), "{r:?}");
    }

    #[test]
    fn nothing_declared_at_the_root_names_reinit() {
        let d = doctor(Path::new("/p"), Path::new("/p"), false);
        let r = guard_row(&d, Path::new("/p"), &found(&[], &[], None), None);
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(r.detail.starts_with("pretooluse is not wired"), "{r:?}");
        assert!(r.hint.contains("`purlis reinit`"), "{r:?}");
    }

    #[test]
    fn nothing_declared_below_the_root_does_not_send_the_reader_to_reinit_as_the_fix() {
        let d = doctor(Path::new("/p"), Path::new("/p/workspaces/a"), false);
        let r = guard_row(
            &d,
            Path::new("/p/workspaces/a"),
            &found(&[], &[], None),
            None,
        );
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(r.detail.contains("/p/workspaces/a"), "{r:?}");
        assert!(r.hint.contains("would not change it"), "{r:?}");
    }

    #[test]
    fn the_preflight_says_it_as_a_fact() {
        let d = doctor(Path::new("/p"), Path::new("/p"), true);
        let r = guard_row(&d, Path::new("/p"), &found(&[], &[], None), None);
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert!(r.detail.contains("armed by the app"), "{r:?}");
    }

    #[test]
    fn the_rows_read_the_files_a_session_here_loads() {
        // The settings files are the session directory's, and the user folder only when one is
        // in use; a doctor a test names reads none.
        let here = Path::new("/p/workspaces/a");
        assert_eq!(
            settings_files(here, None),
            [
                here.join(".claude/settings.json"),
                here.join(".claude/settings.local.json")
            ]
        );
        assert_eq!(
            settings_files(here, Some(Path::new("/h/.claude"))).last(),
            Some(&PathBuf::from("/h/.claude/settings.json"))
        );
        assert_eq!(
            shown(&here.join(".claude/settings.json"), here),
            ".claude/settings.json"
        );
    }

    #[test]
    fn a_project_nobody_worked_in_is_not_warned_about() {
        let now = at("2026-10-13T00:00:00Z");
        assert_eq!(
            seen_row(None, None, || false, now),
            Row::ok(SEEN, "project not worked in yet")
        );
        let r = seen_row(None, None, || true, now);
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert_eq!(r.detail, "no guard has ever run in this project");
    }

    #[test]
    fn a_sighting_is_an_age_and_a_harness() {
        let now = at("2026-10-13T03:00:00Z");
        let seen = sighting(Some("codex"), "settings", None);
        assert_eq!(
            seen_row(Some(&seen), None, || true, now),
            Row::ok(SEEN, "last ran 3h ago under codex")
        );
        let unnamed = sighting(None, "settings", None);
        assert_eq!(
            seen_row(Some(&unnamed), None, || true, now).detail,
            "last ran 3h ago under an unnamed harness"
        );
    }

    #[test]
    fn a_claude_code_sighting_under_another_folder_is_no_evidence_for_this_one() {
        let now = at("2026-10-13T03:00:00Z");
        let seen = sighting(Some("claude-code"), "settings", Some("/h/.claude-work"));
        let r = seen_row(Some(&seen), Some(Path::new("/h/.claude")), || true, now);
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(r.detail.contains("/h/.claude-work"), "{r:?}");
        let same = sighting(Some("claude-code"), "settings", Some("/h/.claude"));
        assert_eq!(
            seen_row(Some(&same), Some(Path::new("/h/.claude")), || true, now).status,
            Status::Ok
        );
        // Another harness has no Claude Code folder to be wrong about.
        let codex = sighting(Some("codex"), "settings", Some("/elsewhere"));
        assert_eq!(
            seen_row(Some(&codex), Some(Path::new("/h/.claude")), || true, now).status,
            Status::Ok
        );
    }

    #[test]
    fn a_settings_file_is_read_for_the_guard_as_init_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        assert_eq!(settings::guard_in(&file), GuardIn::Absent, "not there");
        std::fs::write(
            &file,
            r#"{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "purlis hook pretooluse"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(settings::guard_in(&file), GuardIn::Declared);
        // Another handler is not the guard, and a session-start hook alone protects nothing.
        std::fs::write(
            &file,
            r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "purlis hook pretooluse-read"}]}], "SessionStart": [{"hooks": [{"type": "command", "command": "purlis hook pretooluse"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(settings::guard_in(&file), GuardIn::Absent);
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(settings::guard_in(&file), GuardIn::Unread);
    }

    #[test]
    fn no_project_is_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        let d = Doctor::at(dir.path(), dir.path(), false, false);
        assert_eq!(
            plane_root_guard(&d),
            Row::ok(GUARD, "no control plane found")
        );
        assert_eq!(guard_seen(&d), Row::ok(SEEN, "no control plane found"));
    }

    #[test]
    fn a_guard_that_ran_here_is_read_from_the_record_it_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        guardseen::mark(
            &State::of(&root),
            Some("codex"),
            guardseen::Source::Settings,
            None,
            chrono::Utc::now(),
        )
        .unwrap();
        let r = guard_seen(&doctor(&root, &root, false));
        assert_eq!(r, Row::ok(SEEN, "last ran 0m ago under codex"));
    }
}
