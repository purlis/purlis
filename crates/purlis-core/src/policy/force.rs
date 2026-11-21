// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The policy in force, and the accepted floor** (M75 slice 14, #7023; spec §9.1, §6.2, §3.2).
//!
//! Every decision is made twice over: by the project as it is committed now (and the person's
//! local file), and by what the person accepted, and **the stricter result wins** (deny over ask
//! over allow). A tightening applies at once, and a loosening waits for the person's press, on
//! this machine only. Until the first acceptance the shipped default stands in for the accepted
//! side (Q309 point 2).
//!
//! **A [`Version`] is a project as it was held**: the project file, `policy.cedar`, the shared
//! `vaults.json` and the plane's workspaces as `HEAD` held them, and the local file's text. Each
//! is judged by [`levels::load_held`] on its own, with its own named sets and its own entity
//! attributes ([`world_of`]): the sets and `[tools.*]` are never merged across sides.
//!
//! **The accepted floor** is the conjunction of every version seen since the last press, each a
//! sealed entry of the answer store ([`answers::FLOOR`]), so moving `HEAD` back, committing a
//! broken file or deleting an entity a rule names drops no forbid without a press. A version is
//! sealed when the app sees it ([`seal`]): a committed one that is new, or one where the local
//! file tightens. **The cap** is `trust.accepted-floor-max-versions` (32 until slice 12 reads the
//! limit), counts committed versions only (the local file's sealed tightenings do not), and a
//! commit past it is recorded as [`answers::PAST_THE_CAP`]: the deny state until the press.
//!
//! The press itself (slice 15) is not here: it is [`answers::recommit`], which starts the floor
//! again.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::enforcement::{Committed, committed_files};
use crate::secrets::answers::{self, Standing, State};

use super::decide::{Decision, Request, decide};
use super::levels::{self, Held, RULES_FILE, Sources};
use super::{KEEPS, TIMEDS, World};

/// The shared registry of vaults, at the plane root.
const VAULTS_FILE: &str = "vaults.json";

/// How long git may take to list the workspaces at `HEAD`.
const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// **A project as one moment held it** (see the module's words).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    /// The name the project file is committed by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_file: Option<String>,
    /// The project file's text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<String>,
    /// `policy.cedar`'s text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    /// The shared `vaults.json`'s text: a Vault's tags and Persona come from it, and never from
    /// the local half.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaults: Option<String>,
    /// The plane's workspaces at `HEAD`: a Path's owning workspace is one of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspaces: Vec<String>,
    /// The You level's local file's text, which only tightens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
}

impl Version {
    /// What the plane root's `HEAD` holds now, and the local file: `None` where `HEAD` cannot be
    /// read (no commit, a git that did not answer), which is no version to keep.
    pub fn at_head(root: &Path) -> Option<Self> {
        let names: Vec<&'static str> = crate::names::PLANE_MANIFEST.spellings().collect();
        let mut read = committed_files(root, &[&names, &[RULES_FILE], &[VAULTS_FILE]]);
        let vaults = text_of(read.pop()?)?;
        let rules = text_of(read.pop()?)?;
        let settings = text_of(read.pop()?)?;
        let local = match crate::settings::layer_text(root, crate::settings::Which::Local) {
            crate::settings::LayerText::Text(text)
            | crate::settings::LayerText::LeftOut { text, .. } => Some(text),
            crate::settings::LayerText::Nothing => None,
        };
        Some(Self {
            settings_file: settings.as_ref().map(|(file, _)| (*file).to_owned()),
            settings: settings.map(|(_, text)| text),
            rules: rules.map(|(_, text)| text),
            vaults: vaults.map(|(_, text)| text),
            workspaces: workspaces_at_head(root),
            local,
        })
    }

    /// This version without the local file: what a commit made of it.
    fn committed(&self) -> Self {
        Self {
            local: None,
            ..self.clone()
        }
    }

    fn held(&self) -> Held<'_> {
        Held {
            settings: self
                .settings
                .as_deref()
                .map(|text| (self.settings_file.as_deref().unwrap_or("purlis.toml"), text)),
            rules: self.rules.as_deref(),
            local: self.local.as_deref(),
        }
    }
}

/// A file's text as committed, `None` where `HEAD` cannot say.
#[allow(clippy::option_option, reason = "absent is an answer; unread is none")]
fn text_of(committed: Committed) -> Option<Option<(&'static str, String)>> {
    match committed {
        Committed::Text { file, text } => Some(Some((file, text))),
        Committed::Absent => Some(None),
        Committed::NoHead { .. } | Committed::Unread { .. } => None,
    }
}

/// The folders of `workspaces/` at `HEAD`, as git wrote them: not the working tree's, which an
/// uncommitted edit changes.
fn workspaces_at_head(root: &Path) -> Vec<String> {
    let run = crate::worktree::git::run_as_written(
        root,
        &["ls-tree", "-d", "--name-only", "HEAD", "workspaces/"],
        GIT_WAIT,
    );
    let Ok(run) = run else {
        return Vec::new();
    };
    if !run.ok() {
        return Vec::new();
    }
    let mut names: Vec<String> = run
        .out
        .lines()
        .filter_map(|line| line.strip_prefix("workspaces/"))
        .filter(|name| !name.starts_with('.') && !name.contains('/'))
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

// ---- the world a side decides over ------------------------------------------------------------

/// The set type each kind of resource can be a member of.
fn set_type(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Host" => "HostSet",
        "Branch" => "BranchSet",
        "Path" => "PathSet",
        "Skill" => "SkillSet",
        "Tool" => "ToolSet",
        "Persona" => "PersonaSet",
        _ => return None,
    })
}

/// Whether the set member `member` names the `kind` resource `id`: a path by itself or the folder
/// it is in, a branch by its id or its name, anything else by its id.
fn names(kind: &str, member: &str, id: &str) -> bool {
    match kind {
        "Path" => {
            let folder = member.trim_end_matches('/');
            id == member || id == folder || id.starts_with(&format!("{folder}/"))
        }
        "Branch" => id == member || id.rsplit('#').next() == Some(member),
        _ => id == member,
    }
}

/// **The world one side decides over**: the request's resource, in the named sets *this side's*
/// policy holds it in, with the attributes *this side's* version read at `HEAD` (spec §3.2): a
/// Vault's tags and Persona from the committed `vaults.json`, a Path's owning workspace. Nothing a
/// working file says reaches it. Where the world does not fit the schema it is empty, and a rule
/// that reads an attribute errs, which is a deny.
fn world_of(
    loaded: &levels::Loaded,
    version: &Version,
    root: &Path,
    request: &Request<'_>,
) -> World {
    let (kind, id) = request.resource;
    let id = if kind == "Host" {
        crate::sandbox::hosts::Host::parse(id).map_or_else(|_| id.to_owned(), |h| h.to_string())
    } else {
        id.to_owned()
    };
    let mut parents: Vec<serde_json::Value> = Vec::new();
    if let Some(set) = set_type(kind) {
        for (name, members) in &loaded.sets {
            if members.iter().any(|member| names(kind, member, &id)) {
                parents.push(serde_json::json!({"type": set, "id": name}));
            }
        }
    }
    let mut entity = serde_json::json!({"uid": {"type": kind, "id": id}, "attrs": {}});
    match kind {
        "Path" => {
            if let Some(workspace) = owning_workspace(version, root, &id) {
                entity["attrs"] = serde_json::json!({
                    "workspace": {"__entity": {"type": "Workspace", "id": workspace}}
                });
                parents.push(serde_json::json!({"type": "Workspace", "id": workspace}));
            }
        }
        "Vault" => {
            let (tags, persona) = vault_in(version, &id);
            entity["tags"] = tags;
            if let Some(persona) = persona {
                parents.push(serde_json::json!({"type": "Persona", "id": persona}));
            }
        }
        _ => {}
    }
    entity["parents"] = parents.into();
    let mut entities = vec![entity];
    // The asking chat, in its workspace and persona, where the host knows both: a rule that reads
    // `principal.workspace` or `principal.persona` finds them, and a `dispatch.locked` pair names
    // a chat that is there. A chat that has no workspace or no persona cannot fit the schema,
    // which asks for both, so it is left out and such a rule errs, which is a deny.
    if let (Some(workspace), Some(persona)) = (request.of.workspace, request.of.persona) {
        entities.push(serde_json::json!({
            "uid": {"type": "Chat", "id": request.chat},
            "attrs": {
                "workspace": {"__entity": {"type": "Workspace", "id": workspace}},
                "persona": {"__entity": {"type": "Persona", "id": persona}},
            },
            "parents": [
                {"type": "Workspace", "id": workspace},
                {"type": "Persona", "id": persona},
                {"type": "Plane", "id": "plane"},
            ],
        }));
    }
    World::from_json(serde_json::Value::Array(entities))
        .or_else(|_| World::from_json(serde_json::json!([])))
        .expect("an empty world fits any schema")
}

/// The workspace of this version that holds the path `id`, from the plane root: the first folder
/// of the path, or the one after `workspaces/`.
fn owning_workspace(version: &Version, root: &Path, id: &str) -> Option<String> {
    let relative = Path::new(id).strip_prefix(root).unwrap_or(Path::new(id));
    let mut parts = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .filter(|part| *part != "/");
    let first = parts.next()?;
    let name = if first == "workspaces" {
        parts.next()?
    } else {
        first
    };
    version
        .workspaces
        .iter()
        .any(|w| w == name)
        .then(|| name.to_owned())
}

/// A vault's tags (its string fields but its config) and its Persona, from the version's shared
/// registry. A vault it does not list has none.
fn vault_in(version: &Version, name: &str) -> (serde_json::Value, Option<String>) {
    let entry = version
        .vaults
        .as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|doc| doc.get("vaults")?.get(name).cloned());
    let Some(serde_json::Value::Object(entry)) = entry else {
        return (serde_json::json!({}), None);
    };
    let tags: serde_json::Map<String, serde_json::Value> = entry
        .iter()
        .filter(|(key, value)| *key != "config" && value.is_string())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let persona = entry
        .get("persona")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
        .map(str::to_owned);
    (tags.into(), persona)
}

// ---- the two evaluations ----------------------------------------------------------------------

/// The accepted copy the store holds, if any: `Err` where it does not read, which counts as the
/// strictest policy there is, never as none.
fn accepted_in(standing: &Standing) -> Result<Option<Version>, ()> {
    standing
        .entries
        .iter()
        .find(|e| e.kind == answers::ACCEPTED)
        .map(|e| serde_json::from_str(&e.text).map_err(|_| ()))
        .transpose()
}

/// Every version of the accepted floor the store holds; `Err` for one that does not read.
fn floor_in(standing: &Standing) -> Vec<Result<Version, ()>> {
    standing
        .entries
        .iter()
        .filter(|e| e.kind == answers::FLOOR)
        .map(|e| serde_json::from_str(&e.text).map_err(|_| ()))
        .collect()
}

/// The sources a held version is judged with: the Organisation as it is, and the workspaces of the
/// plane now and of that version, so a table naming one deleted since still reads.
fn sources_for<'a>(sources: &Sources<'a>, workspaces: &'a [&'a str]) -> Sources<'a> {
    Sources {
        root: sources.root,
        workspaces,
        personas: sources.personas,
        organisation: sources.organisation,
        accepted: None,
    }
}

/// One side's decision: `version` judged by its own policy and its own world.
fn side(
    sources: &Sources<'_>,
    version: &Version,
    root: &Path,
    request: &Request<'_>,
    strict: bool,
) -> Decision {
    let mut workspaces: Vec<&str> = sources.workspaces.to_vec();
    workspaces.extend(version.workspaces.iter().map(String::as_str));
    let held = sources_for(sources, &workspaces);
    let loaded = if strict {
        levels::Loaded::shipped_strict()
    } else {
        levels::load_held(&held, &version.held())
    };
    let world = world_of(&loaded, version, root, request);
    decide(&loaded, request, &world)
}

/// The narrower of two `@keep` scopes.
fn narrower_keep(a: Option<String>, b: Option<String>) -> Option<String> {
    let at = |w: &String| KEEPS.iter().position(|k| k == w).unwrap_or(0);
    match (a, b) {
        (Some(a), Some(b)) => Some(if at(&a) <= at(&b) { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// The stricter of two `@timed` scopes: `off`, then the shorter.
fn stricter_timed(a: Option<String>, b: Option<String>) -> Option<String> {
    let rank = |w: &String| match TIMEDS.iter().position(|t| t == w).unwrap_or(0) {
        2 => 0,
        0 => 1,
        _ => 2,
    };
    match (a, b) {
        (Some(a), Some(b)) => Some(if rank(&a) <= rank(&b) { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// **The stricter of the decisions** (deny over ask over allow, by what is enforced): the first of
/// the strictest, so the project's own side names the rules where it ties. The grant scopes it
/// offers are the narrowest any side that decided as strictly gives.
fn stricter(decisions: Vec<Decision>) -> Decision {
    let top = decisions
        .iter()
        .map(|d| d.after_mode)
        .max()
        .expect("a side to decide");
    let mut keep = None;
    let mut timed = None;
    let mut chosen: Option<Decision> = None;
    for decision in decisions.into_iter().filter(|d| d.after_mode == top) {
        keep = narrower_keep(keep, decision.keep.clone());
        timed = stricter_timed(timed, decision.timed.clone());
        chosen.get_or_insert(decision);
    }
    let mut chosen = chosen.expect("a side that decided at the top");
    chosen.keep = keep;
    chosen.timed = timed;
    chosen
}

/// **One decision by the policy in force, with what the app saw sealed on the way** ([`judge`],
/// then [`seal`]): what the session host asks for every request of a chat.
pub fn in_force(
    root: &Path,
    sources: &Sources<'_>,
    standing: &Standing,
    request: &Request<'_>,
) -> Decision {
    let (decision, seen) = judge(root, sources, standing, request);
    if let Some(seen) = seen {
        seal(root, &seen);
    }
    decision
}

/// **One decision by the policy in force, and the version the app saw that it should seal**
/// ([`seal`]): the project as committed now and the local file, the accepted copy (the shipped
/// default until there is one), and every version of the accepted floor, the stricter winning.
pub fn judge(
    root: &Path,
    sources: &Sources<'_>,
    standing: &Standing,
    request: &Request<'_>,
) -> (Decision, Option<Seen>) {
    let current = levels::load(sources);
    let here = Version::at_head(root);
    let none = Version::default();
    let world = world_of(&current, here.as_ref().unwrap_or(&none), root, request);
    let mut decisions = vec![decide(&current, request, &world)];

    let accepted = accepted_in(standing);
    match &accepted {
        Ok(version) => decisions.push(side(
            sources,
            version.as_ref().unwrap_or(&none),
            root,
            request,
            false,
        )),
        Err(()) => decisions.push(side(sources, &none, root, request, true)),
    }
    let mut seen: Vec<Version> = Vec::new();
    for held in floor_in(standing) {
        match held {
            Ok(version) if here.as_ref() == Some(&version) || seen.contains(&version) => {}
            Ok(version) => {
                decisions.push(side(sources, &version, root, request, false));
                seen.push(version);
            }
            Err(()) => decisions.push(side(sources, &none, root, request, true)),
        }
    }

    let local_tightens = !current.tightenings.is_empty();
    let to_seal = here.filter(|version| {
        sealable(standing)
            && current.sandbox_at_least.is_none()
            && is_new(version, standing, &accepted, local_tightens)
    });
    (
        stricter(decisions),
        to_seal.map(|version| Seen {
            version,
            local_tightens,
        }),
    )
}

/// **A version the app saw, to seal** ([`seal`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// The project as committed now and the local file.
    pub version: Version,
    /// Whether the local file tightens something in it.
    local_tightens: bool,
}

/// Whether the plane's answers are in a state that takes a floor: a fresh install or a store that
/// holds. Never one in the deny state, which refuses every write, nor a process that cannot seal.
fn sealable(standing: &Standing) -> bool {
    matches!(standing.state, State::Fresh | State::Holds)
}

/// Whether `version` is one to add to the floor: not the accepted copy nor one held already, and
/// either a committed version none holds, or the local file tightens.
fn is_new(
    version: &Version,
    standing: &Standing,
    accepted: &Result<Option<Version>, ()>,
    local_tightens: bool,
) -> bool {
    let base = accepted
        .as_ref()
        .ok()
        .cloned()
        .flatten()
        .unwrap_or_default();
    let floor: Vec<Version> = floor_in(standing).into_iter().flatten().collect();
    if floor.contains(version) || *version == base {
        return false;
    }
    let committed = version.committed();
    let committed_is_new =
        committed != base.committed() && !floor.iter().any(|v| v.committed() == committed);
    committed_is_new || local_tightens
}

/// **The accepted floor's cap**: `trust.accepted-floor-max-versions`'s shipped value, from the
/// limits key table, until slice 12 reads the limit from the policy.
pub fn cap() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        let doc = super::LIMITS
            .parse::<toml_edit::DocumentMut>()
            .expect("the limits key table is TOML");
        doc["limit"]
            .as_array_of_tables()
            .into_iter()
            .flat_map(|tables| tables.iter())
            .find(|t| {
                t.get("key").and_then(|k| k.as_str()) == Some("trust.accepted-floor-max-versions")
            })
            .and_then(|t| t.get("shipped")?.as_integer())
            .and_then(|n| usize::try_from(n).ok())
            .expect("the cap has a shipped value")
    })
}

/// How many committed versions the floor holds, **not** counting the accepted copy's own nor the
/// local file's sealed tightenings (review round 8).
fn counted(standing: &Standing, accepted: &Result<Option<Version>, ()>) -> BTreeSet<String> {
    let base = accepted
        .as_ref()
        .ok()
        .cloned()
        .flatten()
        .unwrap_or_default();
    floor_in(standing)
        .into_iter()
        .flatten()
        .map(|v| v.committed())
        .filter(|v| *v != base.committed())
        .map(|v| serde_json::to_string(&v).expect("a version is JSON"))
        .collect()
}

/// **Seals a version the app saw into the accepted floor**, with no press, since it only narrows;
/// or, where it would be one committed version past the cap, records that instead, which is the
/// deny state until the person's press. Best effort: a store that cannot be written (no keyring,
/// not the app) changes nothing, and what was seen is judged all the same.
pub fn seal(root: &Path, seen: &Seen) {
    // What the store holds now, read again: the caller's standing may be older, or no read of the
    // store at all (a `start` is judged with none).
    let standing = &answers::standing(root);
    let version = &seen.version;
    let accepted = accepted_in(standing);
    if !sealable(standing) || !is_new(version, standing, &accepted, seen.local_tightens) {
        return;
    }
    let mut committed = counted(standing, &accepted);
    let base = accepted
        .as_ref()
        .ok()
        .cloned()
        .flatten()
        .unwrap_or_default();
    let past = {
        let own = version.committed();
        if own != base.committed() {
            committed.insert(serde_json::to_string(&own).expect("a version is JSON"));
        }
        committed.len() > cap()
    };
    let text = serde_json::to_string(version).expect("a version is JSON");
    let sealed = answers::commit(root, |draft| {
        if past {
            draft.add_narrowing(
                answers::PAST_THE_CAP,
                "a commit went past the accepted floor's cap",
            );
        } else {
            draft.add_narrowing(answers::FLOOR, &text);
        }
    });
    if let Err(why) = sealed {
        tracing::debug!("purlis: a policy version was not sealed into the accepted floor ({why})");
    }
}

#[cfg(test)]
#[path = "force_tests.rs"]
mod tests;
