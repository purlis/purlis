// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(".claude").join("settings.json"),
        r#"{"env":{"CHARTER_HARNESS":"claude-code"}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("workspaces")).unwrap();
    dir
}

fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-05-04T11:32:17Z".parse().unwrap()
}

fn made(root: &Path, name: &str) {
    ensure::ensure(root, name, now(), "fixture").unwrap();
}

fn run(root: &Path, scope: Scope) -> (u8, Vec<String>) {
    let mut lines = Vec::new();
    let code = reinit(root, scope, now(), &mut |s| lines.push(s.to_string()));
    (code, lines)
}

#[test]
fn a_workspace_that_is_current_says_nothing_to_do() {
    let dir = plane();
    made(dir.path(), "gamma");
    let (code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(code, 0);
    assert_eq!(lines, ["✓ Up to date (structure v5) — nothing to do."]);
}

#[test]
fn a_name_this_plane_does_not_have_is_refused_and_creates_nothing() {
    let dir = plane();
    let (code, lines) = run(dir.path(), Scope::One("nowhere"));
    assert_eq!(code, 1);
    assert_eq!(lines, ["✗ no workspace 'nowhere'"]);
    assert!(!dir.path().join("workspaces/nowhere").exists());
}

#[test]
fn a_missing_baseline_file_is_added_and_reported_by_name() {
    let dir = plane();
    made(dir.path(), "gamma");
    std::fs::remove_file(dir.path().join("workspaces/gamma/refs/README.md")).unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(lines, ["✓ Reinitialized 'gamma' → added refs/README.md."]);
    assert!(dir.path().join("workspaces/gamma/refs/README.md").exists());
}

#[test]
fn an_old_stamp_is_bumped_and_the_line_names_both_versions() {
    let dir = plane();
    made(dir.path(), "gamma");
    std::fs::write(dir.path().join("workspaces/gamma/.purlis-structure"), "3\n").unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(
        lines,
        ["✓ Reinitialized 'gamma' → added structure v3 → v5."]
    );
}

#[test]
fn a_layer_the_plane_has_moved_past_is_refreshed_and_said_out_loud() {
    let dir = plane();
    made(dir.path(), "gamma");
    std::fs::write(
        dir.path().join(".claude").join("settings.json"),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(
        lines,
        ["✓ Reinitialized 'gamma' → refreshed .claude/settings.json (purlis's harness layer)."]
    );
    let text =
        std::fs::read_to_string(dir.path().join("workspaces/gamma/.claude/settings.json")).unwrap();
    assert!(text.contains("rm -rf"), "{text}");
}

#[test]
fn a_settings_file_charter_did_not_write_is_named_and_never_overwritten() {
    let dir = plane();
    made(dir.path(), "gamma");
    let settings = dir.path().join("workspaces/gamma/.claude/settings.json");
    std::fs::write(&settings, "MINE\n").unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(
        lines,
        [
            "! 'gamma': .claude/settings.json was not written by purlis — left completely \
             untouched; purlis never overwrites it."
        ]
    );
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), "MINE\n");
    // And it is NOT "nothing to do": a row that says the plane's rules are out of force
    // there must not sit under a line saying the plane is current.
    assert!(!lines.iter().any(|l| l.contains("nothing to do")));
}

#[test]
fn a_settings_file_purlis_cannot_confirm_it_wrote_is_named_and_left_as_is() {
    // A record in the workspace folder naming the operator's own settings, with their
    // digest, while the plane has other text: the record alone is not enough (#1583).
    let dir = plane();
    made(dir.path(), "gamma");
    let ws = dir.path().join("workspaces/gamma");
    let settings = ws.join(".claude/settings.json");
    std::fs::write(&settings, "MINE\n").unwrap();
    std::fs::write(
        ws.join(crate::layer::MARKER),
        serde_json::json!({".claude/settings.json": crate::layer::digest("MINE\n")}).to_string(),
    )
    .unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(
        lines,
        [
            "! 'gamma': purlis has newer text for .claude/settings.json but cannot confirm \
             it wrote the file there now, so it is left as is. If it is purlis's own older \
             copy, move it aside and run `purlis workspace reinit gamma` to write the \
             current one."
        ]
    );
    assert_eq!(std::fs::read(&settings).unwrap(), b"MINE\n");
    assert!(!lines.iter().any(|l| l.contains("nothing to do")));
}

#[test]
fn all_counts_repairs_and_workspaces_as_two_different_numbers() {
    let dir = plane();
    made(dir.path(), "gamma");
    made(dir.path(), "delta");
    // Two repairs in ONE workspace: a counter that printed repairs against the number of
    // workspaces said "2 of 1".
    std::fs::remove_file(dir.path().join("workspaces/gamma/refs/README.md")).unwrap();
    std::fs::write(
        dir.path().join(".claude").join("settings.json"),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    let (_code, lines) = run(dir.path(), Scope::All);
    let closing = lines.last().unwrap();
    assert!(
        closing.contains("Applied 3 repairs across 2 of 2 workspaces"),
        "{lines:?}"
    );
}

#[test]
fn a_baseline_path_with_something_in_the_way_is_named_rather_than_added() {
    let dir = plane();
    made(dir.path(), "gamma");
    std::fs::remove_file(dir.path().join("workspaces/gamma/refs/README.md")).unwrap();
    std::fs::remove_dir(dir.path().join("workspaces/gamma/refs")).unwrap();
    std::fs::write(dir.path().join("workspaces/gamma/refs"), "not a dir\n").unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("refs/README.md cannot be created")
                && l.contains("is not a directory")),
        "{lines:?}"
    );
    assert!(!lines.iter().any(|l| l.contains("added refs/README.md")));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("workspaces/gamma/refs")).unwrap(),
        "not a dir\n"
    );
}

#[test]
fn a_checkout_inside_the_workspace_is_wired_and_its_rows_are_named_under_it() {
    let dir = plane();
    made(dir.path(), "gamma");
    let clone = dir.path().join("workspaces/gamma/svc");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("wrote svc/.claude/settings.json")),
        "the whole of M2.24: a chat started in that clone gets the plane's layer — \
         {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("wrote svc/.git/info/exclude")),
        "and it is hidden in the repo charter is a guest in — {lines:?}"
    );
    assert!(
        clone.join(".claude/settings.json").exists(),
        "the row is a report of a write that happened"
    );
    let block = std::fs::read_to_string(clone.join(".git/info/exclude")).unwrap();
    assert!(block.contains("/.claude/settings.json"), "{block}");
}

#[test]
fn a_second_reinit_over_a_wired_checkout_says_there_is_nothing_to_do() {
    // M2.25, and the measurement behind it: with the rows ported, "up to date" is
    // sayable again over a plane that HAS clones. The warning M2.22 shipped fired on
    // every run of every plane with a checkout in it, which is every plane in use.
    let dir = plane();
    made(dir.path(), "gamma");
    let clone = dir.path().join("workspaces/gamma/svc");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    run(dir.path(), Scope::One("gamma"));
    let (code, lines) = run(dir.path(), Scope::One("gamma"));
    assert_eq!(code, 0);
    assert_eq!(
        lines,
        vec!["✓ Up to date (structure v5) — nothing to do.".to_string()],
        "{lines:?}"
    );
}

#[test]
fn a_file_of_the_operators_in_a_checkout_is_named_with_how_to_commit_it_on_purpose() {
    // A checkout is somebody else's repository, and that is the whole difference: the
    // file stays hidden while it is there, so the only advice that is true is `git add
    // -f`. "Move it aside" is what the workspace directory's own row says, and one
    // command may not advise what the other rules out.
    let dir = plane();
    made(dir.path(), "gamma");
    let clone = dir.path().join("workspaces/gamma/svc");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    std::fs::create_dir_all(clone.join(".claude")).unwrap();
    std::fs::write(clone.join(".claude/settings.json"), "{\"mine\": true}\n").unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    assert!(
        lines.iter().any(|l| l
            .contains("svc/.claude/settings.json holds content purlis did not write")
            && l.contains("git add -f .claude/settings.json")),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("nothing to do")),
        "and the closing line may not call the plane current over it: {lines:?}"
    );
    assert_eq!(
        std::fs::read_to_string(clone.join(".claude/settings.json")).unwrap(),
        "{\"mine\": true}\n",
        "left exactly as it is"
    );
}

/// A plane declaring machine-local rules, a workspace `gamma`, and a checkout in it that
/// charter has already wired — the only shape the co-written path is reachable from.
fn a_wired_checkout(dir: &Path) -> std::path::PathBuf {
    std::fs::write(
        dir.join(".claude/settings.local.json"),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    made(dir, "gamma");
    let clone = dir.join("workspaces/gamma/svc");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    run(dir, Scope::One("gamma"));
    let local = clone.join(".claude/settings.local.json");
    assert!(local.exists(), "charter writes it in a checkout");
    local
}

#[test]
fn a_machine_local_file_the_harness_added_to_is_current_and_reinit_says_nothing() {
    // `.claude/settings.local.json` is the one path the harness writes into too — "Yes,
    // and don't ask again" lands there — and a workspace directory never wants it,
    // because Claude Code already reads the plane's copy at the git root.
    //
    // **charter's `harness-edited`, and this port called it `harness-behind` until
    // M2.26.** charter's last write IS what the plane wants now, so every rule charter
    // mirrored is still in the file and the harness only added to it: nothing is wrong,
    // and `reinit` says nothing. Measured against the oracle, which answers "Up to date
    // (structure v5) — nothing to do" here.
    let dir = plane();
    let local = a_wired_checkout(dir.path());
    let theirs = "{\"permissions\":{\"allow\":[\"Bash(ls)\"]}}\n";
    std::fs::write(&local, theirs).unwrap();

    let (_code, lines) = run(dir.path(), Scope::One("gamma"));

    assert!(
        lines.iter().any(|l| l.contains("nothing to do")),
        "{lines:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&local).unwrap(),
        theirs,
        "deleting it to get charter's copy back destroys the harness's approvals"
    );
}

#[test]
fn a_machine_local_file_the_harness_rewrote_is_kept_and_never_merged_into() {
    // The other half, and the one that IS a finding: once the plane has moved on since
    // charter's last write, the file the harness keeps no longer holds the plane's
    // machine-local rules — and charter will not merge into it, so they are not in force
    // there and `reinit` says so. Telling the two apart is the whole of the split:
    // reported for both, an operator learns nothing from either.
    let dir = plane();
    let local = a_wired_checkout(dir.path());
    std::fs::write(&local, "{\"permissions\":{\"allow\":[\"Bash(ls)\"]}}\n").unwrap();
    run(dir.path(), Scope::One("gamma"));
    std::fs::write(
        dir.path().join(".claude/settings.local.json"),
        r#"{"permissions":{"deny":["Bash(rm -rf *)","Bash(curl *)"]}}"#,
    )
    .unwrap();

    let (_code, lines) = run(dir.path(), Scope::One("gamma"));

    assert!(
        lines.iter().any(|l| l.contains(
            "svc/.claude/settings.local.json is a file purlis cannot vouch for as its own \
             write"
        )),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("nothing to do")),
        "the plane's machine-local rules are NOT in force there: {lines:?}"
    );
}

#[test]
fn a_checkout_linked_out_of_the_plane_is_refused_whole_and_nothing_in_it_is_written() {
    let dir = plane();
    made(dir.path(), "gamma");
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join(".git")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), dir.path().join("workspaces/gamma/svc")).unwrap();
    let (_code, lines) = run(dir.path(), Scope::One("gamma"));
    let wrote = outside.path().join(".claude").exists();
    assert!(
        !wrote,
        "a tree whose own root has escaped is one purlis writes nothing into"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("svc/.git/info/exclude could not be written")),
        "{lines:?}"
    );
}
