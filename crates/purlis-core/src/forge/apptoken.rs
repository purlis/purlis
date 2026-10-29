// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own GitHub identity: an installation token the organisation's own GitHub App mints
//! for it** (#2053, SD-7a; ADR 0077 as amended 2026-10-29, E1 to E3).
//!
//! A persona's tool declared with `mint = "github-app"` ([`crate::sandbox::tools::mint`]) is
//! handed, at each run, a token minted here, in the app's process:
//!
//! 1. **the chat's repos** are the GitHub remotes of the clones in the chat's workspace, read at
//!    that run ([`remotes_of_workspace`], E3);
//! 2. the App's key, read from its vault by the mint alone, signs a JWT of ten minutes at most
//!    whose issuer is the App's client id ([`jwt`]), which is dropped once the token is minted;
//! 3. each repo's installation is asked for (`GET /repos/{owner}/{repo}/installation`), and the
//!    one the run's clone is on, or else the only one, mints a token limited to those of its
//!    repos (`POST /app/installations/{id}/access_tokens` with `repositories`, [`mint_over`]).
//!    It holds the permissions the organisation gave the App's installation.
//!
//! **One token per chat is kept, in memory alone**, until ten minutes before GitHub says it
//! expires ([`token_for`]): a chat runs `gh` many times an hour, and each run would otherwise cost
//! three requests. Nothing here is ever written to disk.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use secrecy::SecretString;
use zeroize::Zeroizing;

use super::http::{ApiRoot, Http, TokenSource};
use super::transport::{Call, Field, Method, Transport};
use super::{Forge, ForgeError, Kind};

/// How long a JWT is good for. GitHub takes at most ten minutes.
const JWT_LIVES: u64 = 9 * 60;

/// How far back a JWT's issue time is put, for a clock that runs a little ahead of GitHub's.
const JWT_BACKDATED: u64 = 60;

/// How long before it expires a kept token is minted again.
const MINTED_AGAIN_BEFORE: Duration = Duration::from_secs(10 * 60);

/// How long a kept token is kept where GitHub's answer says no expiry purlis reads: its hour,
/// less [`MINTED_AGAIN_BEFORE`].
const KEPT_AT_MOST: Duration = Duration::from_secs(50 * 60);

/// The most tokens kept at once, over every chat.
const MOST_KEPT: usize = 256;

/// **One GitHub repo a clone of the chat's workspace has as a remote.**
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Remote {
    /// The forge host it is on (`github.com`, or a GitHub Enterprise Server's).
    pub host: String,
    pub owner: String,
    pub name: String,
    /// The clone it is a remote of.
    pub clone: PathBuf,
}

/// Whether `part`, an owner's or a repo's name, is one GitHub would give: so it goes into an API
/// path as it is.
fn name_ok(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 100
        && part != "."
        && part != ".."
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// **The GitHub remotes `config` names for clone `clone`**: `config` is what
/// `git config --get-regexp ^remote\..*\.url$` prints, one `remote.<name>.url <url>` a line. A
/// remote is taken where `forge_of` says its URL is on a GitHub host and its path is exactly
/// `owner/repo`; any other is left out.
pub fn remotes_in(
    clone: &Path,
    config: &str,
    forge_of: &dyn Fn(&str) -> Option<Forge>,
) -> Vec<Remote> {
    let mut out: Vec<Remote> = config
        .lines()
        .filter_map(|line| line.split_once(' ').map(|(_, url)| url.trim()))
        .filter_map(|url| {
            let forge = forge_of(url).filter(|forge| forge.kind == Kind::GitHub)?;
            let path = super::namespace_of(url)?;
            let (owner, name) = path.split_once('/')?;
            (name_ok(owner) && name_ok(name)).then(|| Remote {
                host: forge.host,
                owner: owner.to_owned(),
                name: name.to_owned(),
                clone: clone.to_path_buf(),
            })
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// **The chat's repos** (E3): the GitHub remotes of every clone in the workspace that `folder`,
/// the chat's folder, is in, in the project at `root`, read now. Refused for a chat in no
/// workspace.
pub fn remotes_of_workspace(root: &Path, folder: Option<&Path>) -> Result<Vec<Remote>, String> {
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let Some(workspace) = folder.and_then(|folder| plane.workspace_of(folder)) else {
        return Err(
            "this chat works in no workspace, so it has no repos a GitHub App's token could be \
             minted for"
                .to_owned(),
        );
    };
    let clones = crate::repos::clones(root, &workspace).map_err(|e| e.to_string())?;
    let known = super::known(root);
    let forge_of = |url: &str| super::resolve_in(url, &known);
    let mut out = Vec::new();
    for repo in clones.repos {
        let Ok(run) = crate::worktree::git::run(
            &repo.path,
            &["config", "--get-regexp", r"^remote\..*\.url$"],
            crate::worktree::git::READ,
        ) else {
            continue;
        };
        out.extend(remotes_in(&repo.path, &run.out, &forge_of));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// The DER of the private key in `pem`, and whether it is PKCS#8 (`PRIVATE KEY`) rather than
/// PKCS#1 (`RSA PRIVATE KEY`), as GitHub hands an App's key out.
fn der_of(pem: &str) -> Result<(Zeroizing<Vec<u8>>, bool), String> {
    let not_a_key = || {
        "the GitHub App's key in the vault is not a private key as GitHub hands it out (PEM, \
         PKCS#1 or PKCS#8)"
            .to_owned()
    };
    for (label, pkcs8) in [("RSA PRIVATE KEY", false), ("PRIVATE KEY", true)] {
        let begin = format!("-----BEGIN {label}-----");
        let end = format!("-----END {label}-----");
        let Some(start) = pem.find(&begin) else {
            continue;
        };
        let body = &pem[start + begin.len()..];
        let Some(stop) = body.find(&end) else {
            return Err(not_a_key());
        };
        let text: Zeroizing<String> = Zeroizing::new(
            body[..stop]
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect(),
        );
        let der = base64::engine::general_purpose::STANDARD
            .decode(text.as_bytes())
            .map_err(|_| not_a_key())?;
        return Ok((Zeroizing::new(der), pkcs8));
    }
    Err(not_a_key())
}

/// **A JWT for GitHub App `app`** (its client id), signed RS256 with the App's private key `pem`,
/// issued a minute before `now` (seconds since 1970) and good for [`JWT_LIVES`]. What GitHub's
/// app endpoints take as a bearer token.
pub fn jwt(pem: &str, app: &str, now: u64) -> Result<Zeroizing<String>, String> {
    let (der, pkcs8) = der_of(pem)?;
    let pair = if pkcs8 {
        ring::signature::RsaKeyPair::from_pkcs8(&der)
    } else {
        ring::signature::RsaKeyPair::from_der(&der)
    }
    .map_err(|why| format!("the GitHub App's key in the vault was not taken ({why})"))?;
    drop(der);
    let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header = url.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = serde_json::json!({
        "iat": now.saturating_sub(JWT_BACKDATED),
        "exp": now + JWT_LIVES,
        "iss": app,
    });
    let signed = format!("{header}.{}", url.encode(claims.to_string().as_bytes()));
    let mut signature = vec![0u8; pair.public().modulus_len()];
    pair.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        signed.as_bytes(),
        &mut signature,
    )
    .map_err(|_| "the GitHub App's key could not sign a token request".to_owned())?;
    Ok(Zeroizing::new(format!(
        "{signed}.{}",
        url.encode(&signature)
    )))
}

/// **A token minted for a chat**: the token, when GitHub says it expires, the installation and
/// the repos it is limited to.
pub struct Minted {
    pub token: Zeroizing<String>,
    /// GitHub's `expires_at`, as it says it.
    pub expires_at: Option<String>,
    pub installation: u64,
    pub repos: Vec<String>,
}

impl std::fmt::Debug for Minted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Minted")
            .field("expires_at", &self.expires_at)
            .field("installation", &self.installation)
            .field("repos", &self.repos)
            .finish_non_exhaustive()
    }
}

/// Whether `reply` is GitHub's "not found": the App is not installed there.
fn not_found(reply: &super::transport::Reply) -> bool {
    reply.status == Some(404) || reply.err.contains("(HTTP 404)")
}

/// **The token `transport`'s App mints for `remotes`** (E3), all on one host, sent with the
/// App's JWT as its bearer: each repo's installation, then the installation the clone `here`
/// (the run's folder) is on, or else the only one, mints a token limited to those of its repos.
/// Refused where none is installed, or where they are on several and `here` is in none.
pub fn mint_over(
    transport: &dyn Transport,
    forge: &Forge,
    remotes: &[Remote],
    here: Option<&Path>,
) -> Result<Minted, String> {
    let mut installed: Vec<(u64, &Remote)> = Vec::new();
    let mut asked: Vec<(&str, &str)> = Vec::new();
    for remote in remotes.iter().filter(|remote| remote.host == forge.host) {
        if asked.contains(&(remote.owner.as_str(), remote.name.as_str())) {
            continue;
        }
        asked.push((remote.owner.as_str(), remote.name.as_str()));
        let call = Call::get(
            format!("repos/{}/{}/installation", remote.owner, remote.name),
            super::LIST_TIMEOUT,
        );
        let reply = transport.send(forge, &call).map_err(|e| {
            format!(
                "GitHub could not be asked for the App's installation ({})",
                e.said()
            )
        })?;
        if reply.code != 0 {
            if not_found(&reply) {
                continue;
            }
            return Err(format!(
                "GitHub refused the App's request for its installation on {}/{} ({})",
                remote.owner,
                remote.name,
                reply.err.trim()
            ));
        }
        let id = serde_json::from_str::<serde_json::Value>(&reply.out)
            .ok()
            .and_then(|answer| answer.get("id")?.as_u64())
            .ok_or_else(|| "GitHub's answer named no installation".to_owned())?;
        installed.push((id, remote));
    }
    let mut installations: Vec<u64> = installed.iter().map(|(id, _)| *id).collect();
    installations.sort_unstable();
    installations.dedup();
    let in_here = |remote: &Remote| {
        here.is_some_and(|here| {
            let at = remote
                .clone
                .canonicalize()
                .unwrap_or_else(|_| remote.clone.clone());
            let here = here.canonicalize().unwrap_or_else(|_| here.to_path_buf());
            here.starts_with(at)
        })
    };
    let installation = match installations.as_slice() {
        [] => {
            return Err(
                "the GitHub App is installed on none of this chat's repos, so no token was \
                 minted: install it on them, or work in a clone of a repo it is installed on"
                    .to_owned(),
            );
        }
        [one] => *one,
        _ => match installed.iter().find(|(_, remote)| in_here(*remote)) {
            Some((id, _)) => *id,
            None => {
                return Err(
                    "this chat's repos are on several installations of the GitHub App, so run \
                     the command in a clone of one of them"
                        .to_owned(),
                );
            }
        },
    };
    let mut repos: Vec<String> = installed
        .iter()
        .filter(|(id, _)| *id == installation)
        .map(|(_, remote)| remote.name.clone())
        .collect();
    repos.sort();
    repos.dedup();
    let call = Call::write(
        Method::Post,
        format!("app/installations/{installation}/access_tokens"),
        repos
            .iter()
            .map(|name| Field::text("repositories[]", name))
            .collect(),
    );
    let reply = transport
        .send(forge, &call)
        .map_err(|e| format!("GitHub could not be asked for a token ({})", e.said()))?;
    let out = Zeroizing::new(reply.out);
    if reply.code != 0 {
        return Err(format!(
            "GitHub refused to mint a token for the App's installation ({})",
            reply.err.trim()
        ));
    }
    let answer: serde_json::Value = serde_json::from_str(&out)
        .map_err(|_| "GitHub's answer to the token request was not JSON".to_owned())?;
    let token = answer
        .get("token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.is_empty())
        .map(|token| Zeroizing::new(token.to_owned()))
        .ok_or_else(|| "GitHub's answer to the token request held no token".to_owned())?;
    let expires_at = answer
        .get("expires_at")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    Ok(Minted {
        token,
        expires_at,
        installation,
        repos,
    })
}

/// A JWT, handed out as the bearer of each request of one mint.
struct Bearer(SecretString);

impl TokenSource for Bearer {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(self.0.clone())
    }
}

/// The native transport to `forge`'s API, its requests sent with `jwt`.
fn over_https(forge: &Forge, jwt: &str) -> Result<Http, String> {
    Http::new(
        Kind::GitHub,
        ApiRoot::github(&forge.host),
        Arc::new(Bearer(SecretString::from(jwt))),
    )
    .map_err(|e| e.to_string())
}

/// **The chat a token is minted for**: the project, the app's number for the chat, its folder
/// (which names its workspace) and the folder the command runs in.
#[derive(Debug, Clone, Copy)]
pub struct For<'a> {
    pub root: &'a Path,
    pub chat: u32,
    pub folder: Option<&'a Path>,
    pub here: Option<&'a Path>,
}

/// What a kept token was minted for: none of it is a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    root: PathBuf,
    chat: u32,
    app: String,
    vault: String,
    key: String,
    host: String,
    remotes: Vec<Remote>,
    here: Option<PathBuf>,
}

/// One token kept for its chat.
struct Kept {
    asked: Asked,
    token: Zeroizing<String>,
    until: Instant,
}

/// The tokens kept, over every chat.
static KEPT: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

/// How long a token minted now is kept: until [`MINTED_AGAIN_BEFORE`] before `expires_at`, and
/// at most [`KEPT_AT_MOST`].
fn kept_for(expires_at: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> Duration {
    expires_at
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .and_then(|at| (at.with_timezone(&chrono::Utc) - now).to_std().ok())
        .map_or(KEPT_AT_MOST, |left| {
            left.saturating_sub(MINTED_AGAIN_BEFORE).min(KEPT_AT_MOST)
        })
}

/// **The GitHub App token tool `tool` is handed for chat `chat`** (module docs): one kept for
/// the same chat, App, key, repos and folder, or one minted now. `read_key` reads the App's key
/// from the tool's vault, and is called only to mint. Refused, with a sentence, for a tool that
/// mints nothing, a chat with no GitHub repo, and anything GitHub refuses.
pub fn token_for(
    chat: &For<'_>,
    tool: &crate::sandbox::tools::Tool,
    read_key: &dyn Fn() -> Result<Zeroizing<String>, String>,
) -> Result<Zeroizing<String>, String> {
    let (Some(mint), [value]) = (&tool.mint, tool.values.as_slice()) else {
        return Err(format!("{} mints no GitHub App token", tool.name));
    };
    let remotes = remotes_of_workspace(chat.root, chat.folder)?;
    let mut hosts: Vec<&str> = remotes.iter().map(|remote| remote.host.as_str()).collect();
    hosts.sort_unstable();
    hosts.dedup();
    let here_host = chat.here.and_then(|here| {
        remotes
            .iter()
            .find(|remote| here.starts_with(&remote.clone))
            .map(|remote| remote.host.clone())
    });
    let host = match (here_host, hosts.as_slice()) {
        (Some(host), _) => host,
        (None, [one]) => (*one).to_owned(),
        (None, []) => {
            return Err(
                "no clone in this chat's workspace has a GitHub remote, so no GitHub App token \
                 was minted"
                    .to_owned(),
            );
        }
        (None, _) => {
            return Err(
                "this chat's clones are on several GitHub hosts, so run the command in one of \
                 them"
                    .to_owned(),
            );
        }
    };
    let asked = Asked {
        root: chat.root.to_path_buf(),
        chat: chat.chat,
        app: mint.app.clone(),
        vault: tool.vault.clone(),
        key: value.key.clone(),
        host: host.clone(),
        remotes: remotes.clone(),
        here: chat.here.map(Path::to_path_buf),
    };
    {
        let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        kept.retain(|one| one.until > now);
        if let Some(one) = kept.iter().find(|one| one.asked == asked) {
            return Ok(one.token.clone());
        }
    }
    let forge = Forge {
        kind: Kind::GitHub,
        host,
    };
    let key = read_key()?;
    let now = chrono::Utc::now();
    let jwt = jwt(&key, &mint.app, u64::try_from(now.timestamp()).unwrap_or(0))?;
    drop(key);
    let transport = over_https(&forge, &jwt)?;
    drop(jwt);
    let minted = mint_over(&transport, &forge, &remotes, chat.here)?;
    tracing::info!(
        "purlis: chat {} was handed a token of GitHub App {} for {} repo(s) of installation {}",
        chat.chat,
        mint.app,
        minted.repos.len(),
        minted.installation
    );
    let until = Instant::now() + kept_for(minted.expires_at.as_deref(), now);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    kept.retain(|one| one.asked != asked);
    if kept.len() >= MOST_KEPT {
        kept.remove(0);
    }
    kept.push(Kept {
        asked,
        token: minted.token.clone(),
        until,
    });
    Ok(minted.token)
}

/// **[`token_for`], its key read from the tool's vault through `ctx`** by the one read a GitHub
/// App's key is given ([`crate::secrets::cmd::read_to_mint`]): what a chat's run is handed, on
/// this machine or sent to its runner.
pub fn token_from_vault(
    ctx: &crate::secrets::Ctx,
    chat: &For<'_>,
    tool: &crate::sandbox::tools::Tool,
) -> Result<Zeroizing<String>, String> {
    token_for(chat, tool, &|| {
        let key = tool
            .values
            .first()
            .ok_or_else(|| format!("{} names no key", tool.name))?;
        crate::secrets::cmd::provider(ctx, &tool.vault)
            .and_then(|vault| crate::secrets::cmd::read_to_mint(ctx, &vault, &key.key))
            .map(Zeroizing::new)
            .map_err(|e| e.message.to_string())
    })
}

#[cfg(test)]
mod tests;
