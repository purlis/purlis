// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signed commits, set up** (#5625, spec #5607, ADR 0116): the one module the window and
//! `purlis signing` share for this device's **signing key** and the forge account it is added to.
//!
//! - **The key** is ADR 0115's, kept in the Keyring tier: [`crate::commitsign`] makes, reads and
//!   forgets it.
//! - **The forge account** is reached only with the window's own sign-in
//!   ([`crate::forge::signingkeys`]): listing its signing keys, adding this device's, removing
//!   one, and asking whether a pushed commit shows as Verified. Never through `gh`.
//! - **`purlis signing` holds no sign-in** (ADR 0070 ruling 5, unchanged by the person's ruling
//!   of 2026-11-07 on #5625): it asks the running window over its line ([`crate::windowline`])
//!   with an [`Asked`], and the window answers with what the forge told it, a [`Heard`]
//!   ([`answered_over`]). From the command a key is added by hand, and with no window running the
//!   command gives those steps and says to open the window.

use std::path::Path;

use crate::commitsign::Signer;
use crate::forge::signingkeys::{self, Checked};
use crate::forge::transport::Transport;
use crate::forge::{Caller, Forge, Kind};
use crate::secrets::keyring::Store;

/// **The forge account an ask is about**, by its forge and host (#5793): the window asks the
/// account it is signed in to there.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OnForge {
    pub kind: Kind,
    pub host: String,
}

impl OnForge {
    pub fn of(forge: &Forge) -> OnForge {
        OnForge {
            kind: forge.kind,
            host: forge.host.clone(),
        }
    }
}

/// What `purlis signing` asks the window, on the person's behalf. Each names the forge and host
/// of the account it is about (`on`); an ask that names none is about GitHub on github.com.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asked {
    /// Whether the account has the public key `key` (one OpenSSH line) as a signing key: read
    /// from the forge's public list of the account's keys (on GitLab, those it may sign with).
    KeyOnAccount {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<OnForge>,
    },
    /// Whether commit `commit` of `repo` (`owner/name`, or a GitLab project's path) shows as
    /// Verified. Asked of the forge only on the person's yes in the window, to this repository,
    /// this commit and `code`, which the command's terminal shows too, as the line's other asks
    /// are.
    Check {
        repo: String,
        commit: String,
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<OnForge>,
        /// The project the command runs in, where it runs in one: the window lists the
        /// confirmation in that project's Asks, as the command's line says (Q233), and under the
        /// project in front where it names none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<std::path::PathBuf>,
    },
}

impl Asked {
    /// The forge the ask is about: the one it names, else GitHub on github.com.
    pub fn forge(&self) -> Forge {
        let (Asked::KeyOnAccount { on, .. } | Asked::Check { on, .. }) = self;
        match on {
            Some(on) => forge_of(on.kind, &on.host.to_ascii_lowercase()),
            None => github(),
        }
    }
}

/// What the window heard from the forge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heard {
    /// Whether the account has the key.
    KeyOnAccount { on: bool },
    /// What the forge says of the commit.
    Checked(Checked),
}

/// GitHub, on github.com: what an ask that names no forge is about.
pub fn github() -> Forge {
    Forge::default_of(Kind::GitHub)
}

/// The forge of `kind` on `host`.
pub fn forge_of(kind: Kind, host: &str) -> Forge {
    Forge {
        kind,
        host: host.to_owned(),
    }
}

/// **The window's answer to `asked`** for the account `login`, sent over `transport` (the
/// window's own sign-in, in the app; a recording, in a test). Refused, in a sentence, where the
/// forge refused it or `key` is not one public key. A Check asks only once the person said yes.
///
/// On GitLab a Check is held to `this_key`, this device's key, as the window's own Check is
/// ([`check_a_pushed_commit`]): a commit signed with another key, or by GitLab itself, says
/// nothing of this device's, and with no key here there is nothing to check (#5793).
pub fn answered_over(
    transport: &dyn Transport,
    forge: &Forge,
    login: &str,
    asked: &Asked,
    this_key: Option<&Signer>,
) -> Result<Heard, String> {
    match asked {
        Asked::KeyOnAccount { key, .. } => {
            if ssh_key::PublicKey::from_openssh(key).is_err() {
                return Err("that is not one public key".to_owned());
            }
            signingkeys::listed_publicly_over(transport, forge, login)
                .map(|listed| Heard::KeyOnAccount {
                    on: listed.has(key).is_some(),
                })
                .map_err(|refused| refused.said(forge.kind))
        }
        Asked::Check { repo, commit, .. } => match (forge.kind, this_key) {
            (Kind::GitLab, Some(signer)) => {
                check_a_pushed_commit(transport, forge, signer, repo, commit).map(Heard::Checked)
            }
            (Kind::GitLab, None) => Err(
                "this device has no signing key, so GitLab's verdict on a commit says nothing of \
                 it. Set signing up first"
                    .to_owned(),
            ),
            (Kind::GitHub, _) => {
                signingkeys::check_over(transport, forge, repo, commit).map(Heard::Checked)
            }
        },
    }
}

/// **The running window's answer to `asked` from the command line**, with its own sign-in:
/// the window asks as itself, since the command holds none (the ruling of 2026-11-07 on #5625).
/// What comes back is a read: whether the account's public list has a key, or what the forge says
/// of a commit the person said yes to checking; never a token. The account is the one signed in
/// on the forge and host the ask names (#5793). Where the window has none there, a sentence
/// that says so.
pub fn answered_in_the_window(asked: &Asked) -> Result<Heard, String> {
    let forge = asked.forge();
    let not_signed_in = || {
        format!(
            "this window is not signed in to {} on {} (Settings › You › Accounts)",
            forge.kind.display(),
            forge.host
        )
    };
    let config = crate::machine::config_root()
        .ok_or_else(|| "this window found no config folder".to_owned())?;
    let account = crate::forge::accounts::signed_in_on(&config, forge.kind, &forge.host)?
        .ok_or_else(not_signed_in)?;
    let transport =
        signingkeys::window(&forge, &Caller::window()).map_err(|why| why.to_string())?;
    // This device's key is read only for a GitLab Check, which is held to it.
    let this_key = match asked {
        Asked::Check { .. } if forge.kind == Kind::GitLab => {
            let keyring = crate::commitsign::this_builds_keyring(&config);
            key_kept(&config, keyring.as_ref())?
        }
        _ => None,
    };
    answered_over(
        transport.as_ref(),
        &forge,
        &account.login,
        asked,
        this_key.as_ref(),
    )
}

/// How an ask of the window went, from the command's side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reached {
    /// The window answered with what the forge said.
    Heard(Heard),
    /// No purlis window is running here.
    NoWindow,
    /// The window did not answer in time.
    NoAnswer,
    /// The window answered another question than this one: a window of another build.
    OtherAnswer,
    /// The window answered that it could not ask, in a sentence.
    Refused(String),
}

/// **`asked`, put to the running window by `ask`** ([`crate::windowline::ask`] in the command).
#[cfg(unix)]
pub fn reach(
    asked: Asked,
    ask: impl FnOnce(&crate::windowline::Asked) -> crate::windowline::Answered,
) -> Reached {
    use crate::windowline::{Answered, Said};
    match ask(&crate::windowline::Asked::Signing(asked)) {
        Answered::Said(Said::Signing(heard)) => Reached::Heard(heard),
        Answered::Said(Said::Refused { why }) => Reached::Refused(why),
        Answered::Said(Said::Asked { .. }) => Reached::OtherAnswer,
        Answered::NoAnswer => Reached::NoAnswer,
        Answered::NoWindow => Reached::NoWindow,
    }
}

/// The window's line answer to a signing ask: what [`answered_in_the_window`] says.
#[cfg(unix)]
pub fn said_in_the_window(asked: &Asked) -> crate::windowline::Said {
    match answered_in_the_window(asked) {
        Ok(heard) => crate::windowline::Said::Signing(heard),
        Err(why) => crate::windowline::Said::Refused { why },
    }
}

/// The page where the person adds a signing key to their GitHub account by hand.
pub const NEW_KEY_PAGE: &str = "https://github.com/settings/ssh/new";

/// **The page where the person adds a signing key by hand on `forge`**: GitHub's new key page,
/// or the GitLab's SSH keys page, where *Signing* is chosen as the usage type (#5793).
pub fn new_key_page(forge: &Forge) -> String {
    match forge.kind {
        Kind::GitHub => NEW_KEY_PAGE.to_owned(),
        Kind::GitLab => format!("https://{}/-/user_settings/ssh_keys", forge.host),
    }
}

/// **`forge` as a sentence names it**: `GitHub` or `GitLab` on its default host, else with the
/// host (`GitLab on gitlab.example.org`).
pub fn forge_said(forge: &Forge) -> String {
    if forge.host.eq_ignore_ascii_case(forge.kind.default_host()) {
        forge.kind.display().to_owned()
    } else {
        format!("{} on {}", forge.kind.display(), forge.host)
    }
}

/// **This device's key**, or `None` where it has none: signing is on exactly while it has one
/// (ADR 0116 §1).
pub fn key_kept(config_root: &Path, store: &dyn Store) -> Result<Option<Signer>, String> {
    crate::commitsign::kept(config_root, store)
}

/// **Whether this device has a key**, asked of the keychain's attributes and never its value, so
/// asking never makes macOS ask the person: `false` only where the keychain provably has none,
/// `true` where it has one, and the key read where it cannot say.
pub fn key_is_there(config_root: &Path, store: &dyn Store) -> Result<bool, String> {
    let Some(device) = crate::machine::known_device_id(config_root) else {
        return Ok(false);
    };
    match store.exists(
        &format!("{}/{device}", crate::commitsign::SERVICE),
        crate::commitsign::ACCOUNT,
    ) {
        crate::secrets::keyring::Presence::Present => Ok(true),
        crate::secrets::keyring::Presence::Absent => Ok(false),
        crate::secrets::keyring::Presence::Unknown => Ok(key_kept(config_root, store)?.is_some()),
    }
}

/// **This device's key, made where it has none**, and whether it was there before.
pub fn key_made(config_root: &Path, store: &dyn Store) -> Result<(Signer, bool), String> {
    let had = key_kept(config_root, store)?.is_some();
    Ok((crate::commitsign::make(config_root, store)?, had))
}

/// **The person's own key file kept** as this device's key (`purlis signing setup --key`):
/// `openssh` as the file holds it, opened with `passphrase` where it has one.
pub fn key_taken(
    config_root: &Path,
    store: &dyn Store,
    openssh: &str,
    passphrase: Option<&str>,
) -> Result<Signer, String> {
    crate::commitsign::take(config_root, store, openssh, passphrase)
}

/// **This device's key removed from its keychain**, and whether one was there. Signing is off
/// from then on; the forge accounts it was added to keep it until it is removed there too.
pub fn key_removed(config_root: &Path, store: &dyn Store) -> Result<bool, String> {
    crate::commitsign::forget(config_root, store)
}

/// **Who a chat's commit is made as**: the person's own git name and email, which a forge
/// matches against the account's verified addresses.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Who {
    pub name: String,
    pub email: String,
}

/// The person's own git name and email (`git config --global`), or a sentence saying which is
/// not set.
pub fn who_commits() -> Result<Who, String> {
    let read = |key: &str| {
        crate::worktree::git::run(
            Path::new("/"),
            &["config", "--global", "--get", key],
            crate::worktree::git::READ,
        )
        .ok()
        .filter(crate::worktree::git::Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|value| !value.is_empty())
    };
    match (read("user.name"), read("user.email")) {
        (Some(name), Some(email)) => Ok(Who { name, email }),
        _ => Err(
            "your git has no name or email set, so a commit can't say who made it. Set them \
             in Terminal with `git config --global user.name` and `user.email`."
                .to_owned(),
        ),
    }
}

/// The message of the test commit Check signs. Never on a branch: the forge keeps it unreferenced.
pub const TEST_MESSAGE: &str = "purlis checks that this device's signing key is verified\n";

/// **The test commit object Check signs, as git writes it**: `tree`, no parent, `who` as author
/// and committer at `when` (seconds since the epoch, UTC), and [`TEST_MESSAGE`]. What the forge
/// rebuilds from the same fields, byte for byte, and checks the signature over.
pub fn test_commit(tree: &str, who: &Who, when: i64) -> Vec<u8> {
    let line = |role: &str| format!("{role} {} <{}> {when} +0000\n", who.name, who.email);
    format!(
        "tree {tree}\n{}{}\n{TEST_MESSAGE}",
        line("author"),
        line("committer")
    )
    .into_bytes()
}

/// `when` as GitHub reads a commit's date: ISO 8601 in UTC, to the second.
fn iso(when: i64) -> Result<String, String> {
    chrono::DateTime::from_timestamp(when, 0)
        .map(|at| at.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .ok_or_else(|| "the clock gave no time to date the test with".to_owned())
}

/// **Check, as the forge answers it** (ADR 0116 §6): a test commit signed with `signer` is made
/// in `repo` (`owner/name`), on the tree of its default branch, and never put on a branch; the
/// forge's own verdict on it comes back. Verified means GitHub said so, not that it lists the key.
pub fn check_with_a_test_commit(
    transport: &dyn Transport,
    forge: &Forge,
    signer: &Signer,
    repo: &str,
    who: &Who,
    when: i64,
) -> Result<Checked, String> {
    use crate::forge::transport::{Call, Field, Method};
    let (owner, name) = signingkeys::repo_named(repo)?;
    let shown = forge.kind.display();
    let head = transport
        .send(
            forge,
            &Call::get(
                format!("repos/{owner}/{name}/commits/HEAD"),
                std::time::Duration::from_secs(30),
            ),
        )
        .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
    if !head.ok() {
        return Err(format!(
            "{shown} could not read {repo} to check with ({})",
            head.said(forge.kind).lines().next().unwrap_or("").trim()
        ));
    }
    let head: serde_json::Value = serde_json::from_str(&head.out)
        .map_err(|_| format!("{shown}'s answer about {repo} did not read"))?;
    let tree = head
        .pointer("/commit/tree/sha")
        .and_then(serde_json::Value::as_str)
        .filter(|sha| sha.len() >= 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| format!("{shown} named no tree for {repo}'s default branch"))?;
    let signature = signer.sign(&test_commit(tree, who, when))?;
    let date = iso(when)?;
    let mut fields = vec![
        Field::text("message", TEST_MESSAGE),
        Field::text("tree", tree),
    ];
    for role in ["author", "committer"] {
        fields.push(Field::text(&format!("{role}[name]"), &who.name));
        fields.push(Field::text(&format!("{role}[email]"), &who.email));
        fields.push(Field::text(&format!("{role}[date]"), &date));
    }
    fields.push(Field::text("signature", &signature));
    let made = transport
        .send(
            forge,
            &Call::write(
                Method::Post,
                format!("repos/{owner}/{name}/git/commits"),
                fields,
            ),
        )
        .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
    if !made.ok() {
        return Err(format!(
            "{shown} did not take the test commit in {repo} ({})",
            made.said(forge.kind).lines().next().unwrap_or("").trim()
        ));
    }
    let made: serde_json::Value = serde_json::from_str(&made.out)
        .map_err(|_| format!("{shown}'s answer about the test commit did not read"))?;
    let said = made
        .get("verification")
        .ok_or_else(|| format!("{shown} named no signature check for the test commit"))?;
    Ok(Checked {
        key: None,
        verified: said
            .get("verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        reason: said
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
    })
}

/// **Whether the commit's email, not the key, is the likely reason Check wasn't Verified**
/// (#5916): GitHub picks the account to verify an SSH signature against by the commit's email,
/// so where that email isn't one of the account's verified ones GitHub answers `unknown_key`
/// (or `not_signing_key`) for a key the account does hold.
///
/// True only on GitHub, where the forge's `reason` is one of those two, the account `login`
/// lists `signer`'s key publicly, and its verified addresses, read, don't hold `email`. Where
/// either list can't be read, false: the window then claims nothing it hasn't seen, and the
/// key's own wording stands. Not on GitLab, which finds a key by its fingerprint and whose public
/// list holds keys for signing in too. The ✓ never comes from here (ADR 0116 §6); this only
/// chooses what the window offers to fix.
pub fn email_is_why(
    transport: &dyn Transport,
    forge: &Forge,
    signer: &Signer,
    login: &str,
    reason: &str,
    email: &str,
) -> bool {
    if forge.kind != Kind::GitHub || !matches!(reason, "unknown_key" | "not_signing_key") {
        return false;
    }
    signingkeys::listed_publicly_over(transport, forge, login)
        .is_ok_and(|listed| listed.this_device(signer).is_some())
        && verified_emails(transport, forge).is_ok_and(|verified| {
            !verified
                .iter()
                .any(|one| one.email.eq_ignore_ascii_case(email))
        })
}

/// **A repository to check in, where setup was opened with none**: the account's own most
/// recently pushed, which its sign-in may write to. `None` where it owns none.
pub fn a_repo_to_check_in(
    transport: &dyn Transport,
    forge: &Forge,
) -> Result<Option<String>, String> {
    use crate::forge::transport::{Call, Field};
    let mut call = Call::get("user/repos", std::time::Duration::from_secs(30));
    for (name, value) in [
        ("affiliation", "owner"),
        ("sort", "pushed"),
        ("per_page", "1"),
    ] {
        call.fields.push(Field::typed(name, value));
    }
    let shown = forge.kind.display();
    let reply = transport
        .send(forge, &call)
        .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
    if !reply.ok() {
        return Err(format!(
            "{shown} did not list your repositories ({})",
            reply.said(forge.kind).lines().next().unwrap_or("").trim()
        ));
    }
    let repos: Vec<serde_json::Value> = serde_json::from_str(&reply.out)
        .map_err(|_| format!("{shown}'s list of your repositories did not read"))?;
    Ok(repos
        .first()
        .and_then(|repo| repo.get("full_name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

// ---------------------------------------------------------------------------------------
// GitLab's Check (#5629): a signed commit already pushed
// ---------------------------------------------------------------------------------------

/// **A signed chat commit the app pushed**, as the GitLab Check finds one: GitLab has no API
/// that makes a signed commit, so its Check asks about one already there (#5629).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignedPush {
    pub kind: Kind,
    pub host: String,
    /// The repository's path on the forge: `owner/name`, or a GitLab project's full path.
    pub repo: String,
    pub commit: String,
    /// When it was pushed, in seconds since the epoch.
    pub at: i64,
}

/// How many signed pushes are kept: the newest, per forge, is the one asked about.
const PUSHES_KEPT: usize = 20;

/// Where this device's signed pushes are kept, under its config home: the app writes it, as the
/// brokered push it made for a chat; no chat writes there.
fn pushes_file(config_root: &Path) -> std::path::PathBuf {
    config_root.join("signing").join("pushed.json")
}

/// The signed pushes kept, newest first; none where the file is missing or does not read.
pub fn signed_pushes(config_root: &Path) -> Vec<SignedPush> {
    std::fs::read_to_string(pushes_file(config_root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// **`push` kept as the newest signed push**, the oldest dropped past [`PUSHES_KEPT`].
pub fn note_signed_push(config_root: &Path, push: SignedPush) -> Result<(), String> {
    let mut kept = signed_pushes(config_root);
    kept.retain(|was| !(was.kind == push.kind && was.host == push.host && was.repo == push.repo));
    kept.insert(0, push);
    kept.truncate(PUSHES_KEPT);
    let file = pushes_file(config_root);
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|why| why.to_string())?;
    }
    let text = serde_json::to_string_pretty(&kept).map_err(|why| why.to_string())?;
    crate::plane::write_private(config_root, &file, text.as_bytes()).map_err(|why| why.to_string())
}

/// The newest signed push to `forge`, where the app has made one.
pub fn last_signed_push(config_root: &Path, forge: &Forge) -> Option<SignedPush> {
    signed_pushes(config_root)
        .into_iter()
        .find(|push| push.kind == forge.kind && push.host == forge.host)
}

/// **Check on GitLab** (ADR 0116 §6, #5629): what GitLab says of `commit` in `repo`, a commit
/// already pushed and signed with `signer`. Verified only where GitLab says `verified` and the
/// key it names is this device's: a commit signed with another key says nothing of this one.
pub fn check_a_pushed_commit(
    transport: &dyn Transport,
    forge: &Forge,
    signer: &Signer,
    repo: &str,
    commit: &str,
) -> Result<Checked, String> {
    let checked = signingkeys::check_over(transport, forge, repo, commit)?;
    // Only GitLab's verdict on this device's own key says anything of it: a commit GitLab signed
    // itself, or one signed with another key, names none or another.
    match &checked.key {
        Some(key) if signingkeys::same_key(key, &signer.public_line()) => Ok(checked),
        _ => Ok(Checked {
            verified: false,
            reason: "other_key".to_owned(),
            key: checked.key.clone(),
        }),
    }
}

// ---------------------------------------------------------------------------------------
// The email chat commits use (#5631)
// ---------------------------------------------------------------------------------------

/// **A forge account, by its forge, host and login**: what a chosen commit email belongs to
/// (#5788). The host is kept in lower case.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccountRef {
    pub kind: Kind,
    pub host: String,
    pub login: String,
}

impl AccountRef {
    pub fn new(kind: Kind, host: &str, login: &str) -> AccountRef {
        AccountRef {
            kind,
            host: host.to_ascii_lowercase(),
            login: login.to_owned(),
        }
    }

    fn is(&self, other: &AccountRef) -> bool {
        self.kind == other.kind
            && self.host.eq_ignore_ascii_case(&other.host)
            && self.login == other.login
    }
}

/// One account's chosen email, as the file keeps it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Chosen {
    #[serde(flatten)]
    account: AccountRef,
    email: String,
}

/// Where the email chosen for the whole device was kept before choices were per account (#5631):
/// read still, for an account with none of its own (#5788).
fn device_email_file(config_root: &Path) -> std::path::PathBuf {
    config_root.join("signing").join("commit-email")
}

/// Where the email chosen for each forge account is kept, under the device's config home.
fn emails_file(config_root: &Path) -> std::path::PathBuf {
    config_root.join("signing").join("commit-emails.json")
}

fn chosen(config_root: &Path) -> Vec<Chosen> {
    std::fs::read_to_string(emails_file(config_root))
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<Chosen>>(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|one| email_reads(&one.email))
        .collect()
}

/// **The email the person chose to sign `account`'s chat commits as** (#5631, #5788): it stands
/// in for their git's `user.email` in the commits the app makes for a chat on that account's
/// forge. Where the account has none of its own, the one chosen for the whole device before.
pub fn commit_email_of(config_root: &Path, account: &AccountRef) -> Option<String> {
    chosen(config_root)
        .into_iter()
        .find(|one| one.account.is(account))
        .map(|one| one.email)
        .or_else(|| {
            std::fs::read_to_string(device_email_file(config_root))
                .ok()
                .map(|text| text.trim().to_owned())
                .filter(|email| email_reads(email))
        })
}

/// Whether `email` reads as one address git may put on a commit.
fn email_reads(email: &str) -> bool {
    let Some((name, host)) = email.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && host.contains('.')
        && email.len() <= 254
        && !email
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '\\'))
}

/// **`email` kept as the one `account`'s chat commits use**, or `None` to clear its choice.
pub fn set_commit_email_of(
    config_root: &Path,
    account: &AccountRef,
    email: Option<&str>,
) -> Result<(), String> {
    if let Some(email) = email
        && !email_reads(email)
    {
        return Err(format!("{email} is not an email address"));
    }
    let mut all = chosen(config_root);
    all.retain(|one| !one.account.is(account));
    if let Some(email) = email {
        all.push(Chosen {
            account: AccountRef::new(account.kind, &account.host, &account.login),
            email: email.to_owned(),
        });
    }
    let file = emails_file(config_root);
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|why| why.to_string())?;
    }
    let text = serde_json::to_string_pretty(&all).map_err(|why| why.to_string())?;
    crate::plane::write_private(config_root, &file, text.as_bytes()).map_err(|why| why.to_string())
}

/// **The email chosen for the account bound to repositories on `host`** (#5788): the account
/// signed in there ([`crate::forge::accounts::signed_in_on`]), as a chat's commit there is made.
pub fn commit_email_on(config_root: &Path, kind: Kind, host: &str) -> Option<String> {
    let account = crate::forge::accounts::signed_in_on(config_root, kind, host)
        .ok()
        .flatten()?;
    commit_email_of(
        config_root,
        &AccountRef::new(account.kind, &account.host, &account.login),
    )
}

/// **Each signed-in forge host and the email its account's chat commits are made as**, where one
/// was chosen (#5788): what the app hands the commits it makes for a chat
/// ([`crate::worktree::git::Isolated::as_email_on`]).
pub fn commit_emails_by_host(config_root: &Path) -> Vec<(String, String)> {
    // The first signed-in account on each host is the one bound there, whether or not it chose
    // an email: another account's choice on that host is never taken in its place.
    let mut seen: Vec<String> = Vec::new();
    let mut found: Vec<(String, String)> = Vec::new();
    for account in crate::forge::accounts::listed(config_root).unwrap_or_default() {
        if account.state != crate::forge::accounts::State::SignedIn
            || seen
                .iter()
                .any(|host| host.eq_ignore_ascii_case(&account.host))
        {
            continue;
        }
        seen.push(account.host.clone());
        let one = AccountRef::new(account.kind, &account.host, &account.login);
        if let Some(email) = commit_email_of(config_root, &one) {
            found.push((one.host, email));
        }
    }
    found
}

/// **Who `account`'s chat commits are made as on this device**: the person's own git name, and
/// the email chosen for that account where one was, else their git's own.
pub fn who_commits_as(config_root: &Path, account: &AccountRef) -> Result<Who, String> {
    let mut who = who_commits()?;
    if let Some(email) = commit_email_of(config_root, account) {
        who.email = email;
    }
    Ok(who)
}

/// **A verified email address of the forge account**, as the Check step offers it (#5631).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VerifiedEmail {
    pub email: String,
    /// The account's primary address.
    pub primary: bool,
    /// The forge's private address for commits (GitHub's noreply), which keeps the person's
    /// own address out of their commits.
    pub private: bool,
}

/// **The account's verified email addresses** (#5631, ADR 0116 §6), over `transport`: on GitHub
/// the verified ones of `GET user/emails` (the `user:email` scope #5627 asks for) and the
/// account's noreply address; on GitLab the account's primary email and its confirmed ones of
/// `GET user/emails` (its `api` scope). Primary first, the private address last.
pub fn verified_emails(
    transport: &dyn Transport,
    forge: &Forge,
) -> Result<Vec<VerifiedEmail>, String> {
    use crate::forge::transport::Call;
    let shown = forge.kind.display();
    let within = std::time::Duration::from_secs(30);
    let read = |path: &str| -> Result<serde_json::Value, String> {
        let reply = transport
            .send(forge, &Call::get(path, within))
            .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
        if !reply.ok() {
            return Err(format!(
                "{shown} did not list your email addresses ({})",
                reply.said(forge.kind).lines().next().unwrap_or("").trim()
            ));
        }
        serde_json::from_str(&reply.out)
            .map_err(|_| format!("{shown}'s list of your email addresses did not read"))
    };
    let text = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut found: Vec<VerifiedEmail> = Vec::new();
    let mut keep = |email: String, primary: bool, private: bool| {
        if !email.is_empty()
            && !found
                .iter()
                .any(|was| was.email.eq_ignore_ascii_case(&email))
        {
            found.push(VerifiedEmail {
                email,
                primary,
                private,
            });
        }
    };
    match forge.kind {
        Kind::GitHub => {
            for one in read("user/emails")?.as_array().into_iter().flatten() {
                let verified = one.get("verified").and_then(serde_json::Value::as_bool);
                if verified == Some(true)
                    && let Some(email) = text(one, "email")
                {
                    let primary = one.get("primary").and_then(serde_json::Value::as_bool);
                    keep(email, primary == Some(true), false);
                }
            }
            let user = read("user")?;
            if let (Some(id), Some(login)) = (
                user.get("id").and_then(serde_json::Value::as_u64),
                text(&user, "login"),
            ) {
                keep(
                    format!("{id}+{login}@users.noreply.github.com"),
                    false,
                    true,
                );
            }
        }
        Kind::GitLab => {
            let user = read("user")?;
            let confirmed = user.get("confirmed_at").is_none_or(|at| !at.is_null());
            if confirmed && let Some(email) = text(&user, "email") {
                keep(email, true, false);
            }
            for one in read("user/emails")?.as_array().into_iter().flatten() {
                let confirmed = one.get("confirmed_at").is_some_and(|at| !at.is_null());
                if confirmed && let Some(email) = text(one, "email") {
                    keep(email, false, false);
                }
            }
        }
    }
    found.sort_by_key(|one| (!one.primary, one.private));
    Ok(found)
}

/// The forge's page where the person adds and verifies an email address.
pub fn email_settings(forge: &Forge) -> String {
    match forge.kind {
        Kind::GitHub => "https://github.com/settings/emails".to_owned(),
        Kind::GitLab => format!("https://{}/-/user_settings/profile", forge.host),
    }
}

/// **What signing kept on this device beside its key, forgotten** (Remove signing, #5632): the
/// emails chosen for chat commits and the signed pushes kept for GitLab's Check.
pub fn forget_records(config_root: &Path) -> Result<(), String> {
    for file in [
        device_email_file(config_root),
        emails_file(config_root),
        pushes_file(config_root),
    ] {
        match std::fs::remove_file(file) {
            Err(why) if why.kind() != std::io::ErrorKind::NotFound => return Err(why.to_string()),
            _ => {}
        }
    }
    Ok(())
}

pub mod manage;
pub mod retry;

#[cfg(test)]
mod tests;
