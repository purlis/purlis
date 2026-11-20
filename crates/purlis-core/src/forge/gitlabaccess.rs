// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **GitLab access: a token in a persona's vault, and the `glab` tool that hands it on** (#1349).
//!
//! A chat whose sandbox holds the person's own credentials back
//! ([`crate::sandbox::Class::Credentials`]) cannot read `~/.config/glab-cli/config.yml`, and
//! never falls back to it. GitLab has no App to mint a token from (GitHub's is
//! [`super::agentidentity`]), so the route is the one every declared tool takes
//! ([`crate::sandbox::tools`]): a token the person keeps in the persona's vault, and a `glab` tool
//! line that names it:
//!
//! ```toml
//! [sandbox.personas.dev.tools]
//! glab = { vault = "dev", as = "GITLAB_TOKEN env", key = "gitlab-token-gitlab.example.com", host = "gitlab.example.com" }
//! ```
//!
//! The chat's `glab` is a shim that runs `purlis secret exec` for it, so the program is handed
//! `GITLAB_TOKEN` and `GITLAB_HOST` and the chat never reads the vault
//! ([`crate::sandbox::tools::forge_env`]).
//!
//! **The token** is the person's to make and to scope: a project or group access token in the
//! Developer role, with `api` scope, is what acts as a bot of its own and can't touch what a
//! Developer can't. It is read from the person's own input and kept in the vault; it is never
//! printed, and never in an error.
//!
//! **The host is the clone's.** A host is taken only where one of the project's clones has a
//! remote on it that is a GitLab ([`hosts`]), so a self-hosted GitLab is the name its remote
//! carries, and the tool's `host` is that name. [`crate::forge::agentidentity::credentials_note`]
//! says it where a tool's `host` is another's.
//!
//! **Only what this flow wrote is acted on** ([`Held::ours`]): a `glab` line written by hand is
//! shown and never replaced or removed. Everything else is read fresh: the tool lines say which
//! hosts are set up ([`held`]), the vault says whether the token is there.

use std::path::Path;

use toml_edit::{DocumentMut, InlineTable};

use super::Kind;
use super::agentidentity::{UNPARSED, allow_as_written, child, read_manifest, rewrite_manifest};
use crate::sandbox::tools::host;
use crate::secrets::{Ctx, cmd};

/// How the token reaches `glab`: its variable, in the run's environment.
pub const VAR_AS: &str = "GITLAB_TOKEN env";

/// The tool the line declares.
pub const TOOL: &str = "glab";

/// The longest token taken. GitLab's are far shorter.
const MOST_TOKEN: usize = 512;

/// The key a host's token is kept under, in a persona's vault.
#[must_use]
pub fn key_of(host: &str) -> String {
    format!("gitlab-token-{host}")
}

/// **The GitLab hosts the project's clones have remotes on**, each once, sorted: the only hosts a
/// token is taken for. A self-hosted GitLab is on this list where the project declares it as one
/// (`[[forge]]`, `kind = "gitlab"`).
#[must_use]
pub fn hosts(root: &Path) -> Vec<String> {
    super::agentidentity::hosts_of_the_project(root)
        .into_iter()
        .filter(|found| found.kind == Some(Kind::GitLab))
        .map(|found| found.host)
        .collect()
}

/// **A `glab` tool line a persona declares**, as the project's file says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub persona: String,
    pub host: String,
    pub vault: String,
    pub key: String,
    /// How the token is handed on, as the line says it.
    pub handed: String,
}

impl Held {
    /// **Whether this flow made the line**: its key for the host, handed as a variable it reads.
    /// Another line is committed text naming any value, so purlis only shows it.
    #[must_use]
    pub fn ours(&self) -> bool {
        self.key == key_of(&self.host) && self.handed == VAR_AS
    }
}

/// The host a `glab` table reaches: its `host`, or GitLab's own.
fn host_of_line(tool: &dyn toml_edit::TableLike) -> String {
    tool.get("host")
        .and_then(toml_edit::Item::as_str)
        .map_or_else(
            || host::default_of(true).to_owned(),
            |host| host.trim().to_ascii_lowercase(),
        )
}

/// **Every persona's `glab` tool line in the project's file.** Nothing where there is no file; a
/// file that doesn't read is refused rather than read as no access.
pub fn held(root: &Path) -> Result<Vec<Held>, String> {
    let cfg: toml::Table = read_manifest(root)?
        .parse()
        .map_err(|_| UNPARSED.to_owned())?;
    let personas = cfg
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("personas"))
        .and_then(toml::Value::as_table);
    let mut found = Vec::new();
    for (persona, grants) in personas.into_iter().flatten() {
        let Some(tool) = grants
            .get("tools")
            .and_then(|tools| tools.get(TOOL))
            .and_then(toml::Value::as_table)
        else {
            continue;
        };
        let text = |key: &str| tool.get(key).and_then(toml::Value::as_str).map(str::trim);
        found.push(Held {
            persona: persona.clone(),
            host: text("host").map_or_else(
                || host::default_of(true).to_owned(),
                str::to_ascii_lowercase,
            ),
            vault: text("vault").unwrap_or_default().to_owned(),
            key: text("key").unwrap_or_default().to_owned(),
            handed: text("as").unwrap_or_default().to_owned(),
        });
    }
    Ok(found)
}

/// The value of a tool line for `host`, in `vault`. The one source of what is shown and written.
fn line_value(host: &str, vault: &str) -> InlineTable {
    let mut line = InlineTable::new();
    line.insert("vault", vault.into());
    line.insert("as", VAR_AS.into());
    line.insert("key", key_of(host).into());
    line.insert("host", host.into());
    line
}

/// **The exact lines setting up adds**, for each `(persona, vault)`.
#[must_use]
pub fn lines_shown(host: &str, personas: &[(String, String)]) -> String {
    personas
        .iter()
        .map(|(persona, vault)| {
            format!(
                "[sandbox.personas.{}.tools]\n{TOOL} = {}",
                toml_edit::Key::new(persona.as_str()).display_repr(),
                line_value(host, vault)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Whether the `glab` table `tool` is this flow's line for `host`.
fn is_ours(tool: &dyn toml_edit::TableLike, host: &str) -> bool {
    let text = |key: &str| tool.get(key).and_then(toml_edit::Item::as_str);
    text("key") == Some(key_of(host).as_str())
        && text("as") == Some(VAR_AS)
        && host_of_line(tool) == host
}

/// **The project file `text` with the `glab` line for `host` added for each of `personas`**
/// (a persona and the vault its token is in), all else as it was. A line of this flow's own is
/// written again; a persona whose `glab` is anything else is refused by name: setting up replaces
/// nothing the person didn't see.
pub fn with_lines(text: &str, host: &str, personas: &[(String, String)]) -> Result<String, String> {
    let mut doc: DocumentMut = text.parse().map_err(|_| UNPARSED.to_owned())?;
    for (persona, vault) in personas {
        let sandbox = child(doc.as_item_mut(), "sandbox")?;
        let all = child(sandbox, "personas")?;
        let grants = child(all, persona)?;
        let tools = child(grants, "tools")?;
        let table = tools.as_table_like_mut().ok_or_else(|| {
            "tools in the project's settings isn't a table. Fix it, then try again.".to_owned()
        })?;
        let line =
            || toml_edit::Item::Value(toml_edit::Value::InlineTable(line_value(host, vault)));
        match table.get(TOOL) {
            None => {
                table.insert(TOOL, line());
            }
            Some(there)
                if there
                    .as_table_like()
                    .is_some_and(|tool| is_ours(tool, host)) =>
            {
                table.insert(TOOL, line());
            }
            Some(_) => {
                return Err(format!(
                    "{} already has a glab tool of its own, so purlis changed nothing. Remove \
                     that line first, or set glab up in your own terminal.",
                    crate::shown::short(persona)
                ));
            }
        }
    }
    Ok(doc.to_string())
}

/// **The project file `text` with this flow's `glab` lines for `host` taken out**
/// ([`Held::ours`]), all else as it was. A hand-written line stays.
pub fn without_lines(text: &str, host: &str) -> Result<String, String> {
    let mut doc: DocumentMut = text.parse().map_err(|_| UNPARSED.to_owned())?;
    let personas = doc
        .get_mut("sandbox")
        .and_then(|sandbox| sandbox.get_mut("personas"))
        .and_then(toml_edit::Item::as_table_like_mut);
    if let Some(personas) = personas {
        for (_, grants) in personas.iter_mut() {
            let Some(grants) = grants.as_table_like_mut() else {
                continue;
            };
            let Some(tools) = grants
                .get_mut("tools")
                .and_then(toml_edit::Item::as_table_like_mut)
            else {
                continue;
            };
            let ours = tools
                .get(TOOL)
                .and_then(toml_edit::Item::as_table_like)
                .is_some_and(|tool| is_ours(tool, host));
            if ours {
                tools.remove(TOOL);
                if tools.is_empty() {
                    grants.remove("tools");
                }
            }
        }
    }
    Ok(doc.to_string())
}

/// **A token as it may be kept**: one word of printable characters, not empty, not longer than
/// GitLab's tokens are. Its text is never in the refusal.
pub fn token_ok(token: &str) -> Result<(), String> {
    let word = !token.is_empty()
        && token.len() <= MOST_TOKEN
        && token.chars().all(|c| c.is_ascii_graphic());
    if word {
        Ok(())
    } else {
        Err(
            "That isn't a GitLab token. Paste the token alone, with no spaces or line breaks."
                .to_owned(),
        )
    }
}

/// **A host as it may be set up**: a GitLab host one of the project's clones has a remote on
/// ([`hosts`]), so the tool's `host` is the remote's name. Answers it lower-cased.
pub fn host_in_project(root: &Path, host: &str) -> Result<String, String> {
    let wanted = host.trim().to_ascii_lowercase();
    let known = hosts(root);
    if known.contains(&wanted) {
        return Ok(wanted);
    }
    let said = if known.is_empty() {
        "None of the project's clones has a GitLab remote.".to_owned()
    } else {
        format!("The project's GitLab remotes are on {}.", known.join(", "))
    };
    Err(format!(
        "{wanted} isn't a GitLab host the project's clones are on, so purlis set nothing up. {said} \
         A self-hosted GitLab is declared with [[forge]] kind = \"gitlab\" in the project's \
         settings."
    ))
}

/// What records the person's Allow of what was shown, where a caller can.
pub type Allow<'a> = dyn FnMut(&crate::sandbox::persona::Shown) -> Result<(), String> + 'a;

/// What setting up did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetUp {
    /// The personas whose Allow stands for the person to give, because they have more waiting
    /// than this line, or are the default persona.
    pub left: Vec<String>,
}

/// **Sets GitLab access up for `host`**: the token in each of `personas`' vaults, its line in each
/// one's tools, and the Allow where what is shown is all that waits ([`allow_as_written`]).
/// The Allow is the app's to record on this machine, so a caller with none (the command line)
/// passes `None`, and every persona is left for the person's own Allow.
///
/// Everything that can refuse does so first, before a token is stored: the host, the token, each
/// persona's vault and a hand-written `glab` of a persona's own. `allow` is called for a persona
/// only where its whole list was already allowed, or it had none.
pub fn set_up(
    ctx: &Ctx,
    root: &Path,
    host: &str,
    token: &str,
    personas: &[String],
    allow: Option<&mut Allow<'_>>,
) -> Result<SetUp, String> {
    let host = host_in_project(root, host)?;
    token_ok(token)?;
    if personas.is_empty() {
        return Err("Choose a persona to give the token to.".to_owned());
    }
    let vaults = vaults_of(ctx, personas)?;
    with_lines(&read_manifest(root)?, &host, &vaults)?;
    for (persona, vault) in &vaults {
        let vault = cmd::provider(ctx, vault).map_err(|e| e.message.to_string())?;
        cmd::set_value(ctx, &vault, &key_of(&host), token).map_err(|e| {
            format!(
                "purlis couldn't keep the token for {}: {}. Nothing was declared.",
                crate::shown::short(persona),
                e.message
            )
        })?;
    }
    let (read, written) = rewrite_manifest(root, |text| with_lines(text, &host, &vaults))
        .map_err(|why| format!("The token is kept, and its glab line wasn't written: {why}"))?;
    let left = match allow {
        Some(allow) => allow_as_written(root, TOOL, personas, &read, &written, allow)?,
        None => personas.to_vec(),
    };
    Ok(SetUp { left })
}

/// Each of `personas` with the vault its secrets live in, or why one has none.
fn vaults_of(ctx: &Ctx, personas: &[String]) -> Result<Vec<(String, String)>, String> {
    personas
        .iter()
        .map(|persona| cmd::persona_vault(ctx, persona).map(|vault| (persona.clone(), vault)))
        .collect()
}

/// **Takes GitLab access for `host` away**: only this flow's lines and keys ([`Held::ours`]). A
/// hand-written line stays, and so does a key nothing of ours names. Where nothing is ours, it
/// is refused, so pressing it never touches what a hand-written line runs on.
pub fn remove(ctx: &Ctx, root: &Path, host: &str) -> Result<(), String> {
    let host = host.trim().to_ascii_lowercase();
    let ours: Vec<Held> = held(root)?
        .into_iter()
        .filter(|one| one.host == host && one.ours())
        .collect();
    if ours.is_empty() {
        return Err(
            "purlis didn't write a glab line for that host, so it removes nothing here. \
                    Take it out of your project's settings."
                .to_owned(),
        );
    }
    // The lines first, so no chat is started with a tool whose token is gone.
    rewrite_manifest(root, |text| without_lines(text, &host)).map(drop)?;
    let mut failed = None;
    for one in &ours {
        let gone = cmd::provider(ctx, &one.vault)
            .and_then(|vault| cmd::delete(ctx, &vault, &one.key))
            .map_err(|e| e.message.to_string());
        if let Err(why) = gone {
            failed.get_or_insert(why);
        }
    }
    match failed {
        None => Ok(()),
        Some(why) => Err(format!(
            "The lines are gone, and not every token was removed: {why}"
        )),
    }
}

#[cfg(test)]
mod tests;
