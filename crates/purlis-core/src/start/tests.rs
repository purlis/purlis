// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_path_the_profile_declared_is_the_path_the_chat_gets_whole() {
    // The escape hatch charter-app#136 was asked to keep: the operator's own line wins,
    // and charter appends nothing to it.
    let declared = vec![
        ("FOO".to_owned(), "bar".to_owned()),
        ("PATH".to_owned(), "/only/this".to_owned()),
    ];
    assert_eq!(
        with_chat_path(declared.clone(), Some(Path::new("/app/charter"))),
        declared
    );
}

#[cfg(unix)]
#[test]
fn a_chat_with_no_declared_path_is_given_one_starting_in_charters_own_directory() {
    let env = with_chat_path(
        vec![("CHARTER_ROOT".to_owned(), "/plane".to_owned())],
        Some(Path::new("/app/bundle/charter")),
    );
    let path = env
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.as_str())
        .expect("a PATH was added");
    assert!(path.starts_with("/app/bundle:"), "{path}");
    assert!(path.contains("/usr/local/bin"), "{path}");
    let mut sorted = env.clone();
    sorted.sort();
    assert_eq!(env, sorted, "the same chat is the same launch");
}

#[cfg(unix)]
#[test]
fn a_persona_s_tools_go_first_in_front_of_charters_own_directory_and_a_declared_path() {
    let tools = (
        crate::sandbox::tools::FIRST_ON_PATH.to_owned(),
        "/p/.purlis/app/tools/devops/bin".to_owned(),
    );
    let path_of = |env: &[(String, String)]| {
        assert!(
            !env.iter()
                .any(|(name, _)| name == crate::sandbox::tools::FIRST_ON_PATH),
            "the hand-over never reaches the chat"
        );
        env.iter()
            .find(|(name, _)| name == "PATH")
            .map(|(_, value)| value.clone())
            .expect("a PATH")
    };
    let given = with_chat_path(vec![tools.clone()], Some(Path::new("/app/bundle/charter")));
    assert!(
        path_of(&given).starts_with("/p/.purlis/app/tools/devops/bin:/app/bundle:"),
        "{given:?}"
    );
    let declared = with_chat_path(
        vec![tools, ("PATH".to_owned(), "/only/this".to_owned())],
        Some(Path::new("/app/charter")),
    );
    assert_eq!(
        path_of(&declared),
        "/p/.purlis/app/tools/devops/bin:/only/this"
    );
}

#[test]
fn a_sandboxed_codex_chat_starts_only_on_a_sandbox_only_profile_and_says_the_way_out() {
    // #2037, ruled 2026-10-15.
    let refused =
        profile_refusal(Some(Harness::Codex), "codex", false, true, false).expect("refused");
    assert!(
        refused.contains("Make or choose a sandbox-only Codex profile"),
        "{refused}"
    );
    assert!(
        refused.contains(crate::profilestore::sandbox_only::ASK),
        "{refused}"
    );
    assert_eq!(
        profile_refusal(Some(Harness::Codex), "codex-sandbox", true, true, false),
        None
    );
    // Without the sandbox, a Codex chat runs on the person's own profiles as before.
    assert_eq!(
        profile_refusal(Some(Harness::Codex), "codex", false, false, false),
        None
    );
    // Another harness is not held to it.
    assert_eq!(
        profile_refusal(Some(Harness::ClaudeCode), "claude", false, true, false),
        None
    );
}

#[test]
fn a_sandbox_only_profile_never_starts_a_chat_without_the_sandbox() {
    let refused = profile_refusal(Some(Harness::Codex), "codex-sandbox", true, false, false)
        .expect("refused");
    assert!(refused.contains("for sandboxed chats only"), "{refused}");
    assert!(refused.contains("Start it sandboxed"), "{refused}");
}

#[test]
fn on_a_runner_a_claude_code_chat_starts_only_wrapped_and_says_so() {
    // #4083, ruled 2026-10-30: a Claude Code profile on a runner is for wrapped chats only, as
    // a sandboxed Codex chat's is (D-88q), so no unsandboxed chat loads what a wrapped one wrote.
    let refused =
        profile_refusal(Some(Harness::ClaudeCode), "claude", false, false, true).expect("refused");
    assert!(refused.contains("on a runner"), "{refused}");
    assert!(
        refused.contains("only inside purlis's sandbox"),
        "{refused}"
    );
    assert!(refused.contains("nothing was started"), "{refused}");
    // Sandboxed, it starts; a profile for sandboxed chats only changes nothing there.
    assert_eq!(
        profile_refusal(Some(Harness::ClaudeCode), "claude", false, true, true),
        None
    );
    // On a person's own machine it starts without the sandbox as before.
    assert_eq!(
        profile_refusal(Some(Harness::ClaudeCode), "claude", false, false, false),
        None
    );
    // Codex and opencode on a runner keep their own rules.
    assert_eq!(
        profile_refusal(Some(Harness::Codex), "codex", false, false, true),
        None
    );
    assert_eq!(
        profile_refusal(Some(Harness::Opencode), "opencode", false, false, true),
        None
    );
}
