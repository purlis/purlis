// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis policy check` (T1a-64, R-432, H-11): what this build reads in a policy file, before an
//! administrator rolls it out. Every file here is a draft in a temporary folder, named on the
//! command line, so nothing reads or writes this machine's `/etc/purlis/policy.json`.

use crate::support;

use std::path::{Path, PathBuf};

struct Ran {
    code: i32,
    out: String,
    err: String,
}

impl Ran {
    fn lines(&self) -> Vec<&str> {
        self.out.lines().collect()
    }
}

/// A draft policy holding `text`, in a folder of its own.
struct Draft {
    _dir: tempfile::TempDir,
    file: PathBuf,
}

impl Draft {
    fn holding(text: &str) -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("policy.json");
        std::fs::write(&file, text).expect("written");
        Self { _dir: dir, file }
    }

    fn shown(&self) -> String {
        self.file.display().to_string()
    }

    fn check(&self) -> Ran {
        check(&self.file)
    }
}

/// `purlis policy check <file>`, run from the draft's own folder: the command needs no plane, and
/// one run from this checkout would find the plane it sits in.
fn check(file: &Path) -> Ran {
    let out = support::purlis()
        .args(["policy", "check"])
        .arg(file)
        .current_dir(file.parent().expect("a folder"))
        .output()
        .expect("the binary runs");
    Ran {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn draft_line(draft: &Draft) -> String {
    format!(
        "file: {} (a draft: its owner and permissions are not judged)",
        draft.shown()
    )
}

#[test]
fn policy_check_prints_the_lowest_reading_version_and_unenforced_keys() {
    // Without `models` and `min-version`: keys every stable release since 0.5.0 reads.
    let old = Draft::holding(r#"{"owner": "Platform", "sandbox": {"opt-out": false}}"#);
    let ran = old.check();
    assert_eq!(
        ran.lines(),
        [
            draft_line(&old).as_str(),
            "lowest purlis version that reads this file: 0.5.0",
            "chats on a key: not refused",
            "chats on a sign-in: not refused",
        ],
        "{}",
        ran.err
    );

    // A key 0.5.1 added raises the version.
    let newer = Draft::holding(r#"{"sandbox": {"opt-out": false, "run-grants": false}}"#);
    assert_eq!(
        newer.check().lines()[1],
        "lowest purlis version that reads this file: 0.5.1"
    );
    let stuck = Draft::holding(r#"{"dispatch": {"depth": 2, "stuck-failing-times": 4}}"#);
    assert_eq!(
        stuck.check().lines()[1],
        "lowest purlis version that reads this file: 0.5.1"
    );

    // With `models` and `min-version`: each key inside `models` is one this version doesn't
    // enforce, and the start's refusal is said in the window's words.
    let new = Draft::holding(
        r#"{"min-version": "0.0.1", "models": {"allowed": ["anthropic"], "denied": []},
            "sandbox": {"opt-out": false}}"#,
    );
    let ran = new.check();
    let refusal = format!(
        "{} sets `models.allowed`, which this version of purlis can't enforce. Chats can't start.",
        new.shown()
    );
    assert_eq!(
        ran.lines(),
        [
            draft_line(&new),
            "lowest purlis version that reads this file: not in a stable release yet \
             (min-version, models)"
                .to_owned(),
            "not enforced by this version: models.allowed".to_owned(),
            "not enforced by this version: models.denied".to_owned(),
            format!("chats on a key: refused: {refusal}"),
            format!("chats on a sign-in: refused: {refusal}"),
        ],
        "{}",
        ran.err
    );
}

#[test]
fn unreleased_policy_keys_print_not_in_a_stable_release_yet() {
    let min_version = Draft::holding(r#"{"min-version": "0.0.1"}"#);
    assert_eq!(
        min_version.check().lines()[1],
        "lowest purlis version that reads this file: not in a stable release yet (min-version)"
    );
    let models = Draft::holding(r#"{"models": {}, "sandbox": {"run-grants": false}}"#);
    assert_eq!(
        models.check().lines()[1],
        "lowest purlis version that reads this file: not in a stable release yet (models)"
    );
}

#[test]
fn policy_check_on_a_draft_path_skips_ownership_and_says_so() {
    // The test's own user owns this file and its folder, never root: the installed file's
    // judgement would refuse it, and a draft is read all the same.
    let draft = Draft::holding(r#"{"sandbox": {"write-grants": false}}"#);
    let ran = draft.check();
    assert_eq!(ran.lines()[0], draft_line(&draft), "{}", ran.err);
    assert_eq!(
        ran.lines()[1],
        "lowest purlis version that reads this file: 0.5.0"
    );
    assert_eq!(ran.code, 0, "{}", ran.out);
}

#[test]
fn policy_check_exit_status_is_0_1_or_2_as_stated() {
    // 0: this build reads the file and enforces every key in it.
    let clean =
        Draft::holding(r#"{"sandbox": {"presets": ["forge"]}, "dispatch": {"allow": false}}"#);
    assert_eq!(clean.check().code, 0, "{}", clean.check().out);

    // 1: a key this build doesn't enforce.
    let unenforced = Draft::holding(r#"{"models": {"allowed": []}}"#);
    assert_eq!(unenforced.check().code, 1, "{}", unenforced.check().out);

    // 1: a key this build doesn't know, named where it is.
    let unknown = Draft::holding(r#"{"sandbox": {"opt-out": false, "colour": "red"}, "theme": 1}"#);
    let ran = unknown.check();
    assert_eq!(ran.code, 1, "{}", ran.out);
    assert!(
        ran.lines()
            .contains(&"not known to this version: sandbox.colour"),
        "{}",
        ran.out
    );
    assert!(
        ran.lines().contains(&"not known to this version: theme"),
        "{}",
        ran.out
    );

    // 1: a `min-version` above this build.
    let above = Draft::holding(r#"{"min-version": "999.0.0"}"#);
    let ran = above.check();
    assert_eq!(ran.code, 1, "{}", ran.out);
    assert!(
        ran.lines().contains(
            &format!(
                "chats on a sign-in: refused: {} requires purlis 999.0.0 or later \
                 (`min-version`). Chats can't start. Update purlis.",
                above.shown()
            )
            .as_str()
        ),
        "{}",
        ran.out
    );

    // 2: the file can't be read or parsed.
    for (text, why) in [
        ("not json", "not read: it is not JSON"),
        (
            r#"{"owner": "a", "owner": "b"}"#,
            "not read: it says \"owner\" twice",
        ),
        (
            r#"{"sandbox": {"opt-out": "no"}}"#,
            "not read: its \"opt-out\" is not true or false",
        ),
        (
            r#"{"min-version": "v1"}"#,
            "not read: its \"min-version\", \"v1\", is not a version written MAJOR.MINOR.PATCH, \
             such as 0.6.0 or 0.6.0-rc.1",
        ),
    ] {
        let broken = Draft::holding(text);
        let ran = broken.check();
        assert_eq!(ran.code, 2, "{text}: {}", ran.out);
        assert_eq!(ran.lines(), [draft_line(&broken).as_str(), why], "{text}");
    }
    let dir = tempfile::tempdir().expect("a folder");
    let missing = check(&dir.path().join("policy.json"));
    assert_eq!(missing.code, 2, "{}", missing.out);
    assert_eq!(missing.lines()[1], "not read: there is no file there");
}
