// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 7b of #7131 (hold-only, no new parsing): `time --` before a compound, a heredoc inside a
//! substitution read past only when its delimiter is a plain word, and a lone `-` to a shell.

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

#[test]
fn round_seven_b_time_with_a_dash_word_other_than_one_p_holds_a_compound_after_it() {
    purlis_core::unsteered!();
    assert_held(&[
        "time -- case x in x) gh pr merge 12;; esac",
        "time -p -- case x in x) gh pr merge 12;; esac",
        "time -- case x in x) {gh,pr,merge,12};; esac",
        "time -- case y in x) echo;; y) gh pr merge 12;; esac",
        "time -- case x in (x) gh pr merge 12;; esac",
        "time -- case x in x) gh api -X PUT repos/o/r/pulls/12/merge;; esac",
        "time -- case x in x) glab mr merge 3;; esac",
        "! time -- case x in x) gh pr merge 12;; esac",
        "time -- f() { gh pr merge 12; }; f",
    ]);
    assert!(
        release_floor_reason("time -p -- case x in x) git tag v1.2.3;; esac", true).is_some(),
        "the release floor lets a tag past time -p --"
    );
}

#[test]
fn round_seven_b_a_heredoc_in_a_substitution_is_read_past_only_with_a_plain_delimiter() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo \"$(cat <<EOF\nfoo \\\nEOF\n)\nEOF\ngh pr merge 12)\"",
        "x=\"$(cat <<EOF\nfoo \\\nEOF\n)\nEOF\ngh pr merge 12)\"",
        "echo \"$(cat <<EOF\nfoo \\\nEOF\n)\nEOF\n{gh,pr,merge,12})\"",
        "echo \"$(cat <<-EOF\n\tfoo \\\n\tEOF\n)\n\tEOF\ngh pr merge 12)\"",
        "echo \"$(cat <<E\\ OF\nE\n)\nE OF\ngh pr merge 12)\"",
        "echo \"$(cat <<E\\;F\nE\n)\nE;F\ngh pr merge 12)\"",
        "echo \"$(cat <<E\\)F\nE\n)\nE)F\ngh pr merge 12)\"",
        "echo \"$(cat <<$'EOF'\n$EOF\n)\nEOF\ngh pr merge 12)\"",
    ]);
}

#[test]
fn round_seven_b_a_lone_dash_makes_a_shell_read_its_script_from_stdin() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo gh pr merge 12 | bash -",
        "echo gh pr merge 12 | sh -",
        "echo gh pr merge 12 | $SHELL -",
    ]);
}

#[test]
fn round_seven_b_harmless_lines_still_pass() {
    purlis_core::unsteered!();
    assert_passes(&[
        "gh pr create --body \"$(cat <<'EOF'\n# heading\nEOF\n)\"",
        "gh pr create --body \"$(cat <<EOF\n# heading\nEOF\n)\"",
        "echo \"$(cat <<\"EOF\"\nbody\nEOF\n)\"",
        "time -p cargo build",
        "git log | $PAGER",
        "case \"$1\" in start) echo up;; stop) echo down;; esac",
    ]);
}
