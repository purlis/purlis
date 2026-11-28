// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7880: the words of `purlis worktree commit` after its options are paths, which it hands git
//! to stage, never programs it runs. A path whose last segment is `gh`, `glab` or `git`
//! (`tests/fakes/glab`) was read as a forge call behind an unknown program, and the next path as
//! its noun, so the commit was refused. The same words as a real program are still held, and a
//! substitution in the message still runs, so it is still read.

use purlis_core::floorguard::{A_HEREDOC_IT_RUNS, merge_reason, release_floor_reason};
use purlis_core::mergehold;

/// Each passes the hold, the floor's merge half and the release floor, attended or not.
fn passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// Each is refused by the release floor read unattended, and, where it merges, held.
fn floored(cmds: &[&str]) {
    for cmd in cmds {
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

#[test]
fn a_path_named_gh_glab_or_git_in_a_worktree_commit_passes() {
    purlis_core::unsteered!();
    passes(&[
        "env -C .worktrees/purlis/x purlis worktree commit -m \"fix\" bin/a.sh tests/fakes/glab tests/b.sh README.md",
        "purlis worktree commit -m fix tests/fakes/gh pr merge.sh",
        "purlis worktree commit -m fix tests/fakes/glab mr accept",
        "purlis worktree commit -m fix tests/fakes/git push origin",
        "purlis worktree commit -m 'land it' crates/gh src/release/create.rs",
        "purlis worktree commit --message=fix -a tests/fakes/gh tests/fakes/glab",
        "charter worktree commit -m fix tests/fakes/gh pr merge",
        "/Applications/purlis.app/Contents/MacOS/purlis worktree commit -m fix tests/fakes/gh pr",
    ]);
}

#[test]
fn the_same_words_as_a_program_are_still_held() {
    purlis_core::unsteered!();
    for cmd in [
        "gh pr merge 5",
        "tests/fakes/gh pr merge 5",
        "frobnicate tests/fakes/gh pr merge 5",
        "purlis worktree commit -m fix a.rs; gh pr merge 5",
        "purlis worktree commit -m fix a.rs && glab mr merge 5",
        "purlis worktree commit -m fix a.rs | gh pr merge 5",
        // Not the commit: another purlis command, or a wrapper in front of the commit's words.
        "purlis worktree update gh pr merge 5",
        "purlis --x worktree commit gh pr merge 5",
        "frobnicate purlis worktree commit -m fix gh pr merge 5",
        // An assignment in front may reach the git it runs.
        "GIT_EDITOR=x purlis worktree commit -m fix gh pr merge 5",
        "env GIT_SSH_COMMAND=x purlis worktree commit -m fix gh pr merge 5",
        // A function the line defines under the name runs its words.
        "purlis() { \"$@\"; }; purlis worktree commit gh pr merge 5",
        "function purlis { \"$@\"; }; purlis worktree commit gh pr merge 5",
    ] {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
    }
    floored(&["purlis worktree commit -m fix a.rs; gh release create v1"]);
}

#[test]
fn a_substitution_in_a_worktree_commits_message_is_still_read() {
    purlis_core::unsteered!();
    for cmd in [
        "purlis worktree commit -m \"$(gh pr merge 1)\" a.rs",
        "purlis worktree commit -m \"`gh pr merge 1`\" a.rs",
        "purlis worktree commit -m fix \"$(gh pr merge 1)\"",
        "purlis worktree commit -m fix <(gh pr merge 1)",
    ] {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
    }
}

#[test]
fn a_refusal_earned_only_by_a_heredoc_a_program_runs_names_the_editor_tool() {
    purlis_core::unsteered!();
    for cmd in [
        "python3 - <<'EOF'\ngh pr merge 5\nEOF",
        "node - <<EOF\nglab mr merge 5\nEOF",
    ] {
        for said in [release_floor_reason(cmd, true), merge_reason(cmd)] {
            let said = said.unwrap_or_else(|| panic!("not refused: {cmd}"));
            assert!(
                said.ends_with(A_HEREDOC_IT_RUNS),
                "no way out named: {cmd}: {said}"
            );
        }
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
    }
}

#[test]
fn a_refusal_the_command_itself_earns_names_no_heredoc() {
    purlis_core::unsteered!();
    for cmd in [
        "gh pr merge 5",
        "gh pr merge 5 <<'EOF'\nx\nEOF",
        // A shell's heredoc is commands: its answer is the one the Python charter recorded.
        "bash <<'EOF'\ngh pr merge 5\nEOF",
        "python3 - <<'EOF'\nprint(1)\nEOF\ngh pr merge 5",
        "gh pr merge 5; bash <<'EOF'\ngh pr merge 6\nEOF",
        "bash <<'EOF'\necho hi\nEOF\ngh pr merge 5",
        "cat <<'EOF'\ngh pr merge 5\nEOF\ngh pr merge 5",
    ] {
        let said = release_floor_reason(cmd, true).unwrap_or_else(|| panic!("not refused: {cmd}"));
        assert!(
            !said.contains(A_HEREDOC_IT_RUNS),
            "names a heredoc: {cmd}: {said}"
        );
    }
}
