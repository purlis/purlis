// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A link is judged when it is read (#5795, the person's ruling): a declared tool, a browser
//! fill or a granted login that would read through a link into a vault its persona may not use
//! is refused, however and whenever the link and the persona's use of its vault came to be. An
//! unreadable grants file refuses the read.

use super::*;

/// `team/MISSING` linked to `other/TOKEN`, which holds `n0t-yours`.
fn linked_out(root: &Path) {
    let ctx = Ctx::new(root, Env::of(&[]));
    let team = crate::secrets::cmd::provider(&ctx, "team").unwrap();
    crate::secrets::links::link(&ctx, &team, "MISSING", "other", "TOKEN").unwrap();
}

/// A run of a declared tool that prints what it was handed under `MISSING`.
fn reads_missing(root: &Path) -> Wanted {
    let mut want = wanted(root, "team", sh(r#"echo "got $X""#));
    want.env = vec!["X=MISSING".into()];
    want
}

/// The refusal that ends `frames`, having run nothing.
fn refused_and_nothing_ran(frames: &[Frame]) -> &str {
    assert!(
        !frames.iter().any(|frame| matches!(frame, Frame::Exit(_))),
        "nothing ran: {frames:?}"
    );
    match frames.last() {
        Some(Frame::Refused { why, code: 1 }) => why,
        other => panic!("a refusal: {other:?} in {frames:?}"),
    }
}

/// What a chat hears names the key and its own vault, and never the other vault or its value
/// (D-5246-8).
fn says_out_of_reach(why: &str, persona: &str) {
    assert!(
        why.contains(&format!(
            "MISSING in vault team reads an entry of a vault {persona} may not use"
        )),
        "says which link and why: {why}"
    );
    assert!(!why.contains("other"), "names no other vault: {why}");
    assert!(!why.contains("n0t-yours"), "never the value: {why}");
}

#[test]
fn a_declared_tool_reading_a_link_into_a_vault_its_persona_may_not_use_is_refused() {
    let project = project();
    linked_out(project.path());

    let frames = served(
        &asker(project.path(), Some("devops")),
        reads_missing(project.path()),
    );

    says_out_of_reach(refused_and_nothing_ran(&frames), "devops");
}

/// The issue's own case: the link was made while only personas that may use both vaults used
/// `team`, and `qa` was allowed `team` later.
#[test]
fn a_persona_allowed_the_links_vault_after_the_link_was_made_is_refused_its_read() {
    let project = project();
    linked_out(project.path());
    crate::sandbox::local::grant_vault(project.path(), "team", "qa").unwrap();

    let frames = served(
        &asker(project.path(), Some("qa")),
        reads_missing(project.path()),
    );

    says_out_of_reach(refused_and_nothing_ran(&frames), "qa");
}

#[test]
fn a_declared_tool_reads_a_link_into_a_vault_its_persona_may_use() {
    let project = project();
    linked_out(project.path());
    crate::sandbox::local::grant_vault(project.path(), "other", "devops").unwrap();

    let frames = served(
        &asker(project.path(), Some("devops")),
        reads_missing(project.path()),
    );

    assert_eq!(frames.last(), Some(&Frame::Exit(0)), "{frames:?}");
    assert_eq!(stdout(&frames), "got ***\n");
}

/// An unreadable grants file is read as a refusal, not as no grants: the link's entry is in a
/// vault the registry tags for the persona, and the read is refused all the same.
#[test]
fn a_link_read_while_this_machines_grants_cant_be_read_is_refused() {
    let project = project();
    let root = project.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "other": {"provider": "plain-file", "config": {"file": "other.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    linked_out(root);
    let grants = crate::sandbox::local::path(root);
    std::fs::create_dir_all(grants.parent().unwrap()).unwrap();
    std::fs::write(&grants, "{ not json").unwrap();

    let frames = served(&asker(root, Some("devops")), reads_missing(root));

    let why = refused_and_nothing_ran(&frames);
    assert!(
        why.contains("could not read which vaults this machine allows"),
        "{why}"
    );
    assert!(!why.contains("n0t-yours"), "never the value: {why}");
}

/// A link whose entry is gone, into a vault the persona may not use, is refused, and the person is
/// asked for nothing: a Fill would go where the persona may not read.
#[test]
fn a_gone_link_into_a_vault_the_persona_may_not_use_is_refused_and_nothing_is_asked() {
    let project = project();
    linked_out(project.path());
    std::fs::write(project.path().join("other.json"), "{}").unwrap();
    let asks = crate::secrets::ask::Asks::new(project.path());

    let frames = served_asking(
        &asker(project.path(), Some("devops")),
        reads_missing(project.path()),
        &asks,
        Duration::from_secs(60),
    );

    says_out_of_reach(refused_and_nothing_ran(&frames), "devops");
    assert!(asks.open().is_empty(), "asked nobody");
}

/// A chat on no persona hears it so, not a blank where a persona's name would be.
#[test]
fn a_fill_for_a_chat_on_no_persona_through_a_link_is_refused_saying_so() {
    let project = project();
    linked_out(project.path());
    let chat = Asker {
        confines: None,
        ..asker(project.path(), None)
    };

    let why = fill_value(&chat, "team", "MISSING")
        .map(|value| value.map(|value| value.expose().to_owned()))
        .unwrap_err();

    assert!(
        why.contains("MISSING in vault team reads an entry of another vault, and this chat runs as no persona"),
        "{why}"
    );
    assert!(!why.contains("  "), "no blank for a name: {why}");
}

#[test]
fn a_fill_reading_a_link_into_a_vault_its_persona_may_not_use_is_refused() {
    let project = project();
    linked_out(project.path());
    let chat = Asker {
        confines: None,
        ..asker(project.path(), Some("qa"))
    };

    let why = fill_value(&chat, "team", "MISSING")
        .map(|value| value.map(|value| value.expose().to_owned()))
        .unwrap_err();

    says_out_of_reach(&why, "qa");
}

#[test]
fn a_granted_login_reading_a_link_into_a_vault_its_persona_may_not_use_is_refused() {
    let project = project();
    linked_out(project.path());
    let chat = asker(project.path(), Some("qa"));
    let tool = shop_login_armed("team", "MISSING", None).tool;

    let why = granted_login(&chat, &tool, None)
        .map(|(login, _)| login.password.expose().to_owned())
        .unwrap_err();

    says_out_of_reach(&why, "qa");
}
