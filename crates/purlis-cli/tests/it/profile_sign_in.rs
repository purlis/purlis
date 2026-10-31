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
        self.command(env, args).output().expect("the binary runs")
    }

    /// The command [`Self::purlis_with`] runs.
    fn command(&self, env: &[(&str, &str)], args: &[&str]) -> std::process::Command {
        let path = std::env::join_paths(std::iter::once(self.path("bin")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        let mut command = support::purlis();
        command
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
            .envs(env.iter().copied());
        command
    }

    /// `name`, a profile of `harness` this machine keeps for a desktop, as a runner does.
    fn keeps_for_a_desktop(&self, name: &str, harness: &str) {
        purlis_core::profilestore::onrunner::keep(
            &purlis_core::profilestore::Homes::at(&self.path("config-home"), &self.path("data")),
            &self.path("home"),
            &purlis_core::profilestore::onrunner::Kept {
                name: name.to_owned(),
                harness: harness.to_owned(),
                sandbox_only: false,
                pinned: None,
                settings_in: None,
            },
            "01JA0DESKT0P00000000000000",
            7,
        )
        .expect("kept");
    }

    /// The records of what this machine's logins wait for.
    fn waiting(&self) -> usize {
        fs::read_dir(self.path("data/app-state/sign-ins-waiting")).map_or(0, Iterator::count)
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

/// **`--pool` is not combined with the hidden runner modes** (#4043): those are a runner's own,
/// for its desktop, and a sign-in on a pool is followed here.
#[test]
fn a_pool_sign_in_is_not_held_as_a_runner_s_own() {
    let machine = Machine::new(0);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    for held in [
        &["--until-input-ends"][..],
        &["--in-own-terminal", "80x16"][..],
    ] {
        let mut args = vec!["profile", "sign-in", "work", "--pool", "builds"];
        args.extend_from_slice(held);
        let out = machine.purlis(&args);
        assert!(!out.status.success(), "{args:?}");
    }
    assert_eq!(machine.ran(), None);
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

/// **A profile this machine does not have, or an opencode profile with no plan, says what to
/// do** before any engine is looked for, and exits non-zero.
#[test]
fn a_missing_profile_or_plan_says_what_to_do() {
    let machine = Machine::new(1);
    machine.said(&["profile", "create", "oc", "--harness", "opencode"]);

    let nobody = machine.purlis(&["profile", "sign-in", "nobody", "--pool", "builds"]);
    assert!(!nobody.status.success());
    let said = String::from_utf8_lossy(&nobody.stderr);
    assert!(said.contains("purlis profile list"), "{said}");

    let no_plan = machine.purlis(&["profile", "sign-in", "oc", "--pool", "builds"]);
    assert!(!no_plan.status.success());
    let said = String::from_utf8_lossy(&no_plan.stderr);
    assert!(said.contains("github-copilot"), "{said}");

    machine.said(&["profile", "create", "fast", "--harness", "codex"]);
    let codex = machine.purlis(&["profile", "sign-in", "fast", "--pool", "builds"]);
    assert!(!codex.status.success());
    let said = String::from_utf8_lossy(&codex.stderr);
    assert!(said.contains("follows only"), "{said}");
    assert_eq!(machine.ran(), None);
}

/// **Sign out and a sign-in held to its input are a runner's, for its desktop** (#4043): on a
/// profile this device keeps for itself, they are refused in words, and no harness runs.
#[test]
fn signing_out_and_a_held_sign_in_are_only_for_a_profile_kept_for_a_desktop() {
    let machine = Machine::new(0);
    machine.said(&["profile", "create", "work", "--harness", "claude"]);

    for args in [
        &["profile", "sign-out", "work"][..],
        &["profile", "sign-in", "work", "--until-input-ends"][..],
        &["profile", "sign-in", "work", "--in-own-terminal", "80x16"][..],
    ] {
        let out = machine.purlis(args);

        assert!(!out.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("not kept on this device for a desktop"),
            "{out:?}"
        );
    }
    assert_eq!(machine.ran(), None);
}

/// **A sign-in cancelled before its login starts starts none**: while a runner fetches its
/// harness, what drives it may go away. One still driven starts.
#[test]
fn a_held_sign_in_whose_input_closed_starts_no_login() {
    let machine = Machine::new(0);
    machine.keeps_for_a_desktop("work", "claude");
    let args = ["profile", "sign-in", "work", "--until-input-ends"];

    let closed = machine
        .command(&[], &args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the binary runs");
    assert!(!closed.status.success(), "{closed:?}");
    assert_eq!(machine.ran(), None);

    let mut driven = machine
        .command(&[], &args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the binary runs");
    let input = driven.stdin.take();
    assert!(driven.wait().expect("it ended").success());
    drop(input);
    assert!(machine.ran().is_some());
}

/// **A login that waits for the browser is noted on the runner as it shows its page**, and no
/// longer once it ended: nothing else is handed a browser's answer there.
#[test]
fn a_held_login_that_waits_for_the_browser_is_noted_while_it_runs() {
    use std::io::BufRead;

    let machine = Machine::new(0);
    machine.keeps_for_a_desktop("work", "codex");
    let page = "https://auth.openai.com/oauth/authorize?response_type=code&client_id=\
                app_EMoamEEZ73f0CkXaXp7hrann&redirect_uri=http%3A%2F%2F127.0.0.1%3A1455%2Fauth%2F\
                callback&code_challenge=c&code_challenge_method=S256&state=\
                Zm9yLXRoaXMtc2lnbi1pbi1hbG9uZQ";
    stand_in::program(
        &machine.path("bin"),
        "codex",
        &format!("#!/bin/sh\necho '{page}'\nexec sleep 60\n"),
    );

    let mut held = machine
        .command(&[], &["profile", "sign-in", "work", "--until-input-ends"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the binary runs");
    let input = held.stdin.take();
    let mut line = String::new();
    std::io::BufReader::new(held.stdout.take().expect("its output"))
        .read_line(&mut line)
        .unwrap();

    assert_eq!(line.trim_end(), page, "its page passes on");
    assert_eq!(machine.waiting(), 1);

    drop(input);
    held.wait().expect("it ended");
    assert_eq!(machine.waiting(), 0);
}
