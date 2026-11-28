// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A `${` in a heredoc's data opens nothing, and a bash 5.3 `${ …; }` nested in a `${…}` runs
//! its commands, so neither hides a command from the guards that read a line by its segments
//! (#7709, round 3).

use std::path::Path;

use purlis_core::{commitguard, credguard, dispatchguard, forge, handoffguard, leakguard};

/// A quoted heredoc whose data holds an unclosed `${`, and then `cmd` and an `echo }` that would
/// close it on the line read as one word.
fn after_a_heredoc(cmd: &str) -> String {
    format!("cat <<'EOF'\n${{x\nEOF\n{cmd}; echo }}")
}

fn leaks(cmd: &str) -> bool {
    leakguard::leak_reason(cmd, "", Path::new("/nonexistent-state")).is_some()
}

#[test]
fn a_brace_in_heredoc_data_hides_no_command_from_a_guard() {
    purlis_core::unsteered!();
    let dispatch = after_a_heredoc("purlis dispatch note hi");
    assert!(dispatchguard::runs_dispatch(&dispatch), "{dispatch:?}");
    let handoff = after_a_heredoc("purlis handoff beta");
    assert!(handoffguard::is_handoff(&handoff), "{handoff:?}");
    let root = tempfile::tempdir().expect("a temporary directory");
    let forges = forge::known_ordered(root.path());
    let cred = after_a_heredoc("GIT_SSH_COMMAND=/tmp/k git push");
    assert!(
        credguard::single_credential_hit(&cred, &forges).is_some(),
        "{cred:?}"
    );
    let commit = after_a_heredoc("git commit --no-verify -m x");
    assert!(
        commitguard::hook_skip_hit(&commit, "").is_some(),
        "{commit:?}"
    );
    let leak = after_a_heredoc("cat .charter/vaults/f.json");
    assert!(leaks(&leak), "{leak:?}");
}

#[test]
fn a_command_in_a_funsub_is_read_by_every_guard() {
    purlis_core::unsteered!();
    for wrap in ["${ CMD; }", "echo ${x:-${ CMD; }}", "echo ${x:-${| CMD; }}"] {
        let leak = wrap.replace("CMD", "cat .charter/vaults/f.json");
        assert!(leaks(&leak), "{leak:?}");
        let commit = wrap.replace("CMD", "git commit --no-verify -m x");
        assert!(
            commitguard::hook_skip_hit(&commit, "").is_some(),
            "{commit:?}"
        );
        let dispatch = wrap.replace("CMD", "purlis dispatch note hi");
        assert!(dispatchguard::runs_dispatch(&dispatch), "{dispatch:?}");
    }
}
