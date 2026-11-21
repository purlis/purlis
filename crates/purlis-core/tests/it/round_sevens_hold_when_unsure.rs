// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round seven of #7131's review (hold-only, no new parsing): six lines the final review
//! confirmed under real bash, each now held where the reader is not sure, and the `$SHELL` stdin
//! gap closed by asking the fed text as bash would read it.
//!
//! Reading the shell's text is the early warning, not the lock (Q331); the lock is the forge
//! credential (#7133).

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold;

fn assert_held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

fn assert_passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// B1: `time` takes `-p` and then `--`, and nothing else.
#[test]
fn round_sevens_time_skips_only_p_and_double_dash() {
    purlis_core::unsteered!();
    assert_held(&[
        "time -x case x in\n( {gh,pr,merge,12} )",
        "time -p -p case x in\n( {gh,pr,merge,12} )",
        "time -- case x in\n( {gh,pr,merge,12} )",
        "coproc c case x in\n( {gh,pr,merge,12} )",
    ]);
}

/// B2: `esac` ends a case only as a reserved word.
#[test]
fn round_sevens_an_esac_that_is_no_reserved_word_ends_no_case() {
    purlis_core::unsteered!();
    let arms = [
        "gh pr merge 12",
        "gh api -X PUT repos/o/r/pulls/12/merge",
        "glab mr merge 3",
        "{gh,pr,merge,12}",
        "f() { gh pr merge 12; }; f",
    ];
    for arm in arms {
        assert_held(&[
            &format!("case y in x) A=1 esac;; y) {arm};; esac"),
            &format!("case y in x) </dev/null esac;; y) {arm};; esac"),
            &format!("case y in x) x=(a b) esac;; y) {arm};; esac"),
            &format!("case y in x) <(true) esac;; y) {arm};; esac"),
            &format!("case y in x) echo hi; A=1 esac;; y) {arm};; esac"),
        ]);
    }
}

/// B3: a pattern or an arm token with a `${` that does not close is held.
#[test]
fn round_sevens_an_unclosed_expansion_in_a_pattern_or_arm_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "case x in x|${y:-(}) {gh,pr,merge,12} ${z:-)} ;; esac",
        "case x in x) echo ${y:-;;}|{gh,pr,merge,12} ${z:-)} ;; esac",
    ]);
}

/// B4: a function body's first word is read as a program the shell may build.
#[test]
fn round_sevens_a_function_body_whose_program_the_shell_builds_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "f() { {gh,pr,merge,12}; }; f",
        "f() ( {gh,pr,merge,12} ); f",
        "f() { case x in x) {gh,pr,merge,12};; esac; }; f",
        "f() { ! {gh,pr,merge,12}; }; f",
        "f() { time {gh,pr,merge,12}; }; f",
        "f() { A=1 {gh,pr,merge,12}; }; f",
        "f() { /usr/bin/en[v] gh pr merge 12; }; f",
        "f() { {gh,api,-X,PUT,repos/o/r/pulls/12/merge}; }; f",
    ]);
}

/// B5: a substitution body with a word-start `#`, a `<<` or a `$'` is held.
#[test]
fn round_sevens_a_substitution_body_with_a_comment_heredoc_or_ansi_quote_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo \"$(echo # )\ngh pr merge 12\n)\"",
        "x=\"$(echo # )\ngh pr merge 12\n)\"",
        "echo \"$(echo $'\\')' ; gh pr merge 12)\"",
        "echo \"$(cat <<EOF\n)\nEOF\ngh pr merge 12)\"",
        "echo \"$(: # )\ncase x in x) gh pr merge 12;; esac\n)\"",
    ]);
}

/// B6: an arithmetic body with a backslash or a quote is held.
#[test]
fn round_sevens_an_arithmetic_body_with_a_backslash_or_quote_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo \"$((: \\( ; gh pr merge 12 ) ; : \\))\"",
        "((: \\( ; {gh,pr,merge,12} ) ; : \\))",
    ]);
}

/// What a literal shell already did, so the built one is read the same way.
#[test]
fn round_sevens_a_literal_shell_reads_what_it_is_fed() {
    purlis_core::unsteered!();
    assert_held(&[
        "bash <(echo gh pr merge 12)",
        "bash < <(echo gh pr merge 12)",
        "echo gh pr merge 12 | bash",
    ]);
}

/// The `$SHELL` gap: a shell the line builds reads its stdin as bash would.
#[test]
fn round_sevens_a_shell_the_line_builds_reads_what_it_is_fed() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo gh pr merge 12 | $SHELL",
        "echo gh pr merge 12 | \"$SHELL\"",
        "echo gh pr merge 12 | ${SHELL:-bash}",
        "echo gh pr merge 12 | $BASH",
        "echo gh pr merge 12 | $(which bash)",
        "$SHELL <<EOF\ngh pr merge 12\nEOF",
        "\"$SHELL\" <<'EOF'\ngh pr merge 12\nEOF",
        "$SHELL -s <<EOF\ngh pr merge 12\nEOF",
        "$SHELL < <(echo gh pr merge 12)",
        "$SHELL <(echo gh pr merge 12)",
    ]);
}

/// Performance: the reader stops at a bound, held, and never overflows the stack.
#[test]
fn round_sevens_a_line_nested_past_the_bound_is_held_without_overflowing() {
    purlis_core::unsteered!();
    let line = format!(": '{}' $(true); gh pr merge 12", "$(".repeat(60_000));
    let held = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || release_floor_reason(&line, true).is_some())
        .expect("a thread")
        .join()
        .expect("no stack overflow");
    assert!(held);
}

#[test]
fn round_sevens_harmless_lines_still_pass() {
    purlis_core::unsteered!();
    assert_passes(&[
        "grep case file",
        "make case",
        "npm run test:case",
        "case \"$1\" in start) echo up;; stop) echo down;; *) echo usage;; esac",
        "time -p cargo build",
        "time cargo build",
        "f() { echo hi; }; f",
        "git log | $PAGER",
        "cat <<'EOF'\ncase x in x) gh pr merge 12;; esac\nEOF",
        "x=$(echo hi; echo there)",
        "gh pr create --title t --body \"$(cat <<'EOF'\n## Summary\n- it # works (really)\nEOF\n)\"",
        "echo \"$(cat <<-EOF\n\tbody\n\tEOF\n)\"",
        "echo $((1 + 2))",
        "f() { ls; }; f",
    ]);
}
