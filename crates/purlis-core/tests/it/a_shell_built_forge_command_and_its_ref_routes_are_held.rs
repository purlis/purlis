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
        "gh pr create --body-file - <<BODY\na `env` b\nBODY\n",
        "cat <<DOC\n$(cat notes.md)\nDOC",
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

/// Each command is held by the merge hold, however it reads the request, and refused unattended
/// by the floor.
fn assert_held_somehow(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

/// Round two, blocker 1: a brace expansion builds the program whatever its words hold, a `/`
/// among them included. The brace is read over the whole word, not its last path component.
#[test]
fn a_brace_that_holds_a_slash_still_builds_the_program() {
    purlis_core::unsteered!();
    assert_held_somehow(&[
        "{/opt/homebrew/bin/gh,pr,merge,12}",
        "{gh,pr,merge,12,-R,o/r}",
        "{gh,api,-X,PATCH,repos/o/r/git/refs/heads/main,-f,sha=abc}",
        "{git,push,origin,HEAD:refs/tags/v1}",
        "{gh,release,create,v1,dist/x.tgz}",
    ]);
}

/// Round two, blocker 2: an unquoted substitution in a program word is split into words, so a
/// path after it does not make it a path: what it prints is the command.
#[test]
fn a_substitution_before_a_path_suffix_is_read_as_what_it_prints() {
    purlis_core::unsteered!();
    assert_held_somehow(&[
        "$(echo gh pr merge 12 --squash --subject x)/y",
        "`echo gh pr merge 12 --subject x`/y",
        "$(echo gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc -f x=y)/z",
    ]);
    assert_passes(&[
        "$(git rev-parse --show-toplevel)/tools/check.sh",
        "\"$(dirname \"$0\")/run.sh\" --fast",
    ]);
}

/// Round two, blocker 3: a glob or a brace counts only in command position. A `case` pattern,
/// an arithmetic expansion, an array's words, a parameter expansion and an interpreter's heredoc
/// body are none, and ordinary shell built from them passes in every mode.
#[test]
fn ordinary_shell_with_a_glob_outside_command_position_passes() {
    purlis_core::unsteered!();
    assert_passes(&[
        "case \"$x\" in a) echo a;; *) echo o;; esac",
        "case $x in y*) echo y;; *) echo n;; esac",
        "case $x in y*|n*) echo yn;; esac",
        "case $x in\n  y*) echo y ;;\n  *) echo n ;;\nesac",
        "if [ -f x ]; then\n  case $1 in\n    -*) echo flag ;;\n    *) echo word ;;\n  esac\nfi",
        "echo $((2*3))",
        "i=$((i*2))",
        "n=$(( $(wc -l < f) + 1 ))",
        "((i*2))",
        "files=(src/*.rs)",
        "files=(src/*.rs) && echo \"${files[@]}\"",
        "\"${cmd[@]}\" --flag",
        "python3 - <<'EOF'\nd[\"k\"] = 1\nx[0] = 3\nEOF",
        "node - <<'EOF'\nconst x = [];\nx[0] = 3\nEOF",
        "ruby <<'EOF'\nx = {}\nx[0] = 3\nEOF",
    ]);
    // What a `case` arm or a shell's heredoc runs is still read.
    assert_held_somehow(&[
        "case $x in *) {gh,pr,merge,12};; esac",
        "bash <<'EOF'\n{gh,pr,merge,12}\nEOF",
        "echo $(( $({gh,pr,merge,12}) + 1 ))",
    ]);
}

/// The ordinary work that must keep passing, beside round two's readings.
#[test]
fn ordinary_forge_and_push_work_still_passes() {
    purlis_core::unsteered!();
    assert_passes(&[
        "git push origin feat",
        "git push -u origin HEAD",
        "git push --force-with-lease origin feat",
        "gh pr view 12",
        "gh pr create --title t --body \"$(cat body.md)\"",
        "gh pr create --title t --body \"$(cat <<'EOF'\nrebased the branch\nEOF\n)\"",
    ]);
}

/// zsh's `=cmd` is the path of `cmd`, so `=gh` is `gh`.
#[test]
fn a_zsh_equals_program_is_read_as_its_command() {
    purlis_core::unsteered!();
    assert_held_somehow(&[
        "=gh pr merge 12",
        "=gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc",
    ]);
}

/// glab fills `:branch` (and `:id`, `:fullpath`) in from the checkout, as gh fills `{branch}`.
#[test]
fn a_glab_colon_placeholder_could_be_the_default_branch() {
    purlis_core::unsteered!();
    assert_held(&[
        "glab api -X DELETE projects/1/repository/branches/:branch",
        "glab api -X POST projects/1/repository/commits -F branch=:branch -f commit_message=m",
    ]);
    assert_passes(&["glab api projects/:id/repository/branches/:branch"]);
}

/// A GraphQL string is text, not a name: a comment that says it rebased the branch changes no
/// ref. A mutation whose input comes in a variable says nothing of what it sets.
#[test]
fn a_graphql_string_is_not_read_and_a_variable_input_is() {
    purlis_core::unsteered!();
    assert_passes(&[
        "gh api graphql -f query='mutation { addComment(input: {subjectId: \"X\", body: \"rebased the branch onto main and merged the tag\"}) { clientMutationId } }'",
    ]);
    assert_held(&[
        "glab api graphql -f query='mutation($i:CommitCreateInput!){commitCreate(input:$i){errors}}'",
        "gh api graphql -f query='mutation { deleteRepositoryRuleset(input: {repositoryRulesetId: \"R\"}) { clientMutationId } }'",
    ]);
}

/// Q330: a ruleset guards what lands on the default branch, and GitLab commits to a branch
/// through its files, cherry-pick and revert endpoints too.
#[test]
fn rulesets_and_gitlabs_other_commits_to_the_default_branch_are_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api -X DELETE repos/o/r/rulesets/12",
        "gh api -X PUT repos/o/r/rulesets/12 --input r.json",
        "glab api -X PUT projects/1/repository/files/README.md -f branch=main -f content=x -f commit_message=m",
        "glab api -X POST projects/1/repository/commits/abc123/cherry_pick -f branch=main",
        "glab api -X POST projects/1/repository/commits/abc123/revert -f branch=main",
    ]);
    assert_passes(&[
        "gh api repos/o/r/rulesets",
        "glab api -X PUT projects/1/repository/files/README.md -f branch=feat -f content=x -f commit_message=m",
        "glab api -X POST projects/1/repository/commits/abc123/cherry_pick -f branch=feat",
    ]);
}

/// A push that carries tags in the spellings round two found: every ref, a `tag <name>`, a
/// refspec over every ref, and the config that follows tags.
#[test]
fn round_twos_tag_pushes_are_refused() {
    purlis_core::unsteered!();
    assert_floored(&[
        "git push --mirror",
        "git push origin --mirror",
        "git push origin tag release",
        "git push origin 'refs/*:refs/*'",
        "git push origin '+refs/*:refs/*'",
        "git -c push.followTags=true push origin feat",
        "git -c push.followtags push origin feat",
    ]);
    assert_passes(&[
        "git -c push.followTags=false push origin feat",
        "git push origin 'refs/heads/*:refs/heads/*'",
        "git push origin tagged-feature",
    ]);
}
