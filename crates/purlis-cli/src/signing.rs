// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis signing`** (#5480, #5625): the key the app signs a chat's commits with, set up by
//! the person in their own terminal, in plain steps. The window's Signed commits card does the
//! same (ADR 0116), from the same module ([`purlis_core::signingsetup`]).
//!
//! A repository that requires signed commits refuses a pull request whose commits are unsigned.
//! The commits a sandboxed chat makes are made by the app, so the app signs them, with a key it
//! keeps in this machine's keychain ([`purlis_core::commitsign`]). The chat never holds it, so
//! none of this runs in a chat.
//!
//! **This command holds no forge sign-in** (ADR 0070 ruling 5; the person's ruling of 2026-11-07
//! on #5625): it never runs `gh`, and it asks the running purlis window whether the forge has the
//! key or shows a commit as Verified, which the window asks with its own sign-in. The key is
//! added on the forge's page by hand from here; with no window running, the command gives those
//! steps and says to open the window.
//!
//! **GitHub and GitLab alike** (#5793): each ask names the forge and host of the account it is
//! about, one the window is signed in to (`Settings › You › Accounts`). The command reads which
//! accounts those are from the window's list of them, which holds no token.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;

use clap::Subcommand;
use purlis_core::commitsign::{self, SETUP, Signer};
use purlis_core::forge::{Forge, Kind, signingkeys};
use purlis_core::signingsetup::{self, Asked, Heard, OnForge, Reached, forge_said};

use crate::voice;

#[derive(Subcommand)]
pub enum SigningCommand {
    /// Set up the key the app signs your chats' commits with, step by step: purlis makes one (or
    /// takes yours with `--key`), shows how to add it to your GitHub or GitLab account as a
    /// signing key, and has the purlis window check that the forge has it.
    Setup {
        /// Use a key you already have (an Ed25519 private key file, such as `~/.ssh/id_ed25519`)
        /// instead of a new one. If it has a passphrase, you type it once here.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// The host of the forge account to add it to, such as `gitlab.com`. Without it, your
        /// GitHub account, or else the first account the window is signed in to.
        #[arg(long, value_name = "HOST")]
        host: Option<String>,
    },
    /// Say whether this machine has a signing key, show its public half, and have the purlis
    /// window ask each of your GitHub and GitLab accounts whether it has it.
    Status,
    /// Have the purlis window ask the forge whether a pushed commit shows as Verified, and if
    /// not, why and what to do.
    Check {
        /// The host of the forge the repository is on, such as `gitlab.com`. Without it,
        /// github.com.
        #[arg(long, value_name = "HOST")]
        host: Option<String>,
        /// The repository, as `owner/name` on GitHub or the project's path on GitLab.
        repo: String,
        /// The commit, by its full or short id.
        commit: String,
    },
    /// Remove the signing key from this machine's keychain. Commits are made unsigned after it.
    Remove,
}

pub fn run(command: &SigningCommand) -> std::process::ExitCode {
    std::process::ExitCode::from(ran(command))
}

fn ran(command: &SigningCommand) -> u8 {
    if std::env::var_os(purlis_core::hookwire::CHAT_ENV).is_some() {
        voice::err(
            "this runs in your own terminal, not in a chat: the signing key is kept where no \
             chat can reach it. Open a terminal and run the same command there.",
        );
        return 1;
    }
    let Some(config) = purlis_core::machine::config_root() else {
        voice::err("purlis found no config folder on this machine to keep a signing key under.");
        return 1;
    };
    let keyring = commitsign::this_builds_keyring(&config);
    let said = match command {
        SigningCommand::Setup { key, host } => forge_for(&config, host.as_deref())
            .and_then(|forge| setup(&config, &*keyring, key.clone(), &forge)),
        SigningCommand::Status => status(&config, &*keyring),
        SigningCommand::Check { host, repo, commit } => {
            let host = host
                .as_deref()
                .unwrap_or(signingsetup::github().host.as_str())
                .to_owned();
            forge_for(&config, Some(&host)).and_then(|forge| check(&forge, repo, commit))
        }
        SigningCommand::Remove => match signingsetup::key_removed(&config, &*keyring) {
            Ok(true) => {
                voice::ok(
                    "the signing key is removed from this machine. Commits a chat makes are \
                     unsigned from now on. Remove it from GitHub too: Settings › SSH and GPG \
                     keys; and from GitLab, where you added it: Preferences › SSH Keys.",
                );
                Ok(0)
            }
            Ok(false) => {
                voice::info("this machine has no signing key, so there was nothing to remove.");
                Ok(0)
            }
            Err(why) => Err(why),
        },
    };
    match said {
        Ok(code) => code,
        Err(why) => {
            voice::err(why);
            1
        }
    }
}

/// The forge accounts the window is signed in to, the first on each host, in the order they
/// were added, with each one's login: read from its list of them, which holds no token.
fn signed_in(config: &std::path::Path) -> Vec<(Forge, String)> {
    let mut found: Vec<(Forge, String)> = Vec::new();
    for account in purlis_core::forge::accounts::listed(config).unwrap_or_default() {
        if account.state == purlis_core::forge::accounts::State::SignedIn
            && !found
                .iter()
                .any(|(was, _)| was.host.eq_ignore_ascii_case(&account.host))
        {
            found.push((
                signingsetup::forge_of(account.kind, &account.host),
                account.login,
            ));
        }
    }
    found
}

/// [`signed_in`]'s forges alone.
fn accounts(config: &std::path::Path) -> Vec<Forge> {
    signed_in(config)
        .into_iter()
        .map(|(forge, _)| forge)
        .collect()
}

/// **The forge account a command is about**: the one signed in on `host`; github.com and
/// gitlab.com stand for themselves where none is (the window then says it isn't signed in).
/// With no host, GitHub's account, else the first signed in, else GitHub.
fn forge_for(config: &std::path::Path, host: Option<&str>) -> Result<Forge, String> {
    let signed_in = accounts(config);
    let Some(host) = host.map(str::trim).filter(|host| !host.is_empty()) else {
        let github = signingsetup::github();
        return Ok(signed_in
            .iter()
            .find(|forge| **forge == github)
            .or(signed_in.first())
            .cloned()
            .unwrap_or(github));
    };
    if let Some(forge) = signed_in
        .into_iter()
        .find(|forge| forge.host.eq_ignore_ascii_case(host))
    {
        return Ok(forge);
    }
    [Kind::GitHub, Kind::GitLab]
        .into_iter()
        .find(|kind| kind.default_host().eq_ignore_ascii_case(host))
        .map(Forge::default_of)
        .ok_or_else(|| {
            format!(
                "the purlis window isn't signed in to an account on {host}. Sign in to it in \
                 Settings › You › Accounts, then run this again."
            )
        })
}

fn setup(
    config: &std::path::Path,
    keyring: &dyn purlis_core::secrets::keyring::Store,
    key: Option<PathBuf>,
    forge: &Forge,
) -> Result<u8, String> {
    let named = forge_said(forge);
    voice::info(format!(
        "Some repositories accept only signed commits, so {named} can show who made each one. \
         The commits a chat makes are made by the purlis app, and this sets up the key it signs \
         them with. Three steps."
    ));
    eprintln!();
    voice::info("Step 1 of 3: a key, kept in this machine's keychain.");
    let signer = match key {
        Some(file) => {
            let text = std::fs::read_to_string(&file)
                .map_err(|why| format!("purlis could not read {} ({why}).", file.display()))?;
            let passphrase = commitsign::take_needs_passphrase(&text)
                .then(|| ask_secret(&format!("Passphrase for {}: ", file.display())))
                .transpose()?;
            let taken = signingsetup::key_taken(config, keyring, &text, passphrase.as_deref())
                .map_err(|why| format!("{why}. Nothing was changed."))?;
            voice::ok(format!(
                "your key {} is kept in this machine's keychain. Your file is left as it was.",
                taken.fingerprint()
            ));
            taken
        }
        None => {
            let (made, had) = signingsetup::key_made(config, keyring)?;
            if had {
                voice::ok(format!(
                    "this machine already has a signing key ({}). It is kept as it is.",
                    made.fingerprint()
                ));
            } else {
                voice::ok(format!(
                    "a new key is made and kept in this machine's keychain ({}). It never \
                     leaves this machine, and no chat can read it.",
                    made.fingerprint()
                ));
            }
            made
        }
    };
    eprintln!();
    voice::info(format!("Step 2 of 3: tell {named} the key is yours."));
    let first = on_the_account(&signer, forge);
    if matches!(first, Reached::Heard(Heard::KeyOnAccount { on: true })) {
        voice::ok(format!(
            "{named} already has this key as a signing key on your account."
        ));
    } else {
        if let Some(why) = why_by_hand(&first, forge) {
            voice::info(why);
        }
        by_hand_steps(&signer, forge);
        if !waited_for_the_key(&signer, forge, &first) {
            return Ok(0);
        }
    }
    eprintln!();
    voice::info(format!("Step 3 of 3: check that {named} has it."));
    let checked_with = match forge.kind {
        Kind::GitHub => "<owner/repo> <commit>".to_owned(),
        Kind::GitLab => format!("--host {} <project> <commit>", forge.host),
    };
    match on_the_account(&signer, forge) {
        Reached::Heard(Heard::KeyOnAccount { on: true }) => {
            voice::ok(format!(
                "{named} has the key as a signing key on your account. The commits your chats \
                 make are signed from now on, and show as Verified on {named}."
            ));
            email_line(forge, chosen_email(config, forge));
            voice::info(format!(
                "Once a chat's commit is pushed, `purlis signing check {checked_with}` asks \
                 {named} how it shows."
            ));
            Ok(0)
        }
        Reached::Heard(Heard::KeyOnAccount { on: false }) => {
            voice::warn(format!(
                "{named} does not list the key on your account yet. Add it as described above, \
                 with {}, then run `{SETUP}` again: the key is kept, so it goes straight to \
                 this check.",
                usage_type(forge)
            ));
            Ok(1)
        }
        other => {
            voice::warn(format!(
                "{} If you added the key, commits are signed already; `purlis signing status` \
                 checks again later.",
                not_asked(&other, forge)
            ));
            Ok(1)
        }
    }
}

/// What the running window heard from `forge` of `signer`'s key on the account there.
fn on_the_account(signer: &Signer, forge: &Forge) -> Reached {
    ask_the_window(Asked::KeyOnAccount {
        key: signer.public_line(),
        on: Some(OnForge::of(forge)),
    })
}

/// The type the key is added as, in the forge's own words.
fn usage_type(forge: &Forge) -> &'static str {
    match forge.kind {
        Kind::GitHub => "Key type Signing Key",
        Kind::GitLab => "Usage type Signing",
    }
}

/// A code the window shows beside the question, for the person to compare with this terminal.
#[cfg(unix)]
fn a_new_code() -> String {
    purlis_core::windowline::a_new_code()
}

#[cfg(not(unix))]
fn a_new_code() -> String {
    String::new()
}

#[cfg(unix)]
fn ask_the_window(asked: Asked) -> Reached {
    signingsetup::reach(asked, |asked| {
        purlis_core::windowline::ask(asked, &mut |line| voice::info(line.to_owned()))
    })
}

/// The window line is a Unix socket: elsewhere this command reaches no window.
#[cfg(not(unix))]
fn ask_the_window(_asked: Asked) -> Reached {
    Reached::NoWindow
}

/// Why the key is added by hand, said before the steps, where there is more to say than the steps.
fn why_by_hand(reached: &Reached, forge: &Forge) -> Option<String> {
    match reached {
        Reached::Heard(Heard::KeyOnAccount { on: false }) => None,
        Reached::NoWindow => Some(format!(
            "No purlis window is running, so {} was not asked. Add the key by hand, then open \
             purlis and run `purlis signing status` to check it:",
            forge_said(forge)
        )),
        other => Some(format!("{} Add the key by hand:", not_asked(other, forge))),
    }
}

/// Why `forge` was not asked whether it has the key, or how a commit shows, in a sentence.
fn not_asked(reached: &Reached, forge: &Forge) -> String {
    let named = forge_said(forge);
    match reached {
        Reached::NoWindow => format!(
            "No purlis window is running, so {named} was not asked. Open purlis and run this \
             again."
        ),
        Reached::NoAnswer => {
            format!("The purlis window did not answer in time, so {named} was not asked.")
        }
        Reached::OtherAnswer | Reached::Heard(Heard::Checked(_) | Heard::KeyOnAccount { .. }) => {
            purlis_core::windowline::OTHER_ANSWER.to_owned()
        }
        Reached::Refused(why) => format!(
            "The purlis window could not ask {named}: {}.",
            why.trim_end_matches('.')
        ),
    }
}

/// The steps to add the key on the forge's site: GitHub's new key page, or the GitLab's SSH
/// keys page with *Signing* as the usage type.
fn by_hand_steps(signer: &Signer, forge: &Forge) {
    let page = signingsetup::new_key_page(forge);
    match forge.kind {
        Kind::GitHub => voice::info(format!(
            "1. Open {page} (GitHub › Settings › SSH and GPG keys › New SSH key)."
        )),
        Kind::GitLab => voice::info(format!(
            "1. Open {page} (GitLab › Preferences › SSH Keys › Add new key)."
        )),
    }
    voice::info("2. Key: paste this line, whole:");
    println!("{}", signer.public_line());
    voice::info(format!("3. Title: {}", signingkeys::title()));
    match forge.kind {
        Kind::GitHub => {
            voice::info("4. Key type: Signing Key (not Authentication Key).");
            voice::info("5. Press Add SSH key.");
        }
        Kind::GitLab => {
            voice::info(
                "4. Usage type: Signing (not Authentication & Signing). Leave the expiry empty, \
                 or set one you'll renew.",
            );
            voice::info("5. Press Add key.");
        }
    }
}

/// How often the window is asked whether GitHub has the key yet, while the person adds it.
const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// How long setup watches for the key before it goes on to its check all the same.
const WATCH_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// **Waits while the person adds the key by hand**, where they are at a terminal: it goes on once
/// the window hears GitHub has it, or the person presses Enter. With no window, Enter alone. `false`
/// where nobody is at a terminal to wait for.
fn waited_for_the_key(signer: &Signer, forge: &Forge, first: &Reached) -> bool {
    if !std::io::stdin().is_terminal() {
        voice::info(format!(
            "Once it is added, run `{SETUP}` again to check it."
        ));
        return false;
    }
    let watching = matches!(first, Reached::Heard(Heard::KeyOnAccount { on: false }));
    if watching {
        eprint!(
            "purlis goes on by itself once {} has the key, or press Enter. ",
            forge_said(forge)
        );
    } else {
        eprint!("Press Enter once it is added. ");
    }
    let _ = std::io::stderr().flush();
    let (pressed, enter) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
        let _ = pressed.send(());
    });
    if !watching {
        return enter.recv().is_ok();
    }
    let until = std::time::Instant::now() + WATCH_FOR;
    while std::time::Instant::now() < until {
        if enter.recv_timeout(LOOK_EVERY).is_ok() {
            return true;
        }
        if matches!(
            on_the_account(signer, forge),
            Reached::Heard(Heard::KeyOnAccount { on: true })
        ) {
            eprintln!();
            return true;
        }
    }
    true
}

/// A secret typed at the terminal, never echoed.
fn ask_secret(prompt: &str) -> Result<String, String> {
    rpassword::prompt_password(prompt)
        .map_err(|why| format!("purlis could not read the passphrase ({why})."))
}

/// The email chosen in the window for `forge`'s account to sign chat commits as (#5788), where
/// one was.
fn chosen_email(config: &std::path::Path, forge: &Forge) -> Option<String> {
    let (_, login) = signed_in(config)
        .into_iter()
        .find(|(one, _)| one == forge)?;
    signingsetup::commit_email_of(
        config,
        &signingsetup::AccountRef::new(forge.kind, &forge.host, &login),
    )
}

/// Says which email address `forge`'s commits carry, which the forge must know as the
/// account's: the one chosen for that account in the window where one was, else git's own.
fn email_line(forge: &Forge, chosen: Option<String>) {
    let named = forge_said(forge);
    if let Some(email) = chosen {
        voice::info(format!(
            "Commits a chat makes on {named} carry {email}, the address chosen for that account \
             in the purlis window."
        ));
        return;
    }
    let settings = match forge.kind {
        Kind::GitHub => "GitHub › Settings › Emails",
        Kind::GitLab => "GitLab › Preferences › Emails",
    };
    let email = purlis_core::worktree::git::run(
        std::path::Path::new("/"),
        &["config", "--global", "--get", "user.email"],
        purlis_core::worktree::git::READ,
    )
    .ok()
    .filter(purlis_core::worktree::git::Run::ok)
    .map(|run| run.line().trim().to_owned())
    .filter(|email| !email.is_empty());
    match email {
        Some(email) => voice::info(format!(
            "Commits carry your email {email}. {named} shows them as Verified only if that \
             address is a verified one on your account ({settings}), or one you chose in the \
             purlis window's Signed commits setup."
        )),
        None => voice::warn(format!(
            "your git has no email set (`git config --global user.email`). Set it to an address \
             your {named} account has verified, or {named} can't show the commits as Verified."
        )),
    }
}

fn status(
    config: &std::path::Path,
    keyring: &dyn purlis_core::secrets::keyring::Store,
) -> Result<u8, String> {
    let Some(signer) = signingsetup::key_kept(config, keyring)? else {
        voice::warn(format!(
            "this machine has no signing key, so the commits chats make are unsigned, and a \
             repository that requires signed commits refuses pull requests made of them. Run \
             `{SETUP}` to set one up."
        ));
        return Ok(1);
    };
    voice::ok(format!(
        "this machine signs the commits chats make, with key {}.",
        signer.fingerprint()
    ));
    println!("{}", signer.public_line());
    // Each account the window is signed in to, GitHub's and GitLab's alike (#5793); GitHub
    // where it is signed in to none, and the window says so.
    let mut forges = accounts(config);
    if forges.is_empty() {
        forges.push(signingsetup::github());
    }
    for forge in &forges {
        email_line(forge, chosen_email(config, forge));
        let named = forge_said(forge);
        let host = if forge.kind == Kind::GitLab {
            format!(" --host {}", forge.host)
        } else {
            String::new()
        };
        match on_the_account(&signer, forge) {
            Reached::Heard(Heard::KeyOnAccount { on: true }) => {
                voice::ok(format!("{named} has it as a signing key on your account."));
            }
            Reached::Heard(Heard::KeyOnAccount { on: false }) => voice::warn(format!(
                "{named} does not list it on your account, so its commits don't show as \
                 Verified there. Run `{SETUP}{host}` to add it."
            )),
            other => voice::warn(not_asked(&other, forge)),
        }
    }
    Ok(0)
}

fn check(forge: &Forge, repo: &str, commit: &str) -> Result<u8, String> {
    match forge.kind {
        Kind::GitHub => signingkeys::repo_named(repo).map(|_| ())?,
        Kind::GitLab => signingkeys::project_named(repo)?,
    }
    signingkeys::commit_named(commit)?;
    let code = a_new_code();
    let on = if *forge == signingsetup::github() {
        String::new()
    } else {
        format!(" on {}", forge_said(forge))
    };
    voice::info(format!(
        "Confirm checking {commit} in {repo}{on} in the purlis window, if it shows the code \
         {code}…"
    ));
    let reached = ask_the_window(Asked::Check {
        repo: repo.to_owned(),
        commit: commit.to_owned(),
        code,
        on: Some(OnForge::of(forge)),
    });
    let Reached::Heard(Heard::Checked(checked)) = reached else {
        return Err(not_asked(&reached, forge));
    };
    let said = purlis_core::commitsign::reason_said_on(&checked.reason, &forge_said(forge));
    if checked.verified {
        voice::ok(format!("{commit} in {repo}: {said}."));
        Ok(0)
    } else {
        voice::warn(format!("{commit} in {repo} is not Verified: {said}."));
        Ok(1)
    }
}
