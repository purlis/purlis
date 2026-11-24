// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis policy why` through the binary** (M75 slice 18, #7027; spec §10.1 and §10.2): a
//! person asks why an action is decided as it is, and the answer names the deciding rule with its
//! file and line, the permit it overrode and the level that could loosen it, and the project is
//! as it was. `--policy <branch>` answers as the branch would and shows the diff the press shows.
//! A chat may ask only about itself.

use std::path::Path;
use std::process::Output;

use crate::support;

const STRICT: &str = "[policy]\ntemplate = \"strict\"\n";
const BAN: &str = "// purlis-policy 1\n@id(\"guards-merge-ban\")\nforbid (principal, action == Action::\"merge\", resource);\n";

fn git(dir: &Path, args: &[&str]) {
    let ran = support::command("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=nobody"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// A plane whose commit holds `files`.
fn plane(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(
        dir.path().join(".gitignore"),
        "*.local.toml\n.charter/\n.purlis/\n",
    )
    .expect("a .gitignore");
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).expect("a file");
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn why(dir: &Path, args: &[&str]) -> Output {
    support::at_a_terminal(
        support::purlis()
            .args(["policy", "why"])
            .args(args)
            .current_dir(dir),
    )
    .output()
    .expect("purlis runs")
}

fn said(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn policy_why_names_the_deciding_rule_its_file_and_line_and_runs_nothing() {
    let dir = plane(&[("purlis.toml", STRICT), ("policy.cedar", BAN)]);
    let out = said(&why(dir.path(), &["merge", "acme/web#main"]));
    assert!(out.contains("outcome: deny"), "{out}");
    assert!(
        out.contains("`guards-merge-ban` (project, this project) at policy.cedar:"),
        "{out}"
    );
    assert!(
        out.contains("permit overridden by a forbid: `policy-everything-else`"),
        "{out}"
    );
    assert!(
        out.contains("the project level (this project): change or remove `guards-merge-ban`"),
        "{out}"
    );
    // No app answered here, so the sealed side was not read, and the answer says so.
    assert!(out.contains("note: the sealed side was not read"), "{out}");
    // Asked twice, it says the same, and the plane is as it was: nothing was written.
    assert_eq!(out, said(&why(dir.path(), &["merge", "acme/web#main"])));
    assert!(!dir.path().join(".purlis").exists());
}

#[test]
fn policy_why_json_carries_the_same_answer() {
    let dir = plane(&[("purlis.toml", STRICT), ("policy.cedar", BAN)]);
    let out = said(&why(dir.path(), &["merge", "acme/web#main", "--json"]));
    let json: serde_json::Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(json["decision"]["after_mode"], "deny", "{out}");
    assert_eq!(
        json["decision"]["deciding"][0]["id"], "guards-merge-ban",
        "{out}"
    );
    assert_eq!(
        json["decision"]["deciding"][0]["file"], "policy.cedar",
        "{out}"
    );
    assert_eq!(json["loosen"][0]["level"], "project", "{out}");
    assert_eq!(json["overrode"][0]["id"], "policy-everything-else", "{out}");
    assert!(json.get("dry_run").is_none(), "{out}");
    assert!(
        json["sealed_unread"]
            .as_str()
            .is_some_and(|said| said.contains("sealed side was not read")),
        "{out}"
    );
}

#[test]
fn chat_n_from_outside_a_chat_is_refused_where_no_app_gives_its_facts() {
    let dir = plane(&[("purlis.toml", STRICT)]);
    let out = why(dir.path(), &["merge", "acme/web#main", "--chat", "3"]);
    assert!(!out.status.success(), "answered with invented facts");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("chat 3"), "{err}");
    assert!(out.stdout.is_empty(), "printed an answer");
}

#[test]
fn policy_why_with_a_branch_policy_answers_as_the_branch_would_through_the_binary() {
    let dir = plane(&[("purlis.toml", STRICT)]);
    git(dir.path(), &["checkout", "-q", "-b", "ban"]);
    std::fs::write(dir.path().join("policy.cedar"), BAN).expect("a ban");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "ban"]);
    git(dir.path(), &["checkout", "-q", "main"]);
    // On main the shipped hold asks; the branch would refuse, and says which rule changed.
    let now = said(&why(dir.path(), &["merge", "acme/web#main"]));
    assert!(now.contains("outcome: ask"), "{now}");
    let branch = said(&why(
        dir.path(),
        &["merge", "acme/web#main", "--policy", "ban"],
    ));
    assert!(branch.contains("outcome: deny"), "{branch}");
    assert!(
        branch.contains("rule `guards-merge-ban` tightens"),
        "{branch}"
    );
    let json: serde_json::Value = serde_json::from_str(&said(&why(
        dir.path(),
        &["merge", "acme/web#main", "--policy", "ban", "--json"],
    )))
    .expect("JSON");
    assert_eq!(json["dry_run"]["policy"], "ban");
    // It says which commits it was computed for and against.
    assert!(
        json["dry_run"]["head"]
            .as_str()
            .is_some_and(|sha| sha.len() == 40)
    );
    assert!(
        json["dry_run"]["resolved"]
            .as_str()
            .is_some_and(|sha| sha.len() == 40)
    );
    assert!(branch.contains("computed for HEAD "), "{branch}");
    assert_eq!(json["dry_run"]["diff"]["rules"][0]["kind"], "tightens");
    // A name that is neither a file nor a branch is refused, and says so.
    let nope = why(dir.path(), &["merge", "acme/web#main", "--policy", "nope"]);
    assert!(!nope.status.success());
    assert!(
        String::from_utf8_lossy(&nope.stderr).contains("neither a file nor a branch"),
        "{}",
        String::from_utf8_lossy(&nope.stderr)
    );
}

#[test]
fn a_chat_may_ask_policy_why_only_about_itself_through_the_binary() {
    let dir = plane(&[("purlis.toml", STRICT)]);
    let asked = |extra: &[&str]| {
        support::purlis()
            .args(["policy", "why", "merge", "acme/web#main"])
            .args(extra)
            .current_dir(dir.path())
            .env(purlis_core::hookwire::CHAT_ENV, "7")
            .output()
            .expect("purlis runs")
    };
    for flag in [
        &["--workspace", "web"][..],
        &["--persona", "builder"],
        &["--unattended"],
        &["--sandbox", "off"],
        &["--depth", "2"],
        &["--chat", "8"],
        &["--policy", "main"],
    ] {
        let out = asked(flag);
        assert!(!out.status.success(), "{flag:?} was answered");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("only about itself"), "{flag:?}: {err}");
        assert!(err.contains(flag[0]), "{flag:?}: {err}");
        assert!(out.stdout.is_empty(), "{flag:?} printed an answer");
    }
}

#[test]
fn policy_why_names_the_actions_when_the_verb_is_no_action() {
    let dir = plane(&[("purlis.toml", STRICT)]);
    let out = why(dir.path(), &["frobnicate", "x"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("`frobnicate` is no action of the policy"),
        "{err}"
    );
    assert!(err.contains("merge"), "{err}");
}
