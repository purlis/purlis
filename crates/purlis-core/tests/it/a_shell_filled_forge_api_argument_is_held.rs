// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A `gh api` / `glab api` call whose arguments the shell fills in is held, read or not (#7416).
//! An unquoted expansion is word-split, and a word that begins with an expansion may be a flag,
//! so `gh api repos/o/r/pulls/5/merge $(cat n)` merges when `n` holds `-XPUT`, and a merge
//! request's title or a user's bio can hold it. The rule is not a list of spellings (#7423):
//! every word of the call is held unless it is plainly one word, so what passes is a word with
//! no expansion, glob or brace, or one literal start and one double-quoted span of a plain
//! `$name` or `$(…)`; a variable the line binds to digits is digits.

use purlis_core::floorguard::{merge_reason, release_floor_reason};
use purlis_core::mergehold;

/// Each is held by the hold, and refused by the floor's merge half and the release floor.
fn held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            merge_reason(cmd).is_some(),
            "not a merge to the floor: {cmd}"
        );
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

/// Each passes the hold and the floor's merge half.
fn passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
    }
}

#[test]
fn the_shapes_in_the_issue_are_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge $(gh api user --jq '\"-XPUT\"')",
        "gh api repos/o/r/pulls/5/merge $(cat n)",
        "gh api repos/o/r/pulls/5/merge $(git config x.y)",
        "gh api repos/o/r/pulls/5/merge $(tr a b < n)",
        "gh api repos/o/r/pulls/5/merge $F",
        "F=$(cat n); gh api repos/o/r/pulls/5/merge $F",
        "gh api repos/o/r/pulls/5/merge \"$(cat n)\"",
        "gh api repos/o/r/pulls/5/merge \"$F\"",
        "gh api repos/o/r/pulls/5/merge `cat n`",
        "gh api repos/o/r/pulls/5/merge \"`cat n`\"",
    ]);
}

#[test]
fn the_table_in_the_review_of_7414_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge $(gh api user | jq -r .bio)",
        "glab api projects/:id/merge_requests/5/merge $(glab api user | jq -r .bio)",
        "glab api projects/:id/merge_requests/$(glab api projects/:id/merge_requests | jq -r '.[0].title')/notes",
        "gh api $(gh api repos/o/r/pulls/5 | jq -r .title)",
        "gh api repos/o/r/git/refs/heads/$(gh api user | jq -r .bio)",
        "gh api repos/o/r/pulls/5/merge $(gh api user | tail -1)",
    ]);
}

#[test]
fn an_expansion_that_may_add_a_word_or_begin_one_is_held_in_any_position() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/$N",
        "gh api repos/o/r/pulls/5 -f title=$T",
        "gh api repos/o/r/pulls/5 \"$M\"",
        "gh api -X \"$M\" repos/o/r/pulls/5/merge",
        "gh api repos/o/r/pulls/5/merge \"-X$M\"",
        "gh api repos/o/r/pulls/5/merge -\"$M\"",
        "gh api repos/o/r/pulls/5/merge \"\"$F",
        "gh api repos/o/r/pulls/5/merge $@",
        "glab api -R g/p projects/$ID/jobs",
        "id=$(gh api repos/o/r/pulls/$N | jq -r .id)",
        "bash -c 'gh api repos/o/r/pulls/5/merge $F'",
    ]);
}

#[test]
fn the_refusal_says_the_way_through_and_no_way_round() {
    purlis_core::unsteered!();
    let said =
        release_floor_reason("gh api repos/o/r/pulls/5/merge $(cat n)", true).expect("a refusal");
    // The release floor's own opening names `bypassPermissions`; what is said of this call is
    // what follows it.
    let said = said
        .split("treated as one.")
        .nth(1)
        .expect("the call's own part");
    assert!(said.contains("`$(…)`"), "{said}");
    assert!(said.contains("double-quoted"), "{said}");
    assert!(said.contains("literal"), "{said}");
    assert!(said.contains("Write the endpoint out"), "{said}");
    for round in ["bypass", "disable", "eval", "unset"] {
        assert!(!said.contains(round), "{round}: {said}");
    }
}

#[test]
fn a_quoted_expansion_after_a_literal_start_that_is_no_flag_passes() {
    purlis_core::unsteered!();
    passes(&[
        "gh api \"repos/o/r/pulls/$N\"",
        "glab api \"projects/:id/pipelines/$(glab api projects/:id/pipelines | jq -r '.[0].id')/jobs\"",
        "gh api repos/o/r/pulls/5 -f title=\"$T\"",
        "gh api repos/o/r/issues/5/comments -f body=\"$B\" --jq .id",
        "gh api 'repos/o/r/pulls/$N'",
        "gh api repos/o/r/pulls/5 -H 'Accept: application/json'",
    ]);
}

#[test]
fn plain_literal_reads_and_digit_bound_variables_pass() {
    purlis_core::unsteered!();
    passes(&[
        "gh api repos/o/r/pulls/5",
        "gh api repos/{owner}/{repo}/pulls --jq '.[].number'",
        "glab api projects/:id/merge_requests/5 | jq -r .title",
        "for id in 1 2; do gh api repos/o/r/pulls/$id; done",
        "N=5; gh api repos/o/r/pulls/$N",
        "gh api repos/o/r/pulls/5 > $TMPDIR/pr.json",
    ]);
}

#[test]
fn a_variable_the_line_does_not_bind_to_digits_is_held_even_in_a_loop() {
    purlis_core::unsteered!();
    held(&[
        "for id in 1 $X; do gh api repos/o/r/pulls/$id; done",
        "for id in $(cat ids); do gh api repos/o/r/pulls/$id; done",
        "N=$(cat n); gh api repos/o/r/pulls/$N",
    ]);
}

#[test]
fn a_quoted_literal_then_an_unquoted_expansion_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api \"repos/o/r/pulls/5/merge\"$(cat n)",
        "gh api repos/o/r/pulls/5/merge \"\"$(cat n)",
        "gh api 'repos/o/r/pulls/5/merge'$(cat n)",
        "gh api \"repos/o/r/pulls/5/merge\"`cat n`",
        "gh api 'repos/o/r/pulls/5/merge'`cat n`",
        "gh api repos/o/r/pulls/5/merge $''\"$F\"",
        "gh api repos/o/r/pulls/5/merge $\"$F\"",
        "gh api repos/o/r/pulls/5/merge $=F",
        "gh api repos/o/r/pulls/5/merge $~F",
        "gh api repos/o/r/pulls/5/merge $^F",
        "gh api repos/o/r/pulls/5/merge $+F",
    ]);
}

#[test]
fn an_expansion_in_double_quotes_that_is_several_words_or_more_than_a_name_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api \"repos/o/r/pulls/${a[@]}\"",
        "gh api \"repos/o/r/pulls/$@\"",
        "gh api \"repos/o/r/pulls/$*\"",
        "gh api \"repos/o/r/pulls/${@:1}\"",
        "gh api \"repos/o/r/pulls/$1\"",
        "gh api \"repos/o/r/pulls/$a[@]\"",
        "gh api \"repos/o/r/pulls/${=F}\"",
        "gh api \"repos/o/r/pulls/${(s: :)F}\"",
        "gh api \"repos/o/r/pulls/${(f)F}\"",
        "gh api \"repos/o/r/pulls/${(z)F}\"",
        "gh api \"repos/o/r/pulls/$=F\"",
        "gh api \"repos/o/r/pulls/${F:-5}\"",
        "gh api \"$F/pulls\"",
        "gh api \"-$F\"",
        "gh api \"\\-$F\"",
    ]);
}

#[test]
fn a_bracket_in_a_quote_does_not_close_a_substitution_early() {
    purlis_core::unsteered!();
    held(&[
        "gh api \"a$(echo '(')\"$F\")\"",
        "gh api \"a${x:-{}\"$F\"}\"",
        "gh api \"a$(echo \")\")\"$F",
    ]);
}

#[test]
fn a_call_behind_a_wrapper_a_header_or_a_flag_is_found() {
    purlis_core::unsteered!();
    held(&[
        ">gh gh api repos/o/r/pulls/5/merge $F",
        "env -u gh gh api repos/o/r/pulls/5/merge $F",
        "nice -n gh gh api repos/o/r/pulls/5/merge $F",
        "timeout -s gh 5 gh api repos/o/r/pulls/5/merge $F",
        "nohup gh api repos/o/r/pulls/5/merge $F",
        "command gh api repos/o/r/pulls/5/merge $F",
        "exec gh api repos/o/r/pulls/5/merge $F",
        "time gh api repos/o/r/pulls/5/merge $F",
        "gh --hostname x api repos/o/r/pulls/5/merge $F",
        "gh > /dev/null api repos/o/r/pulls/5/merge $F",
        "f() { gh api repos/o/r/pulls/5/merge $F; }; f",
        "function f { gh api repos/o/r/pulls/5/merge $F; }; f",
        "noglob gh api repos/o/r/pulls/5/merge $F",
        "nocorrect gh api repos/o/r/pulls/5/merge $F",
        "- gh api repos/o/r/pulls/5/merge $F",
        "FOO=1 gh api repos/o/r/pulls/5/merge $F",
    ]);
}

#[test]
fn words_the_rule_never_sees_are_held() {
    purlis_core::unsteered!();
    held(&[
        "xargs gh api repos/o/r/pulls/5/merge",
        "echo merge 5 | xargs gh pr",
        "xargs gh < n",
        "xargs -I{} glab api projects/:id/merge_requests/{}/notes",
        "find . -exec gh api repos/o/r/pulls/5/merge {} \\;",
        "find . -exec gh api repos/o/r/pulls/5/merge $F \\;",
        "env -S 'gh api repos/o/r/pulls/5/merge'",
        "env -S \"gh api repos/o/r/pulls/5/merge $F\"",
    ]);
}

#[test]
fn an_inner_shells_string_the_outer_shell_fills_in_is_held() {
    purlis_core::unsteered!();
    held(&[
        "bash -c \"N=5; gh api repos/o/r/pulls/$N\"",
        "eval \"gh api \\\"repos/o/r/pulls/$F\\\"\"",
        "eval gh api repos/o/r/pulls/5/merge $F",
        "sh -c 'gh api repos/o/r/pulls/\"$@\"' x 5/merge -XPUT",
        "sh -c 'gh api \"$1\"' x repos/o/r/pulls/5/merge",
        "zsh -c \"gh api repos/o/r/pulls/5/merge $F\"",
    ]);
}

#[test]
fn a_glob_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge *",
        "gh api repos/o/r/pulls/5/merge ?XPUT",
        "gh api repos/o/r/pulls/5/merge [-]XPUT",
        "gh api repos/o/r/pulls/5/merge {-XPUT,}",
        "gh api repos/o/r/pulls?state=open",
    ]);
}

#[test]
fn literal_merges_found_on_the_way_are_held() {
    purlis_core::unsteered!();
    held(&[
        "gh --method PUT api repos/o/r/pulls/5/merge",
        "gh -X PUT api repos/o/r/pulls/5/merge",
        "glab -X PUT api projects/:id/merge_requests/5/merge",
        "noglob gh pr merge 5",
        "- gh pr merge 5",
        "nocorrect gh pr merge 5",
        "gh api repos/o/r/pulls/5/merge &>/dev/null -XPUT",
        "gh api repos/o/r/pulls/5/merge &>>/dev/null -XPUT",
        "find . -exec gh pr merge 5 \\;",
        "find . -execdir gh pr merge 5 \\;",
        "alias g='gh pr merge'\ng 5",
        "alias g='gh pr'; g merge 5",
        "alias gm='gh pr merge 5'; gm",
        "alias g=gh; g pr merge 5",
        "alias g='gh api'\ng repos/o/r/pulls/5/merge -XPUT",
    ]);
}

#[test]
fn the_allowed_forms_pass() {
    purlis_core::unsteered!();
    passes(&[
        "gh api \"repos/o/r/pulls/$N/comments\"",
        "gh api \"repos/o/r/pulls/${N}/comments\"",
        "gh api \"repos/o/r/pulls/$(cat n)/comments\"",
        // The lexer reads a backtick pair as the `$(…)` it is.
        "gh api \"repos/o/r/pulls/`cat n`/comments\"",
        "gh api \"repos/o/r/pulls/$(echo '(')/comments\"",
        "gh api repos/o/r/pulls/5 -f body=\"$B\" -f title=\"a $T b\"",
        "gh api repos/{owner}/{repo}/pulls/5 --jq '.[0].title'",
        "gh api repos/o/r/pulls/5 --jq .title",
        "gh api repos/o/r/pulls -f 'q=a b' -H 'Accept: x'",
        "gh --repo o/r api repos/o/r/pulls/5",
        "gh -R o/r api repos/o/r/pulls/5",
        ">gh gh api repos/o/r/pulls/5",
        "env -u gh gh api repos/o/r/pulls/5",
        "f() { gh api repos/o/r/pulls/5; }; f",
        "noglob gh api repos/o/r/pulls/5",
        "find . -name x -exec echo {} \\;",
        "bash -c 'gh api repos/o/r/pulls/5'",
        "bash -c \"echo $HOME\"",
        "xargs echo",
        "gh api repos/o/r/pulls/5 &>/dev/null",
        "gh api repos/o/r/pulls/5 2>&1 | jq .title",
    ]);
}

/// 3,000 references and 3,000 words on one line: each is read once, and a reference is a name
/// compared to the one the line binds, not a scan of the line (#7423). The hook's budget is
/// ten seconds, and a hook that times out may let the command through.
#[test]
fn a_line_padded_with_thousands_of_words_and_references_is_read_in_time() {
    purlis_core::unsteered!();
    let mut cmd = String::from("gh api repos/o/r/pulls/5");
    for i in 0..3000 {
        cmd.push_str(&format!(
            " -H \"X-k{i}: a$N$M\" \"repos/o/r/pulls/${{N}}/$(echo {i})\" w{i}"
        ));
    }
    let start = std::time::Instant::now();
    assert_eq!(merge_reason(&cmd), None);
    assert_eq!(release_floor_reason(&cmd, true), None);
    let took = start.elapsed();
    let bound = if cfg!(debug_assertions) { 4 } else { 1 };
    assert!(
        took < std::time::Duration::from_secs(bound),
        "a padded line took {took:?}"
    );
    // The same line with one held word at the end is held, whatever its length.
    cmd.push_str(" $F");
    assert!(merge_reason(&cmd).is_some());
}
