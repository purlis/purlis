// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The leaks #7131's second round opened, closed by reading the grammar rather than one more
//! pattern (#7131, round three).
//!
//! Reading the shell's text is the early warning, not the lock (Q331); the lock is the forge
//! credential (#7133). A `case` counts only where a command starts and opens patterns only after
//! `case WORD in`. A GraphQL document is lexed, so a `#` comment and a string are both text, and
//! only a mutation's operation names are read. A command substitution in a program word is held
//! unless what it runs only prints a path.

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold;

/// Each command is held by the merge hold, however it reads the request, and refused unattended
/// by the floor.
fn assert_held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

/// Each command is refused unattended by the floor.
fn assert_floored(cmds: &[&str]) {
    for cmd in cmds {
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

/// Each command passes both.
fn assert_passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// Blocker 1: `case` as an argument opens no pattern, and a `case` with no `WORD in` after it
/// opens none either, so the rest of the line is still read.
#[test]
fn the_word_case_as_an_argument_hides_nothing_after_it() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo do case; {gh,pr,merge,12}",
        "echo then case; $(echo gh pr merge 12)",
        "echo then case; `echo gh pr merge 12`",
        "echo ! case; g[h] pr merge 12",
        "git log --grep fix | case; {gh,pr,merge,12}",
        "echo if case x in; {gh,pr,merge,12}",
        "case x; {gh,pr,merge,12}",
        "case x in a b; {gh,pr,merge,12}",
    ]);
    assert_passes(&[
        "case $x in y*) echo y;; esac",
        "for f in a b; do case $f in a*) echo a;; esac; done",
        "if true; then case $1 in -*) echo flag;; esac; fi",
        "echo case in a; echo done",
    ]);
}

/// Blocker 2: a GraphQL `#` comment is lexed as a comment, so a `"` in it opens no string that
/// hides the mutation after it.
#[test]
fn a_graphql_comment_with_a_quote_hides_no_mutation() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api graphql -f query='#\"\nmutation { mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } }'",
        "gh api graphql -f query='#\"\nmutation { updateRef(input: {refId: \"R\", oid: \"abc\"}) { clientMutationId } }'",
        "glab api graphql -f query='#\"\nmutation { mergeRequestAccept(input: {projectPath: \"g/p\", iid: \"3\"}) { errors } }'",
        "gh api graphql -F id=X -f query='# it\"s a comment\nmutation($id: ID!) { mergePullRequest(input: {pullRequestId: $id}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } } \"'",
    ]);
    assert_passes(&[
        "gh api graphql -f query='# merges nothing\nquery { viewer { login } }'",
        "gh api graphql -f query='mutation { addComment(input: {subjectId: \"X\", body: \"# not a comment\"}) { clientMutationId } }'",
    ]);
}

/// A mutation is read by its operation names, not by a field name that holds `ref` or `branch`.
#[test]
fn a_graphql_mutation_is_read_by_its_operation_names() {
    purlis_core::unsteered!();
    assert_passes(&[
        "gh api graphql -f query='mutation { createPullRequest(input: {repositoryId: \"R\", baseRefName: \"main\", headRefName: \"feat\", title: \"t\"}) { pullRequest { number } } }'",
    ]);
    assert_held(&[
        "gh api graphql -f query='mutation { m: mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } }'",
        "gh api graphql -f query='mutation M { ...F } fragment F on Mutation { mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } }'",
    ]);
}

/// Blocker 3: a program word that holds a command substitution anywhere is held, unless every
/// substitution only prints a path.
#[test]
fn a_substitution_in_a_program_word_is_held_unless_it_prints_a_path() {
    purlis_core::unsteered!();
    assert_held(&[
        "/opt/homebrew/bin/$(echo gh pr merge 12 --body x)/y",
        "./$(echo gh pr merge 12 --body x)/y",
        "\"\"$(echo gh pr merge 12 --body x)/y",
        "$X$(echo gh pr merge 12 --body x)/y",
        "${X:-}$(echo gh pr merge 12 --body x)/y",
        "$(echo gh; echo pr merge 12 --body x)/y",
        "$(true && echo gh pr merge 12 --body x)/y",
        "$(echo $(echo gh pr merge 12 --body x))/y",
        "$(printf 'gh\\tpr\\tmerge\\t12\\t--body\\tx')/y",
        "$(echo Z2ggcHIgbWVyZ2UgMTI= | base64 -d)/y",
        "$(cat cmd.txt)/y",
        "$(which gh) pr merge 12",
        "$(dirname 'gh pr merge 12/x')/y",
    ]);
    assert_passes(&[
        "$(git rev-parse --show-toplevel)/x.sh",
        "$(dirname \"$0\")/build.sh",
        "$(which python3) --version",
        "`which python3` --version",
        "$(command -v python3) --version",
        "$(pwd)/run.sh",
        "$(brew --prefix)/bin/thing --help",
        "$(realpath tools)/check.sh",
        "$(readlink -f tools)/check.sh",
    ]);
}

/// `gh repo sync --force` resets the branch it syncs, the default one when it names none.
#[test]
fn a_forced_repo_sync_of_the_default_branch_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh repo sync o/r --branch main --force",
        "gh repo sync o/r --force",
        "gh repo sync --force --branch=master",
        "gh repo sync o/r -b $B --force",
    ]);
    assert_passes(&[
        "gh repo sync o/r --branch feat --force",
        "gh repo sync o/r --branch main",
    ]);
}

/// An organisation's ruleset guards its repositories' default branches as a repository's does.
#[test]
fn an_organisations_ruleset_write_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api -X DELETE orgs/o/rulesets/5",
        "gh api -X PUT orgs/o/rulesets/5 --input r.json",
        "gh api -X POST orgs/o/rulesets --input r.json",
    ]);
    assert_passes(&["gh api orgs/o/rulesets"]);
}

/// git takes any unambiguous prefix of a long option, and a remote's configured push refspec or
/// mirror flag pushes what it names.
#[test]
fn a_push_option_git_reads_by_its_prefix_or_its_config_is_refused() {
    purlis_core::unsteered!();
    assert_floored(&[
        "git push --mirr",
        "git push origin --tag",
        "git push origin --ta",
        "git push --follow origin feat",
        "git -c remote.origin.push=refs/tags/* push origin",
        "git -c remote.origin.mirror=true push origin",
        "git -c remote.origin.mirror push origin",
    ]);
    assert_passes(&[
        "git push --force-with origin feat",
        "git push --set-up origin feat",
        "git -c remote.origin.mirror=false push origin feat",
    ]);
}

/// `source` and `.` of stdin run what feeds it, as a shell reading stdin does.
#[test]
fn sourcing_stdin_runs_what_feeds_it() {
    purlis_core::unsteered!();
    assert_held(&[
        "source /dev/stdin <<< \"gh pr merge 12\"",
        ". /dev/stdin <<< 'gh pr merge 12'",
        "source /dev/stdin <<'EOF'\ngh pr merge 12\nEOF",
        "echo gh pr merge 12 | . /dev/stdin",
    ]);
}

/// A glob alone, whose every match is one character long, names no CLI the floor reads.
#[test]
fn a_lone_one_character_glob_is_no_forge_command() {
    purlis_core::unsteered!();
    assert_passes(&["echo a; ?"]);
}

