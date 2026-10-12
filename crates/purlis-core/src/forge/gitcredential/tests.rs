// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's git credential helper (#752): what it answers, what it refuses, and that the git the
//! window runs hands the token to no other helper.
//!
//! The helper git starts here is this test binary, started again through a stand-in script
//! ([`Helper`]) that passes the helper's arguments to [`the_helper_child`] and keeps only its
//! answer: the app's binary does the same through `main`'s [`super::serve_if_asked`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;
use crate::forge::accounts::{Pasted, sign_in_with_token};
use crate::forge::http::TokenSource;
use crate::forge::recorded::Recorded;
use crate::forge::transport::Transport;
use crate::forge::{Forge, ForgeError, helper_for};
use crate::secrets::keyring::Store;
use crate::worktree::git;

/// The token every case signs in with. Not shaped like any forge's token: the commit hook scans
/// for those.
const TOKEN: &str = "kept-for-the-window-git-only";

/// The variable that tells a copy of this test binary it is the helper git started, holding the
/// helper's arguments one per line.
const CHILD: &str = "PURLIS_TEST_GIT_CREDENTIAL_CHILD";

/// The prefix the child's answer lines carry through libtest's own output.
const SAYS: &str = "HELPER-SAYS:";

/// A config home with one GitHub account signed in on `host`.
pub(crate) struct Machine {
    home: tempfile::TempDir,
    keyring: Box<dyn Store>,
    account: ForgeAccount,
}

impl Machine {
    pub(crate) fn signed_in_on(host: &str) -> Machine {
        let home = tempfile::tempdir().unwrap();
        let keyring = accounts::keyring_at(home.path());
        let recording = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract/accounts/github/token_check.json");
        let recorded =
            Arc::new(Recorded::parse(&std::fs::read_to_string(recording).unwrap()).unwrap());
        let reach =
            |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
                Ok(recorded.clone() as Arc<dyn Transport>)
            };
        let account = sign_in_with_token(
            &HostScope::for_a_test(),
            home.path(),
            keyring.as_ref(),
            &reach,
            Kind::GitHub,
            host,
            Pasted::new(TOKEN.to_string()),
        )
        .expect("the recorded forge takes the token");
        Machine {
            home,
            keyring,
            account,
        }
    }

    pub(crate) fn config(&self) -> &Path {
        self.home.path()
    }

    /// The helper's arguments as the window names them, with git's `operation` last.
    fn args(&self, operation: &str) -> Vec<String> {
        vec![
            "--config".into(),
            self.config().display().to_string(),
            "--account".into(),
            self.account.id.clone(),
            operation.into(),
        ]
    }

    /// What the helper answers `operation` with `sent` on its input, in an environment that
    /// answers `env`; and what it said on its standard error.
    fn ask(
        &self,
        operation: &str,
        sent: &str,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> (String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        serve(
            &self.args(operation),
            &mut sent.as_bytes(),
            &mut out,
            &mut err,
            env,
        );
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn token_is_kept(&self) -> bool {
        accounts::token(self.keyring.as_ref(), &self.account)
            .ok()
            .flatten()
            .is_some_and(|t| t.expose_secret() == TOKEN)
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

fn asked(protocol: &str, host: &str) -> String {
    format!("protocol={protocol}\nhost={host}\n\n")
}

#[test]
fn get_is_answered_with_the_accounts_login_and_token_for_its_host() {
    let machine = Machine::signed_in_on("github.example.com");

    let (out, _) = machine.ask("get", &asked("https", "github.example.com"), &no_env);

    assert_eq!(out, format!("username=octocat\npassword={TOKEN}\n"));
}

#[test]
fn store_and_erase_are_answered_with_nothing_and_change_nothing() {
    let machine = Machine::signed_in_on("github.example.com");
    let handed =
        "protocol=https\nhost=github.example.com\nusername=octocat\npassword=another-value\n\n";

    for operation in ["store", "erase", "a-later-operation"] {
        let (out, err) = machine.ask(operation, handed, &no_env);
        assert_eq!((out.as_str(), err.as_str()), ("", ""), "{operation}");
    }

    assert!(machine.token_is_kept(), "the account's token is as it was");
}

#[test]
fn a_chats_process_tree_is_refused_and_answered_nothing() {
    let machine = Machine::signed_in_on("github.example.com");
    for mark in [crate::hookwire::CHAT_ENV, crate::active::SESSION_ID_ENV] {
        let in_a_chat = |name: &str| (name == mark).then(|| "1".to_string());

        let (out, err) = machine.ask("get", &asked("https", "github.example.com"), &in_a_chat);

        assert_eq!(out, "", "{mark}");
        assert!(err.contains(REFUSED_IN_A_CHAT), "{mark}: {err}");
    }
}

#[test]
fn another_host_or_a_cleartext_protocol_gets_nothing() {
    let machine = Machine::signed_in_on("github.example.com");
    for (protocol, host) in [
        ("https", "gitlab.example.com"),
        ("https", "github.example.com:8443"),
        ("http", "github.example.com"),
        ("ssh", "github.example.com"),
    ] {
        let (out, _) = machine.ask("get", &asked(protocol, host), &no_env);
        assert_eq!(out, "", "{protocol}://{host}");
    }
}

#[test]
fn a_signed_out_account_gets_nothing() {
    let machine = Machine::signed_in_on("github.example.com");
    accounts::sign_out(
        &HostScope::for_a_test(),
        machine.config(),
        machine.keyring.as_ref(),
        &machine.account.id,
    )
    .unwrap();

    let (out, _) = machine.ask("get", &asked("https", "github.example.com"), &no_env);

    assert_eq!(out, "");
}

#[test]
fn the_helper_is_named_only_for_the_window_and_never_for_a_chats_brokered_call() {
    let machine = Machine::signed_in_on("github.example.com");
    let forge = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".into(),
    };
    let exe = Path::new("/Applications/purlis.app/Contents/MacOS/charter-app");

    assert!(for_the_window("github.example.com").is_none(), "outside");
    let (inside, other_host, brokered) =
        as_the_window(&HostScope::for_a_test(), exe, machine.config(), || {
            let inside = helper_for(&forge);
            let other_host = for_the_window("gitlab.example.com");
            let isolation = git::Isolated::identity(None, None);
            let brokered = git::isolated(&isolation, || for_the_window("github.example.com"));
            (inside, other_host, brokered)
        });

    assert_eq!(
        inside,
        format!(
            "!'{}' {HELPER_ARG} --config '{}' --account {}",
            exe.display(),
            machine.config().display(),
            machine.account.id
        )
    );
    assert!(!inside.contains("gh ") && !inside.contains("auth git-credential"));
    assert_eq!(other_host, None, "no account there: the CLI's helper stays");
    assert_eq!(brokered, None, "a call the app runs for a chat");
    assert!(for_the_window("github.example.com").is_none(), "after");
    assert!(
        helper_for(&forge).contains("auth git-credential"),
        "outside the window, the CLI's helper"
    );
}

#[test]
fn a_path_the_shell_could_not_quote_names_no_helper() {
    let machine = Machine::signed_in_on("github.example.com");
    let named = as_the_window(
        &HostScope::for_a_test(),
        Path::new("/Apps/it's here/charter-app"),
        machine.config(),
        || for_the_window("github.example.com"),
    );
    assert_eq!(named, None);
}

// ---------------------------------------------------------------------------------------
// Through git.

/// A stand-in for the app's binary: a script that starts this test binary as
/// [`the_helper_child`], with the helper's arguments in [`CHILD`], and keeps only its answer.
/// `extra` is shell run first, as a chat's environment would set it.
struct Helper {
    _dir: tempfile::TempDir,
    exe: PathBuf,
}

impl Helper {
    fn new(extra: &str) -> Helper {
        let dir = tempfile::tempdir().unwrap();
        let me = std::env::current_exe().unwrap();
        let script = format!(
            "#!/bin/sh\n\
             {extra}\n\
             {CHILD}=$(printf '%s\\n' \"$@\"); export {CHILD}\n\
             '{}' --exact forge::gitcredential::tests::the_helper_child --nocapture -q \
             --test-threads=1 2>/dev/null | sed -n 's/^{SAYS}//p'\n",
            me.display()
        );
        let exe = stand_in::program(dir.path(), "charter-app", &script);
        Helper { _dir: dir, exe }
    }
}

/// Not a test of its own: the helper git started, when [`CHILD`] is set; nothing otherwise.
#[test]
fn the_helper_child() {
    let Some(args) = std::env::var_os(CHILD) else {
        return;
    };
    let args: Vec<String> = args.to_string_lossy().lines().map(str::to_owned).collect();
    assert_eq!(args.first().map(String::as_str), Some(HELPER_ARG));
    let mut out = Vec::new();
    serve(
        &args[1..],
        &mut std::io::stdin().lock(),
        &mut out,
        &mut std::io::sink(),
        &crate::envvar::var,
    );
    for line in String::from_utf8_lossy(&out).lines() {
        println!("{SAYS}{line}");
    }
}

/// An alias that runs a `credential` subcommand of a git the call starts, which sees the call's
/// `-c` settings and so its helper list.
fn alias(name: &str, run: &str) -> String {
    format!("alias.{name}=!{run}")
}

/// The variable that tells a copy of this test binary to run the keychain case, its `HOME` a
/// folder whose git config names a recording helper.
const KEYCHAIN_CHILD: &str = "PURLIS_TEST_GIT_CREDENTIAL_KEYCHAIN";

#[test]
fn window_git_never_hands_the_token_to_a_helper_configured_outside_it() {
    // Apple's git names `osxkeychain` in its system-wide config. git asks every helper it is
    // configured with to store a credential that worked, so without the reset the token would
    // land in the login keychain. The stand-in here is named in the global config instead (a
    // test cannot write the system one); the system file is read before the global one, and
    // both before the command line, so the reset drops both alike. On a Mac the real
    // `osxkeychain` sits in the system file all along, and is dropped the same way.
    if let Some(recorded) = std::env::var_os(KEYCHAIN_CHILD) {
        let recorded = PathBuf::from(recorded);
        let machine = Machine::signed_in_on("github.example.com");
        let helper = Helper::new("");
        let forge = Forge {
            kind: Kind::GitHub,
            host: "github.example.com".into(),
        };
        let work = tempfile::tempdir().unwrap();
        let filled = work.path().join("filled");
        // Fill, as a fetch does; then approve what came back, as git does once the forge took
        // it; then reject it, as git does when the forge refused it.
        let flow = format!(
            "printf 'protocol=https\\nhost=github.example.com\\n\\n' | git credential fill > '{f}' \
             && git credential approve < '{f}' && git credential reject < '{f}'",
            f = filled.display()
        );
        let run = as_the_window(
            &HostScope::for_a_test(),
            &helper.exe,
            machine.config(),
            || {
                git::run_network(
                    work.path(),
                    Some(&helper_for(&forge)),
                    &["-c", &alias("flow", &flow), "flow"],
                )
            },
        )
        .expect("git runs");

        assert!(run.ok(), "{run:?}");
        let filled = std::fs::read_to_string(&filled).unwrap();
        assert!(
            filled.contains(&format!("password={TOKEN}")),
            "purlis's helper answered: {filled}"
        );
        assert!(
            !recorded.exists(),
            "the configured helper was never asked anything: {}",
            std::fs::read_to_string(&recorded).unwrap_or_default()
        );
        assert!(machine.token_is_kept());
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let recorded = home.path().join("keychain-asked");
    let keychain = stand_in::program(
        home.path(),
        "git-credential-osxkeychain",
        &format!(
            "#!/bin/sh\n{{ echo \"$1\"; cat; }} >> '{}'\n",
            recorded.display()
        ),
    );
    // Named twice: as every host's helper, and as the forge host's own
    // (`credential.<url>.helper`), which git keeps in the same list and the reset drops too.
    std::fs::write(
        home.path().join(".gitconfig"),
        format!(
            "[credential]\n\thelper = !'{k}'\n\
             [credential \"https://github.example.com\"]\n\thelper = !'{k}'\n",
            k = keychain.display()
        ),
    )
    .unwrap();
    let me = std::env::current_exe().unwrap();
    let out = crate::forklock::output(
        std::process::Command::new(me)
            .args([
                "--exact",
                "forge::gitcredential::tests::window_git_never_hands_the_token_to_a_helper_configured_outside_it",
                "--nocapture",
            ])
            .env(KEYCHAIN_CHILD, &recorded)
            .env("HOME", home.path()),
    )
    .expect("the test binary runs again");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!recorded.exists(), "the stand-in keychain was never asked");
}

#[test]
fn the_helper_git_starts_in_a_chats_process_tree_answers_nothing() {
    let machine = Machine::signed_in_on("github.example.com");
    let helper = Helper::new(&format!(
        "{}=1; export {}",
        crate::hookwire::CHAT_ENV,
        crate::hookwire::CHAT_ENV
    ));
    let forge = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".into(),
    };
    let work = tempfile::tempdir().unwrap();
    let filled = work.path().join("filled");
    let fill = format!(
        "printf 'protocol=https\\nhost=github.example.com\\n\\n' | git credential fill > '{}'",
        filled.display()
    );

    let run = as_the_window(
        &HostScope::for_a_test(),
        &helper.exe,
        machine.config(),
        || {
            git::run_network(
                work.path(),
                Some(&helper_for(&forge)),
                &["-c", &alias("fill", &fill), "fill"],
            )
        },
    )
    .expect("git runs");

    assert!(!run.ok(), "git found no credential: {run:?}");
    let filled = std::fs::read_to_string(&filled).unwrap_or_default();
    assert!(!filled.contains(TOKEN), "{filled}");
    assert!(!run.out.contains(TOKEN) && !run.err.contains(TOKEN));
}

mod over_http;

/// The window's helper on `host`, for a push from `clone` to `url`, as the window would ask.
fn push_helper(
    machine: &Machine,
    host: &str,
    clone: &Path,
    url: &str,
) -> Result<String, &'static str> {
    let on = Forge {
        kind: Kind::GitHub,
        host: host.to_string(),
    };
    as_the_window(
        &HostScope::for_a_test(),
        Path::new("/Applications/purlis.app/purlis"),
        machine.config(),
        || helper_for_push(&on, clone, url),
    )
}

#[test]
fn an_unrecorded_clones_automatic_push_gets_the_clis_helper_and_records_nothing() {
    // A clone from before the record, or one a chat's remote was rewritten in before any save:
    // an automatic push never trusts its remote on first use (P-82-3 review).
    let machine = Machine::signed_in_on("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let on = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".to_string(),
    };
    let first = |has_cli: bool| {
        as_the_window(
            &HostScope::for_a_test(),
            Path::new("/Applications/purlis.app/purlis"),
            machine.config(),
            || {
                helper_for_push_with(
                    &on,
                    clone.path(),
                    "https://github.example.com/someone/public.git",
                    &|_| has_cli,
                    None,
                )
            },
        )
    };
    assert!(
        first(true).is_ok_and(|helper| !helper.contains(HELPER_ARG)),
        "the forge CLI's helper, never purlis's"
    );
    // With no forge CLI on the machine, the save says to save once from the window, rather
    // than failing on a helper that is not there (#1848).
    assert_eq!(first(false), Err(NOT_SAVED_FROM_THE_WINDOW));
    assert!(
        !crate::machine::dir(machine.config())
            .join(PUSH_REMOTES)
            .exists(),
        "and nothing is recorded"
    );
}

#[test]
fn an_automatic_push_to_a_changed_remote_gets_no_helper() {
    let machine = Machine::signed_in_on("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let first = confirmed_if(true, || {
        push_helper(
            &machine,
            "github.example.com",
            clone.path(),
            "https://github.example.com/acme/widget.git",
        )
    });
    assert!(
        first.is_ok_and(|helper| helper.contains(HELPER_ARG)),
        "the window's Save records"
    );

    let moved = push_helper(
        &machine,
        "github.example.com",
        clone.path(),
        "https://github.example.com/someone/public.git",
    );
    assert_eq!(moved, Err(REMOTE_CHANGED));
}

#[test]
fn an_automatic_push_to_the_recorded_remote_gets_the_helper() {
    let machine = Machine::signed_in_on("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    confirmed_if(true, || {
        push_helper(
            &machine,
            "github.example.com",
            clone.path(),
            "https://github.example.com/acme/widget.git",
        )
    })
    .unwrap();
    for url in [
        "https://github.example.com/acme/widget.git",
        // The same repository, spelled as git and the forge also spell it.
        "https://GitHub.example.com/acme/widget",
    ] {
        let helper = push_helper(&machine, "github.example.com", clone.path(), url);
        assert!(
            helper.is_ok_and(|helper| helper.contains(HELPER_ARG)),
            "{url}"
        );
    }
}

#[test]
fn the_windows_save_confirms_a_changed_remote_and_records_it() {
    let machine = Machine::signed_in_on("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let (old, new) = (
        "https://github.example.com/acme/widget.git",
        "https://github.example.com/acme/widget-renamed.git",
    );
    confirmed_if(true, || {
        push_helper(&machine, "github.example.com", clone.path(), old)
    })
    .unwrap();

    let saved = confirmed_if(true, || {
        push_helper(&machine, "github.example.com", clone.path(), new)
    });
    assert!(saved.is_ok_and(|helper| helper.contains(HELPER_ARG)));

    let after = push_helper(&machine, "github.example.com", clone.path(), new);
    assert!(
        after.is_ok(),
        "the next automatic push goes to the confirmed remote"
    );
    assert_eq!(
        push_helper(&machine, "github.example.com", clone.path(), old),
        Err(REMOTE_CHANGED),
        "and no longer to the old one"
    );
    let record =
        std::fs::read_to_string(crate::machine::dir(machine.config()).join(PUSH_REMOTES)).unwrap();
    assert!(
        record.contains("github.example.com/acme/widget-renamed"),
        "{record}"
    );
}

#[test]
fn a_push_outside_the_window_is_not_held_to_a_record() {
    let clone = tempfile::tempdir().unwrap();
    let on = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".to_string(),
    };
    let helper = helper_for_push(&on, clone.path(), "https://github.example.com/a/b.git");
    assert!(helper.is_ok_and(|helper| !helper.contains(HELPER_ARG)));
}

#[test]
fn a_remote_is_placed_by_its_host_owner_and_repo_in_every_spelling() {
    for url in [
        "https://github.example.com/acme/widget.git",
        "https://GitHub.example.com/acme/widget/",
        "ssh://git@github.example.com/acme/widget.git",
        // An SSH port is SSH's own: a save pushes over HTTPS.
        "ssh://git@github.example.com:2222/acme/widget.git",
        "git@github.example.com:acme/widget.git",
        "github.example.com:acme/widget",
    ] {
        assert_eq!(
            place_of(url).as_deref(),
            Some("github.example.com/acme/widget"),
            "{url}"
        );
    }
    // An HTTPS port names another server.
    assert_eq!(
        place_of("https://git.example.com:8443/acme/widget.git").as_deref(),
        Some("git.example.com:8443/acme/widget")
    );
    for not_a_remote in ["file:///tmp/widget", "/tmp/widget", "./widget", "widget"] {
        assert_eq!(place_of(not_a_remote), None, "{not_a_remote}");
    }
}

#[test]
fn an_account_on_a_port_answers_git_for_that_port_alone() {
    let machine = Machine::signed_in_on("git.example.com:8443");

    let (out, _) = machine.ask("get", &asked("https", "git.example.com:8443"), &no_env);
    assert_eq!(out, format!("username=octocat\npassword={TOKEN}\n"));
    for another in ["git.example.com", "git.example.com:9443"] {
        let (out, _) = machine.ask("get", &asked("https", another), &no_env);
        assert_eq!(out, "", "{another}");
    }
}

/// **A save to a forge on a port signs in with the port's account** (#1864): the forge the
/// project declares on `host:port` is the one its HTTPS remote resolves to, and the account on
/// that port is the one the save's push names, held to the recorded remote as on any host.
#[test]
fn a_save_to_a_forge_on_a_port_gets_the_ports_own_sign_in() {
    let machine = Machine::signed_in_on("git.example.com:8443");
    let clone = tempfile::tempdir().unwrap();
    let cfg: toml::Table =
        toml::from_str("[[forge]]\nkind = \"github\"\nhost = \"git.example.com:8443\"\n").unwrap();
    let url = "https://git.example.com:8443/acme/widget.git";
    let on = crate::forge::resolve_in(url, &crate::forge::known_in(&cfg)).expect("its forge");
    let https = crate::forge::https_of(url, &on).expect("pushed over HTTPS");
    let push = |confirmed: bool, to: &str| {
        let places = [place_of(to).unwrap()];
        confirmed_if(confirmed, || {
            as_the_window(
                &HostScope::for_a_test(),
                Path::new("/Applications/purlis.app/purlis"),
                machine.config(),
                || helper_for_push_with(&on, clone.path(), to, &|_| true, Some(&places)),
            )
        })
    };
    let helper = push(true, &https).expect("the window's Save records the remote");
    assert!(helper.contains(HELPER_ARG), "{helper}");
    assert!(
        helper.contains(&machine.account.id),
        "the port's account: {helper}"
    );
    let automatic = push(false, &https).expect("an automatic save signs in");
    assert!(automatic.contains(HELPER_ARG), "{automatic}");
    // The same repository on the host's default port is another server, never recorded.
    assert_eq!(
        push(false, "https://git.example.com/acme/widget.git"),
        Err(REMOTE_CHANGED)
    );
}

/// The window's helper on `host` for a fetch of `clone` whose remote names `places`.
fn fetch_helper(
    machine: &Machine,
    clone: &Path,
    places: Option<&[String]>,
) -> Result<String, &'static str> {
    let on = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".to_string(),
    };
    as_the_window(
        &HostScope::for_a_test(),
        Path::new("/Applications/purlis.app/purlis"),
        machine.config(),
        || helper_for_fetch_with(&on, clone, places),
    )
}

#[test]
fn a_fetch_signs_in_only_to_the_recorded_remote_and_never_trusts_one_on_first_use() {
    let machine = Machine::signed_in_on("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let widget = "github.example.com/acme/widget".to_string();
    let public = "github.example.com/someone/public".to_string();

    let unrecorded = fetch_helper(&machine, clone.path(), Some(std::slice::from_ref(&widget)));
    assert!(
        unrecorded.is_ok_and(|helper| !helper.contains(HELPER_ARG)),
        "an unrecorded clone's fetch gets the forge CLI's helper, never purlis's"
    );

    confirmed_if(true, || {
        push_helper(
            &machine,
            "github.example.com",
            clone.path(),
            "https://github.example.com/acme/widget.git",
        )
    })
    .unwrap();
    let held = fetch_helper(
        &machine,
        clone.path(),
        Some(&[widget.clone(), widget.clone()]),
    );
    assert!(held.is_ok_and(|helper| helper.contains(HELPER_ARG)));
    for (places, why) in [
        (Some(vec![public.clone()]), "the remote changed"),
        (
            Some(vec![widget.clone(), public.clone()]),
            "its push URL names another place",
        ),
        (None, "git could not say where the remote is"),
        (Some(vec![]), "the remote names no address"),
    ] {
        assert_eq!(
            fetch_helper(&machine, clone.path(), places.as_deref()),
            Err(FETCH_REMOTE_CHANGED),
            "{why}"
        );
    }
}

#[test]
fn a_fetch_outside_the_window_is_not_held_to_a_record() {
    let clone = tempfile::tempdir().unwrap();
    let on = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".to_string(),
    };
    let helper = helper_for_fetch_with(&on, clone.path(), None);
    assert!(helper.is_ok_and(|helper| !helper.contains(HELPER_ARG)));
}

#[test]
fn a_push_whose_remote_pushes_elsewhere_by_name_is_never_signed_in_even_when_confirmed() {
    let machine = Machine::signed_in_on("github.example.com");
    let clone = tempfile::tempdir().unwrap();
    let on = Forge {
        kind: Kind::GitHub,
        host: "github.example.com".to_string(),
    };
    let places = [
        "github.example.com/acme/widget".to_string(),
        "github.example.com/someone/public".to_string(),
    ];
    for confirmed in [false, true] {
        let pushed = confirmed_if(confirmed, || {
            as_the_window(
                &HostScope::for_a_test(),
                Path::new("/Applications/purlis.app/purlis"),
                machine.config(),
                || {
                    helper_for_push_with(
                        &on,
                        clone.path(),
                        "https://github.example.com/acme/widget.git",
                        &|_| true,
                        Some(&places),
                    )
                },
            )
        });
        assert_eq!(pushed, Err(REMOTE_SPLIT), "confirmed: {confirmed}");
    }
    assert!(
        !crate::machine::dir(machine.config())
            .join(PUSH_REMOTES)
            .exists(),
        "and nothing is recorded"
    );
}
