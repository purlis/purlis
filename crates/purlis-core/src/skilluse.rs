// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which skills each persona actually invokes — the writer of `charter/skilluse.py`.
//!
//! `personas/_skills/<YYYY-MM>.<device>.jsonl`, append-only, one row per `Skill` call, written by
//! `posttooluse-skill`. It is committed beside the dispatch log for the same reason: a persona
//! whose declared `skills:` are never used, or which leans on skills it never declared, is a
//! fact about the whole team's use of it, not about one laptop. `charter persona` reads it back
//! as drift between what a persona declares and what it does.

use std::path::{Path, PathBuf};

/// The directory under `personas/` — `skilluse.DIR_NAME`.
pub const DIR_NAME: &str = "_skills";

/// `personas/_skills/<YYYY-MM>.<device>.jsonl` — `skilluse.path_for`; `<device>` is
/// [`crate::machine::log_name`]'s (FD-25).
pub fn path_for(root: &Path, when: chrono::DateTime<chrono::Utc>, device: &str) -> PathBuf {
    root.join("personas")
        .join(DIR_NAME)
        .join(format!("{}.{device}.jsonl", when.format("%Y-%m")))
}

/// Log one use of `skill` by `persona` — `skilluse.record`. Nothing is logged in a project this
/// purlis may not write ([`crate::compat::read_only`], #826): the log is committed.
pub fn record(
    root: &Path,
    skill: &str,
    persona: Option<&str>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    let skill = crate::memstore::py_strip(skill);
    if skill.is_empty() || crate::compat::read_only(root).is_some() {
        return None;
    }
    let persona = persona
        .map(crate::memstore::py_strip)
        .filter(|p| !p.is_empty())
        .map_or(serde_json::Value::Null, |p| {
            serde_json::Value::String(p.to_string())
        });
    crate::dispatch::append(
        &path_for(root, when, device),
        root,
        &serde_json::json!({
            "persona": persona,
            "skill": skill,
            "ts": crate::dispatch::stamp(when),
        }),
    )
}

#[cfg(test)]
mod tests;
