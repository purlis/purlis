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
//! on #5625): it never runs `gh`, and it asks the running purlis window whether GitHub has the
//! key or shows a commit as Verified, which the window asks with its own sign-in. The key is
//! added on GitHub's page by hand from here; with no window running, the command gives those
//! steps and says to open the window.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;

use clap::Subcommand;
use purlis_core::commitsign::{self, SETUP, Signer};
use purlis_core::forge::signingkeys;
use purlis_core::signingsetup::{self, Asked, Heard, Reached};

use crate::voice;

#[derive(Subcommand)]
pub enum SigningCommand {
    /// Set up the key the app signs your chats' commits with, step by step: purlis makes one (or
    /// takes yours with `--key`), shows how to add it to your GitHub account as a signing key,
    /// and has the purlis window check that GitHub has it.
    Setup {
        /// Use a key you already have (an Ed25519 private key file, such as `~/.ssh/id_ed25519`)
        /// instead of a new one. If it has a passphrase, you type it once here.
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
    },
    /// Say whether this machine has a signing key, show its public half, and have the purlis
    /// window ask GitHub whether your account has it.
    Status,
    /// Have the purlis window ask GitHub whether a pushed commit shows as Verified, and if not,
    /// why and what to do.
    Check {
        /// The repository on GitHub, as `owner/name`.
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
        SigningCommand::Setup { key } => setup(&config, &*keyring, key.clone()),
        SigningCommand::Status => status(&config, &*keyring),
        SigningCommand::Check { repo, commit } => check(repo, commit),
        SigningCommand::Remove => match signingsetup::key_removed(&config, &*keyring) {
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
    voice::info("Step 2 of 3: tell GitHub the key is yours.");
    let first = on_github(&signer);
    if matches!(first, Reached::Heard(Heard::KeyOnAccount { on: true })) {
        voice::ok("GitHub already has this key as a signing key on your account.");
    } else {
        if let Some(why) = why_by_hand(&first) {
            voice::info(why);
        }
        by_hand_steps(&signer);
        if !waited_for_the_key(&signer, &first) {
            return Ok(0);
        }
    }
    eprintln!();
    voice::info("Step 3 of 3: check that GitHub has it.");
    match on_github(&signer) {
        Reached::Heard(Heard::KeyOnAccount { on: true }) => {
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
        Reached::Heard(Heard::KeyOnAccount { on: false }) => {
            voice::warn(format!(
                "GitHub does not list the key on your account yet. Add it as described above, \
                 with Key type Signing Key, then run `{SETUP}` again: the key is kept, so it \
                 goes straight to this check."
            ));
            Ok(1)
        }
        other => {
            voice::warn(format!(
                "{} If you added the key, commits are signed already; `purlis signing status` \
                 checks again later.",
                not_asked(&other)
            ));
            Ok(1)
        }
    }
}

/// What the running window heard from GitHub of `signer`'s key on the account.
fn on_github(signer: &Signer) -> Reached {
    ask_the_window(Asked::KeyOnAccount {
        key: signer.public_line(),
    })
}

#[cfg(unix)]
fn ask_the_window(asked: Asked) -> Reached {
    signingsetup::reach(asked, purlis_core::windowline::ask)
}

/// The window line is a Unix socket: elsewhere this command reaches no window.
#[cfg(not(unix))]
fn ask_the_window(_asked: Asked) -> Reached {
    Reached::NoWindow
}

/// Why the key is added by hand, said before the steps, where there is more to say than the steps.
fn why_by_hand(reached: &Reached) -> Option<String> {
    match reached {
        Reached::Heard(Heard::KeyOnAccount { on: false }) => None,
        Reached::NoWindow => Some(
            "No purlis window is running, so GitHub was not asked. Add the key by hand, then \
             open purlis and run `purlis signing status` to check it:"
                .to_owned(),
        ),
        other => Some(format!("{} Add the key by hand:", not_asked(other))),
    }
}

/// Why GitHub was not asked whether it has the key, or how a commit shows, in a sentence.
fn not_asked(reached: &Reached) -> String {
    match reached {
        Reached::NoWindow => {
            "No purlis window is running, so GitHub was not asked. Open purlis and run this \
             again."
                .to_owned()
        }
        Reached::NoAnswer => {
            "The purlis window did not answer in time, so GitHub was not asked.".to_owned()
        }
        Reached::OtherAnswer | Reached::Heard(Heard::Checked(_) | Heard::KeyOnAccount { .. }) => {
            purlis_core::windowline::OTHER_ANSWER.to_owned()
        }
        Reached::Heard(Heard::NeedsPermission) => "purlis's sign-in to GitHub may not read your \
             signing keys yet, so GitHub was not asked."
            .to_owned(),
        Reached::Refused(why) => format!(
            "The purlis window could not ask GitHub: {}.",
            why.trim_end_matches('.')
        ),
    }
}

/// The steps to add the key on GitHub's site.
fn by_hand_steps(signer: &Signer) {
    voice::info(format!(
        "1. Open {} (GitHub › Settings › SSH and GPG keys › New SSH key).",
        signingsetup::NEW_KEY_PAGE
    ));
    voice::info(format!("2. Title: {}", signingkeys::title()));
    voice::info("3. Key type: Signing Key (not Authentication Key).");
    voice::info("4. Key: paste this line, whole:");
    println!("{}", signer.public_line());
    voice::info("5. Press Add SSH key.");
    voice::info(
        "On GitLab it is Preferences › SSH Keys, with Usage type Signing; GitLab's check below is \
         not made yet.",
    );
}

/// How often the window is asked whether GitHub has the key yet, while the person adds it.
const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// How long setup watches for the key before it goes on to its check all the same.
const WATCH_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// **Waits while the person adds the key by hand**, where they are at a terminal: it goes on once
/// the window hears GitHub has it, or the person presses Enter. With no window, Enter alone. `false`
/// where nobody is at a terminal to wait for.
fn waited_for_the_key(signer: &Signer, first: &Reached) -> bool {
    if !std::io::stdin().is_terminal() {
        voice::info(format!(
            "Once it is added, run `{SETUP}` again to check it."
        ));
        return false;
    }
    let watching = matches!(first, Reached::Heard(Heard::KeyOnAccount { on: false }));
    if watching {
        eprint!("purlis goes on by itself once GitHub has the key, or press Enter. ");
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
            on_github(signer),
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
    match on_github(&signer) {
        Reached::Heard(Heard::KeyOnAccount { on: true }) => {
            voice::ok("GitHub has it as a signing key on your account.");
        }
        Reached::Heard(Heard::KeyOnAccount { on: false }) => voice::warn(format!(
            "GitHub does not list it on your account, so its commits don't show as Verified. \
             Run `{SETUP}` to add it."
        )),
        other => voice::warn(not_asked(&other)),
    }
    email_line();
    Ok(0)
}

fn check(repo: &str, commit: &str) -> Result<u8, String> {
    signingkeys::repo_named(repo)?;
    signingkeys::commit_named(commit)?;
    let reached = ask_the_window(Asked::Check {
        repo: repo.to_owned(),
        commit: commit.to_owned(),
    });
    let Reached::Heard(Heard::Checked(checked)) = reached else {
        return Err(not_asked(&reached));
    };
    let said = checked.said();
    if checked.verified {
        voice::ok(format!("{commit} in {repo}: {said}."));
        Ok(0)
    } else {
        voice::warn(format!("{commit} in {repo} is not Verified: {said}."));
        Ok(1)
    }
}
