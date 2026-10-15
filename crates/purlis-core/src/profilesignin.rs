// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a profile of this machine's signs in** (ADR 0095 §4, ADR 0087 as amended by #2036).
//!
//! A profile's record in `<config>/profiles.json` ([`crate::profilestore`]) may carry its
//! sign-in: its kind (one of the four its harness declares, ADR 0073 `[sign_in] kinds`), a
//! label the picker shows it is billed to, and, for an `api-key` or `cloud` profile, its
//! credentials: **each variable its harness is handed, by vault reference, never a value.**
//!
//! Which variables a harness is handed for a kind is adapter code, not a declaration's word
//! ([`handed`]): a declaration may not name a variable that reads as a credential (ADR 0073 §5,
//! ADR 0087's ADR 0073 amendment), so only a harness purlis ships is handed any. Each list is
//! the harness's own documented variables for that sign-in.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::harness_declaration::SignInKind;

/// A profile's sign-in, as its record keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignIn {
    /// One of [`SignInKind`]'s words.
    pub kind: String,
    /// What the picker says the profile is billed to: an account's or a key's name. None: the
    /// profile's own name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// For `api-key` and `cloud`: each variable the harness is handed, and the vault reference
    /// (`vault:<vault>/<key>`) its value is read from at a chat's start.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub credentials: BTreeMap<String, String>,
    /// Every field this build does not know, written back as it was read.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// A vault reference: the vault and the key in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub vault: String,
    pub key: String,
}

impl Reference {
    /// `vault:<vault>/<key>`, each a name a vault and a key may have; `None` for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let (vault, key) = text.strip_prefix("vault:")?.split_once('/')?;
        let key_ok = !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            && !key.starts_with(['.', '-']);
        (crate::secrets::registry::name_ok(vault) && key_ok).then(|| Self {
            vault: vault.to_owned(),
            key: key.to_owned(),
        })
    }
}

impl SignIn {
    /// Its kind, where it is one of the four words.
    pub fn kind(&self) -> Option<SignInKind> {
        SignInKind::ALL
            .into_iter()
            .find(|kind| kind.word() == self.kind)
    }

    /// Why a profile of `harness` cannot sign in so, or `Ok` where it can: a kind its harness
    /// declares; for `api-key` and `cloud`, at least one credential, each a variable the
    /// harness is handed for that kind and a vault reference; for the others, none. A refusal
    /// names a variable, never what was written for it.
    pub fn check(&self, harness: &str) -> Result<(), String> {
        let Some(declaration) = crate::harness_declaration::builtin(harness) else {
            return Err(format!(
                "purlis signs in only a profile of a harness it ships, and '{}' is not one",
                crate::shown::short(harness)
            ));
        };
        let Some(kind) = self
            .kind()
            .filter(|kind| declaration.sign_in.kinds.contains(kind))
        else {
            return Err(format!(
                "{} signs in with {}, and not with '{}'",
                declaration.title,
                declaration
                    .sign_in
                    .kinds
                    .iter()
                    .map(|k| k.word())
                    .collect::<Vec<_>>()
                    .join(", "),
                crate::shown::short(&self.kind)
            ));
        };
        let names_credentials = matches!(kind, SignInKind::ApiKey | SignInKind::Cloud);
        if !names_credentials {
            return match self.credentials.keys().next() {
                Some(variable) => Err(format!(
                    "a {} profile names no vault entry, and this one names one for {}",
                    kind.word(),
                    crate::shown::short(variable)
                )),
                None => Ok(()),
            };
        }
        if self.credentials.is_empty() {
            return Err(format!(
                "a {} profile names its credentials by vault reference, and this one names none",
                kind.word()
            ));
        }
        let handed = handed(harness, kind);
        for (variable, reference) in &self.credentials {
            if !handed.contains(&variable.as_str()) {
                return Err(format!(
                    "{} is not handed {} for a {} profile; it is handed {}",
                    declaration.title,
                    crate::shown::short(variable),
                    kind.word(),
                    handed.join(", ")
                ));
            }
            if Reference::parse(reference).is_none() {
                return Err(format!(
                    "{} is named by a vault reference, vault:<vault>/<key>, and what is written \
                     for it is not one",
                    crate::shown::short(variable)
                ));
            }
        }
        Ok(())
    }
}

/// The variables a profile of `harness` is handed for `kind`: the harness's own documented
/// variables for that sign-in. None for a kind that names no credential, and none for a harness
/// purlis does not ship.
pub fn handed(harness: &str, kind: SignInKind) -> &'static [&'static str] {
    match (harness, kind) {
        ("claude", SignInKind::ApiKey) => &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"],
        // Bedrock, Vertex and Microsoft Foundry, as Claude Code's own documentation sets each.
        ("claude", SignInKind::Cloud) => &[
            "CLAUDE_CODE_USE_BEDROCK",
            "AWS_REGION",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_BEARER_TOKEN_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "CLOUD_ML_REGION",
            "ANTHROPIC_VERTEX_PROJECT_ID",
            "CLAUDE_CODE_USE_FOUNDRY",
            "ANTHROPIC_FOUNDRY_RESOURCE",
            "ANTHROPIC_FOUNDRY_API_KEY",
        ],
        ("codex", SignInKind::ApiKey) => &["OPENAI_API_KEY", "CODEX_API_KEY"],
        ("codex", SignInKind::Cloud) => &["AZURE_OPENAI_API_KEY"],
        ("opencode", SignInKind::ApiKey) => &[
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "OPENROUTER_API_KEY",
            "GEMINI_API_KEY",
            "GOOGLE_GENERATIVE_AI_API_KEY",
        ],
        ("opencode", SignInKind::Cloud) => &[
            "AWS_REGION",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_BEARER_TOKEN_BEDROCK",
            "GOOGLE_CLOUD_PROJECT",
            "VERTEX_LOCATION",
            "AZURE_RESOURCE_NAME",
            "AZURE_API_KEY",
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests;

/// **What a chat's harness is handed at its start**: each variable and its value, read from the
/// vault as the chat starts. It lives only in memory, from the read to the program's start: it
/// is never written, never logged, and its `Debug` says the names alone. Default: nothing,
/// which is what every chat but one on an `api-key` or `cloud` profile is handed, a shell tab's
/// among them.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Handed {
    pairs: Vec<(String, String)>,
}

impl Handed {
    /// Each variable and its value, as a caller that read them hands them on.
    pub fn of(pairs: Vec<(String, String)>) -> Self {
        Self { pairs }
    }

    /// The variables it sets, never a value.
    pub fn names(&self) -> Vec<&str> {
        self.pairs.iter().map(|(name, _)| name.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl std::fmt::Debug for Handed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handed")
            .field("names", &self.names())
            .finish_non_exhaustive()
    }
}

/// **Reads each credential a chat's profile names**, from the vaults of the project `ctx` is
/// for, as the chat starts. A vault opens only as it opens to the chat's `persona` today
/// ([`crate::secrets::brokered::authorise`]: tagged for it, or allowed for it on this machine),
/// so a profile never widens who reads a vault: a chat on no persona, or on one the vault is not
/// open to, is handed nothing and does not start, and the refusal says which vault and how to
/// open it. A missing entry says which variable it was for. Each read is recorded by name
/// (`profile-sign-in`), never by value.
pub fn read(
    ctx: &crate::secrets::Ctx,
    persona: Option<&str>,
    credentials: &[(String, Reference)],
) -> Result<Handed, String> {
    use crate::secrets::{brokered, cmd};
    let mut pairs = Vec::with_capacity(credentials.len());
    let mut resolved = Vec::with_capacity(credentials.len());
    for (variable, reference) in credentials {
        brokered::authorise(ctx, persona, &reference.vault).map_err(|why| {
            format!(
                "this chat's profile names {variable} in vault '{}': {why}",
                reference.vault
            )
        })?;
        let value = cmd::provider(ctx, &reference.vault)
            .and_then(|vault| cmd::get_value(ctx, &vault, &reference.key))
            .map_err(|e| {
                format!(
                    "this chat's profile names {variable} as '{}' in vault '{}', and it could \
                     not be read ({}), so nothing was started",
                    crate::shown::short(&reference.key),
                    reference.vault,
                    e.message
                )
            })?;
        pairs.push((variable.clone(), value));
        resolved.push(format!("{}/{}", reference.vault, reference.key));
    }
    // The values are what the record is scrubbed of; it names the entries.
    let values: Vec<String> = pairs.iter().map(|(_, value)| value.clone()).collect();
    cmd::trace_secret_use(
        ctx,
        PROFILE_SIGN_IN,
        &values,
        &[
            ("persona", serde_json::json!(persona)),
            ("vaults", serde_json::json!(resolved)),
        ],
    );
    Ok(Handed { pairs })
}

/// The event a profile's read at a chat's start is recorded under.
pub const PROFILE_SIGN_IN: &str = "profile-sign-in";

/// `env`, the environment a chat's program starts from, with what it is `handed` set last: a
/// handed variable replaces one of the same name the chat would otherwise have, and nothing
/// else moves. Applied where the program is spawned, after everything else is settled, so the
/// values never pass through what a start records or shows.
pub fn handed_onto(
    mut env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    handed: &Handed,
) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    if handed.is_empty() {
        return env;
    }
    env.retain(|(name, _)| !handed.pairs.iter().any(|(set, _)| name == set.as_str()));
    env.extend(
        handed
            .pairs
            .iter()
            .map(|(name, value)| (name.into(), value.into())),
    );
    env
}

/// **How a chat on a profile signs in, as it starts**: its kind, what it is billed to, and for
/// a key or a cloud the vault reference of each variable its harness is handed. References
/// only: the values are read where the program is spawned ([`read`]), never here, so nothing
/// that records or shows a start can hold one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtStart {
    pub kind: SignInKind,
    /// What the picker and the chat say it is billed to ([`billed_to`]).
    pub billed_to: String,
    /// Each variable and where its value is read from. Empty but for `api-key` and `cloud`.
    pub credentials: Vec<(String, Reference)>,
}

/// What a chat's model is said as until a model can be chosen (ADR 0087 §1 and §6).
pub const THE_HARNESS_DEFAULT: &str = "the harness's default";

/// What a chat on `profile` is billed to, in words: its sign-in's kind (`kind`, `None` where
/// purlis cannot see it) and its `label`, the profile's name where it has none. `title` is the
/// harness as people call it.
pub fn billed_to(kind: Option<SignInKind>, label: &str, title: &str) -> String {
    let whose = match kind {
        Some(SignInKind::Subscription) => "your plan".to_owned(),
        Some(SignInKind::ApiKey) => "your key".to_owned(),
        Some(SignInKind::Cloud) => "your cloud account".to_owned(),
        Some(SignInKind::Local) => "nobody, a model on this machine".to_owned(),
        None => format!("whatever {title} is signed in with"),
    };
    format!("{whose} · {}", crate::shown::short(label))
}

/// What a chat on `profile` is billed to, as the picker shows it beside every choice: from
/// this machine's records (`machine`) and the variables `[chat_env] pass` lists (`passed`), as
/// [`at_start`] reads them. A row whose start would be refused says why in a few words, and a
/// row whose credentials go into the harness's environment says that the commands it runs can
/// read them (ADR 0087 §7). `None` for a harness purlis does not ship, whose sign-in purlis
/// knows nothing of.
pub fn billed(
    profile: &crate::profiles::Profile,
    machine: &[crate::profilestore::MachineProfile],
    passed: &[String],
) -> Option<String> {
    billed_in(profile, machine, passed, CLOUD_IS_HANDED)
}

/// [`billed`], with whether a cloud's credentials are handed (`cloud_handed`, [`handing`]).
fn billed_in(
    profile: &crate::profiles::Profile,
    machine: &[crate::profilestore::MachineProfile],
    passed: &[String],
    cloud_handed: bool,
) -> Option<String> {
    let seen = Seen::of(profile, machine, passed)?;
    let said = billed_to(seen.kind, &seen.label, &seen.declaration.title);
    let Some(kept) = &seen.kept else {
        return Some(said);
    };
    if kept.check(&profile.kind).is_err() {
        return Some(format!("{said} · {CANNOT_SIGN_IN_AS_KEPT}"));
    }
    let credentials = references(kept);
    let (Some(kind), false) = (kept.kind(), credentials.is_empty()) else {
        return Some(said);
    };
    Some(
        match handing(&profile.kind, kind, &credentials, cloud_handed) {
            Handing::Environment => format!("{said} · {READABLE_BY_ITS_COMMANDS}"),
            Handing::Refused { row, .. } => format!("{said} · {row}"),
        },
    )
}

/// What a key profile's row and refusal say until the key proxy is there (#822).
pub const WAITS_FOR_THE_PROXY: &str = "waits for the key proxy";

/// What a row says where its credentials go into the harness's environment (ADR 0087 §7).
pub const READABLE_BY_ITS_COMMANDS: &str = "the commands it runs can read these credentials";

/// What a row says where its kept sign-in is one its harness cannot take.
pub const CANNOT_SIGN_IN_AS_KEPT: &str = "cannot sign in as it is kept";

/// What purlis sees of how a chat on a profile signs in.
struct Seen {
    declaration: &'static crate::harness_declaration::Declaration,
    /// The record's sign-in, where it keeps one.
    kept: Option<SignIn>,
    /// Its kind: the record's, else what the profile's variables show, else not known.
    kind: Option<SignInKind>,
    /// The record's label, else the profile's name.
    label: String,
}

impl Seen {
    fn of(
        profile: &crate::profiles::Profile,
        machine: &[crate::profilestore::MachineProfile],
        passed: &[String],
    ) -> Option<Self> {
        let declaration = crate::harness_declaration::builtin(&profile.kind)?;
        let kept = record_of(profile, machine).and_then(|record| record.sign_in);
        let label = kept
            .as_ref()
            .and_then(|s| s.label.clone())
            .unwrap_or_else(|| profile.name.clone());
        let kind = match &kept {
            Some(kept) => kept.kind(),
            None => declaration.profile_sign_in(&profile.env, passed),
        };
        Some(Self {
            declaration,
            kept,
            kind,
            label,
        })
    }
}

/// Each credential `kept` names that reads as a vault reference.
fn references(kept: &SignIn) -> Vec<(String, Reference)> {
    kept.credentials
        .iter()
        .filter_map(|(variable, reference)| Some((variable.clone(), Reference::parse(reference)?)))
        .collect()
}

/// The record of this machine's that `profile` runs as, where one does: a profile purlis made,
/// or a built-in whose folder was adopted under its name. A profile the project declares under
/// that name is the project's, and has none.
fn record_of(
    profile: &crate::profiles::Profile,
    machine: &[crate::profilestore::MachineProfile],
) -> Option<crate::profilestore::MachineProfile> {
    use crate::profiles::Source;
    let mine = match profile.source {
        Source::Machine => true,
        Source::BuiltIn => true,
        Source::Local | Source::Declared => false,
    };
    mine.then(|| machine.iter().find(|r| r.name == profile.name).cloned())
        .flatten()
}

/// How a chat on `profile` signs in, as the start sees it, from this machine's records
/// (`machine`) and the variables `[chat_env] pass` lists (`passed`): the record's sign-in where
/// it has one, else what purlis can see of it
/// ([`crate::harness_declaration::Declaration::profile_sign_in`]), else nothing known. `Err`
/// where the record's sign-in is one its harness cannot take, and where its credentials are
/// not handed to a chat ([`handing`]): the chat does not start, and says why.
pub fn at_start(
    profile: &crate::profiles::Profile,
    machine: &[crate::profilestore::MachineProfile],
    passed: &[String],
) -> Result<Option<AtStart>, String> {
    at_start_in(profile, machine, passed, CLOUD_IS_HANDED)
}

/// [`at_start`], with whether a cloud's credentials are handed (`cloud_handed`, [`handing`]).
fn at_start_in(
    profile: &crate::profiles::Profile,
    machine: &[crate::profilestore::MachineProfile],
    passed: &[String],
    cloud_handed: bool,
) -> Result<Option<AtStart>, String> {
    let Some(seen) = Seen::of(profile, machine, passed) else {
        return Ok(None);
    };
    let billed = |kind| billed_to(Some(kind), &seen.label, &seen.declaration.title);
    let Some(kept) = &seen.kept else {
        return Ok(seen.kind.map(|kind| AtStart {
            kind,
            billed_to: billed(kind),
            credentials: Vec::new(),
        }));
    };
    kept.check(&profile.kind).map_err(|why| {
        format!(
            "profile '{}' {CANNOT_SIGN_IN_AS_KEPT}: {why}. Nothing was started.",
            crate::shown::short(&profile.name)
        )
    })?;
    // Checked: a kind of the four, and each credential a reference.
    let Some(kind) = kept.kind() else {
        return Ok(None);
    };
    let credentials = references(kept);
    if !credentials.is_empty()
        && let Handing::Refused { why, .. } =
            handing(&profile.kind, kind, &credentials, cloud_handed)
    {
        return Err(format!(
            "profile '{}' signs in with {why}. Nothing was started.",
            crate::shown::short(&profile.name)
        ));
    }
    Ok(Some(AtStart {
        kind,
        billed_to: billed(kind),
        credentials,
    }))
}

/// **Whether this version hands a cloud's credentials to its chat's harness.** The operator
/// ruled on 2026-10-15 (#2151) that a cloud's credentials go into the harness's process
/// environment, and a key waits for the key proxy; this version does not open the cloud path,
/// and #2151 is where it opens.
const CLOUD_IS_HANDED: bool = false;

/// How a sign-in's credentials reach its chat's harness.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Handing {
    /// Set on the harness's program as it starts. The commands, hooks and tools it starts
    /// inherit them.
    Environment,
    /// Not handed: the chat does not start. `row` is what the picker says, `why` the refusal.
    Refused { row: &'static str, why: String },
}

/// How the `credentials` of a `kind` sign-in reach a `harness` chat, as the operator ruled on
/// 2026-10-15 (#2151, amending how ADR 0095 §4 applies ADR 0087 §7), with `cloud_handed`
/// whether this version opens the cloud path ([`CLOUD_IS_HANDED`]):
///
/// - **A cloud's credentials go into the harness's process environment.** A cloud signs its
///   requests in ways a proxy that swaps one header cannot carry. Only where variables alone
///   sign the harness in: Vertex signs in with a credentials file, which purlis does not write
///   for a chat, and Codex reaches Azure through a provider set in its configuration, so both
///   are refused in words.
/// - **A key waits for the key proxy** (#822, ruling V36), wherever the harness takes a base
///   URL, which every harness purlis ships does: the chat does not start, and says so.
fn handing(
    harness: &str,
    kind: SignInKind,
    credentials: &[(String, Reference)],
    cloud_handed: bool,
) -> Handing {
    let title = crate::harness_declaration::builtin(harness)
        .map_or_else(|| harness.to_owned(), |d| d.title.clone());
    let names = |wanted: &[&str]| {
        credentials
            .iter()
            .any(|(v, _)| wanted.contains(&v.as_str()))
    };
    let vertex = names(&[
        "CLAUDE_CODE_USE_VERTEX",
        "CLOUD_ML_REGION",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "GOOGLE_CLOUD_PROJECT",
        "VERTEX_LOCATION",
    ]);
    match kind {
        SignInKind::Cloud if harness == "codex" => Handing::Refused {
            row: "Codex on Azure is not signed in from a vault",
            why: format!(
                "cloud credentials from your vault, and {title} reaches Azure through a \
                 provider set in its own configuration, which purlis does not write for a chat \
                 (#2151)"
            ),
        },
        SignInKind::Cloud if vertex => Handing::Refused {
            row: "Vertex is not signed in from a vault",
            why: format!(
                "Vertex credentials from your vault, and {title} signs in to Vertex with a \
                 credentials file, which purlis does not write for a chat (#2151)"
            ),
        },
        SignInKind::Cloud if !cloud_handed => Handing::Refused {
            row: "cloud credentials are not handed to a chat in this version",
            why: "cloud credentials from your vault, which this version of purlis does not \
                  hand to a chat (#2151)"
                .to_owned(),
        },
        SignInKind::Cloud => Handing::Environment,
        _ => Handing::Refused {
            row: WAITS_FOR_THE_PROXY,
            why: format!(
                "a key from your vault, and {title} is handed a key only through purlis's key \
                 proxy, which this version of purlis does not have, so a chat on it \
                 {WAITS_FOR_THE_PROXY} (#822)"
            ),
        },
    }
}
