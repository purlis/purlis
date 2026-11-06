// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis signing`** (#5480): the key the app signs a chat's commits with, set up once by the
//! person in their own terminal, in plain steps.
//!
//! A repository that requires signed commits refuses a pull request whose commits are unsigned.
//! The commits a sandboxed chat makes are made by the app, so the app signs them, with a key it
//! keeps in this machine's keychain ([`purlis_core::commitsign`]). The chat never holds it, so
//! none of this runs in a chat.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::time::Duration;

use clap::Subcommand;
use purlis_core::commitsign::{self, SETUP, Signer};
use purlis_core::forge::Kind;
use purlis_core::forge::cli::Cli;

use crate::voice;

#[derive(Subcommand)]
pub enum SigningCommand {
    /// Set up the key the app signs your chats' commits with, step by step: purlis makes one (or
    /// takes yours with `--key`), adds it to your GitHub account as a signing key, and checks
    /// that GitHub has it.
    Setup {
        /// Use a key you already have (an Ed25519 private key file, such as `~/.ssh/id_ed25519`)
        /// instead of a new one. If it has a passphrase, you type it once here.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// Don't add the key to GitHub with `gh`: show the steps to add it by hand.
        #[arg(long)]
        by_hand: bool,
    },
    /// Say whether this machine has a signing key, show its public half, and ask GitHub whether
    /// your account has it.
    Status,
    /// Ask GitHub whether a pushed commit shows as Verified, and if not, why and what to do.
    Check {
        /// The repository on GitHub, as `owner/name`.
        repo: String,
        /// The commit, by its full or short id.
        commit: String,
    },
    /// Remove the signing key from this machine's keychain. Commits are made unsigned after it.
    Remove,
}

/// How long one GitHub call may take.
const GITHUB: Duration = Duration::from_secs(30);

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
        SigningCommand::Setup { key, by_hand } => setup(&config, &*keyring, key.clone(), *by_hand),
        SigningCommand::Status => status(&config, &*keyring),
        SigningCommand::Check { repo, commit } => check(repo, commit),
        SigningCommand::Remove => match commitsign::forget(&config, &*keyring) {
            Ok(true) => {
                voice::ok(
                    "the signing key is removed from this machine. Commits a chat makes are \
                     unsigned from now on. Remove it from GitHub too: Settings › SSH and GPG \
                     keys.",
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

fn setup(
    config: &std::path::Path,
    keyring: &dyn purlis_core::secrets::keyring::Store,
    key: Option<PathBuf>,
    by_hand: bool,
) -> Result<u8, String> {
    voice::info(
        "Some repositories accept only signed commits, so GitHub can show who made each one. \
         The commits a chat makes are made by the purlis app, and this sets up the key it signs \
         them with. Three steps.",
    );
    eprintln!();
    voice::info("Step 1 of 3: a key, kept in this machine's keychain.");
    let signer = match key {
        Some(file) => {
            let text = std::fs::read_to_string(&file)
                .map_err(|why| format!("purlis could not read {} ({why}).", file.display()))?;
            let passphrase = needs_passphrase(&text)
                .then(|| ask_secret(&format!("Passphrase for {}: ", file.display())))
                .transpose()?;
            let taken = commitsign::take(config, keyring, &text, passphrase.as_deref())
                .map_err(|why| format!("{why}. Nothing was changed."))?;
            voice::ok(format!(
                "your key {} is kept in this machine's keychain. Your file is left as it was.",
                taken.fingerprint()
            ));
            taken
        }
        None => {
            let had = commitsign::kept(config, keyring)?;
            let made = commitsign::make(config, keyring)?;
            if had.is_some() {
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
    voice::info("Step 2 of 3: tell GitHub the key is yours.");
    let added = if by_hand { false } else { add_with_gh(&signer) };
    if !added {
        by_hand_steps(&signer);
        if !wait_for_enter() {
            return Ok(0);
        }
    }
    eprintln!();
    voice::info("Step 3 of 3: check that GitHub has it.");
    match on_github(&signer) {
        Ok(true) => {
            voice::ok(
                "GitHub has the key as a signing key on your account. The commits your chats \
                 make are signed from now on, and show as Verified on GitHub.",
            );
            email_line();
            voice::info(
                "Once a chat's commit is pushed, `purlis signing check <owner/repo> <commit>` \
                 asks GitHub how it shows.",
            );
            Ok(0)
        }
        Ok(false) => {
            voice::warn(format!(
                "GitHub does not list the key on your account yet. Add it as described above, \
                 with Key type Signing Key, then run `{SETUP}` again: the key is kept, so it \
                 goes straight to this check."
            ));
            Ok(1)
        }
        Err(why) => {
            voice::warn(format!(
                "purlis could not ask GitHub ({why}). If you added the key, commits are signed \
                 already; `purlis signing status` checks again later."
            ));
            Ok(1)
        }
    }
}

/// Whether the key file `text` is locked with a passphrase.
fn needs_passphrase(text: &str) -> bool {
    commitsign::take_needs_passphrase(text)
}

/// The key added to the GitHub account `gh` is signed in to, as a signing key, or `false`
/// where it was not, said.
fn add_with_gh(signer: &Signer) -> bool {
    if on_github(signer).unwrap_or(false) {
        voice::ok("GitHub already has this key as a signing key on your account.");
        return true;
    }
    let args = vec![
        "api".to_owned(),
        "--method".to_owned(),
        "POST".to_owned(),
        "user/ssh_signing_keys".to_owned(),
        "-f".to_owned(),
        format!("title={}", title()),
        "-f".to_owned(),
        format!("key={}", signer.public_line()),
    ];
    match Cli::as_the_operator().run(Kind::GitHub, &args, GITHUB) {
        Ok(reply) if reply.code == 0 => {
            voice::ok("the key is added to your GitHub account as a signing key.");
            true
        }
        Ok(reply) if reply.err.contains("admin:ssh_signing_key") || reply.status == Some(404) => {
            voice::warn(
                "GitHub's command-line tool `gh` is not allowed to add signing keys yet. To let \
                 it, run `gh auth refresh -h github.com -s admin:ssh_signing_key`, follow what it \
                 says, then run this again. Or add the key by hand:",
            );
            false
        }
        Ok(reply) => {
            voice::warn(format!(
                "GitHub did not take the key ({}). Add it by hand:",
                reply.err.lines().next().unwrap_or("no reason given").trim()
            ));
            false
        }
        Err(_) => {
            voice::info(
                "GitHub's command-line tool `gh` is not set up here, so add the key by hand:",
            );
            false
        }
    }
}

/// The steps to add the key on GitHub's site.
fn by_hand_steps(signer: &Signer) {
    voice::info(
        "1. Open https://github.com/settings/ssh/new (GitHub › Settings › SSH and GPG keys › New SSH key).",
    );
    voice::info(format!("2. Title: {}", title()));
    voice::info("3. Key type: Signing Key (not Authentication Key).");
    voice::info("4. Key: paste this line, whole:");
    println!("{}", signer.public_line());
    voice::info("5. Press Add SSH key.");
    voice::info(
        "On GitLab it is Preferences › SSH Keys, with Usage type Signing; GitLab's check below is \
         not made yet.",
    );
}

/// Waits for the person to press Enter, where they are at a terminal: `false` where they can't.
fn wait_for_enter() -> bool {
    if !std::io::stdin().is_terminal() {
        voice::info(format!(
            "Once it is added, run `{SETUP}` again to check it."
        ));
        return false;
    }
    eprint!("Press Enter once it is added. ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).is_ok()
}

/// A secret typed at the terminal, never echoed.
fn ask_secret(prompt: &str) -> Result<String, String> {
    rpassword::prompt_password(prompt)
        .map_err(|why| format!("purlis could not read the passphrase ({why})."))
}

/// What GitHub calls the key: which machine's it is.
fn title() -> String {
    let machine = std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::env::consts::OS.to_owned());
    format!("purlis on {machine}")
}

/// Whether the GitHub account `gh` is signed in to has the key as a signing key.
fn on_github(signer: &Signer) -> Result<bool, String> {
    let args = vec![
        "api".to_owned(),
        "user/ssh_signing_keys".to_owned(),
        "--paginate".to_owned(),
    ];
    let reply = Cli::as_the_operator()
        .run(Kind::GitHub, &args, GITHUB)
        .map_err(|_| "GitHub's command-line tool `gh` is not set up here".to_owned())?;
    if reply.code != 0 {
        if reply.err.contains("admin:ssh_signing_key") || reply.status == Some(404) {
            return Err(
                "`gh` may not read your signing keys: run `gh auth refresh -h \
                 github.com -s admin:ssh_signing_key`"
                    .to_owned(),
            );
        }
        return Err(reply
            .err
            .lines()
            .next()
            .unwrap_or("no reason given")
            .trim()
            .to_owned());
    }
    // `--paginate` writes one list after another: read them as one.
    let joined = reply.out.trim().replace("][", ",");
    commitsign::listed_on_github(&joined, signer)
}

/// Says which email address the commits carry, which GitHub must know as the account's.
fn email_line() {
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
            "Commits carry your email {email}. GitHub shows them as Verified only if that address \
             is a verified one on your account (GitHub › Settings › Emails)."
        )),
        None => voice::warn(
            "your git has no email set (`git config --global user.email`). Set it to an address \
             your GitHub account has verified, or GitHub can't show the commits as Verified.",
        ),
    }
}

fn status(
    config: &std::path::Path,
    keyring: &dyn purlis_core::secrets::keyring::Store,
) -> Result<u8, String> {
    let Some(signer) = commitsign::kept(config, keyring)? else {
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
    match on_github(&signer) {
        Ok(true) => voice::ok("GitHub has it as a signing key on your account."),
        Ok(false) => voice::warn(format!(
            "GitHub does not list it on your account, so its commits don't show as Verified. \
             Run `{SETUP}` to add it."
        )),
        Err(why) => voice::warn(format!("purlis could not ask GitHub ({why}).")),
    }
    email_line();
    Ok(0)
}

fn check(repo: &str, commit: &str) -> Result<u8, String> {
    let named = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    let (owner, name) = repo
        .split_once('/')
        .filter(|(owner, name)| named(owner) && named(name))
        .ok_or_else(|| format!("{repo} is not a repository as `owner/name`."))?;
    if commit.is_empty() || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{commit} is not a commit id."));
    }
    let args = vec![
        "api".to_owned(),
        format!("repos/{owner}/{name}/commits/{commit}"),
    ];
    let reply = Cli::as_the_operator()
        .run(Kind::GitHub, &args, GITHUB)
        .map_err(|_| "GitHub's command-line tool `gh` is not set up here.".to_owned())?;
    if reply.code != 0 {
        return Err(format!(
            "GitHub did not find commit {commit} in {repo}: is it pushed? ({})",
            reply.err.lines().next().unwrap_or("").trim()
        ));
    }
    let (verified, reason) = commitsign::verification(&reply.out)?;
    let said = commitsign::reason_said(&reason);
    if verified {
        voice::ok(format!("{commit} in {repo}: {said}."));
        Ok(0)
    } else {
        voice::warn(format!("{commit} in {repo} is not Verified: {said}."));
        Ok(1)
    }
}
