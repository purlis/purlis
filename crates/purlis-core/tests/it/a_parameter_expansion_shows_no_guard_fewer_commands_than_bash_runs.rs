// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Reading `${…}` as one word never shows a guard fewer commands than bash runs, compared with
//! the reading before it (#7709, round 4). A `${` is never read past its line, so one a heredoc's
//! body leaves open hides nothing after the delimiter, whatever reads the body. A `${…}` the word
//! closed before a `${` read again keeps what it said. And a line whose `${…}` holds a blank is
//! read the old way too, wherever the word stands, since the shell splits what it gives into
//! words and a program handed them may run them.

use std::path::Path;

use purlis_core::floorguard::release_floor_reason;
use purlis_core::{
    commitguard, credguard, dispatchguard, forge, handoffguard, leakguard, mergehold,
};

/// Each is held, and refused unattended.
fn held(cmd: &str) {
    assert!(mergehold::held(cmd).is_some(), "not held: {cmd:?}");
    assert!(
        release_floor_reason(cmd, true).is_some(),
        "not floored: {cmd:?}"
    );
}

fn leaks(cmd: &str) -> bool {
    leakguard::leak_reason(cmd, "", Path::new("/nonexistent-state")).is_some()
}

/// Heredocs whose body something runs, and two whose body nothing does, each with a `${` its
/// body leaves open.
const HEREDOCS: [&str; 11] = [
    "bash <<'EOF'",
    "sh <<EOF",
    "python3 - <<'EOF'",
    "cat <<'EOF' | bash",
    "bash -s <<'EOF'",
    "ruby <<'EOF'",
    "cat <<'EOF' | python3",
    "cat <<EOF",
    "cat <<'EOF'",
    ": <<'EOF'",
    "bash <<EOF",
];

/// `cmd` after each of [`HEREDOCS`], followed by a `}` that would close the body's `${` if it
/// were read past its line.
fn after_each_heredoc(cmd: &str) -> Vec<String> {
    HEREDOCS
        .iter()
        .flat_map(|heredoc| {
            [
                format!("{heredoc}\n${{x\nEOF\n{cmd}; echo }}"),
                format!("{heredoc}\n${{x\nEOF\n{cmd}; {{ :; }}"),
                format!("{heredoc}\n: ${{x:-a\nEOF\n{cmd}\necho }}"),
            ]
        })
        .collect()
}

#[test]
fn a_parameter_expansion_a_heredoc_body_leaves_open_hides_nothing_after_it() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().expect("a temporary directory");
    let forges = forge::known_ordered(root.path());
    for cmd in after_each_heredoc("purlis dispatch note hi") {
        assert!(dispatchguard::runs_dispatch(&cmd), "{cmd:?}");
    }
    for cmd in after_each_heredoc("purlis handoff beta") {
        assert!(handoffguard::is_handoff(&cmd), "{cmd:?}");
    }
    for cmd in after_each_heredoc("GIT_SSH_COMMAND=/tmp/k git push") {
        assert!(
            credguard::single_credential_hit(&cmd, &forges).is_some(),
            "{cmd:?}"
        );
    }
    for cmd in after_each_heredoc("git commit --no-verify -m x") {
        assert!(commitguard::hook_skip_hit(&cmd, "").is_some(), "{cmd:?}");
    }
    for cmd in after_each_heredoc("cat .charter/vaults/f.json") {
        assert!(leaks(&cmd), "{cmd:?}");
    }
    for cmd in after_each_heredoc("gh pr merge 1") {
        held(&cmd);
    }
}

#[test]
fn a_parameter_expansion_read_again_keeps_the_blank_of_one_closed_before_it() {
    purlis_core::unsteered!();
    for second in [
        "${y:+$(pwd)}",
        "${y:+`pwd`}",
        "${y:+$(which gh)}",
        "${y:+(}",
        "${y+(}",
        "${y:-(}",
        "${y:+a(b)}",
        "${y:+<(true)}",
        "${y:+${ true; }}",
        "${y:-${ true; }}",
        "${y:+${|true; }}",
        "${y",
    ] {
        let word = format!("${{x:-gh pr merge 1}}{second}");
        for cmd in [
            word.clone(),
            format!("a{word}"),
            format!("{word} -x"),
            format!("env {word}"),
            format!("true; {word}"),
            format!("<(true){word}"),
        ] {
            held(&cmd);
        }
    }
}

#[test]
fn a_parameter_expansion_that_holds_a_blank_is_read_the_old_way_too_wherever_it_stands() {
    purlis_core::unsteered!();
    let word = "${x:-gh pr merge 1}";
    for before in [
        "2>&1",
        "2>/dev/null",
        "2>f",
        "2> f",
        "1>&2",
        "2>&-",
        "3<&0",
        "2>>f",
        ">/dev/null 2>&1",
        "2>&1 >/dev/null",
        "2>&1\t",
        "A=1 2>&1",
        "2>&1 A=1",
        "2>&1 env",
        "env 2>&1",
        "eval --",
        "trap --",
        "timeout 5",
        "chronic",
        "strace",
        "ltrace",
        "ssh h",
        "find . -exec",
        "find . -execdir",
        "find . -ok",
    ] {
        held(&format!("{before} {word}"));
    }
    for cmd in [
        "find . -exec ${x:-gh pr merge 1} \\;",
        "find . -execdir ${x:-gh pr merge 1} \\;",
        "find . -ok ${x:-gh pr merge 1} \\;",
        "find . -exec ${x:-gh pr merge 1 ;}",
        "eval true ${x:-; gh pr merge 1}",
        "env ${x:-A=1 gh pr merge 1}",
        "env A=1 ${x:-B=2 gh pr merge 1}",
        "timeout ${x:-5 gh pr merge 1}",
        "2>&1 ${x:-glab mr merge 2}",
        "2>&1 ${x:-gh api -X PUT repos/o/r/pulls/1/merge}",
    ] {
        held(cmd);
    }
}
