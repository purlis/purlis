// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The persona verbs: `charter persona list`, `use` and `stats` (three of the four the
//! operator's planes called that the app's `charter` did not have, charter-app 0.1.0 audit,
//! 2026-09-24), and `create`, `show`, `clear`, `remove` and `lint` (#365).
//!
//! A port of `commands_persona.cmd_persona_list`, `cmd_persona_use`, `cmd_persona_stats`,
//! `cmd_persona_create`, `cmd_persona_show`, `cmd_persona_clear`, `cmd_persona_remove` and
//! `cmd_persona_lint`, and of the parts of `persona.py`, `mcpseen.py`, `dispatch.py` and
//! `skilluse.py` beneath them that no earlier port needed. The recorded scenarios named
//! `persona-list-…`, `persona-use-…` and `persona-stats-…` hold their sentences.
//!
//! The fourth verb of that audit, `sync-agents`, is retired with the persona sub-agent it
//! generated (#1451): [`retired`] holds what is left of it.
//!
//! # The vault registry is [`crate::secrets`]'s
//!
//! Which vault a persona uses when it declares none, and how a registered vault is (`persona
//! list`'s VAULT STATUS), are asked of the secrets port's registry and providers, so a vault
//! name it refuses or a half it cannot read is refused here in the same words.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod approve;
pub mod chatstart;
pub mod define;
pub mod lint;
pub mod list;
pub mod mcp;
pub mod retired;
pub mod select;
pub mod show;
pub mod stats;
#[cfg(test)]
pub(crate) mod tests_plane;
pub mod upkeep;

/// `vault: none` — a persona that deliberately holds no credentials. `persona.NO_VAULT`.
pub const NO_VAULT: &str = "none";

/// A persona with its `extends:` chain applied: the merged frontmatter, the concatenated
/// charter and the chain — `persona.resolve`'s `{meta, charter, lineage}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub meta: BTreeMap<String, String>,
    pub charter: String,
    pub lineage: Vec<String>,
}

impl Resolved {
    /// One merged value, `None` when the chain does not set it. Empty values are never
    /// merged (`resolve` skips them), so a present value is a non-empty one.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(String::as_str)
    }
}

/// `persona.resolve`: the frontmatter merge is [`crate::personagrant::resolve`]'s (one merge,
/// shared with the tool gate), and the charter is concatenated here, root first, each
/// descendant's own charter under a heading naming what it extends.
pub fn resolve(root: &Path, name: &str) -> Option<Resolved> {
    let merged = crate::personagrant::resolve(root, name)?;
    let mut charter = String::new();
    let mut prev: Option<&str> = None;
    for ancestor in merged.lineage.iter().rev() {
        let (_, body) = crate::personas::load_with_charter(root, ancestor)?;
        if !body.is_empty() {
            match prev {
                None => charter.push_str(&body),
                Some(parent) => charter.push_str(&format!(
                    "\n\n---\n\n### ⤷ `{ancestor}` extends `{parent}` — its own charter\n\n{body}"
                )),
            }
        }
        prev = Some(ancestor);
    }
    Some(Resolved {
        meta: merged.meta,
        charter,
        lineage: merged.lineage,
    })
}

/// A persona's OWN frontmatter as `dict(pairs)` would hold it: the last line for each key.
pub fn own_meta(root: &Path, name: &str) -> Option<BTreeMap<String, String>> {
    let pairs = crate::personas::load(root, name)?;
    Some(pairs.into_iter().collect())
}

/// Every persona this plane defines — `persona.list_personas`, by ITS rules: every
/// `personas/*.md` but a README, by stem, and every directory not starting `_` that holds a
/// `persona.md`, by its FULL name.
///
/// Not [`crate::personagrant::list_personas`], which takes a directory's `file_stem` and so
/// lists `personas/ops.v2/` as `ops`. The names here decide which definitions a command reads
/// and rewrites, so they are Python's exactly.
pub fn names(root: &Path) -> Vec<String> {
    let Ok(reader) = std::fs::read_dir(root.join("personas")) else {
        return Vec::new();
    };
    let mut out: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in reader.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".md") {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if stem.to_lowercase() != "readme" {
                out.insert(stem);
            }
        }
        if path.is_dir() && !name.starts_with('_') && path.join("persona.md").exists() {
            out.insert(name);
        }
    }
    out.into_iter().collect()
}

/// `persona.is_draft`: the RESOLVED `draft:` is one of `true`, `yes`, `1`, `on`, so a child
/// of a draft is a draft unless it says otherwise.
pub fn is_draft(root: &Path, name: &str) -> bool {
    resolve(root, name)
        .and_then(|r| {
            r.get("draft")
                .map(|d| crate::memstore::py_strip(d).to_lowercase())
        })
        .is_some_and(|d| matches!(d.as_str(), "true" | "yes" | "1" | "on"))
}

/// `persona.declared_skills`: the persona's OWN `skills:`, comma-separated, in order.
pub fn declared_skills(root: &Path, name: &str) -> Vec<String> {
    own_list(root, name, "skills")
}

/// **The skills the persona pins** (SK-15, ADR 0102 §4): its OWN `pinned-skills:`,
/// comma-separated, in order. Not inherited along `extends`, as `skills:` is not.
pub fn pinned_skills(root: &Path, name: &str) -> Vec<String> {
    own_list(root, name, crate::skills::persona::PINS_KEY)
}

/// **The router the persona's chats are routed by**, as written (SK-15b, ADR 0102 §5): its OWN
/// `skill-router:`, `None` where it writes none. Not inherited along `extends`, as
/// `pinned-skills:` is not.
pub fn skill_router(root: &Path, name: &str) -> Option<String> {
    let meta = own_meta(root, name)?;
    let written = crate::memstore::py_strip(meta.get(crate::skills::router::PERSONA_KEY)?);
    (!written.is_empty()).then(|| written.to_owned())
}

/// The persona's OWN `key:`, comma-separated, in order, blanks dropped.
fn own_list(root: &Path, name: &str, key: &str) -> Vec<String> {
    let Some(meta) = own_meta(root, name) else {
        return Vec::new();
    };
    let raw = meta
        .get(key)
        .map(|s| crate::memstore::py_strip(s))
        .unwrap_or("");
    raw.split(',')
        .map(|t| crate::memstore::py_strip(t).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// The secrets registry as these commands read it: the tree's committed half
/// (`<root>/vaults.json`) and this machine's (`<state>/vaults.json`), and this process's
/// environment for the providers' own lookups (`op`, `~`).
pub fn vault_ctx(root: &Path, state: &Path) -> crate::secrets::Ctx {
    crate::secrets::Ctx {
        root: root.to_path_buf(),
        state: state.to_path_buf(),
        env: crate::secrets::Env::from_process(),
        chat_writes: Vec::new(),
        kept: crate::secrets::identity::Kept::default(),
    }
}

/// `persona.vault_of`: the resolved `vault:` (`None` for [`NO_VAULT`]), else the first vault
/// the registry tags with this persona, by name. A registry that does not read names no
/// vault — Python catches the error here.
pub fn vault_of(root: &Path, state: &Path, name: &str) -> Option<String> {
    vault(root, state, name).name().map(str::to_string)
}

/// [`vault_of`], saying where its answer came from — the persona card's vault, so the card and
/// the CLI cannot name different ones (charter-app#185).
pub fn vault(root: &Path, state: &Path, name: &str) -> crate::personas::Vault {
    use crate::personas::Vault;
    if let Some(r) = resolve(root, name)
        && let Some(v) = r.get("vault")
    {
        return match crate::memstore::py_strip(v) {
            NO_VAULT => Vault::DeclaredNone,
            // Python's `vault_of` answers `None` for a value that strips to nothing and does
            // not go on to the registry. `Resolved` never carries a blank value, so this is
            // the port being exact rather than a case a definition reaches.
            "" => Vault::Unnamed,
            v => Vault::Named(v.to_string()),
        };
    }
    match crate::secrets::registry::load_registry(&vault_ctx(root, state)) {
        Err(why) => Vault::RegistryUnreadable(why.message.to_string()),
        Ok(doc) => crate::secrets::registry::vaults_for_persona(&doc, name)
            .into_iter()
            .next()
            .map_or(Vault::Unnamed, Vault::Registered),
    }
}

/// Where a persona's definition sits, relative to the plane — `def_path(name).relative_to(ROOT)`.
pub fn def_rel(root: &Path, name: &str) -> String {
    rel(root, &crate::personas::def_path(root, name))
}

/// `path` relative to `root` when it is inside it, else as it is — `commands_persona._rel`.
pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Python's `str.title()`: the first cased character after an uncased one is upper-cased and
/// every other cased character lower-cased — `ops-lite` is `Ops-Lite`.
pub fn py_title(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut after_cased = false;
    for c in text.chars() {
        let cased = c.is_lowercase() || c.is_uppercase();
        if cased {
            if after_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
        } else {
            out.push(c);
        }
        after_cased = cased;
    }
    out
}

/// The plane's state directory, where the MCP approvals and this machine's vault registry
/// live — `config.STATE_DIR`, `$CHARTER_HOME` included.
pub fn state_dir(plane: &Path) -> PathBuf {
    crate::plane::state_dir(plane)
}

#[cfg(test)]
mod tests;
