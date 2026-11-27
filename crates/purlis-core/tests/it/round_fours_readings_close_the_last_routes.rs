// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round four of #7131's review: a `dirname` of several operands, a `case` whose word is a
//! substitution, a GraphQL definition the floor cannot name, a `$((` that is no arithmetic, and
//! a shell found by `which` or filled in by `$SHELL`.
//!
//! Reading the shell's text is the early warning, not the lock (Q331); the lock is the forge
//! credential (#7133).

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
    }
}

/// Each command passes both.
fn assert_passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// Must-fix 1: `dirname` prints a line per operand, and the unquoted substitution splits them,
/// so several operands spell a command word by word. Only one literal operand prints a path.
#[test]
fn a_dirname_of_more_than_one_operand_could_print_a_command() {
    purlis_core::unsteered!();
    assert_held(&[
        "$(dirname gh/x pr/x merge/x 12/x)",
        "$(dirname a/gh b/pr c/merge d/12)/y",
        "$(dirname $X)/run.sh",
        "$(dirname -z gh/x)/y",
    ]);
    assert_passes(&["$(dirname \"$0\")/build.sh", "$(dirname tools/x)/check.sh"]);
}

/// Must-fix 2: `case WORD in` with a substitution for its word is a `case`, so its patterns
/// are no programs, attended or not.
#[test]
fn a_case_on_a_substitution_reads_its_patterns_as_patterns() {
    purlis_core::unsteered!();
    assert_passes(&[
        "case $(uname) in Darwin) echo mac;; *) echo other;; esac",
        "case `uname -s` in Linux*) echo linux;; *) echo other;; esac",
        "case $(uname -s) in\n  Darwin) echo mac ;;\n  Linux*) echo linux ;;\nesac",
        "case $(uname)-$(uname -m) in Darwin-arm64) echo m;; *) echo o;; esac",
    ]);
    // The substitution still runs, and is read where it stands.
    assert_held(&[
        "case $({gh,pr,merge,12}) in *) echo x;; esac",
        "case `{gh,pr,merge,12}` in *) echo x;; esac",
        "case $(uname) in *) {gh,pr,merge,12};; esac",
    ]);
}

/// Nice-to-have: `case` in the ways a script writes it.
#[test]
fn a_case_in_its_other_spellings_is_read() {
    purlis_core::unsteered!();
    assert_passes(&[
        "echo else while { case",
        "case $x in a) case $y in b*) echo ab;; esac;; *) echo o;; esac",
        "case $x in a*) echo a;& b*) echo b;; esac",
        "case $x in (a*) echo a;; (*) echo o;; esac",
    ]);
    assert_held(&["case $x in a) case $y in b*) {gh,pr,merge,12};; esac;; esac"]);
}

/// Should-fix 3: a GraphQL definition that opens with neither an operation's keyword, a
/// fragment nor `{` (a description string, say) is not read, so it is held.
#[test]
fn a_graphql_definition_the_floor_cannot_name_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api graphql -f query='\"d\" mutation { mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } }'",
        "gh api graphql -f query='\"\"\"d\"\"\" mutation { addComment(input: {subjectId: \"X\", body: \"b\"}) { clientMutationId } }'",
        "gh api graphql -f query='query { viewer { login } } extend type Q { x: Int }'",
    ]);
}

/// Nice-to-have: the GraphQL lexer and reader on the shapes a review named.
#[test]
fn graphql_strings_fragments_and_several_operations_are_read() {
    purlis_core::unsteered!();
    assert_passes(&[
        "gh api graphql -f query='mutation { addComment(input: {subjectId: \"X\", body: \"say \\\"mergePullRequest\\\" { \"}) { clientMutationId } }'",
        "gh api graphql -f query='query { viewer { ... on User { login } } }'",
    ]);
    assert_held(&[
        "gh api graphql -f query='mutation { addComment(input: {subjectId: \"X\", body: \"\"\"open}) { x } }'",
        "gh api graphql -f query='mutation { ... on Mutation { mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } } }'",
        "gh api graphql -f query='query { viewer { login } } mutation { mergePullRequest(input: {pullRequestId: \"X\"}) { clientMutationId } }'",
    ]);
}

/// Should-fix 4: a `$((` that does not close as `))` is a command substitution of a subshell.
#[test]
fn a_dollar_double_paren_that_is_no_arithmetic_is_a_substitution() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo $((gh pr merge 12) )",
        "echo $((gh pr merge 12); true)",
        "X=$((gh pr merge 12) )",
        "$(pwd)$((echo gh pr merge 12) )",
        "echo \"$((gh pr merge 12) )\"",
    ]);
    assert_passes(&["echo $((2*3))", "n=$(( $(wc -l < f) + 1 ))"]);
}

/// Should-fix 5 and `$SHELL`: a shell found by `which` or `command -v`, or filled in by a
/// variable, runs its `-c` string as a shell does.
#[test]
fn a_shell_found_by_which_or_filled_in_reads_its_script() {
    purlis_core::unsteered!();
    assert_held(&[
        "$(which bash) -c 'gh pr merge 12'",
        "$(command -v sh) -c 'gh pr merge 12'",
        "`which zsh` -c 'gh pr merge 12'",
        "$SHELL -c 'gh pr merge 12'",
        "\"$SHELL\" -c 'gh pr merge 12'",
    ]);
    assert_passes(&[
        "$(which bash) -c 'echo hi'",
        "$SHELL -c 'cargo test'",
        "$(which python3) --version",
    ]);
}

/// Nice-to-have: the path-printing allowlist on the shapes a review named.
#[test]
fn the_path_printing_allowlist_holds_what_could_print_a_command() {
    purlis_core::unsteered!();
    assert_held(&[
        "$(which -a gh) pr merge 12",
        "$(git rev-parse HEAD) pr merge 12",
    ]);
}

/// Nice-to-have: a command follows `function name {`, `coproc` and `select … do`.
#[test]
fn a_command_after_function_coproc_or_select_is_read() {
    purlis_core::unsteered!();
    assert_held(&[
        "function f { {gh,pr,merge,12}; }",
        "coproc {gh,pr,merge,12}",
        "select x in a b; do {gh,pr,merge,12}; done",
    ]);
}

/// Held by design, as the PR declares: a GraphQL mutation whose name speaks of a branch or a
/// merge, or whose input object comes whole in a variable, is held whatever it does.
#[test]
fn the_graphql_holds_by_design_stay_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api graphql -f query='mutation { updatePullRequestBranch(input: {pullRequestId: \"X\"}) { clientMutationId } }'",
        "gh api graphql -f query='mutation { deleteRef(input: {refId: \"feature-ref\"}) { clientMutationId } }'",
        "glab api graphql -f query='mutation { mergeRequestCreate(input: {projectPath: \"g/p\", sourceBranch: \"feat\", targetBranch: \"main\", title: \"t\"}) { errors } }'",
        "gh api graphql -f query='mutation($i: AddCommentInput!) { addComment(input: $i) { clientMutationId } }'",
    ]);
}
