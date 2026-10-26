// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`rename-plane`** (RN-7, V93g): the committed files of a project, renamed to purlis's names
//! in ONE commit the operator asked for and can review.
//!
//! Never automatic: it is a fix applied only by its id ([`super::FixId::by_name_only`]), because
//! it changes what every teammate pulls. What the commit holds, and nothing else:
//!
//! - `charter.toml` → `purlis.toml`, with `requires = [{ feature = "purlis-names", … }]` (and
//!   `schema = 2`, which a project that lists `requires` declares, V37a). A build without the
//!   feature opens the project read-only and says which build has it ([`crate::compat`]). A
//!   build older than the purlis manifest reads no `charter.toml` here and does not see a
//!   project at all.
//! - `.charter-scan-allow.toml` → `.purlis-scan-allow.toml`.
//! - The managed blocks' markers, rewritten in place: the merge rules in `.gitattributes`, the
//!   live workspaces in `.gitignore`, the personas block in `README.md`, and the marker on each
//!   agent `persona sync-agents` generated. A committed `workspace.json` that charter stamped
//!   has its `charter_generated` key renamed where it stands; the digest does not cover the key,
//!   so the file stays charter's.
//! - `.claude/settings.json`: `CHARTER_HARNESS`, and the `charter …` ask and deny rules each
//!   given a `purlis …` twin, through [`settings::renamed_to_purlis`]; `opencode.json`'s ask and
//!   deny globs the same. The `charter` rules stay, because `charter` still runs the command line
//!   for the rename's window. Hook and status-line commands keep `charter` for the whole window
//!   (D-RN7-11): the alias resolves on every build, and a hook whose program is not on `PATH`
//!   fails open.
//! - Personas' `charter:<skill>` references, in their definitions and in the agents generated
//!   from them, become `purlis:<skill>`: a reference only, never prose, an image name or a line
//!   of a fenced code block (`skill_refs`, the rule `project-skills` shares; ADR 0091's
//!   D-RN7-13).
//!
//! Only files git tracks are touched: an ignored `workspace.json` is this machine's, and is
//! renamed the next time charter writes it.
//!
//! # Refused, with nothing written
//!
//! From inside a chat (D-RN7-12): it rewrites committed files for everyone, so it is the
//! operator's own act, from the app or a terminal. On a project this charter may not write
//! ([`super::refusal`], as every fix), outside a git repository whose top is the project, with tracked changes not committed (the commit must
//! hold the rename and nothing else), and with a file under both names (which one holds the
//! settings is the operator's call).
//!
//! # All or nothing
//!
//! Every change is made in the working tree, staged by its path, and committed — through
//! charter's own git runner, which runs no hooks, as every commit charter makes. When a step
//! fails before the commit is made — a write, the stage, the commit itself — every path it
//! touched is put back from `HEAD`, the files it created are removed, and the fix answers why.
//! Once the commit is made nothing is undone.

use std::path::Path;

use crate::scaffold::settings;

use super::Fixed;
use super::commit::{
    CLAIM_WAIT, Work, commit_paths, committable, git, read, tracked, with_feature_named, write,
};
use crate::counts::counted;

/// The commit's subject line.
pub const SUBJECT: &str = "Rename this project's files to purlis names (rename-plane)";

/// What a chat asking for the fix is told (D-RN7-12).
pub const FROM_A_CHAT: &str = "rename-plane rewrites this project's committed files for everyone \
     who works in it, so it is not run from inside a chat; nothing was written. Run `purlis \
     doctor --fix rename-plane` from a terminal, or apply the fix from the app's Doctor";

/// Apply `rename-plane` to the project at `root`, as this process's environment says it runs.
/// The caller has already refused a project this charter may not write.
pub fn apply(root: &Path) -> Fixed {
    apply_in(root, &|name| crate::envvar::var(name))
}

/// [`apply`], reading the environment through `env`: refused when it carries a chat's session
/// (the product's own variable, under either name).
pub fn apply_in(root: &Path, env: &dyn Fn(&str) -> Option<String>) -> Fixed {
    if env(crate::active::SESSION_ID_ENV).is_some_and(|id| !id.is_empty()) {
        return Fixed::Refused(FROM_A_CHAT.to_owned());
    }
    let Some(_claim) = crate::planegit::Claim::within(root, CLAIM_WAIT) else {
        return Fixed::Refused(
            "a save is running in this project; nothing was written. Run the fix again once it \
             has finished"
                .to_owned(),
        );
    };
    let _held = crate::rewrite::Lock::on(root);
    if let Err(why) = ready(root) {
        return Fixed::Refused(why);
    }
    let tracked = match tracked(root) {
        Ok(tracked) => tracked,
        Err(why) => return Fixed::Refused(why),
    };
    let mut work = Work::default();
    if let Err(why) = rename(root, &tracked, &mut work) {
        let restored = work.undo(root);
        return Fixed::Refused(format!("{why}; {restored}"));
    }
    if work.touched.is_empty() {
        return Fixed::Ran {
            said: vec![
                "✓ this project's committed files already use purlis's names — nothing to do."
                    .to_owned(),
            ],
            complete: true,
        };
    }
    let before = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    match commit(root, &work) {
        Ok(sha) => {
            let mut said = work.said;
            match sha {
                Some(sha) => said.push(format!("✓ committed {sha}: {SUBJECT}")),
                None => said.push(format!(
                    "✓ committed: {SUBJECT} (git could not name the commit; `git log -1` shows it)"
                )),
            }
            said.push(format!(
                "  A build without the `{}` feature now opens this project read-only; teammates \
                 update the app to work in it.",
                crate::names::PURLIS_NAMES_FEATURE
            ));
            // The rules are in force in every workspace layer at once, as `guard ask` carries
            // them (#449). Layers are this machine's and not in the commit.
            for line in crate::guardcmd::mirror(root).lines() {
                said.push(format!("  {line}"));
            }
            Fixed::Ran {
                said,
                complete: true,
            }
        }
        Err(why) => {
            // A commit git reports failing may still have been made (a deadline that passed
            // after it wrote): a moved HEAD is a commit, and a commit is never undone.
            let now = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
            if !before.is_empty() && now != before {
                return Fixed::Ran {
                    said: vec![format!(
                        "✗ {why}, but HEAD moved: the rename is committed; check it with `git \
                         log -1`"
                    )],
                    complete: false,
                };
            }
            let restored = work.undo(root);
            Fixed::Refused(format!("{why}; {restored}"))
        }
    }
}

/// Why the fix may not run here, or `Ok`.
fn ready(root: &Path) -> Result<(), String> {
    committable(root, "the rename")?;
    for name in [crate::names::PLANE_MANIFEST, crate::names::SCAN_ALLOW] {
        let at = name.file_in(root);
        if !at.leftovers.is_empty() {
            return Err(format!(
                "this project has both {} and {}, and which of them holds what you meant is \
                 yours to say; nothing was written. Keep one — move what you need from {} into \
                 {} and remove {} — then run the fix again",
                name.write,
                name.newest_old(),
                name.newest_old(),
                name.write,
                name.newest_old()
            ));
        }
    }
    Ok(())
}

/// Every change, made in the working tree and recorded in `work`.
fn rename(root: &Path, tracked: &[String], work: &mut Work) -> Result<(), String> {
    let has = |path: &str| tracked.iter().any(|p| p == path);
    manifest(root, &has, work)?;

    let old_allow = crate::names::SCAN_ALLOW.newest_old();
    let new_allow = crate::names::SCAN_ALLOW.write;
    if has(old_allow) {
        git(root, &["mv", "--", old_allow, new_allow])
            .map_err(|why| format!("could not rename {old_allow} ({why})"))?;
        work.touch(old_allow);
        work.touch(new_allow);
        work.created.push(new_allow.to_owned());
        work.said
            .push(format!("✓ renamed {old_allow} to {new_allow}"));
    }

    use crate::names::{
        LIVE_BEGIN, LIVE_END, MERGE_RULES_BEGIN, MERGE_RULES_END, PERSONAS_BEGIN,
        SYNC_AGENTS_MARKER,
    };
    if has(".gitattributes") {
        markers(
            root,
            ".gitattributes",
            &[MERGE_RULES_BEGIN, MERGE_RULES_END],
            work,
        )?;
    }
    if has(".gitignore") {
        markers(root, ".gitignore", &[LIVE_BEGIN, LIVE_END], work)?;
    }
    if has("README.md") {
        markers(root, "README.md", &[PERSONAS_BEGIN], work)?;
    }
    for path in tracked {
        if is_workspace_manifest(path) {
            workspace_key(root, path, work)?;
        } else if is_agent(path) {
            let text = read(root, path)?;
            if crate::personaverbs::retired::carries_marker(&text) {
                let renamed = skill_refs(&swap(&text, &[SYNC_AGENTS_MARKER]));
                write_if_changed(root, path, &text, &renamed, "the agent marker", work)?;
            }
        } else if is_persona(path) {
            let text = read(root, path)?;
            write_if_changed(
                root,
                path,
                &text,
                &skill_refs(&text),
                "skill references",
                work,
            )?;
        }
    }

    if has(settings::SETTINGS)
        && let Some(text) = settings::renamed_to_purlis(root)?
    {
        write(root, settings::SETTINGS, &text)?;
        work.touch(settings::SETTINGS);
        work.said.push(format!(
            "✓ {}: the harness variable and rules under purlis's name",
            settings::SETTINGS
        ));
    }
    if has(settings::OPENCODE) {
        let twins = settings::opencode_rules_to_twin(root)?;
        // Touched before the first write, so a write that fails part-way is put back too.
        if !twins.is_empty() {
            work.touch(settings::OPENCODE);
        }
        let mut added = 0;
        for (glob, decision) in twins {
            match settings::ensure_opencode_rule(root, &glob, &decision, false) {
                settings::Wrote::Created => added += 1,
                settings::Wrote::Present | settings::Wrote::Denied => {}
                settings::Wrote::Malformed(what) => {
                    return Err(format!("{what} is not a file purlis can read"));
                }
                settings::Wrote::Blocked(dir) => {
                    return Err(format!("{} is not a directory", dir.display()));
                }
                settings::Wrote::Failed(path, e) => {
                    return Err(format!("could not write {} ({e})", path.display()));
                }
            }
        }
        if added == 0 {
            // Every twin was there already: nothing was written, so nothing to commit.
            work.touched.retain(|p| p != settings::OPENCODE);
        } else {
            work.said.push(format!(
                "✓ {}: {} under purlis's name, beside charter's",
                settings::OPENCODE,
                counted(added, "rule", "rules")
            ));
        }
    }
    Ok(())
}

/// The manifest, renamed and marked as requiring purlis's names.
fn manifest(root: &Path, has: &dyn Fn(&str) -> bool, work: &mut Work) -> Result<(), String> {
    let old = crate::names::PLANE_MANIFEST.newest_old();
    let new = crate::names::PLANE_MANIFEST.write;
    let at = crate::names::manifest_name(root);
    if !has(at) {
        return Err(format!(
            "git does not track {at}, so a commit cannot carry its rename; commit it first"
        ));
    }
    let text = read(root, at)?;
    let marked = with_feature(&text).map_err(|why| format!("{at}: {why}"))?;
    if at == old {
        git(root, &["mv", "--", old, new])
            .map_err(|why| format!("could not rename {old} ({why})"))?;
        work.touch(old);
        work.touch(new);
        work.created.push(new.to_owned());
        work.said.push(format!("✓ renamed {old} to {new}"));
    }
    if let Some(marked) = marked {
        write(root, new, &marked)?;
        work.touch(new);
        work.said.push(format!(
            "✓ {new} requires `{}`",
            crate::names::PURLIS_NAMES_FEATURE
        ));
    }
    Ok(())
}

/// `text` with `requires` holding the purlis-names feature and `schema` at least 2, in the
/// file's own layout, or `None` when it already says both.
fn with_feature(text: &str) -> Result<Option<String>, String> {
    with_feature_named(text, crate::names::PURLIS_NAMES_FEATURE)
}

/// Each whole line of `rel` that is one of `names`' old spellings, as that name's purlis one.
fn markers(
    root: &Path,
    rel: &str,
    names: &[crate::names::Name],
    work: &mut Work,
) -> Result<(), String> {
    let text = read(root, rel)?;
    let renamed: String = text
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches(['\n', '\r']);
            let end = &line[body.len()..];
            names
                .iter()
                .find(|name| name.reads.contains(&body))
                .map_or_else(|| line.to_owned(), |name| format!("{}{end}", name.write))
        })
        .collect();
    // The personas marker is found anywhere in the README, as the roster finds it.
    let renamed = if rel == "README.md" {
        swap(&renamed, names)
    } else {
        renamed
    };
    write_if_changed(root, rel, &text, &renamed, "markers", work)
}

/// `text` with every old spelling of each of `names` replaced by its purlis one.
fn swap(text: &str, names: &[crate::names::Name]) -> String {
    names.iter().fold(text.to_owned(), |text, name| {
        name.reads
            .iter()
            .fold(text, |text, old| text.replace(old, name.write))
    })
}

/// `text` with each `charter:<skill>` reference as `purlis:<skill>`: the skill namespace
/// followed by a skill's name (`[a-z0-9][a-z0-9-]*`), not preceded by a word character, a dot, a
/// slash, an `@` or a `:`, so `mycharter:x`, `charter: a sentence` and an image such as
/// `ghcr.io/x/charter:v1` are left alone. Nothing inside a fenced code block (```` ``` ```` or
/// `~~~`) is a reference: what is quoted there is somebody's code, as written.
pub(super) fn skill_refs(text: &str) -> String {
    static REF: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        let old = regex::escape(crate::names::SKILL_NAMESPACE.newest_old());
        regex::Regex::new(&format!(r"(^|[^A-Za-z0-9_./@:-]){old}([a-z0-9][a-z0-9-]*)"))
            .expect("the pattern compiles")
    });
    let namespace = crate::names::SKILL_NAMESPACE.write;
    let mut out = String::with_capacity(text.len());
    let mut fence: Option<&str> = None;
    for line in text.split_inclusive('\n') {
        let opens = ["```", "~~~"]
            .into_iter()
            .find(|mark| line.trim_start().starts_with(mark));
        match (fence, opens) {
            (None, Some(mark)) => fence = Some(mark),
            (Some(open), Some(mark)) if open == mark => fence = None,
            _ => {}
        }
        if fence.is_some() || opens.is_some() {
            out.push_str(line);
            continue;
        }
        out.push_str(&REF.replace_all(line, |c: &regex::Captures<'_>| {
            format!("{}{namespace}{}", &c[1], &c[2])
        }));
    }
    out
}

/// A tracked `workspaces/<name>/workspace.json` whose digest charter wrote under the old key,
/// with the key renamed where it stands. A manifest a hand edited is the operator's and is left
/// alone ([`crate::manifest::Ownership`]).
fn workspace_key(root: &Path, rel: &str, work: &mut Work) -> Result<(), String> {
    use crate::manifest::{Ownership, ownership};
    let text = read(root, rel)?;
    if ownership(Some(&text)) != Ownership::Charter {
        return Ok(());
    }
    let old = format!("\"{}\"", crate::names::GENERATED_KEY.newest_old());
    let new = format!("\"{}\"", crate::names::GENERATED_KEY.write);
    if text.matches(&old).count() != 1 || text.contains(&new) {
        return Ok(());
    }
    let renamed = text.replacen(&old, &new, 1);
    // Renamed as the key it was, and nothing else: the digest still matches the body.
    if ownership(Some(&renamed)) != Ownership::Charter {
        return Ok(());
    }
    write_if_changed(root, rel, &text, &renamed, "its digest key", work)
}

fn is_workspace_manifest(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), ["workspaces", _, "workspace.json"])
}

fn is_agent(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), [".claude", "agents", file] if file.ends_with(".md"))
}

/// A persona's definition: `personas/<name>/persona.md`, or the flat `personas/<name>.md`.
pub(super) fn is_persona(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["personas", _, "persona.md"] => true,
        ["personas", file] => file.ends_with(".md") && *file != "README.md",
        _ => false,
    }
}

fn write_if_changed(
    root: &Path,
    rel: &str,
    was: &str,
    now: &str,
    what: &str,
    work: &mut Work,
) -> Result<(), String> {
    if was == now {
        return Ok(());
    }
    write(root, rel, now)?;
    work.touch(rel);
    work.said
        .push(format!("✓ {rel}: {what} under purlis's name"));
    Ok(())
}

/// Stage exactly the touched paths and commit them; charter's git runner runs no hooks.
/// Answers the short sha, or `None` when the commit was made and git could not name it. `Err`
/// only when no commit was made.
fn commit(root: &Path, work: &Work) -> Result<Option<String>, String> {
    let paths: Vec<&str> = work.touched.iter().map(String::as_str).collect();
    let body = format!(
        "{SUBJECT}\n\nMade by `purlis doctor --fix rename-plane`. The project now requires the \
         `{}` feature, so a build without it opens it read-only.\n",
        crate::names::PURLIS_NAMES_FEATURE
    );
    commit_paths(root, &paths, &body)
}

#[cfg(test)]
mod tests;
