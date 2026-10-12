// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Get a project by URL (#1815), against a local bare repository standing in for the forge.
//!
//! The URL a person pastes is an `https://` one on a made-up host; a `url.<base>.insteadOf` in
//! the runner's isolation ([`git::Isolated::also`]) sends it to the bare repository, so the
//! whole HTTPS path is driven with no network. An SSH failure is a stand-in `ssh` that says what
//! a forge's says, set as `core.sshCommand` the same way.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::testgit::run;
use crate::worktree::git;

/// The host the stand-in forge answers for.
const HOST: &str = "forge.invalid";

/// A temporary tree holding a bare repository `acme/widget.git` with one commit.
struct Forge {
    _dir: tempfile::TempDir,
    top: PathBuf,
}

impl Forge {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let work = top.join("work");
        std::fs::create_dir_all(&work).unwrap();
        run(&work, &["init", "-q", "-b", "main", "."]);
        run(&work, &["config", "user.name", "Fixture"]);
        run(&work, &["config", "user.email", "fixture@example.invalid"]);
        std::fs::write(work.join("README.md"), "the widget\n").unwrap();
        run(&work, &["add", "-A"]);
        run(&work, &["commit", "-q", "-m", "one"]);
        let bare = top.join("forge/acme/widget.git");
        std::fs::create_dir_all(&bare).unwrap();
        run(&bare, &["init", "-q", "--bare", "-b", "main", "."]);
        run(&work, &["push", "-q", &bare.display().to_string(), "main"]);
        Self { _dir: dir, top }
    }

    /// The runner's isolation, with the made-up host sent to this forge.
    fn isolation(&self) -> git::Isolated {
        git::Isolated::default().also(
            &format!("url.file://{}/.insteadOf", self.top.join("forge").display()),
            &format!("https://{HOST}/"),
        )
    }

    /// Where the person's projects go in this test.
    fn projects(&self) -> PathBuf {
        self.top.join("home/Projects")
    }

    /// Gets `url` into `into` inside `isolation`, hearing `off`; answers with what each
    /// progress said too.
    fn get(
        &self,
        isolation: &git::Isolated,
        url: &str,
        into: &Path,
        off: &AtomicBool,
    ) -> (Result<Got, Refused>, Vec<Progress>) {
        let url = ProjectUrl::parse(url).expect("a project URL");
        let mut heard = Vec::new();
        let answer = git::isolated(isolation, || {
            get(&url, into, off, &mut |progress| heard.push(progress))
        });
        (answer, heard)
    }
}

/// A stand-in `ssh` that runs `script` in place of reaching a forge.
fn stand_in_ssh(top: &Path, script: &str) -> String {
    let path = top.join("ssh-stand-in");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path.display().to_string()
}

// --- The URL ---

#[test]
fn https_and_both_ssh_spellings_are_project_urls_named_by_their_last_part() {
    for (text, host, name, transport) in [
        (
            "https://github.com/acme/widget",
            "github.com",
            "widget",
            Transport::Https,
        ),
        (
            "https://github.com/acme/widget.git",
            "github.com",
            "widget",
            Transport::Https,
        ),
        (
            "  https://gitlab.com/group/sub/widget/  ",
            "gitlab.com",
            "widget",
            Transport::Https,
        ),
        (
            "HTTPS://git.example.com:8443/acme/widget",
            "git.example.com",
            "widget",
            Transport::Https,
        ),
        (
            "git@github.com:acme/widget.git",
            "github.com",
            "widget",
            Transport::Ssh,
        ),
        (
            "github.com:acme/widget",
            "github.com",
            "widget",
            Transport::Ssh,
        ),
        (
            "ssh://git@gitlab.com/acme/widget.git",
            "gitlab.com",
            "widget",
            Transport::Ssh,
        ),
        (
            "ssh://git@host.example:2222/acme/widget",
            "host.example",
            "widget",
            Transport::Ssh,
        ),
    ] {
        let url = ProjectUrl::parse(text).unwrap_or_else(|err| panic!("{text}: {err}"));
        assert_eq!(url.host(), host, "{text}");
        assert_eq!(url.name(), name, "{text}");
        assert_eq!(url.transport(), transport, "{text}");
        assert_eq!(url.as_str(), text.trim());
    }
}

#[test]
fn every_other_url_is_refused_as_a_bad_url_before_git_runs() {
    for text in [
        "",
        "   ",
        "http://github.com/acme/widget",
        "git://github.com/acme/widget",
        "file:///srv/widget.git",
        "ext::sh -c touch% /tmp/pwned",
        "github.com/acme/widget",
        "/srv/widget.git",
        "./widget",
        "https://github.com",
        "https://github.com/",
        "https:///acme/widget",
        "https://github.com/acme/..",
        "git@github.com:",
        "-oProxyCommand=evil:acme/widget",
        "git@-oProxyCommand=evil:acme/widget",
        "ssh://-oProxyCommand=evil/acme/widget",
        "https://github.com/acme/wid get",
    ] {
        let refused = ProjectUrl::parse(text).expect_err(text);
        assert_eq!(refused.why, Why::BadUrl, "{text}");
        assert!(!refused.sentence.is_empty());
    }
}

#[test]
fn a_url_holding_a_password_is_refused_and_says_so() {
    let refused = ProjectUrl::parse("https://git:hunter2@forge.invalid/acme/widget").unwrap_err();
    assert_eq!(refused.why, Why::BadUrl);
    assert!(
        refused.sentence.contains("password"),
        "{}",
        refused.sentence
    );
    assert!(!refused.sentence.contains("hunter2"));
}

#[test]
fn a_user_name_or_a_token_before_an_https_host_is_refused_without_repeating_it() {
    // A token pasted as the whole sign-in has no colon, and git would keep it in the clone's
    // config (#1815 review).
    let at = |user: &str, rest: &str| format!("https://{user}{}{rest}", '@');
    for text in [
        at("git", "github.com/acme/widget"),
        at("canaryTOKENuser41c7e", "github.com/acme/widget"),
        at("canary%3ATOKEN", "gitlab.com/acme/widget"),
    ] {
        let refused = ProjectUrl::parse(&text).expect_err(&text);
        assert_eq!(refused.why, Why::BadUrl, "{text}");
        assert!(!refused.sentence.contains("canary"), "{}", refused.sentence);
    }
    // An SSH user name is the account ssh signs in as, and holds no secret.
    assert!(ProjectUrl::parse("ssh://git@gitlab.com/acme/widget.git").is_ok());
}

#[test]
fn another_scheme_is_told_to_use_https_or_ssh() {
    let refused = ProjectUrl::parse("http://github.com/acme/widget").unwrap_err();
    assert!(
        refused.sentence.contains("HTTPS or SSH"),
        "{}",
        refused.sentence
    );
}

// --- The folder ---

#[test]
fn the_folder_is_projects_in_home_until_the_person_picks_a_place() {
    let url = ProjectUrl::parse("https://github.com/acme/widget.git").unwrap();
    let home = Path::new("/Users/someone");
    assert_eq!(
        folder_for(&url, None, home),
        PathBuf::from("/Users/someone/Projects/widget")
    );
    assert_eq!(
        folder_for(&url, Some(Path::new("/Volumes/work/code")), home),
        PathBuf::from("/Volumes/work/code/widget")
    );
}

// --- Progress and git's words ---

#[test]
fn gits_progress_lines_are_read_as_stages_with_their_share_done() {
    for (line, stage, percent) in [
        (
            "remote: Enumerating objects: 5, done.",
            Stage::Connecting,
            None,
        ),
        (
            "remote: Counting objects:  40% (2/5)",
            Stage::Connecting,
            Some(40),
        ),
        (
            "Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s",
            Stage::Downloading,
            Some(45),
        ),
        (
            "Resolving deltas: 100% (3/3), done.",
            Stage::Unpacking,
            Some(100),
        ),
        (
            "Updating files:  12% (12/100)",
            Stage::WritingFiles,
            Some(12),
        ),
    ] {
        assert_eq!(
            read_progress(line),
            Some(Progress { stage, percent }),
            "{line}"
        );
    }
    for line in [
        "Cloning into '/Users/someone/Projects/widget'...",
        "fatal: repository 'https://github.com/acme/nope/' not found",
        "remote: Total 5 (delta 0), reused 0 (delta 0), pack-reused 0",
    ] {
        assert_eq!(read_progress(line), None, "{line}");
    }
}

#[test]
fn what_each_forge_says_is_read_as_its_own_reason() {
    let https = ProjectUrl::parse("https://github.com/acme/widget").unwrap();
    let ssh = ProjectUrl::parse("git@github.com:acme/widget.git").unwrap();
    let lines = |text: &str| text.lines().map(str::to_owned).collect::<Vec<_>>();
    for (url, said, why) in [
        (
            &https,
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
            Why::NoAccess,
        ),
        (
            &https,
            "remote: HTTP Basic: Access denied\nfatal: Authentication failed for 'https://gitlab.com/acme/widget.git/'",
            Why::NoAccess,
        ),
        (
            &ssh,
            "git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.",
            Why::NoAccess,
        ),
        (
            &ssh,
            "Host key verification failed.\nfatal: Could not read from remote repository.",
            Why::NoAccess,
        ),
        (
            &https,
            "remote: Repository not found.\nfatal: repository 'https://github.com/acme/widget/' not found",
            Why::Missing,
        ),
        (
            &ssh,
            "ERROR: Repository not found.\nfatal: Could not read from remote repository.",
            Why::Missing,
        ),
        (
            &https,
            "remote: The project you were looking for could not be found or you don't have permission to view it.",
            Why::Missing,
        ),
        (
            &https,
            "fatal: unable to access 'https://github.com/acme/widget/': Could not resolve host: github.com",
            Why::Unreachable,
        ),
        (&https, "fatal: early EOF", Why::Git),
    ] {
        let refused = read_failure(url, &lines(said));
        assert_eq!(refused.why, why, "{said}");
    }
}

#[test]
fn each_reason_has_its_own_sentence_naming_the_project() {
    let https = ProjectUrl::parse("https://github.com/acme/widget").unwrap();
    let ssh = ProjectUrl::parse("git@github.com:acme/widget.git").unwrap();
    let no_access = read_failure(&https, &["fatal: Authentication failed".to_owned()]);
    let no_key = read_failure(&ssh, &["Permission denied (publickey).".to_owned()]);
    let missing = read_failure(&https, &["remote: Repository not found.".to_owned()]);
    let other = read_failure(&https, &["fatal: early EOF".to_owned()]);
    for refused in [&no_access, &no_key, &missing, &other] {
        assert!(
            refused.sentence.contains("acme/widget"),
            "{}",
            refused.sentence
        );
    }
    assert!(no_key.sentence.contains("SSH key"), "{}", no_key.sentence);
    assert!(
        no_access.sentence.contains("Sign in"),
        "{}",
        no_access.sentence
    );
    assert!(
        missing.sentence.contains("no project"),
        "{}",
        missing.sentence
    );
    assert!(other.sentence.ends_with("early EOF"), "{}", other.sentence);
    let sentences = [&no_access, &no_key, &missing, &other].map(|one| one.sentence.clone());
    for (i, one) in sentences.iter().enumerate() {
        assert!(
            !sentences[i + 1..].contains(one),
            "two reasons say the same: {one}"
        );
    }
}

// --- The clone, against the stand-in forge ---

#[test]
fn a_pasted_https_url_is_cloned_into_its_folder_making_the_folders_above_it() {
    let forge = Forge::new();
    let into = forge.projects().join("widget");
    let off = AtomicBool::new(false);

    let (answer, heard) = forge.get(
        &forge.isolation(),
        &format!("https://{HOST}/acme/widget.git"),
        &into,
        &off,
    );

    assert_eq!(answer, Ok(Got::Cloned(into.clone())));
    assert_eq!(
        std::fs::read_to_string(into.join("README.md")).unwrap(),
        "the widget\n"
    );
    assert_eq!(heard.first().map(|p| p.stage), Some(Stage::Connecting));
    // The clone's origin is the URL as pasted, so the project's remote is the forge's.
    let origin = run(&into, &["config", "--get", "remote.origin.url"]);
    assert_eq!(origin.out.trim(), format!("https://{HOST}/acme/widget.git"));
}

#[test]
fn a_missing_project_says_so_and_leaves_no_folder_behind() {
    let forge = Forge::new();
    let into = forge.projects().join("nope");
    let off = AtomicBool::new(false);

    let (answer, _) = forge.get(
        &forge.isolation(),
        &format!("https://{HOST}/acme/nope.git"),
        &into,
        &off,
    );

    let refused = answer.unwrap_err();
    assert_eq!(refused.why, Why::Missing, "{}", refused.sentence);
    assert!(!into.exists(), "the failed clone left its folder");
    // `home/Projects` was made for this clone, so it is gone too.
    assert!(
        !forge.top.join("home").exists(),
        "the folders made for it were left"
    );
}

#[test]
fn a_failure_into_an_empty_folder_that_was_there_keeps_the_folder_and_empties_it() {
    let forge = Forge::new();
    let into = forge.projects().join("nope");
    std::fs::create_dir_all(&into).unwrap();
    let off = AtomicBool::new(false);

    let (answer, _) = forge.get(
        &forge.isolation(),
        &format!("https://{HOST}/acme/nope.git"),
        &into,
        &off,
    );

    assert!(answer.is_err());
    assert!(into.is_dir(), "the person's own folder was removed");
    assert_eq!(std::fs::read_dir(&into).unwrap().count(), 0);
}

#[test]
fn a_folder_that_holds_something_is_refused_and_left_as_it_was() {
    let forge = Forge::new();
    let into = forge.projects().join("widget");
    std::fs::create_dir_all(&into).unwrap();
    std::fs::write(into.join("mine.txt"), "keep me\n").unwrap();
    let off = AtomicBool::new(false);

    let (answer, heard) = forge.get(
        &forge.isolation(),
        &format!("https://{HOST}/acme/widget.git"),
        &into,
        &off,
    );

    assert_eq!(answer.unwrap_err().why, Why::FolderTaken);
    assert!(heard.is_empty(), "git ran into a folder in use");
    assert_eq!(
        std::fs::read_to_string(into.join("mine.txt")).unwrap(),
        "keep me\n"
    );
}

#[test]
fn an_ssh_key_the_forge_refuses_is_no_access() {
    let forge = Forge::new();
    let ssh = stand_in_ssh(
        &forge.top,
        "echo 'git@forge.invalid: Permission denied (publickey).' >&2; exit 255",
    );
    let isolation = forge.isolation().also("core.sshCommand", &ssh);
    let into = forge.projects().join("widget");
    let off = AtomicBool::new(false);

    let (answer, _) = forge.get(
        &isolation,
        &format!("git@{HOST}:acme/widget.git"),
        &into,
        &off,
    );

    let refused = answer.unwrap_err();
    assert_eq!(refused.why, Why::NoAccess, "{}", refused.sentence);
    assert!(!into.exists());
}

#[test]
fn a_project_the_forge_does_not_have_over_ssh_is_missing() {
    let forge = Forge::new();
    let ssh = stand_in_ssh(
        &forge.top,
        "echo 'ERROR: Repository not found.' >&2; exit 1",
    );
    let isolation = forge.isolation().also("core.sshCommand", &ssh);
    let into = forge.projects().join("widget");
    let off = AtomicBool::new(false);

    let (answer, _) = forge.get(
        &isolation,
        &format!("git@{HOST}:acme/widget.git"),
        &into,
        &off,
    );

    assert_eq!(answer.unwrap_err().why, Why::Missing);
    assert!(!into.exists());
}

#[test]
fn calling_off_a_clone_mid_way_removes_its_partial_folder() {
    let forge = Forge::new();
    // A forge that never answers: the clone has made its folder and waits on the connection.
    // It waits only while git does, so stopping git leaves nothing of it running.
    let ssh = stand_in_ssh(
        &forge.top,
        "while kill -0 $PPID 2>/dev/null; do sleep 0.1; done; exit 1",
    );
    let isolation = forge.isolation().also("core.sshCommand", &ssh);
    let into = forge.projects().join("widget");
    let off = Arc::new(AtomicBool::new(false));
    let raise = Arc::clone(&off);
    let watched = into.clone();
    let caller = std::thread::spawn(move || {
        // Called off once git has made the folder: a clone truly under way.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !watched.exists() && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        raise.store(true, Ordering::SeqCst);
    });
    let started = std::time::Instant::now();

    let (answer, _) = forge.get(
        &isolation,
        &format!("git@{HOST}:acme/widget.git"),
        &into,
        &off,
    );
    caller.join().unwrap();

    assert_eq!(answer, Ok(Got::CalledOff));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "the clone was not stopped"
    );
    assert!(!into.exists(), "the called-off clone left its folder");
    assert!(
        !forge.top.join("home").exists(),
        "the folders made for it were left"
    );
}

#[test]
fn an_https_clone_the_window_runs_signs_in_with_the_persons_account_and_no_other_helper() {
    use crate::forge::accounts::{self, Pasted};
    use crate::forge::gitcredential::{HELPER_ARG, as_the_window};
    use crate::forge::route::HostScope;
    use crate::forge::transport::Transport as Wire;

    let home = tempfile::tempdir().unwrap();
    let keyring = accounts::keyring_at(home.path());
    let recording = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/accounts/github/token_check.json");
    let recorded = Arc::new(
        crate::forge::recorded::Recorded::parse(&std::fs::read_to_string(recording).unwrap())
            .unwrap(),
    );
    let reach = |_: &crate::forge::Forge,
                 _: Arc<dyn crate::forge::http::TokenSource>|
     -> Result<Arc<dyn Wire>, crate::forge::ForgeError> {
        Ok(recorded.clone() as Arc<dyn Wire>)
    };
    accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        home.path(),
        keyring.as_ref(),
        &reach,
        crate::forge::Kind::GitHub,
        "github.example.com",
        Pasted::new("kept-for-the-window-git-only".to_string()),
    )
    .unwrap();
    let https = ProjectUrl::parse("https://github.example.com/acme/widget.git").unwrap();
    let ssh = ProjectUrl::parse("git@github.example.com:acme/widget.git").unwrap();
    let elsewhere = ProjectUrl::parse("https://gitlab.example.com/acme/widget.git").unwrap();
    let exe = Path::new("/Applications/purlis.app/Contents/MacOS/charter-app");

    assert!(window_credential(&https).is_empty(), "outside the window");
    let (inside, over_ssh, no_account) =
        as_the_window(&HostScope::for_a_test(), exe, home.path(), || {
            (
                window_credential(&https),
                window_credential(&ssh),
                window_credential(&elsewhere),
            )
        });

    assert_eq!(inside.len(), 2, "{inside:?}");
    assert_eq!(
        inside[0], "credential.helper=",
        "every other helper dropped first"
    );
    assert!(
        inside[1].starts_with(&format!(
            "credential.helper=!'{}' {HELPER_ARG} ",
            exe.display()
        )),
        "{inside:?}"
    );
    assert!(over_ssh.is_empty(), "SSH keeps the person's own");
    assert!(
        no_account.is_empty(),
        "a host with no account keeps the person's own helpers"
    );
}
