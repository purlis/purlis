// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A profile pins its harness version** (#2038, ADR 0095, spec #2030), so a conversation
//! resumes on the version that wrote it.
//!
//! A profile of this machine's records the version of its harness its chats run on
//! ([`Pinned`], `pinned` in its record). This machine has one version of each harness installed,
//! so "a chat runs on its profile's pinned version" means: **a chat starts on the pinned version,
//! or on a newer one of the same major version, which the pin then follows** ([`at_start`]),
//! asked of the program the chat runs (the operator's ruling of 2026-10-15).
//!
//! - **The first chat pins it.** A profile with no pin yet, made or adopted before this build,
//!   is pinned to the version its first chat finds installed: no conversation is in the profile
//!   before its first chat, so the pin holds from the first one on. Adoption runs no program, so
//!   an adopted profile is pinned the same way.
//! - **A profile purlis made follows its harness's minor and patch updates**: its pin moves and
//!   the chat's tab says so. **Another major version, or an older version, is refused**, a new
//!   chat and a resumed one alike, with the way out: **Update the pin**, the person's act on the
//!   profile's page (the window's alone, [`update_pin`]), or the pinned version installed again.
//!   A harness declaration does not yet say which versions read another's conversations, so a
//!   conversation resumes on a version of the same major as the one that wrote it, and the
//!   person's Update the pin is what moves the profile's conversations across a major version.
//! - **An adopted profile follows its harness** (the no-loss condition of ADR 0095 §2): the
//!   harness's own folder is what the person ran before purlis adopted it, and its own updater
//!   moves it. A new version moves its pin and the chat's tab says so; it never refuses a chat
//!   the person could start the day before.
//!
//! What a harness answers `--version` with is read by [`version_in`], from each harness's own
//! shape of answer, and compared as Semantic Versioning orders versions.

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use semver::Version;

use super::MachineProfile;

/// The harness version a profile is pinned to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pinned {
    /// As Semantic Versioning writes it: `2.1.300`.
    pub version: String,
    /// When it was pinned, in seconds since the epoch.
    pub at: u64,
}

/// The version in one line of what `kind`'s program answers `--version` with, in that harness's
/// own shape: Claude Code's `2.1.300 (Claude Code)`, Codex's `codex-cli 0.147.0`, opencode's
/// bare `1.18.33`. `None` for a harness purlis does not ship.
fn in_line<'a>(kind: &str, line: &'a str) -> Option<&'a str> {
    let line = line.trim();
    match kind {
        "claude" => line.strip_suffix(" (Claude Code)"),
        "codex" => line.strip_prefix("codex-cli "),
        "opencode" => Some(line),
        _ => None,
    }
}

/// The version in what `kind`'s program answered `--version` with: the first line in that
/// harness's shape that holds one. `None` where no line does.
pub fn version_in(kind: &str, said: &str) -> Option<Version> {
    said.lines()
        .filter_map(|line| in_line(kind, line))
        .find_map(|version| Version::parse(version).ok())
}

/// The version of `kind` installed on this machine, as its program on `PATH` answers
/// `--version`, asked as a sandboxed start asks it ([`crate::sandbox::program::version_of`]):
/// what the profile page sets beside each pin. `None` where it is not installed or gives no
/// answer in its shape.
pub fn installed_here(kind: &str) -> Option<Version> {
    let program = crate::harness_declaration::builtin(kind)?.program.clone();
    let words = crate::programs::resolve_argv(&[program]).ok()?;
    let cwd = std::env::temp_dir();
    let said = crate::sandbox::program::version_of(
        &words,
        crate::sandbox::program::Chat {
            cwd: &cwd,
            writable: &[],
            env: &[],
        },
    )
    .ok()?;
    version_in(kind, &said)
}

/// The record of this machine's that a chat on `profile` runs on, among `records`: the one purlis
/// made under its name, or the harness folder adopted in place under its built-in's name. `None`
/// for every other profile (a built-in nobody adopted, or one a project declares), which has no
/// pin.
pub fn record_of<'a>(
    profile: &crate::profiles::Profile,
    records: &'a [MachineProfile],
) -> Option<&'a MachineProfile> {
    let adopted = match profile.source {
        crate::profiles::Source::Machine => false,
        crate::profiles::Source::BuiltIn => true,
        _ => return None,
    };
    records
        .iter()
        .find(|r| r.name == profile.name && r.adopted.is_some() == adopted)
}

/// What a start on a profile does about its pin.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Judged {
    /// It runs as it is, and the pin stays.
    Runs,
    /// It runs, and the profile is pinned to this version from now on.
    Pins(Version),
    /// Its harness moved where the pin follows it: it runs, its pin moves, and its tab says so.
    Follows { from: String, to: Version },
}

/// What a chat on `record` does about its pin, `installed` being the version its program
/// answered (`None` where purlis could not read one), resuming a conversation or not.
fn judge(
    record: &MachineProfile,
    installed: Option<&Version>,
    resuming: bool,
) -> Result<Judged, String> {
    let adopted = record.adopted.is_some();
    let (pinned, installed) = match (&record.pinned, installed) {
        // Nothing read: nothing to pin it to, and nothing a pin can be shown to hold.
        (None, None) => return Ok(Judged::Runs),
        (None, Some(installed)) => return Ok(Judged::Pins(installed.clone())),
        (Some(_), None) if adopted => return Ok(Judged::Runs),
        (Some(pinned), None) => return Err(unread(record, &pinned.version)),
        (Some(pinned), Some(installed)) => (pinned, installed),
    };
    if Version::parse(&pinned.version).ok().as_ref() == Some(installed) {
        return Ok(Judged::Runs);
    }
    if follows(record, installed) {
        return Ok(Judged::Follows {
            from: pinned.version.clone(),
            to: installed.clone(),
        });
    }
    Err(moved(record, &pinned.version, installed, resuming))
}

/// Whether a chat on `record`, pinned to another version than `installed`, runs on it and its
/// pin follows: always for an adopted profile; for one purlis made, a newer version of the same
/// major (the operator's ruling of 2026-10-15), and never an older one, which may not read what
/// a newer one wrote.
pub fn follows(record: &MachineProfile, installed: &Version) -> bool {
    record.adopted.is_some()
        || record.pinned.as_ref().is_some_and(|pinned| {
            Version::parse(&pinned.version)
                .is_ok_and(|was| was.major == installed.major && installed > &was)
        })
}

/// The harness of `record` as people call it: *Claude Code*.
fn harness_title(record: &MachineProfile) -> String {
    crate::harness_declaration::builtin(&record.harness)
        .map_or_else(|| record.harness.clone(), |d| d.title.clone())
}

/// Why a chat on a made profile pinned to `pinned` does not start on `installed`, and the way
/// out.
fn moved(record: &MachineProfile, pinned: &str, installed: &Version, resuming: bool) -> String {
    let harness = harness_title(record);
    let name = crate::shown::short(&record.name);
    if resuming {
        format!(
            "This chat's conversation was written by {harness} {pinned}, the version the profile \
             '{name}' is pinned to, and {harness} {installed} is installed. purlis resumes a \
             conversation only on a newer version of the same major version as the one that \
             wrote it, so nothing was started. To resume it on {installed}, press Update the pin in Settings › You › Profiles, or install \
             {harness} {pinned} again."
        )
    } else {
        format!(
            "The profile '{name}' is pinned to {harness} {pinned}, and {harness} {installed} is \
             installed: a profile's chats follow its harness only to a newer version of the same \
             major version, so nothing was started. To run its chats on {installed}, press Update \
             the pin in Settings › You › Profiles, or install {harness} {pinned} again."
        )
    }
}

/// Why a chat on a made profile pinned to `pinned` does not start where its version could not
/// be read.
fn unread(record: &MachineProfile, pinned: &str) -> String {
    let harness = harness_title(record);
    let program = crate::harness_declaration::builtin(&record.harness)
        .map_or_else(|| record.harness.clone(), |d| d.program.clone());
    format!(
        "purlis could not read which version of {harness} the profile '{}' runs, so it cannot \
         tell it is {pinned}, the version the profile is pinned to, and nothing was started. \
         Check that `{program} --version` answers with its version.",
        crate::shown::short(&record.name)
    )
}

/// What a profile's tab says when its harness moved from `from` to `to` and its pin followed.
fn followed(record: &MachineProfile, from: &str, to: &Version) -> String {
    if record.adopted.is_none() {
        return format!(
            "The profile '{}' moved from {} {from} to {to}, and its pin with it: its chats \
             follow its harness to a newer version of the same major version.",
            crate::shown::short(&record.name),
            harness_title(record)
        );
    }
    format!(
        "{} moved from {from} to {to}, and its pin with it: it is the harness's own folder, so \
         its chats run on the version installed.",
        record.title
    )
}

/// **A chat's start on the profile `name`** of the store under `config_root`, `installed`
/// being the version its program answered `--version` with (`None` where purlis could not read
/// one), resuming a conversation or not: `Ok` with the sentence its tab says, where there is
/// one, or the refusal, with its way out. Pins a profile on its first chat and moves its pin
/// with its harness where it [`follows`], at `now`; judged under the store's lock, on the record as it
/// is then. A name the store does not have starts as it would have.
pub fn at_start(
    config_root: &Path,
    name: &str,
    installed: Option<&Version>,
    resuming: bool,
    now: u64,
) -> Result<Option<String>, String> {
    let mut adopted = false;
    let answer = super::update(config_root, |store| {
        let Some(record) = store.profiles.iter_mut().find(|p| p.name == name) else {
            return Ok(None);
        };
        adopted = record.adopted.is_some();
        let pin = |version: &Version| {
            Some(Pinned {
                version: version.to_string(),
                at: now,
            })
        };
        match judge(record, installed, resuming)? {
            Judged::Runs => Ok(None),
            Judged::Pins(version) => {
                record.pinned = pin(&version);
                Ok(None)
            }
            Judged::Follows { from, to } => {
                record.pinned = pin(&to);
                Ok(Some(followed(record, &from, &to)))
            }
        }
    });
    match answer {
        Ok(judged) => judged,
        // A store purlis cannot write never refuses a chat on the harness's own folder.
        Err(_) if adopted => Ok(None),
        Err(why) => Err(format!("{why}, so nothing was started.")),
    }
}

/// **Update the pin**: the person's act, from the profile's page (the window's alone), that pins
/// `name` to `version` at `now`, so its chats run, and its conversations resume, on that version
/// from then on. The record as it is now, or why not.
pub fn update_pin(
    config_root: &Path,
    name: &str,
    version: &str,
    now: u64,
) -> Result<MachineProfile, String> {
    let Ok(version) = Version::parse(version) else {
        return Err(format!(
            "'{}' is not a version purlis can pin a profile to.",
            crate::shown::short(version)
        ));
    };
    super::update(config_root, |store| {
        let Some(record) = store.profiles.iter_mut().find(|p| p.name == name) else {
            return Err(format!(
                "this machine has no profile named '{}'.",
                crate::shown::short(name)
            ));
        };
        record.pinned = Some(Pinned {
            version: version.to_string(),
            at: now,
        });
        Ok(record.clone())
    })?
}

#[cfg(test)]
mod tests;
