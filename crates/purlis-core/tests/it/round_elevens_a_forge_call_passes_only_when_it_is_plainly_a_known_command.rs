// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 11 of the merge guard (#7423): ten rounds held what looked dangerous, and each confirm
//! found what the rule did not recognise (an empty word, `-R"$UNSET"`, a program that appends
//! arguments). The rule is turned round: **a `gh` or `glab` call passes only if every word up to
//! its verb is a non-empty literal, every flag before the verb takes no value or is the
//! repository's with a plain value, and its noun and verb are a command the allowlist names**.
//! Behind a program that never executes its operands `gh` is a name; behind any other, it is a
//! call that may be given more words. The default is hold: a missing entry is a false hold.

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
fn an_empty_literal_word_before_the_verb_is_skipped_by_the_cli_and_held() {
    purlis_core::unsteered!();
    held(&[
        "gh pr \"\" merge --squash",
        "gh '' pr merge --auto --squash",
        "glab mr \"\" merge --yes",
        "gh \"\" pr $V 5",
        "echo merge --squash | gxargs gh pr \"\"",
        "chronic /opt/homebrew/bin/gh \"\" pr $V 5",
    ]);
}

#[test]
fn a_repository_flag_whose_glued_value_the_shell_may_empty_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh pr -R\"$NOPE\" o/r merge 5",
        "gh pr \"-R$NOPE\" o/r merge 5",
        "gh pr -R\"$(true)\" o/r merge 5",
        "glab mr -R\"$NOPE\" o/r merge 5",
        "glab mr \"-R$NOPE\" o/r merge 5",
        "gh pr --repo=\"$NOPE\" o/r merge 5",
        "gh pr -R o/r --repo= merge 5",
    ]);
}

#[test]
fn a_program_that_names_a_package_but_executes_it_is_not_a_names_only_program() {
    purlis_core::unsteered!();
    held(&[
        "brew bundle exec gh pr merge 5",
        "brew bundle exec -- gh pr merge 5",
        "mise exec -- gh pr merge 5",
        "npm exec gh pr merge 5",
    ]);
}

#[test]
fn a_program_that_appends_arguments_without_stdin_is_held() {
    purlis_core::unsteered!();
    held(&[
        "touch merge; fd -g merge --strip-cwd-prefix -x gh pr --squash=true",
        "gxargs -a words.txt gh",
        "xargs -a words.txt gh pr",
        "fd -x gh",
        "parallel -a words.txt gh pr",
    ]);
}

#[test]
fn a_path_to_gh_followed_by_filled_words_behind_a_program_is_held() {
    purlis_core::unsteered!();
    held(&[
        "mise exec -- /opt/homebrew/bin/gh $A $B 5",
        "chronic ./gh $V view 12",
        "chronic ~/bin/gh -b x $A $B",
        "chronic /opt/homebrew/bin/gh -b x $A $B 5",
        "frob /usr/bin/gh /tmp/x",
    ]);
}

#[test]
fn zshs_extended_glob_characters_in_a_noun_or_verb_are_held() {
    purlis_core::unsteered!();
    held(&[
        "gh pr merge# 5",
        "gh pr merge~x 5",
        "gh pr merge^x 5",
        "glab mr merge# 5",
        "gh pr =merge 5",
    ]);
}

#[test]
fn a_name_among_the_operands_of_a_program_that_runs_none_is_no_call() {
    purlis_core::unsteered!();
    passes(&[
        "git grep gh src/*.rs",
        "git grep gh ~/src",
        "git grep -w gh -- \"$DIR\"",
        "git log --grep gh -- crates/*",
        "ack gh src/*",
        "find / -name gh -type f $X",
        "asdf install gh $(asdf latest gh)",
        "mise install gh $V",
        "snap install gh $OPTS",
        "brew install gh $PKGS",
        "cp /opt/homebrew/bin/gh ~/merge-tools/",
        "which gh",
        "cp /opt/homebrew/bin/gh $DEST",
    ]);
}

#[test]
fn a_known_read_passes_with_its_flags_after_the_verb() {
    purlis_core::unsteered!();
    passes(&[
        "gh pr view 5 --json title",
        "gh pr list --state open",
        "gh -R o/r pr view 5",
        "gh pr -R o/r view 5",
        "gh pr -Ro/r view 5",
        "gh pr --repo=o/r diff 5",
        "gh pr -R \"$REPO\" view 5",
        "glab mr view 5",
        "glab mr list",
        "glab -R o/r mr view 5",
        "gh issue view 3",
        "ssh h gh pr view 5",
        "chronic gh pr view 5",
    ]);
}

/// The default is hold, as a property of the whole matrix: for a noun and a verb that no table
/// names (a gh alias or an extension may stand for a merge), the call is held on every path.
#[test]
fn a_noun_or_verb_no_table_names_is_held_on_every_path() {
    purlis_core::unsteered!();
    for cli in ["gh", "glab"] {
        for call in [
            format!("{cli} frobnicate 5"),
            format!("{cli} pr frobnicate 5"),
            format!("{cli} pr 5"),
            format!("{cli} mr frobnicate 5"),
            format!("{cli} m 5"),
        ] {
            held(&[
                &call,
                &format!("chronic {call}"),
                &format!("env X=1 {call}"),
                &format!("echo x | xargs {call}"),
                &format!("for i in 1; do {call}; done"),
            ]);
        }
    }
}

/// Words that stand where the noun or the verb stands, each of which makes the call unreadable:
/// `{call}` is the call with that word in the place of the verb. Every one is crossed with every
/// path, for `gh` and for `glab`.
#[test]
fn every_unreadable_word_is_held_on_every_path() {
    purlis_core::unsteered!();
    let words = [
        "\"\"",
        "''",
        "\"$V\"",
        "$V",
        "''$V''",
        "merge#",
        "merge~x",
        "\"-R$NOPE\"",
        "-R\"$NOPE\"",
        "-b",
    ];
    for (cli, noun) in [("gh", "pr"), ("glab", "mr")] {
        for word in words {
            // The word before the verb, after it, or as the flag before the noun.
            let calls = [
                format!("{cli} {noun} {word} merge 5"),
                format!("{cli} {word} {noun} merge 5"),
                format!("{cli} {noun} {word} 5"),
            ];
            for call in calls {
                for cmd in [
                    call.clone(),
                    // behind a known wrapper
                    format!("env X=1 {call}"),
                    format!("sudo {call}"),
                    // behind a program no table knows
                    format!("chronic {call}"),
                    format!("chronic /opt/homebrew/bin/{call}"),
                    // fed
                    format!("echo 5 | xargs {call}"),
                    format!("{call} <<< 5"),
                    // the `for` over a forge answer
                    format!("for id in $({cli} {noun} list); do {call}; done"),
                    // a digit word apart from a redirection (`seg.also`)
                    format!("{cli} 5 >f {} {word} merge 5", noun),
                ] {
                    held(&[&cmd]);
                }
            }
        }
    }
}

#[test]
fn a_names_only_program_with_a_subcommand_that_runs_things_is_not_names_only() {
    purlis_core::unsteered!();
    held(&[
        "brew bundle exec gh pr merge 5",
        "snap run gh pr merge 5",
        "asdf exec gh pr merge 5",
        "pip run gh pr merge 5",
        "git rebase --exec 'true' main; git submodule foreach gh pr merge 5",
    ]);
}
