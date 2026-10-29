// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own GitHub identity: an installation token the organisation's own GitHub App mints
//! for it** (#2053, SD-7a; ADR 0077 as amended 2026-10-29, E1 to E3).
//!
//! A persona's tool declared with `mint = "github-app"` ([`crate::sandbox::tools::mint`]) is
//! handed, at each run, a token minted here, in the app's process:
//!
//! 1. **the chat's repos** are the github.com remotes of the clones the chat's workspace had when
//!    the chat started, read at that run from each clone's own config alone, and only of clones
//!    whose git directory is in no folder a chat may be granted ([`remotes_of_workspace`], E3).
//!    A GitHub Enterprise Server is not minted for yet: a run in a clone of one is refused,
//!    naming it;
//! 2. the App's key, read from its vault by the mint alone, signs a JWT of ten minutes at most
//!    whose issuer is the App's client id ([`jwt`]), which is dropped once the token is minted;
//! 3. each repo's installation is asked for (`GET /repos/{owner}/{repo}/installation`), and the
//!    one the run's clone is on, or else the only one, mints a token limited to those of its
//!    repos (`POST /app/installations/{id}/access_tokens` with `repositories`, [`mint_over`]).
//!    It holds the permissions the organisation gave the App's installation.
//!
//! **One token per chat and clone is kept, in memory alone**, until ten minutes before GitHub
//! says it expires ([`token_for`]): a chat runs `gh` many times an hour, and each run would
//! otherwise cost three requests. Nothing here is ever written to disk. The one native transport
//! this builds, carrying the App's JWT to github.com alone, is [`jwt_transport`].

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use zeroize::Zeroizing;

use super::transport::{Call, Field, Method, Transport};
use super::{Forge, Kind};

pub mod jwt_transport;

/// What a mint is refused with where the chat's repos are on several installations and the run
/// is in a clone of none of them.
pub const SEVERAL_INSTALLATIONS: &str = "this chat's repos are on several installations of the \
                                         GitHub App, so run the command in a clone of one of them";

/// The one host a GitHub App's token is minted on (#2053, part 1). A GitHub Enterprise Server's
/// comes later.
pub const HOST: &str = "github.com";

/// The most repos one token is limited to: GitHub refuses a longer `repositories` list.
pub const MOST_REPOS: usize = 500;

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

/// **Whether `path` is inside one of `writes`**, as written or as the kernel names either.
fn inside_any(path: &Path, writes: &[PathBuf]) -> bool {
    let real = crate::sandbox::real(path);
    writes.iter().any(|dir| {
        let dir_real = crate::sandbox::real(dir);
        [path, real.as_path()]
            .iter()
            .any(|p| p.starts_with(dir) || p.starts_with(&dir_real))
    })
}

/// **The clones of `repos` whose git directories no chat writes**: each whose `.git`, and for a
/// worktree the common directory its config is in, is inside none of `writes`, the folders a
/// chat may be granted. Only such a clone's remotes name the chat's repos.
pub fn held_from_chats(
    repos: Vec<crate::repos::Repo>,
    writes: &[PathBuf],
) -> Vec<crate::repos::Repo> {
    repos
        .into_iter()
        .filter(|repo| {
            let dirs = crate::sandbox::planted::git_dirs(&repo.path);
            !dirs.is_empty() && dirs.iter().all(|dir| !inside_any(dir, writes))
        })
        .collect()
}

/// The folders a chat of the project at `root` may be granted, beside `writes`: Settings' list of
/// grantable folders. A chat's temporary folder is its own and holds no clone of a workspace.
fn grantable_beside(root: &Path, writes: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = writes.to_vec();
    out.extend(crate::sandbox::local::grantable_folders(root));
    out
}

/// **One clone of a workspace as a chat started**: the clone, as the kernel names it, and its
/// `.git` directory's identity then (device and inode, where the system has them).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Started {
    pub clone: PathBuf,
    pub git: Option<(u64, u64)>,
}

/// The identity of the directory at `path`: its device and inode, where the system has them.
fn identity(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let found = std::fs::symlink_metadata(path).ok()?;
        found.is_dir().then(|| (found.dev(), found.ino()))
    }
    #[cfg(not(unix))]
    {
        std::fs::symlink_metadata(path)
            .ok()?
            .is_dir()
            .then_some((0, 0))
    }
}

/// **Every clone of every workspace of the plane at `root`, now**, with its `.git`'s identity:
/// what a chat's start records ([`crate::sandbox::Confines::clones`]).
pub fn clones_now(root: &Path) -> Vec<Started> {
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let Ok(workspaces) = plane.workspaces() else {
        return Vec::new();
    };
    workspaces
        .iter()
        .filter_map(|workspace| crate::repos::clones(root, workspace).ok())
        .flat_map(|clones| clones.repos)
        .map(|repo| Started {
            clone: crate::sandbox::real(&repo.path),
            git: identity(&repo.path.join(".git")),
        })
        .collect()
}

/// **The clones of `repos` that were there, as they were, when the chat started**: each that
/// `started` names, whose `.git` is the same directory it was then.
pub fn there_at_start(
    repos: Vec<crate::repos::Repo>,
    started: &[Started],
) -> Vec<crate::repos::Repo> {
    repos
        .into_iter()
        .filter(|repo| {
            let clone = crate::sandbox::real(&repo.path);
            let git = identity(&repo.path.join(".git"));
            git.is_some()
                && started
                    .iter()
                    .any(|then| then.clone == clone && then.git == git)
        })
        .collect()
}

/// **The chat's repos** (E3): the GitHub remotes of the clones in the workspace that `folder`,
/// the chat's folder, is in, in the project at `root`, read now from each clone's own config
/// (`git config --local --no-includes`), of the clones [`held_from_chats`] keeps against
/// `writes`, the chat's grants, and, where `started` is given, of those the workspace had when
/// the chat started ([`there_at_start`]). Refused for a chat in no workspace.
pub fn remotes_of_workspace(
    root: &Path,
    folder: Option<&Path>,
    writes: &[PathBuf],
    started: Option<&[Started]>,
) -> Result<Vec<Remote>, String> {
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
    let repos = match started {
        Some(started) => there_at_start(clones.repos, started),
        None => clones.repos,
    };
    for repo in held_from_chats(repos, &grantable_beside(root, writes)) {
        let Ok(run) = crate::worktree::git::run(
            &repo.path,
            &[
                "config",
                "--local",
                "--no-includes",
                "--get-regexp",
                r"^remote\..*\.url$",
            ],
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

/// **The clone of `remotes` the folder `here` is in**, canonical, where it is in one.
pub fn clone_of(remotes: &[Remote], here: Option<&Path>) -> Option<PathBuf> {
    let here = here?;
    let here = here.canonicalize().unwrap_or_else(|_| here.to_path_buf());
    remotes
        .iter()
        .map(|remote| {
            remote
                .clone
                .canonicalize()
                .unwrap_or_else(|_| remote.clone.clone())
        })
        .find(|clone| here.starts_with(clone))
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
    let mut distinct: Vec<(&str, &str)> = remotes
        .iter()
        .filter(|remote| remote.host == forge.host)
        .map(|remote| (remote.owner.as_str(), remote.name.as_str()))
        .collect();
    distinct.sort_unstable();
    distinct.dedup();
    // Deliberately conservative: every repo of the workspace counts here, whichever
    // installation it is on, though the token names only one installation's.
    if distinct.len() > MOST_REPOS {
        return Err(format!(
            "this chat's workspace has {} GitHub repos, and a GitHub App's token is limited to \
             at most {MOST_REPOS}, so none was minted",
            distinct.len()
        ));
    }
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
    let here_clone = clone_of(remotes, here);
    let in_here = |remote: &Remote| {
        here_clone
            .as_ref()
            .is_some_and(|clone| remote_is_in(remote, clone))
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
        _ => match installed.iter().find(|(_, remote)| in_here(remote)) {
            Some((id, _)) => *id,
            None => return Err(SEVERAL_INSTALLATIONS.to_owned()),
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

/// **The chat a token is minted for**: the project, the app's number for the chat, its folder
/// (which names its workspace), the folder the command runs in, the folders the chat may be
/// granted, the clones its workspace had when it started (none recorded for a runner chat,
/// which writes no folder of this machine's), and whether it runs on a runner, whose folder is
/// not one of this machine's.
#[derive(Debug, Clone, Copy)]
pub struct For<'a> {
    pub root: &'a Path,
    pub chat: u32,
    pub folder: Option<&'a Path>,
    pub here: Option<&'a Path>,
    pub writes: &'a [PathBuf],
    pub started: Option<&'a [Started]>,
    pub on_a_runner: bool,
}

/// What a kept token was minted for: none of it is a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    root: PathBuf,
    chat: u32,
    app: String,
    vault: String,
    key: String,
    remotes: Vec<Remote>,
    /// The clone the run is in, where it is in one: one token per chat and clone, whatever
    /// folder of it the run is in.
    clone: Option<PathBuf>,
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
    let all = remotes_of_workspace(chat.root, chat.folder, chat.writes, chat.started)?;
    // github.com alone for now: a run in a clone on another host is refused, naming it.
    if let Some(clone) = clone_of(&all, chat.here)
        && let Some(other) = all
            .iter()
            .find(|remote| remote.host != HOST && remote_is_in(remote, &clone))
    {
        return Err(not_minted_on(&other.host));
    }
    let remotes: Vec<Remote> = all
        .iter()
        .filter(|remote| remote.host == HOST)
        .cloned()
        .collect();
    if remotes.is_empty() {
        return Err(match all.first() {
            Some(other) => not_minted_on(&other.host),
            None => "no clone in this chat's workspace has a github.com remote, so no GitHub \
                     App token was minted"
                .to_owned(),
        });
    }
    let clone = clone_of(&remotes, chat.here);
    let asked = Asked {
        root: chat.root.to_path_buf(),
        chat: chat.chat,
        app: mint.app.clone(),
        vault: tool.vault.clone(),
        key: value.key.clone(),
        remotes: remotes.clone(),
        clone: clone.clone(),
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
        host: HOST.to_owned(),
    };
    let key = read_key()?;
    let now = chrono::Utc::now();
    let jwt = jwt(&key, &mint.app, u64::try_from(now.timestamp()).unwrap_or(0))?;
    drop(key);
    let transport = jwt_transport::to_github_com(&jwt)?;
    drop(jwt);
    let minted = mint_over(&transport, &forge, &remotes, clone.as_deref())
        .map_err(|why| on_a_runner_said(why, chat.on_a_runner))?;
    tracing::info!(
        "purlis: chat {} was handed a token of GitHub App {} for {} of installation {}",
        chat.chat,
        mint.app,
        crate::counts::counted(minted.repos.len(), "repo", "repos"),
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

/// Whether `remote` is a remote of the clone at `clone`, canonical.
fn remote_is_in(remote: &Remote, clone: &Path) -> bool {
    remote
        .clone
        .canonicalize()
        .unwrap_or_else(|_| remote.clone.clone())
        == clone
}

/// What a run on `host`, a host that is not github.com, is refused with.
fn not_minted_on(host: &str) -> String {
    format!(
        "{host} is not github.com, and purlis mints a GitHub App's token for github.com alone \
         for now, so none was minted"
    )
}

/// `why` a mint was refused, said for a runner chat where it asks for a clone to run in: a
/// runner's folder is not one of this machine's, so it is told what to change instead.
fn on_a_runner_said(why: String, on_a_runner: bool) -> String {
    if on_a_runner && why == SEVERAL_INSTALLATIONS {
        "this runner chat's repos are on several installations of the GitHub App, and on a \
         runner purlis mints for one alone: keep its workspace's clones on one installation"
            .to_owned()
    } else {
        why
    }
}

/// A test's own mint: handed the App's key as the vault holds it, it answers the token.
#[cfg(test)]
pub(crate) type TestMint = Box<dyn Fn(&str) -> Result<Zeroizing<String>, String>>;

#[cfg(test)]
thread_local! {
    /// A test's own mint, in place of GitHub, for the calls on its thread.
    pub(crate) static MINT_HERE: std::cell::RefCell<Option<TestMint>> =
        const { std::cell::RefCell::new(None) };
}

/// **[`token_for`], its key read from the tool's vault through `ctx`** by the one read a GitHub
/// App's key is given ([`crate::secrets::cmd::read_to_mint`]): what a chat's run is handed, on
/// this machine or sent to its runner.
pub fn token_from_vault(
    ctx: &crate::secrets::Ctx,
    chat: &For<'_>,
    tool: &crate::sandbox::tools::Tool,
) -> Result<Zeroizing<String>, String> {
    let read_key = || {
        let key = tool
            .values
            .first()
            .ok_or_else(|| format!("{} names no key", tool.name))?;
        crate::secrets::cmd::provider(ctx, &tool.vault)
            .and_then(|vault| crate::secrets::cmd::read_to_mint(ctx, &vault, &key.key))
            .map(Zeroizing::new)
            .map_err(|e| e.message.to_string())
    };
    #[cfg(test)]
    if let Some(minted) = MINT_HERE.with(|mint| {
        mint.borrow()
            .as_ref()
            .map(|mint| read_key().and_then(|key| mint(&key)))
    }) {
        return minted;
    }
    token_for(chat, tool, &read_key)
}

#[cfg(test)]
mod tests;
