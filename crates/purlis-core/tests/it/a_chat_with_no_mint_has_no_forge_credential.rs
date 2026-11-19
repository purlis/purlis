// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat with no mint holds no forge credential** (#7205, #7133 design §6.1): for each
//! harness, a chat's environment is composed as a start composes it, from an app whose own
//! environment and home hold a person's forge login, and every route the probe tries returns
//! nothing of that person.
//!
//! The person is made for the test: a home with a `gh` login file, a git credential helper and an
//! ssh config naming an agent, an agent socket in the app's environment, and a stand-in `gh`
//! that answers as the real one does from its config folder. No real credential is read: the
//! probe never touches the machine's own system git config, keychain or home.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use purlis_core::chatenv::{self, Starting};
use purlis_core::forgelock::{self, PERSON, Route};
use purlis_core::harness::Harness;

/// The person: a home, and a stand-in `gh` that reads `$GH_CONFIG_DIR/hosts.yml`, or the home's.
struct Person {
    dir: tempfile::TempDir,
}

impl Person {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join(".config/gh")).unwrap();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(
            home.join(".config/gh/hosts.yml"),
            format!("github.com:\n    user: {PERSON}\n"),
        )
        .unwrap();
        std::fs::write(
            home.join(".gitconfig"),
            format!(
                "[user]\n\tname = {PERSON}\n[credential]\n\thelper = \"!f() {{ echo username={PERSON}; echo password=pw; }}; f\"\n"
            ),
        )
        .unwrap();
        std::fs::write(
            home.join(".ssh/config"),
            format!("Host *\n  IdentityAgent /nowhere/{PERSON}.sock\n"),
        )
        .unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let gh = bin.join("gh");
        std::fs::write(
            &gh,
            "#!/bin/sh\n\
             dir=\"${GH_CONFIG_DIR:-$HOME/.config/gh}\"\n\
             if [ -f \"$dir/hosts.yml\" ]; then cat \"$dir/hosts.yml\"; exit 0; fi\n\
             echo 'gh: not logged in' >&2; exit 4\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self { dir }
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn gh(&self) -> PathBuf {
        self.dir.path().join("bin/gh")
    }

    /// The app's own environment: the person's home, an agent socket, and `gh` on the path.
    fn app(&self) -> Vec<(OsString, OsString)> {
        let path = format!("{}:/usr/bin:/bin", self.dir.path().join("bin").display());
        vec![
            ("HOME".into(), self.home().into()),
            ("PATH".into(), path.into()),
            (
                "SSH_AUTH_SOCK".into(),
                format!("/nowhere/{PERSON}.agent").into(),
            ),
        ]
    }
}

/// A chat of `harness`'s environment as its start composes it, the lock applied as the sandbox's
/// line applies it for a chat that holds the credentials class or has a mint.
fn chat_env(person: &Person, harness: Harness, scratch: &Path) -> Vec<(String, String)> {
    let starting = Starting {
        harness: Some(harness),
        operator: &[],
        strip: &[],
        set: &[],
        git_hooks: None,
        declared: &[],
    };
    let composed: Vec<(String, String)> = chatenv::compose(person.app(), &starting)
        .into_iter()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect();
    forgelock::apply(composed, Some(scratch), Some(&person.home())).expect("the lock's files")
}

#[test]
fn a_chat_with_no_mint_has_no_forge_credential() {
    purlis_core::unsteered!();
    let person = Person::new();
    let mut leaks = Vec::new();
    for harness in Harness::ALL {
        let scratch = tempfile::tempdir().unwrap();
        let env = chat_env(&person, harness, scratch.path());
        let found = forgelock::probe(&env, scratch.path(), &person.gh());
        // Every route the environment closes was tried.
        for route in [
            Route::GhNoMint,
            Route::GhAbsolutePath,
            Route::GitHttps,
            Route::SshAgent,
        ] {
            assert!(
                found.iter().any(|one| one.route == route),
                "{harness:?} {route:?} was not probed"
            );
        }
        leaks.extend(
            found
                .iter()
                .filter(|one| one.leaked)
                .map(|one| format!("{harness:?} {:?}: {}", one.route, one.said.trim())),
        );
    }
    assert!(
        leaks.is_empty(),
        "a route returned the person's identity:\n{}",
        leaks.join("\n")
    );
}
