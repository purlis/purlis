// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The merge guard keeps the quoting inside a process substitution and reads `${…}` as one word
//! (#7709). A `|` in a single-quoted jq program fed by `< <( … )`, or in a parameter expansion
//! (`${t%%|*}`), is no pipe, so the line is read as its real programs. A merge inside a
//! substitution is still read and held.

use purlis_core::floorguard::{merge_reason, release_floor_reason};
use purlis_core::mergehold;
use purlis_core::shellseg::segment_argv;

const JQ: &str = "jq -r 'sort_by(.iid) | .[] | [.iid, .source_branch] | @tsv' f.json";

/// Each passes the hold and the floor's merge half, unattended.
fn passes(cmd: &str) {
    assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
    assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
}

/// Each is held, and refused unattended.
fn held(cmd: &str) {
    assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
    assert!(
        release_floor_reason(cmd, true).is_some(),
        "not floored: {cmd}"
    );
}

/// The report's first line: a `while` loop of GET reads fed by a process substitution whose jq
/// program is single-quoted and holds pipes. A loop body that runs a script shares the loop's
/// stdin, so the substitution's body is read for what it could feed it: jq's own words, its
/// quoting kept, are no program.
#[test]
fn a_jq_program_with_pipes_in_a_process_substitution_is_read_as_jq() {
    purlis_core::unsteered!();
    for body in [
        "glab api \"projects/1/merge_requests/$iid\"",
        "glab api \"projects/1/merge_requests/$iid\" | jq -r .state; bash ./note.sh \"$iid\"",
        "\"$GLAB\" api \"projects/1/merge_requests/$iid/commits\"",
    ] {
        passes(&format!(
            "while IFS=$'\\t' read -r iid br; do {body}; done < <({JQ})"
        ));
    }
    // The same line as text in a script fed to an interpreter on its stdin.
    passes(&format!(
        "python3 - <<'EOF'\nJ=\"done < <({JQ})\"\nvs=[\n\"a \\\"$iid\\\"; \"+J,\n]\nEOF"
    ));
}

/// The report's second line: `${t%%|*}` is one word, an assignment, and no pipe into `*}`.
#[test]
fn a_pipe_inside_a_parameter_expansion_is_part_of_the_word() {
    purlis_core::unsteered!();
    assert_eq!(
        segment_argv("t='a|b'; f=${t%%|*}; echo \"$f\""),
        [vec!["t=a|b"], vec!["f=${t%%|*}"], vec!["echo", "$f"]]
    );
    assert_eq!(
        segment_argv("echo ${x:-a b} | wc"),
        [vec!["echo", "${x:-a b}"], vec!["wc"]]
    );
    assert_eq!(
        segment_argv("echo ${x:-{a}}; ls"),
        [vec!["echo", "${x:-{a}}"], vec!["ls"]]
    );
    for cmd in [
        "t=a; f=${t%%|*}",
        "f=${t%%|*}; echo $f",
        "for t in \"a|b\"; do f=${t%%|*}; glab api \"projects/1/merge_requests/$f\"; done",
        "echo ${t//|/ }",
    ] {
        passes(cmd);
    }
}

/// A merge in a process substitution or a command substitution is still read and held, with a
/// quoted pipe or a `${…}` beside it, and fed to a shell in every form the floor reads.
#[test]
fn a_merge_hidden_in_a_substitution_is_still_held() {
    purlis_core::unsteered!();
    for cmd in [
        "cat < <(gh pr merge 1)",
        "while read -r l; do echo \"$l\"; done < <(gh pr merge 1)",
        "x=$(glab mr merge 2)",
        "f=${t%%|*}; x=$(glab mr merge 2)",
        "echo ${x:-$(gh pr merge 1)}",
        "f=${t%%|*} gh pr merge 1",
        "echo ${t%%|*} | gh pr merge 1",
        "echo ${ gh pr merge 1; }",
        &format!("while read -r iid br; do bash ./note.sh; done < <({JQ}; gh pr merge 1)"),
        &format!("while read -r iid br; do glab mr merge \"$iid\"; done < <({JQ})"),
        "bash < <(jq -n 1; echo gh pr merge 3)",
        "bash <(echo '{gh,pr,merge} 3')",
        "$SHELL < <(printf '%s\\n' 'gh pr merge 3')",
    ] {
        held(cmd);
    }
}

/// bash and dash close a `${…}` at its first unquoted `}`, counting only a nested `${` (#7709,
/// round 2): a bare `{` in the word opens nothing, so the commands after that `}` are segments
/// of their own. zsh counts each `{`, so it reads these as fewer commands, never more.
#[test]
fn a_parameter_expansion_closes_at_its_first_unquoted_brace() {
    purlis_core::unsteered!();
    assert_eq!(
        segment_argv("{ v=${s#{}; cat ~/.ssh/id_ed25519; }"),
        [vec!["v=${s#{}"], vec!["cat", "~/.ssh/id_ed25519"]]
    );
    assert_eq!(
        segment_argv("echo ${x:-${y:-a b}} | wc"),
        [vec!["echo", "${x:-${y:-a b}}"], vec!["wc"]]
    );
    for cmd in [
        "{ v=${s#{}; gh pr merge 1; }",
        "f() { v=${s#{}; gh pr merge 1; }; f",
        "echo ${x:-{a}; gh pr merge 1; echo }",
        "echo ${x:-{}; gh pr merge 1 #}",
        "v=${s%%{*}; gh pr merge 1; w=${s##*}}",
        "bash -c 'echo ${x:-{a}; gh pr merge 1; echo }'",
        "{ v=${s#{}; glab mr merge 2; }",
        "echo ${x:-{a}; glab mr merge 2; echo }",
        "{ v=${s#{}; gh api -X PUT repos/o/r/pulls/1/merge; }",
        "echo ${x:-{}; gh api -X PUT repos/o/r/pulls/1/merge #}",
    ] {
        held(cmd);
    }
}

/// A `${…}` in a program word that holds a blank is split into words by the shell and run, so
/// its default is a command: `${x:-gh pr merge 1}` runs the merge (#7709, round 2).
#[test]
fn a_parameter_expansion_that_holds_a_command_is_held() {
    purlis_core::unsteered!();
    for cmd in [
        "${x:-gh pr merge 1}",
        "${x:=gh pr merge 1}",
        "${x:+gh pr merge 1}",
        "${x-glab mr merge 2}",
        "env ${x:-gh pr merge 1}",
        "true; ${x:-gh api -X PUT repos/o/r/pulls/1/merge}",
    ] {
        held(cmd);
    }
    // One with no blank names one program, read as the shell fills it in.
    passes("${x:-ls} -la");
}

/// A merge among the printed words of a program that does not print its words is still read
/// exactly: past a redirection, through a shell-filled API path and in a shell those words run
/// (#7709, round 2).
#[test]
fn a_merge_a_non_printer_spells_out_is_read_exactly() {
    purlis_core::unsteered!();
    for cmd in [
        "bash <(yes 'gh 2>/dev/null pr merge 5' | head -1)",
        "bash <(yes 'gh api $U' | head -1)",
        "bash <(yes 'gh api -X PUT repos/o/r/pulls/$N/merge' | head -1)",
        "bash <(yes 'sh -c \"gh 2>/dev/null pr merge 5\"' | head -1)",
        "bash <(yes 'sh -c \"{gh,pr,merge} 5\"' | head -1)",
        "bash <(cat gh pr merge 3)",
    ] {
        held(cmd);
    }
}

/// Every consumer of a process substitution's output as a script reads its body: `source`, `.`,
/// `eval` and a shell's `-c` (#7709, round 2).
#[test]
fn every_consumer_of_a_substitution_reads_its_body() {
    purlis_core::unsteered!();
    for cmd in [
        "source <(echo gh pr merge 3)",
        ". <(echo gh pr merge 3)",
        "source <(yes 'gh pr merge 3' | head -1)",
        "eval \"$(cat <(echo gh pr merge 3))\"",
        "sh -c \"$(cat <(echo gh pr merge 3))\"",
        "sh -c 'bash <(echo gh pr merge 3)'",
        "sh -c 'source <(yes \"gh 2>/dev/null pr merge 5\" | head -1)'",
    ] {
        held(cmd);
    }
}

/// **An accepted trade** (ADR 0124, as amended 2026-11-23): the printed words of a program that
/// does not print its words are read for a merge spelled out, not for a program the shell would
/// build from them, since what jq prints was never its words. `jq -rn '"{gh,pr,merge} 3"'` does
/// print `{gh,pr,merge} 3`, and a shell fed it would brace-expand it into a merge; this pins that
/// the floor lets it through, so the trade is seen if it moves.
#[test]
fn a_program_built_from_a_non_printers_words_is_an_accepted_loosening() {
    purlis_core::unsteered!();
    passes("bash <(jq -rn '\"{gh,pr,merge} 3\"')");
}
