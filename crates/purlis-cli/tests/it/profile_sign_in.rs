// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis profile sign-in` (#2035, ADR 0095 §3): a profile of this machine's is signed in
//! through its harness's own sign-in, run in the person's terminal with the profile's folder
//! set, and `--check` says whether its harness says it is signed in here. A stand-in for each
//! harness is first on `PATH`, and says how it was run.

use crate::support;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

/// A machine of the test's own, with stand-ins for Claude Code and Codex that write the words
/// they were run with, and the folder variable they were given, to `ran`, and answer their
/// status words with `status`.
struct Machine {
    dir: tempfile::TempDir,
}

impl Machine {
    fn new(status: u8) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for one in ["home", "config-home", "data", "project", "bin"] {
            fs::create_dir_all(dir.path().join(one)).unwrap();
        }
        fs::write(dir.path().join("project/charter.toml"), "").unwrap();
        let ran = dir.path().join("ran");
        for (harness, status_words, variable) in [
            ("claude", "auth status", "CLAUDE_CONFIG_DIR"),
            ("codex", "login status", "CODEX_HOME"),
            ("opencode", "no status", "XDG_DATA_HOME"),
        ] {
            stand_in::program(
                &dir.path().join("bin"),
                harness,
                &format!(
                    "#!/bin/sh\nif [ \"$1 $2\" = '{status_words}' ]; then exit {status}; fi\nprintf '%s\\n' \"$*\" \"${variable}\" > '{}'\n",
                    ran.display()
                ),
            );
        }
        Self { dir }
    }

    fn path(&self, under: &str) -> PathBuf {
        self.dir.path().join(under)
    }

    fn purlis(&self, args: &[&str]) -> Output {
        self.purlis_with(&[], args)
    }

    /// [`Self::purlis`], with `env` set too.
    fn purlis_with(&self, env: &[(&str, &str)], args: &[&str]) -> Output {
        let path = std::env::join_paths(std::iter::once(self.path("bin")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        support::purlis()
            .args(args)
            .current_dir(self.path("project"))
            .env("CHARTER_ROOT", self.path("project"))
            .env("HOME", self.path("home"))
            .env("PATH", path)
            .env("PURLIS_DATA_HOME", self.path("data"))
            .env("PURLIS_CONFIG_HOME", self.path("config-home"))
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("PURLIS_CHAT")
            .env_remove("CHARTER_CHAT")
            .envs(env.iter().copied())
            .output()
            .expect("the binary runs")
    }

    fn said(&self, args: &[&str]) -> String {
        let out = self.purlis(args);
        assert!(out.status.success(), "{out:?}");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    /// What the stand-in harness was last run with: its words, and its folder variable.
    fn ran(&self) -> Option<String> {
        fs::read_to_string(self.path("ran")).ok()
    }
}

#[test]
fn signing_in_runs_claude_codes_own_sign_in_with_the_profiles_folder_set() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let said = machine.said(&["profile", "sign-in", "work"]);

    let config = machine.path("data/profiles/work/config");
    assert_eq!(
        machine.ran().as_deref(),
        Some(format!("auth login\n{}\n", config.display()).as_str())
    );
    assert!(said.contains("signing 'work' in"), "{said}");
}

#[test]
fn signing_in_runs_codexs_own_login_with_the_profiles_home_set() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "client", "--harness", "codex"]);

    machine.said(&["profile", "sign-in", "client"]);

    let config = machine.path("data/profiles/client/config");
    assert_eq!(
        machine.ran().as_deref(),
        Some(format!("login\n{}\n", config.display()).as_str())
    );
}

#[test]
fn check_says_a_profile_its_harness_says_is_signed_out_is_not_signed_in_and_runs_no_sign_in() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let out = machine.purlis(&["profile", "sign-in", "work", "--check"]);

    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("'work' is not signed in"), "{said}");
    assert!(said.contains("purlis profile sign-in work"), "{said}");
    assert_eq!(machine.ran(), None, "nothing but the status was run");
}

#[test]
fn check_says_a_profile_its_harness_says_is_signed_in_is() {
    let machine = Machine::new(0);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let said = machine.said(&["profile", "sign-in", "work", "--check"]);

    assert!(said.contains("'work' is signed in on"), "{said}");
}

#[test]
fn a_chat_may_not_sign_a_profile_in() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let out = support::purlis()
        .args(["profile", "sign-in", "work"])
        .current_dir(machine.path("project"))
        .env("HOME", machine.path("home"))
        .env("PURLIS_DATA_HOME", machine.path("data"))
        .env("PURLIS_CONFIG_HOME", machine.path("config-home"))
        .env("PURLIS_CHAT", "7")
        .output()
        .expect("the binary runs");

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("a chat may not"),
        "{out:?}"
    );
    assert_eq!(machine.ran(), None);
}

#[test]
fn a_profile_this_machine_does_not_have_is_named_as_missing() {
    let machine = Machine::new(1);

    let out = machine.purlis(&["profile", "sign-in", "nobody"]);

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no profile named 'nobody'"),
        "{out:?}"
    );
}

#[test]
fn an_opencode_profile_signs_in_to_the_plan_named_and_never_without_one() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "oc", "--harness", "opencode"]);

    let refused = machine.purlis(&["profile", "sign-in", "oc"]);
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("never"),
        "{refused:?}"
    );
    let anthropic = machine.purlis(&["profile", "sign-in", "oc", "--provider", "anthropic"]);
    assert!(!anthropic.status.success());
    assert_eq!(machine.ran(), None, "no sign-in ran");

    machine.said(&["profile", "sign-in", "oc", "--provider", "github-copilot"]);

    let data = machine.path("data/profiles/oc/data");
    assert_eq!(
        machine.ran().as_deref(),
        Some(format!("auth login --provider github-copilot\n{}\n", data.display()).as_str())
    );
}

/// An engine at a socket only root could have put there, so a pool's refusals past it are read.
const LOCAL_ENGINE: (&str, &str) = ("DOCKER_HOST", "unix:///var/run/docker.sock");

/// **`--check` says what a sign-in on a pool's runner reads it by**: the words of each answer.
#[test]
fn a_runner_is_read_by_the_words_check_writes() {
    use purlis_core::dockerpool::standing_said;
    use purlis_core::profilestore::sign_in::Standing;
    for (status, standing) in [(0, Standing::SignedIn), (1, Standing::SignedOut)] {
        let machine = Machine::new(status);
        machine.said(&["profile", "create", "work", "--harness", "claude"]);
        let out = machine.purlis(&["profile", "sign-in", "work", "--check"]);
        let said = String::from_utf8_lossy(&out.stderr);
        assert_eq!(standing_said("work", &said), Ok(standing), "{said}");
    }

    let machine = Machine::new(0);
    machine.said(&["profile", "create", "oc", "--harness", "opencode"]);
    let said = machine.said(&["profile", "sign-in", "oc", "--check"]);
    assert_eq!(
        standing_said("oc", &said),
        Ok(Standing::CannotTell),
        "{said}"
    );

    let out = machine.purlis(&["profile", "sign-in", "nobody", "--check"]);
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(standing_said("nobody", &said).is_err(), "{said}");
}

/// **`--pool` is in the help**, and asks nothing of whether a profile is signed in here.
#[test]
fn the_help_names_the_pool_and_a_check_is_not_of_a_pool() {
    let machine = Machine::new(0);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let help = machine.purlis(&["profile", "sign-in", "--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("--pool <POOL>"), "{help}");

    let out = machine.purlis(&["profile", "sign-in", "work", "--pool", "builds", "--check"]);
    assert!(!out.status.success());
}

/// **A pool's sign-in is the person's too**: a chat is refused it, and nothing is run.
#[test]
fn a_chat_may_not_sign_a_profile_in_on_a_pool() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let out = machine.purlis_with(
        &[("PURLIS_CHAT", "a-chat")],
        &["profile", "sign-in", "work", "--pool", "builds"],
    );

    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("a chat may not"), "{said}");
}

/// **Only a pool shown to be the person's own is signed in on**: an engine elsewhere is refused
/// before anything else, and says why.
#[test]
fn an_engine_not_shown_to_be_on_this_machine_is_refused() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let out = machine.purlis_with(
        &[("DOCKER_HOST", "tcp://10.0.0.1:2375")],
        &["profile", "sign-in", "work", "--pool", "builds"],
    );

    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("not known to be yours"), "{said}");
    assert_eq!(machine.ran(), None);
}

/// **A profile or a pool this machine does not have says where to find them**, and exits non-zero.
#[test]
fn a_profile_or_pool_this_machine_does_not_have_says_what_to_do() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    let nobody = machine.purlis_with(
        &[LOCAL_ENGINE],
        &["profile", "sign-in", "nobody", "--pool", "builds"],
    );
    assert!(!nobody.status.success());
    let said = String::from_utf8_lossy(&nobody.stderr);
    assert!(said.contains("purlis profile list"), "{said}");

    let no_pool = machine.purlis_with(
        &[LOCAL_ENGINE],
        &["profile", "sign-in", "work", "--pool", "builds"],
    );
    assert!(!no_pool.status.success());
    let said = String::from_utf8_lossy(&no_pool.stderr);
    assert!(said.contains("Settings › Pools"), "{said}");
    assert_eq!(machine.ran(), None);
}

/// **An opencode profile on a pool names its plan**, as it does here.
#[test]
fn a_pool_sign_in_of_opencode_names_its_plan() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "oc", "--harness", "opencode"]);

    let out = machine.purlis_with(
        &[LOCAL_ENGINE],
        &["profile", "sign-in", "oc", "--pool", "builds"],
    );

    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("github-copilot"), "{said}");
}
