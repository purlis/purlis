// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge commands #7124's reading missed, held as the acts they could be (#7131).
//!
//! Reading the shell's text is the early warning, not the lock (Q331); the lock is the forge
//! credential (#7133). These are the routes a review found past #7124's reading: a ref path gh
//! fills in from the checkout, a GraphQL mutation of a ref, a branch or a tag, a program the
//! shell builds, a push to a tag through a refspec or a flag the shell fills in, and the other
//! ways to move or replace the default branch through the API (Q330).

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold::{self, Held};

/// Each command is held as unreadable by the merge hold, and refused unattended by the floor.
fn assert_held(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), Some(Held::Unreadable), "{cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

/// Each command is refused unattended by the floor, and is no merge.
fn assert_floored(cmds: &[&str]) {
    for cmd in cmds {
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

/// Each command passes both.
fn assert_passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// Blocker 1: gh fills `{branch}`, `{owner}` and `{repo}` in from the checkout, so a ref path
/// with a placeholder could be the default branch's.
#[test]
fn a_ref_path_gh_fills_in_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api -X PATCH repos/o/r/git/refs/heads/{branch} -f sha=abc123",
        "gh api -X PATCH repos/{owner}/{repo}/git/refs/heads/{branch} -f sha=abc -F force=false",
        "gh api -X DELETE repos/o/r/git/refs/{ref}",
    ]);
    assert_passes(&["gh api -X PATCH repos/{owner}/{repo}/git/refs/heads/feat -f sha=abc123"]);
}

/// Blocker 2: a GraphQL mutation that touches a ref, a branch or a tag can move the default
/// branch or publish a tag, whatever it is called.
#[test]
fn a_graphql_mutation_of_a_ref_branch_or_tag_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api graphql -f query='mutation { updateRef(input: {refId: \"R\", oid: \"abc\"}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { createRef(input: {repositoryId: \"R\", name: \"refs/tags/v1\", oid: \"abc\"}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { deleteRef(input: {refId: \"R\"}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { updateRefs(input: {repositoryId: \"R\", refUpdates: []}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { createCommitOnBranch(input: {branch: {branchName: \"main\"}}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { updateBranchProtectionRule(input: {branchProtectionRuleId: \"B\"}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { deleteBranchProtectionRule(input: {branchProtectionRuleId: \"B\"}) { clientMutationId } }'",
        "glab api graphql -f query='mutation { commitCreate(input: {projectPath: \"g/p\", branch: \"main\"}) { errors } }'",
    ]);
    assert_passes(&[
        "gh api graphql -f query='mutation { addComment(input: {subjectId: \"X\", body: \"hi\"}) { clientMutationId } }'",
        "gh api graphql -f query='query { repository(owner: \"o\", name: \"r\") { defaultBranchRef { name } } }'",
    ]);
}

/// Blocker 3: a program built by a brace expansion or a glob could be any command with any
/// words after it.
#[test]
fn a_program_a_brace_or_a_glob_builds_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "{gh,pr,merge,12}",
        "{gh,} pr merge 12",
        "/opt/homebrew/bin/g[h] pr merge 12",
        "/opt/homebrew/bin/g? pr merge 12",
    ]);
    assert_floored(&["{git,push,--tags}"]);
    assert_passes(&["[ -f x ] && echo y", "[[ -n $X ]]", "./build/{a}.sh"]);
}

/// Blocker 4: a program that is a command substitution is unreadable whatever words follow.
#[test]
fn a_program_a_substitution_builds_is_held_whatever_follows() {
    purlis_core::unsteered!();
    assert_held(&[
        "$(echo gh pr) merge 12",
        "$(echo gh pr merge 12)",
        "`echo gh pr merge 12`",
        "/usr/bin/$(echo gh) pr merge 12",
    ]);
    assert_passes(&[
        "$(git rev-parse --show-toplevel)/tools/check.sh",
        "\"$(dirname \"$0\")/run.sh\" --fast",
    ]);
}

/// A refspec's destination is what the remote gets: `HEAD:refs/tags/v1` pushes a tag.
#[test]
fn a_push_whose_refspec_lands_on_a_tag_is_refused() {
    purlis_core::unsteered!();
    assert_floored(&[
        "git push origin HEAD:refs/tags/v1",
        "git push origin +abc123:refs/tags/release-1",
        "git push origin main:v1.2.3",
    ]);
    assert_passes(&[
        "git push origin HEAD:refs/heads/feat",
        "git push origin HEAD:feat",
    ]);
}

/// A `git push` flag whose value the shell fills in is no better read than one whose name it
/// fills in.
#[test]
fn a_push_flag_whose_value_the_shell_fills_in_is_refused() {
    purlis_core::unsteered!();
    assert_floored(&[
        "git push --repo=$T",
        "git push --receive-pack=\"$(echo Z2l0 | base64 -d)\" origin feat",
    ]);
    assert_passes(&["git push --force-with-lease=feat:$SHA origin feat"]);
}

/// Q330's other routes: renaming the default branch, making another branch the default, and
/// GitLab's own endpoints for deleting, committing to and protecting a branch.
#[test]
fn the_other_routes_to_the_default_branch_are_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api -X POST repos/o/r/branches/main/rename -f new_name=old",
        "gh api -X POST repos/o/r/branches/master/rename -f new_name=old",
        "gh api -X POST repos/o/r/branches/{branch}/rename -f new_name=old",
        "gh api -X PATCH repos/o/r -f default_branch=feat",
        "gh api -X PUT repos/o/r/branches/main/protection --input p.json",
        "gh api -X DELETE repos/o/r/branches/main/protection",
        "glab api -X DELETE projects/1/repository/branches/main",
        "glab api -X POST projects/1/repository/commits -f branch=main -f commit_message=m",
        "glab api -X POST projects/1/repository/commits --input c.json",
        "glab api -X POST projects/1/repository/branches -f branch=main -f ref=feat",
        "glab api -X PUT projects/1 -f default_branch=feat",
        "glab api -X POST projects/1/protected_branches -f name=main",
        "glab api -X DELETE projects/1/protected_branches/main",
    ]);
    assert_passes(&[
        "gh api -X POST repos/o/r/branches/feat/rename -f new_name=feat-2",
        "gh api -X PATCH repos/o/r -f description=x",
        "glab api -X DELETE projects/1/repository/branches/feat",
        "glab api -X POST projects/1/repository/commits -f branch=feat -f commit_message=m",
        "glab api projects/1/protected_branches",
    ]);
}
