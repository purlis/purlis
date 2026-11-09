// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis artifact pin | unpin | list | reorder` asked of the binary (ADR 0120, #5950): a pin
//! is a line of the workspace's committed pins file, a pin never leads out of its workspace's
//! `artifacts/`, and two people's pins meet in one file through the plane's own git.

#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const IDENTITY: [(&str, &str); 5] = [
    ("GIT_AUTHOR_NAME", "Tester"),
    ("GIT_AUTHOR_EMAIL", "t@e.invalid"),
    ("GIT_COMMITTER_NAME", "Tester"),
    ("GIT_COMMITTER_EMAIL", "t@e.invalid"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
];

fn git(at: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .envs(IDENTITY)
        .output()
        .expect("git runs")
}

fn git_ok(at: &Path, args: &[&str]) -> String {
    let out = git(at, args);
    assert!(out.status.success(), "git {args:?}: {}", said(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A plane scaffolded by `purlis init`, with a workspace called alpha holding two artifact
/// folders, `board` and `notes`, each with its page.
struct Plane {
    _tmp: tempfile::TempDir,
    base: PathBuf,
}

impl Plane {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(tmp.path()).unwrap();
        std::fs::create_dir_all(base.join("home")).unwrap();
        std::fs::create_dir_all(base.join("plane")).unwrap();
        let plane = Self { _tmp: tmp, base };
        let out = plane.purlis(&["init", "--forge", "github", "--owner", "acme"]);
        assert!(out.status.success(), "{}", said(&out));
        std::fs::create_dir_all(plane.alpha()).unwrap();
        plane.page("board", "<title>Team board</title><p>hi</p>");
        plane.page("notes", "<p>no title</p>");
        plane
    }

    fn root(&self) -> PathBuf {
        self.base.join("plane")
    }

    fn alpha(&self) -> PathBuf {
        self.root().join("workspaces/alpha")
    }

    fn artifacts(&self) -> PathBuf {
        self.alpha().join("artifacts")
    }

    fn pins(&self) -> String {
        std::fs::read_to_string(self.artifacts().join("pins.toml")).unwrap_or_default()
    }

    fn page(&self, folder: &str, html: &str) {
        let dir = self.artifacts().join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), html).unwrap();
    }

    fn purlis(&self, args: &[&str]) -> Output {
        self.purlis_at(&self.root(), args)
    }

    /// `purlis <args>` in the plane at `root`, in workspace alpha, with nothing of the caller's
    /// environment but what is named.
    fn purlis_at(&self, root: &Path, args: &[&str]) -> Output {
        support::purlis()
            .args(args)
            .current_dir(root)
            .env_clear()
            .env("CHARTER_ROOT", root)
            .env("CHARTER_WORKSPACE", "alpha")
            .env("CHARTER_CONFIG_HOME", self.base.join("home/config"))
            .env("HOME", self.base.join("home"))
            .env("TMPDIR", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .envs(IDENTITY)
            .output()
            .expect("the binary runs")
    }

    fn listed(&self) -> Vec<String> {
        let out = self.purlis(&["artifact", "list"]);
        assert!(out.status.success(), "{}", said(&out));
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    }
}

#[test]
fn a_pinned_folder_is_listed_with_its_title_and_unpinning_it_keeps_its_files() {
    let plane = Plane::new();

    let out = plane.purlis(&["artifact", "pin", "board"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("Pinned 'Team board' (board, "),
        "{}",
        said(&out)
    );
    assert!(
        said(&out).contains("the workspace is LOCAL, so it stays on this machine"),
        "{}",
        said(&out)
    );
    let out = plane.purlis(&["artifact", "pin", "notes", "--icon", "book"]);
    assert!(out.status.success(), "{}", said(&out));

    let listed = plane.listed();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed[0].starts_with("1  "), "{listed:?}");
    assert!(
        listed[0].ends_with("  board  chart  Team board"),
        "{listed:?}"
    );
    assert!(listed[1].starts_with("2  "), "{listed:?}");
    assert!(listed[1].ends_with("  notes  book  notes"), "{listed:?}");
    assert!(
        plane.pins().starts_with("version = 1\n"),
        "{}",
        plane.pins()
    );

    let out = plane.purlis(&["artifact", "unpin", "board"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("its folder stays"), "{}", said(&out));
    let listed = plane.listed();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(listed[0].contains("  notes  "), "{listed:?}");
    assert!(plane.artifacts().join("board/index.html").is_file());
}

#[test]
fn a_pin_that_leads_out_of_the_workspaces_artifacts_is_refused_and_nothing_is_written() {
    let plane = Plane::new();
    let outside = plane.base.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("index.html"), "<p>elsewhere</p>").unwrap();
    std::os::unix::fs::symlink(&outside, plane.artifacts().join("away")).unwrap();
    std::fs::create_dir_all(plane.alpha().join("memory")).unwrap();
    std::fs::write(plane.alpha().join("memory/index.html"), "<p>memory</p>").unwrap();

    for (folder, sentence) in [
        ("..", "is not the name of a folder in artifacts/"),
        ("../memory", "is not the name of a folder in artifacts/"),
        (
            "board/../../memory",
            "is not the name of a folder in artifacts/",
        ),
        ("/etc", "is not the name of a folder in artifacts/"),
        ("away", "'away' is a link that leads out of artifacts/"),
        ("absent", "there is no folder 'absent' in artifacts/"),
    ] {
        let out = plane.purlis(&["artifact", "pin", folder]);
        assert_eq!(out.status.code(), Some(1), "{folder}: {}", said(&out));
        assert!(said(&out).contains(sentence), "{folder}: {}", said(&out));
    }
    std::fs::create_dir_all(plane.artifacts().join("empty")).unwrap();
    let out = plane.purlis(&["artifact", "pin", "empty"]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(
        said(&out).contains("'empty' has no index.html"),
        "{}",
        said(&out)
    );
    assert!(
        !plane.artifacts().join("pins.toml").exists(),
        "a refused pin writes nothing"
    );
}

#[test]
fn an_artifacts_folder_that_is_a_link_out_of_the_workspace_pins_nothing() {
    let plane = Plane::new();
    let elsewhere = plane.base.join("elsewhere");
    std::fs::rename(plane.artifacts(), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, plane.artifacts()).unwrap();

    let out = plane.purlis(&["artifact", "pin", "board"]);

    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(
        said(&out).contains("artifacts/ is a link that leads out of the workspace"),
        "{}",
        said(&out)
    );
    assert!(!elsewhere.join("pins.toml").exists());
}

#[test]
fn pinning_a_folder_again_with_another_command_records_a_new_pin_in_its_place() {
    let plane = Plane::new();
    plane.purlis(&["artifact", "pin", "notes"]);
    plane.purlis(&["artifact", "pin", "board", "--command", "make board"]);
    let before = plane.listed();
    let old_id = before[1].split("  ").nth(1).unwrap().to_string();

    // A new name and icon on the same command is the same pin.
    let out = plane.purlis(&[
        "artifact",
        "pin",
        "board",
        "--command",
        "make board",
        "--name",
        "Board",
        "--icon",
        "star",
    ]);
    assert!(out.status.success(), "{}", said(&out));
    let same = plane.listed();
    assert_eq!(same[1], format!("2  {old_id}  board  star  Board"));

    let out = plane.purlis(&["artifact", "pin", "board", "--command", "make other"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains(&format!("as a new pin in the place of {old_id}")),
        "{}",
        said(&out)
    );
    let after = plane.listed();
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(after[1].starts_with("2  "), "it keeps its place: {after:?}");
    assert!(after[1].contains("  board  "), "{after:?}");
    assert!(!after[1].contains(&old_id), "a new id: {after:?}");
    assert!(plane.pins().contains("command = \"make other\""));
    assert!(!plane.pins().contains("make board"));
}

#[test]
fn reorder_puts_the_named_pins_first_for_everyone() {
    let plane = Plane::new();
    plane.page("third", "<title>Third</title>");
    for folder in ["board", "notes", "third"] {
        let out = plane.purlis(&["artifact", "pin", folder]);
        assert!(out.status.success(), "{}", said(&out));
    }

    let out = plane.purlis(&["artifact", "reorder", "third", "notes"]);

    assert!(out.status.success(), "{}", said(&out));
    let folders: Vec<String> = plane
        .listed()
        .iter()
        .map(|line| line.split("  ").nth(2).unwrap().to_string())
        .collect();
    assert_eq!(folders, ["third", "notes", "board"]);
    let out = plane.purlis(&["artifact", "reorder", "nope"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        said(&out).contains("no pin 'nope' in this workspace"),
        "{}",
        said(&out)
    );
}

#[test]
fn a_pins_file_keeps_the_keys_a_newer_purlis_wrote() {
    let plane = Plane::new();
    std::fs::write(
        plane.artifacts().join("pins.toml"),
        "version = 1\nnote = \"theirs\"\npin = [\n  { id = \"aaaaaaaaaaaa\", folder = \"notes\", name = \"Notes\", icon = \"book\", order = 1, tint = \"teal\" },\n]\n",
    )
    .unwrap();

    let out = plane.purlis(&["artifact", "pin", "board"]);

    assert!(out.status.success(), "{}", said(&out));
    let pins = plane.pins();
    assert!(pins.contains("tint = \"teal\""), "{pins}");
    assert!(pins.contains("note = \"theirs\""), "{pins}");
    std::fs::write(plane.artifacts().join("pins.toml"), "version = 2\n").unwrap();
    let out = plane.purlis(&["artifact", "pin", "board"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(said(&out).contains("version 2"), "{}", said(&out));
    assert_eq!(
        plane.pins(),
        "version = 2\n",
        "a newer file is never rewritten"
    );
}

#[test]
fn a_pin_whose_folder_is_gone_is_listed_as_missing() {
    let plane = Plane::new();
    plane.purlis(&["artifact", "pin", "board"]);
    std::fs::remove_dir_all(plane.artifacts().join("board")).unwrap();

    let listed = plane.listed();

    assert_eq!(listed.len(), 1);
    assert!(
        listed[0].ends_with("(missing: there is no folder 'board' in artifacts/)"),
        "{listed:?}"
    );
}

#[test]
fn a_live_workspace_commits_its_templates_and_pins_and_never_its_data() {
    let plane = Plane::new();
    git_ok(&plane.root(), &["init", "-q", "-b", "main"]);
    let out = plane.purlis(&["workspace", "live", "alpha"]);
    assert!(out.status.success(), "{}", said(&out));
    plane.purlis(&["artifact", "pin", "board"]);
    let board = plane.artifacts().join("board");
    std::fs::write(board.join("data.json"), "{}").unwrap();
    std::fs::create_dir_all(board.join("data")).unwrap();
    std::fs::write(board.join("data/2026-11-09.json"), "{}").unwrap();
    std::fs::write(board.join("style.css"), "p{}").unwrap();

    let ignored = |rel: &str| {
        git(&plane.root(), &["check-ignore", "-q", rel])
            .status
            .success()
    };

    for shared in [
        "workspaces/alpha/artifacts/pins.toml",
        "workspaces/alpha/artifacts/board/index.html",
        "workspaces/alpha/artifacts/board/style.css",
    ] {
        assert!(!ignored(shared), "{shared} is committed while LIVE");
    }
    for data in [
        "workspaces/alpha/artifacts/board/data.json",
        "workspaces/alpha/artifacts/board/data/2026-11-09.json",
    ] {
        assert!(ignored(data), "{data} is each person's own");
    }

    let out = plane.purlis(&["workspace", "live", "alpha", "--off"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(
        ignored("workspaces/alpha/artifacts/pins.toml"),
        "a LOCAL workspace's pins stay on this machine"
    );
}

#[test]
fn two_people_pinning_at_once_merge_into_one_pins_file() {
    let plane = Plane::new();
    let root = plane.root();
    git_ok(&root, &["init", "-q", "-b", "main"]);
    let out = plane.purlis(&["workspace", "live", "alpha"]);
    assert!(out.status.success(), "{}", said(&out));
    plane.page("third", "<title>Third</title>");
    plane.purlis(&["artifact", "pin", "third"]);
    git_ok(&root, &["add", "-A"]);
    git_ok(&root, &["commit", "-qm", "base"]);

    git_ok(&root, &["checkout", "-q", "-b", "theirs"]);
    plane.purlis(&["artifact", "pin", "notes"]);
    git_ok(&root, &["commit", "-qam", "theirs"]);
    git_ok(&root, &["checkout", "-q", "main"]);
    plane.purlis(&["artifact", "pin", "board"]);
    git_ok(&root, &["commit", "-qam", "ours"]);

    let merged = git(&root, &["merge", "-q", "--no-edit", "theirs"]);

    assert!(merged.status.success(), "a clean merge: {}", said(&merged));
    let folders: Vec<String> = plane
        .listed()
        .iter()
        .map(|line| line.split("  ").nth(2).unwrap().to_string())
        .collect();
    assert_eq!(folders.len(), 3, "{folders:?}\n{}", plane.pins());
    assert_eq!(folders[0], "third");
    for folder in ["board", "notes"] {
        assert!(folders.contains(&folder.to_string()), "{folders:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Scripted pages and each person's approval (#5959, ADR 0120 §4)
// ---------------------------------------------------------------------------------------------

/// A page with script: an inline handler and a script file beside it.
const SCRIPTED: &str =
    "<title>Live</title><button onclick=go()>Go</button><script src=app.js></script>";

impl Plane {
    fn config(&self) -> PathBuf {
        self.base.join("home/config")
    }

    /// `purlis artifact approve-status`'s line for `folder`.
    fn standing(&self, folder: &str) -> String {
        let out = self.purlis(&["artifact", "approve-status"]);
        assert!(out.status.success(), "{}", said(&out));
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find(|line| line.starts_with(&format!("{folder}  ")))
            .unwrap_or_else(|| panic!("no line for {folder}: {}", said(&out)))
            .to_owned()
    }

    /// The person's press of Approve in the artifact's bar: the pin and the fingerprint of the
    /// template the window was served, recorded through the core as the bar records them.
    fn approve_as_the_bar_does(&self, folder: &str) {
        let pins = purlis_core::artifact::read(&self.alpha()).expect("pins");
        let pin = pins
            .pins
            .iter()
            .find(|pin| pin.folder == folder)
            .expect("pinned");
        let served =
            purlis_core::artifact::serve::Snapshot::read(&self.artifacts().join(folder)).unwrap();
        purlis_core::artifact::approval::approve(
            &self.config(),
            &self.alpha(),
            folder,
            &pin.id,
            served.fingerprint(),
        )
        .expect("approved");
    }
}

#[test]
fn a_page_without_script_opens_without_asking_and_one_with_script_asks() {
    let plane = Plane::new();
    plane.page("live", SCRIPTED);
    std::fs::write(plane.artifacts().join("live/app.js"), "draw()").unwrap();
    plane.page(
        "json",
        r#"<p>data block</p><script type="application/json">{"open": 3}</script>"#,
    );
    for folder in ["board", "live", "json"] {
        let out = plane.purlis(&["artifact", "pin", folder]);
        assert!(out.status.success(), "{}", said(&out));
    }

    assert!(
        plane.standing("board").starts_with("board  script-free  "),
        "{}",
        plane.standing("board")
    );
    assert!(
        plane.standing("json").starts_with("json  script-free  "),
        "a JSON block is data: {}",
        plane.standing("json")
    );
    let live = plane.standing("live");
    assert!(live.starts_with("live  new  "), "{live}");
    assert!(live.contains("asks before its script runs"), "{live}");
}

#[test]
fn an_approval_holds_through_new_data_and_any_change_to_the_template_asks_again() {
    let plane = Plane::new();
    plane.page("live", SCRIPTED);
    std::fs::write(plane.artifacts().join("live/app.js"), "draw()").unwrap();
    plane.purlis(&["artifact", "pin", "live"]);

    plane.approve_as_the_bar_does("live");
    assert!(
        plane.standing("live").starts_with("live  approved  "),
        "{}",
        plane.standing("live")
    );

    // Regenerated data never asks again.
    let live = plane.artifacts().join("live");
    std::fs::write(live.join("data.json"), r#"{"open": 12}"#).unwrap();
    std::fs::create_dir_all(live.join("data")).unwrap();
    std::fs::write(live.join("data/2026-11-09.json"), "[]").unwrap();
    assert!(
        plane.standing("live").starts_with("live  approved  "),
        "{}",
        plane.standing("live")
    );

    // A file beside the page, not the page itself, changed: asked again.
    std::fs::write(live.join("app.js"), "draw(); send()").unwrap();
    let changed = plane.standing("live");
    assert!(changed.starts_with("live  changed  "), "{changed}");
    assert!(
        changed.contains("changed since you approved it"),
        "{changed}"
    );

    // Approved again, then a new file planted beside it: asked again.
    plane.approve_as_the_bar_does("live");
    assert!(plane.standing("live").starts_with("live  approved  "));
    std::fs::write(live.join("more.js"), "x()").unwrap();
    assert!(
        plane.standing("live").starts_with("live  changed  "),
        "{}",
        plane.standing("live")
    );
}

#[test]
fn a_pin_whose_command_changed_asks_again_and_names_who_changed_it() {
    let plane = Plane::new();
    let alpha = plane.alpha();
    git_ok(&alpha, &["init", "-q", "-b", "main"]);
    plane.page("live", SCRIPTED);
    plane.purlis(&["artifact", "pin", "live"]);
    plane.approve_as_the_bar_does("live");
    git_ok(&alpha, &["add", "-A"]);
    git_ok(&alpha, &["commit", "-qm", "pin live"]);

    let out = plane.purlis(&["artifact", "pin", "live", "--command", "make data"]);
    assert!(out.status.success(), "{}", said(&out));
    git_ok(&alpha, &["add", "-A"]);
    // Committed by a teammate.
    let out = Command::new("git")
        .arg("-C")
        .arg(&alpha)
        .args([
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "repoint live",
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .envs(IDENTITY)
        .env("GIT_AUTHOR_NAME", "Grace")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "{}", said(&out));

    let line = plane.standing("live");
    assert!(line.starts_with("live  pin-changed  "), "{line}");
    assert!(line.contains("pin changed by Grace"), "{line}");

    plane.approve_as_the_bar_does("live");
    assert!(plane.standing("live").starts_with("live  approved  "));
}

#[test]
fn a_teammates_committed_approval_file_counts_for_nothing() {
    let plane = Plane::new();
    plane.page("live", SCRIPTED);
    plane.purlis(&["artifact", "pin", "live"]);
    // Every name an approval could be planted under in the plane or the workspace.
    let served =
        purlis_core::artifact::serve::Snapshot::read(&plane.artifacts().join("live")).unwrap();
    let forged = format!(
        r#"{{"version": 1, "approved": {{"{}": {{"pin": "000000000000", "fingerprint": "{}"}}}}}}"#,
        purlis_core::artifact::approval::key(&plane.alpha(), "live"),
        served.fingerprint()
    );
    for at in [
        plane.root().join("artifact-approvals.json"),
        plane.alpha().join("artifact-approvals.json"),
        plane.artifacts().join("artifact-approvals.json"),
        plane.artifacts().join("live/artifact-approvals.json"),
    ] {
        std::fs::write(at, &forged).unwrap();
    }
    let line = plane.standing("live");
    assert!(line.starts_with("live  "), "{line}");
    assert!(line.starts_with("live  new  "), "{line}");
}
