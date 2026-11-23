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
        "gh api repos/o/r/pulls/5/merge {-X,PUT}",
        "gh api repos/o/r/pulls/5/merge -*",
        "gh api repos/o/r/pulls/5/merge \"x\"*",
        "gh api repos/o/r/pulls/5/merge \"\"?XPUT",
        "gh api repos/o/r/pulls/5/merge -X?UT",
        "gh api repos/o/r/pulls/5/merge a*$F",
    ]);
}

/// Every word a pattern or a brace makes begins with the literal text before it, so after a
/// plain start that is no `-` it is another path under the same prefix, never a flag (#7423).
#[test]
fn a_glob_or_brace_after_a_plain_literal_start_passes() {
    purlis_core::unsteered!();
    passes(&[
        "gh api repos/o/r/pulls?state=open",
        "gh api repos/o/r/pulls/*",
        "gh api repos/o/r/pulls/[5]",
        "gh api repos/o/r/pulls/{1,2}",
        "gh api repos/o/r/actions/runs?head_sha=abc | head -n 1",
        "gh api repos/o/r/pulls/5 -f title=a*b",
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

#[test]
fn a_literal_start_the_shell_deletes_is_no_literal_start() {
    purlis_core::unsteered!();
    held(&["gh api repos/o/r/pulls/5/merge \"\\\n$M\""]);
    // A continuation after a literal start leaves the start as it was.
    passes(&["gh api repos/o/r/pulls/5 -f title=\"\\\n$M\""]);
}

#[test]
fn a_quoted_operator_word_is_an_argument_and_not_a_redirection() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge --jq '>' $F",
        "gh api repos/o/r/pulls/5/merge --jq \">\" $F",
        "gh api repos/o/r/pulls/5/merge --jq '2>' $F",
        "gh api repos/o/r/pulls/5/merge \\> $F",
    ]);
}

#[test]
fn a_line_the_lexer_rejects_is_held_when_it_fills_in_a_word() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5/merge 'x $F",
        "gh api repos/o/r/pulls/5/merge \"x $(cat n)",
        "find . -exec gh api repos/o/r/pulls/5/merge $F \\;",
        "find . -exec gh api repos/o/r/pulls/5/merge \"$F\" \\;",
        "find . -exec gh api \"repos/o/r/pulls/$N\" \\;",
        "find . -exec gh api repos/o/r/pulls/5/merge $(cat n) \\;",
    ]);
}

#[test]
fn a_single_quote_inside_double_quotes_opens_nothing() {
    purlis_core::unsteered!();
    held(&[
        "sh -c \"gh api repos/o/r/pulls/5/merge '$F'\"",
        "bash -c \"gh api repos/o/r/pulls/5/merge '$(cat n)'\"",
        "eval \"gh api repos/o/r/pulls/5/merge '$F'\"",
    ]);
}

#[test]
fn a_redirection_with_a_descriptor_does_not_hide_the_call() {
    purlis_core::unsteered!();
    held(&[
        "gh 2>/dev/null api repos/o/r/pulls/5/merge $F",
        "2>/dev/null gh api repos/o/r/pulls/5/merge $F",
        "gh 2>&1 api repos/o/r/pulls/5/merge $F",
        "gh 10>e api repos/o/r/pulls/5/merge $F",
        "glab 2>/dev/null api projects/:id/merge_requests/5/merge $F",
        "gh 2>/dev/null pr merge 5",
        "gh pr 2>/dev/null merge 5",
        "1>/dev/null gh pr merge 5",
        "gh 2>&1 pr merge 5",
        "gh 10>e pr merge 5",
        "glab 2>/dev/null mr merge 5",
        "glab mr 2>/dev/null merge 5",
        "2>/dev/null glab mr merge 5",
    ]);
}

#[test]
fn zsh_dash_is_seen_through_after_a_precommand_and_when_repeated() {
    purlis_core::unsteered!();
    held(&[
        "noglob - gh api repos/o/r/pulls/5/merge $F",
        "nocorrect - gh api repos/o/r/pulls/5/merge $F",
        "FOO=1 - gh api repos/o/r/pulls/5/merge $F",
        "- - gh api repos/o/r/pulls/5/merge $F",
        "noglob - gh pr merge 5",
        "nocorrect - gh pr merge 5",
        "FOO=1 - gh pr merge 5",
        "- - gh pr merge 5",
        "- noglob - gh pr merge 5",
    ]);
}

#[test]
fn a_flag_before_the_noun_that_may_take_a_value_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh -b x pr merge 5",
        "gh --body x pr merge 5",
        "glab -m x mr merge 5",
        "gh --unknown-flag x pr merge 5",
    ]);
    passes(&[
        "gh -R o/r pr view 5",
        "gh --repo o/r pr view 5",
        "gh pr view 5 --json title",
    ]);
}

#[test]
fn a_zsh_qualifier_or_a_tilde_in_an_allowed_pattern_word_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api repos/o/r/pulls/5 a*(e:'gh pr merge 7':)",
        "gh api repos/o/r/pulls/5 a*(+f)",
        "gh api repos/o/r/pulls/5 a*(e:gh\\ pr\\ merge\\ 7:)",
        "gh api repos/o/r/pulls/5 !(a)",
        "gh api repos/o/r/pulls/5 ^a",
        "gh api repos/o/r/pulls/5/merge ~-",
        "gh api repos/o/r/pulls/5/merge ~+",
        "gh api repos/o/r/pulls/5/merge ~name",
    ]);
    passes(&[
        "gh api 'repos/o/r/pulls?state=open'",
        "gh api repos/o/r/pulls/5 a*",
    ]);
}

#[test]
fn an_option_value_that_looks_like_a_dash_or_a_digit_does_not_hide_the_program() {
    purlis_core::unsteered!();
    held(&[
        "env -u - gh api repos/o/r/pulls/5/merge $F",
        "exec -a - gh api repos/o/r/pulls/5/merge $F",
        "sudo -u - gh api repos/o/r/pulls/5/merge $F",
        "env -C - gh api repos/o/r/pulls/5/merge $F",
        "nice -n - gh api repos/o/r/pulls/5/merge $F",
        "nice -n 5 >/dev/null gh api repos/o/r/pulls/5/merge $F",
        "env -u 2 >/dev/null gh api repos/o/r/pulls/5/merge $F",
    ]);
}

#[test]
fn a_flag_between_the_noun_and_the_verb_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh pr -b x merge 5",
        "gh pr --body x merge 5",
        "gh -R o/r pr -b x merge 5",
        "glab mr -m x merge 5",
        "glab mr -m x accept 5",
        "gh repo -b x edit --enable-auto-merge",
    ]);
    passes(&["gh pr -R o/r view 5", "gh pr view 5 --json title"]);
}

#[test]
fn an_input_redirection_does_not_hide_the_noun_the_verb_or_api() {
    purlis_core::unsteered!();
    held(&[
        "gh </dev/null pr merge 5",
        "gh pr </dev/null merge 5",
        "gh 0</dev/null pr merge 5",
        "gh pr <<<x merge 5",
        "gh pr <>e merge 5",
        "gh </dev/null api -X PUT repos/o/r/pulls/5/merge",
        "gh <<'EOF' api graphql --input -\n{\"query\":\"mutation { mergePullRequest(input:{pullRequestId:\\\"x\\\"}) { clientMutationId } }\"}\nEOF",
        "gh {fd}>/dev/null pr merge 5",
    ]);
}

#[test]
fn an_escaped_expansion_in_an_inner_shell_string_is_held() {
    purlis_core::unsteered!();
    held(&[
        "sh -c \"gh api repos/o/r/pulls/5/merge \\$F\"",
        "bash -c \"gh api repos/o/r/pulls/5/merge \\$F\"",
        "eval \"gh api repos/o/r/pulls/5/merge \\$F\"",
        "sh -c \"gh api repos/o/r/pulls/5/merge \\$(cat n)\"",
        "sh -c \"gh api repos/o/r/pulls/5/merge \\`cat n\\`\"",
        "sh -c \"gh api \\$F\"",
    ]);
}

#[test]
fn the_refusal_for_a_glob_names_the_character_and_says_to_quote_the_word() {
    purlis_core::unsteered!();
    for (cmd, ch) in [
        ("gh api repos/o/r/pulls/5/merge ?XPUT", "`?`"),
        ("gh api repos/o/r/pulls/5/merge *", "`*`"),
        ("gh api repos/o/r/pulls/5/merge [-]XPUT", "`[`"),
    ] {
        let said = release_floor_reason(cmd, true).expect("a refusal");
        assert!(said.contains(ch), "{cmd}: {said}");
        assert!(said.contains("Quote the whole word"), "{cmd}: {said}");
        assert!(!said.contains("`$name`"), "{cmd}: {said}");
    }
    passes(&["gh api 'repos/o/r/pulls?state=open'"]);
}

/// 60,000 `$N` references on one line read in linear time (#7423): the hook's budget is ten
/// seconds, and a hook that times out may let the command through.
#[test]
fn sixty_thousand_references_to_one_digit_variable_are_read_in_time() {
    purlis_core::unsteered!();
    let mut cmd = String::from("N=5; gh api repos/o/r/pulls/5");
    for _ in 0..60_000 {
        cmd.push_str(" repos/o/r/pulls/$N/c");
    }
    let start = std::time::Instant::now();
    assert_eq!(merge_reason(&cmd), None);
    assert_eq!(release_floor_reason(&cmd, true), None);
    let took = start.elapsed();
    let bound = if cfg!(debug_assertions) { 8 } else { 3 };
    assert!(
        took < std::time::Duration::from_secs(bound),
        "60,000 references took {took:?}"
    );
}

/// Round 5, regression A: a wrapper's numeric option value (`nice -n 5`) is no descriptor, and an
/// input redirection before the program is no part of the command, whichever way the lexer's split
/// of `2>` is read. Round three held every one of these.
#[test]
fn a_numeric_option_value_and_redirections_before_the_program_are_held() {
    purlis_core::unsteered!();
    held(&[
        "nice -n 5 </dev/null 2>&1 gh pr merge 5",
        "nice -n 5 </dev/null gh 2>&1 pr merge 5",
        "nice -n 5 <<<x 2>/dev/null gh pr merge 5",
        "nice -n 5 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "nice -n 5 </dev/null 2>/dev/null glab mr merge 5",
        "nice -n 5 </dev/null 2>&1 sh -c 'gh pr merge 5'",
        "ionice -c 2 </dev/null 2>&1 gh pr merge 5",
        "ionice -c 2 </dev/null gh 2>&1 pr merge 5",
        "ionice -c 2 <<<x 2>/dev/null gh pr merge 5",
        "ionice -c 2 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "ionice -c 2 </dev/null 2>/dev/null glab mr merge 5",
        "ionice -c 2 </dev/null 2>&1 sh -c 'gh pr merge 5'",
        "env -u 2 </dev/null 2>&1 gh pr merge 5",
        "env -u 2 </dev/null gh 2>&1 pr merge 5",
        "env -u 2 <<<x 2>/dev/null gh pr merge 5",
        "env -u 2 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "env -u 2 </dev/null 2>/dev/null glab mr merge 5",
        "env -u 2 </dev/null 2>&1 sh -c 'gh pr merge 5'",
        "stdbuf -o 0 </dev/null 2>&1 gh pr merge 5",
        "stdbuf -o 0 </dev/null gh 2>&1 pr merge 5",
        "stdbuf -o 0 <<<x 2>/dev/null gh pr merge 5",
        "stdbuf -o 0 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "stdbuf -o 0 </dev/null 2>/dev/null glab mr merge 5",
        "stdbuf -o 0 </dev/null 2>&1 sh -c 'gh pr merge 5'",
        "caffeinate -t 5 </dev/null 2>&1 gh pr merge 5",
        "caffeinate -t 5 </dev/null gh 2>&1 pr merge 5",
        "caffeinate -t 5 <<<x 2>/dev/null gh pr merge 5",
        "caffeinate -t 5 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "caffeinate -t 5 </dev/null 2>/dev/null glab mr merge 5",
        "caffeinate -t 5 </dev/null 2>&1 sh -c 'gh pr merge 5'",
        "sudo -u 0 </dev/null 2>&1 gh pr merge 5",
        "sudo -u 0 </dev/null gh 2>&1 pr merge 5",
        "sudo -u 0 <<<x 2>/dev/null gh pr merge 5",
        "sudo -u 0 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "sudo -u 0 </dev/null 2>/dev/null glab mr merge 5",
        "sudo -u 0 </dev/null 2>&1 sh -c 'gh pr merge 5'",
        "chrt -f 5 </dev/null 2>&1 gh pr merge 5",
        "chrt -f 5 </dev/null gh 2>&1 pr merge 5",
        "chrt -f 5 <<<x 2>/dev/null gh pr merge 5",
        "chrt -f 5 </dev/null 2>&1 gh api -X PUT repos/o/r/pulls/5/merge",
        "chrt -f 5 </dev/null 2>/dev/null glab mr merge 5",
        "chrt -f 5 </dev/null 2>&1 sh -c 'gh pr merge 5'",
    ]);
}

/// Round 5, regression B: a quoted `<` that is a wrapper option's value is a word, not an operator
/// that takes the program for its target.
#[test]
fn a_quoted_operator_as_a_wrappers_option_value_is_a_word_and_held() {
    purlis_core::unsteered!();
    held(&[
        "env -u '<' gh >/dev/null pr merge 5",
        "env -u '<' gh 2>&1 pr merge 5",
        "env -u '<<<' gh >/dev/null pr merge 5",
        "exec -a '<' gh >/dev/null pr merge 5",
        "sudo -u '<' gh 2>/dev/null pr merge 5",
        "env -C '<' gh 2>/dev/null pr merge 5",
        "env -u '<' gh >/dev/null api -X PUT repos/o/r/pulls/5/merge",
        "env -u '<' sh >/dev/null -c 'gh pr merge 5'",
        "- env -u '<' gh pr merge 5",
    ]);
}

/// Round 5, sibling 1: the value of `-R o/r` is no noun or verb, so a redirection before the verb
/// is stripped wherever it stands.
#[test]
fn a_redirection_after_the_repo_flag_stands_in_no_word_count() {
    purlis_core::unsteered!();
    held(&[
        "gh -R o/r pr </dev/null merge 5",
        "gh pr -R o/r </dev/null merge 5",
        "gh pr --repo o/r <<<x merge 5",
        "gh pr -R o/r {fd}</dev/null merge 5",
        "glab mr -R o/r </dev/null merge 5",
        "glab -R o/r mr </dev/null merge 5",
    ]);
}

/// Round 5, sibling 2: the words are counted from where the program really is, past every option
/// value of the wrappers in front of it.
#[test]
fn a_wrapper_with_several_option_values_places_the_program_where_it_is() {
    purlis_core::unsteered!();
    held(&[
        "env -u A -u B gh </dev/null pr merge 5",
        "sudo -u root -g wheel gh </dev/null pr merge 5",
        "timeout -s KILL 5 gh </dev/null pr merge 5",
        "env -C /tmp -u A gh </dev/null pr merge 5",
        "env -u A -u B gh </dev/null api -X PUT repos/o/r/pulls/5/merge",
        "env -u A -u B gh <<'EOF' api graphql --input -\n{\"query\":\"mutation { mergePullRequest(input:{pullRequestId:\\\"x\\\"}) { clientMutationId } }\"}\nEOF",
        "sudo -u root -g wheel gh <<'EOF' api graphql --input -\n{\"query\":\"mutation { mergePullRequest(input:{pullRequestId:\\\"x\\\"}) { clientMutationId } }\"}\nEOF",
        "timeout -s KILL 5 gh <<'EOF' api graphql --input -\n{\"query\":\"mutation { mergePullRequest(input:{pullRequestId:\\\"x\\\"}) { clientMutationId } }\"}\nEOF",
        "env -C /tmp -u A gh <<'EOF' api graphql --input -\n{\"query\":\"mutation { mergePullRequest(input:{pullRequestId:\\\"x\\\"}) { clientMutationId } }\"}\nEOF",
    ]);
}

/// Round 5, sibling 3: a redirection is no alias name or expansion, and a `-` reads the stdin it
/// feeds.
#[test]
fn an_alias_set_with_a_redirection_or_stdin_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh alias set </dev/null m 'pr merge'",
        "gh alias set m <<<x 'pr merge'",
        "gh alias set m - <<<'pr merge'",
        "gh alias set m - <<'EOF'\npr merge\nEOF",
    ]);
}

/// Round 5, sibling 4: one segment can have a `-` that is zsh's precommand and another that is a
/// value, a digit that is a value and another that is a descriptor.
#[test]
fn a_dash_or_a_digit_that_is_an_option_value_and_a_redirection_together_are_held() {
    purlis_core::unsteered!();
    held(&[
        "env -u - gh 2>/dev/null pr merge 5",
        "env -u - gh >/dev/null pr merge 5",
        "env -u - gh pr 2>/dev/null merge 5",
        "env -u - gh 2>/dev/null api -X PUT repos/o/r/pulls/5/merge",
        "- env -u - gh pr merge 5",
        "nice -n 5 >/dev/null gh 2>/dev/null pr merge 5",
        "nice -n 5 >/dev/null gh 2>/dev/null api repos/o/r/pulls/5/merge $F",
        "- env -u - gh api repos/o/r/pulls/5/merge $F",
    ]);
}

/// Round 5, sibling 5: a program word that is `=gh`, an expansion, or behind a wrapper or an
/// assignment is a forge CLI that could run `api`; zsh expands `=gh` before `env` or `sudo` runs it.
#[test]
fn a_forge_program_the_reader_does_not_place_is_held_before_a_shell_filled_api_argument() {
    purlis_core::unsteered!();
    held(&[
        "=gh api repos/o/r/pulls/5/merge $F",
        "=glab api projects/:id/merge_requests/5/merge $F",
        "$G api repos/o/r/pulls/5/merge $F",
        "${GH:-gh} api repos/o/r/pulls/5/merge $F",
        "\"$(which gh)\" api repos/o/r/pulls/5/merge $F",
        "env $G api repos/o/r/pulls/5/merge $F",
        "command $G api repos/o/r/pulls/5/merge $F",
        "noglob $G api repos/o/r/pulls/5/merge $F",
        "X=1 $G api repos/o/r/pulls/5/merge $F",
        "env =gh api repos/o/r/pulls/5/merge $F",
        "env =gh pr merge 5",
        "sudo =gh pr merge 5",
        "=gh pr merge 5",
    ]);
}

/// Round 5, sibling 6: inside backticks the shell turns `\$` into `$`, in a command, an assignment
/// and an unquoted heredoc body.
#[test]
fn a_backtick_substitution_turns_an_escaped_dollar_into_an_expansion() {
    purlis_core::unsteered!();
    held(&[
        "echo `gh api repos/o/r/pulls/5/merge \\$F`",
        "x=`gh api repos/o/r/pulls/5/merge \\$F`",
        "cat <<EOF\n`gh api repos/o/r/pulls/5/merge \\$F`\nEOF",
        "cat <<EOF\nx=`gh api repos/o/r/pulls/5/merge \\$F`\nEOF",
    ]);
}

/// Round 5: the readings made to hold the shapes above hold nothing that reads.
#[test]
fn the_reads_the_round_five_readings_could_swallow_still_pass() {
    purlis_core::unsteered!();
    passes(&[
        "gh pr view 5",
        "gh pr list --state open",
        "gh api repos/o/r/pulls/5",
        "gh api \"repos/o/r/pulls?state=open\"",
        "gh api repos/o/r/pulls?state=open",
        "gh run view 123 --log",
        "gh pr checks 5",
        "glab mr view 5",
        "gh api repos/o/r/actions/runs --jq '.workflow_runs[0].id'",
        "gh pr -R o/r view 5",
        "bash -c \"gh pr view \\$N\"",
        "gh pr view 5 2>/dev/null",
        "gh pr view 5 2>&1 | head",
        "gh pr list 2>&1 | head",
        "$EDITOR file",
        "nice -n 5 </dev/null 2>&1 gh pr view 5",
        "env -u - gh 2>/dev/null pr view 5",
        "env -u '<' gh >/dev/null pr view 5",
        "env -u A -u B gh </dev/null pr view 5",
        "gh pr -R o/r </dev/null view 5",
        "gh alias set </dev/null m 'pr view'",
        "env =gh pr view 5",
        "=gh pr view 5",
        "echo `gh api repos/o/r/pulls/5`",
        "sudo =gh pr list",
    ]);
}
