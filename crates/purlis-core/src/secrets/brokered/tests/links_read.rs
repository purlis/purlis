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

/// A link to another entry of the same vault reaches no new vault, so a fill the person allowed
/// reads through it, as the press lets it be made.
#[test]
fn a_fill_reads_a_link_to_another_entry_of_its_own_vault() {
    let project = project();
    let ctx = Ctx::new(project.path(), Env::of(&[]));
    let team = crate::secrets::cmd::provider(&ctx, "team").unwrap();
    crate::secrets::links::link(&ctx, &team, "ALIAS", "team", "TOKEN").unwrap();
    let chat = Asker {
        confines: None,
        ..asker(project.path(), Some("qa"))
    };

    let read = fill_value(&chat, "team", "ALIAS").expect("read");

    assert_eq!(
        read.map(|value| value.expose().to_owned()).as_deref(),
        Some(TOKEN)
    );
}

/// A key out of reach is neither read nor offered on a run's notice, whatever else it lacks: the
/// run is refused before anyone is asked for the others.
#[test]
fn a_key_out_of_reach_is_never_asked_for_beside_one_a_run_lacks() {
    let project = project();
    linked_out(project.path());
    std::fs::write(project.path().join("other.json"), "{}").unwrap();
    let chat = asker(project.path(), Some("devops"));
    let mut want = wanted(project.path(), "team", sh("true"));
    want.env = vec!["X=MISSING".into(), "Y=ABSENT".into()];

    let fills = fills_missing(&chat, &reading_for(&chat), &want);

    assert_eq!(
        fills
            .iter()
            .map(|fill| fill.key.as_str())
            .collect::<Vec<_>>(),
        ["ABSENT"]
    );
}

/// A run with a key out of reach is refused before its one notice asks for anything (#5795, #5242):
/// not a value for the key it lacks, nor the vault's Allow, which would grant the persona the vault
/// for good for a run that is then refused.
#[test]
fn a_run_with_a_key_out_of_reach_is_refused_before_its_notice_asks_for_another() {
    use crate::sandbox::tools::{Handed, Ready, Tool, Value};
    let project = project();
    let root = project.path();
    linked_out(root);
    let mut chat = asker(root, Some("qa"));
    let mut tool = Tool::one("printenv", "team", "X", Handed::Env, "MISSING");
    tool.values.push(Value {
        var: "Y".into(),
        handed: Handed::Env,
        key: "ABSENT".into(),
    });
    chat.confines.as_mut().unwrap().tools.push(Ready {
        tool,
        program: PathBuf::from("/usr/bin/printenv"),
    });
    let mut want = wanted(root, "team", vec!["/usr/bin/printenv".into(), "X".into()]);
    want.env = vec!["X=MISSING".into(), "Y=ABSENT".into()];
    let asks = crate::secrets::ask::Asks::new(root);

    let frames = served_asking(&chat, want, &asks, Duration::from_secs(60));

    says_out_of_reach(refused_and_nothing_ran(&frames), "qa");
    assert!(asks.open().is_empty(), "no Secret ask");
    assert!(asks.open_runs().is_empty(), "no run's notice");
    assert!(
        crate::sandbox::local::granted_vaults(root).is_empty(),
        "nothing granted"
    );
}

/// The up-front judge sees every key a run hands over, a `--dotenv` entry's too (#5795).
#[test]
fn every_key_a_run_hands_over_is_one_the_up_front_judge_sees() {
    let project = project();
    let mut want = wanted(project.path(), "team", sh("true"));
    want.env = vec!["X=ONE".into()];
    want.file = vec!["F=TWO".into()];
    want.dotenv = vec!["D=NAME:THREE".into()];

    assert_eq!(
        keys_handed(&want).collect::<Vec<_>>(),
        ["ONE", "TWO", "THREE"]
    );
}

/// A fill's notice is judged before it is raised (#5831): a fill that reads through a link into a
/// vault its persona may not use is refused at the judge, so the person is never asked to allow,
/// or to grant the vault for, a fill that can only be refused.
#[test]
fn a_fill_with_a_key_out_of_reach_is_refused_before_its_notice() {
    let project = project();
    linked_out(project.path());
    let ctx = Ctx::new(project.path(), Env::of(&[]));

    let why = crate::secretfill::judged(&ctx, Some("qa"), "team", "MISSING").unwrap_err();

    says_out_of_reach(&why, "qa");
}

/// The judge before the notice refuses only what the read would: a link into a vault the persona
/// may use is offered, warned of as before where the fill's own vault is not its persona's.
#[test]
fn a_fill_through_a_link_into_a_vault_its_persona_may_use_is_still_offered() {
    let project = project();
    linked_out(project.path());
    crate::sandbox::local::grant_vault(project.path(), "other", "qa").unwrap();
    let ctx = Ctx::new(project.path(), Env::of(&[]));

    let warned = crate::secretfill::judged(&ctx, Some("qa"), "team", "MISSING").expect("offered");

    assert!(
        warned.is_some_and(|warning| warning.contains("Vault team")),
        "warned that team is not qa's"
    );
}

/// Vaults `team` and `other` both tagged for `devops`, `team/MISSING` linked to `other/TOKEN`,
/// and this machine's grants file holding `text`.
fn both_devops_with_grants(root: &Path, text: &str) {
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
    std::fs::write(&grants, text).unwrap();
}

/// Only the vault grants of this machine's file are read to judge a link (#5857): a field beside
/// them that is off its shape refuses no link.
#[test]
fn a_link_reads_while_a_field_beside_the_vault_grants_is_off_its_shape() {
    let project = project();
    let root = project.path();
    both_devops_with_grants(root, r#"{"hosts_seen": 5, "vaults_mine": []}"#);
    let chat = Asker {
        confines: None,
        ..asker(root, Some("devops"))
    };

    let read = fill_value(&chat, "team", "MISSING").expect("read");

    assert_eq!(
        read.map(|value| value.expose().to_owned()).as_deref(),
        Some("n0t-yours")
    );
}

/// The vault grants themselves off their shape still refuse the read (#5857): not known is not
/// none.
#[test]
fn a_link_read_while_the_vault_grants_are_off_their_shape_is_refused() {
    let project = project();
    let root = project.path();
    both_devops_with_grants(root, r#"{"vaults_mine": "every vault"}"#);
    let chat = Asker {
        confines: None,
        ..asker(root, Some("devops"))
    };

    let why = fill_value(&chat, "team", "MISSING")
        .map(|value| value.map(|value| value.expose().to_owned()))
        .unwrap_err();

    assert!(
        why.contains("could not read which vaults this machine allows"),
        "{why}"
    );
}

/// A link into a vault the person granted the persona on this machine alone reads while a field
/// beside the grants is off its shape (#5857): the judge and `authorise` read the same grants.
#[test]
fn a_link_into_a_vault_granted_here_reads_while_a_field_beside_the_grants_is_off_its_shape() {
    let project = project();
    let root = project.path();
    linked_out(root);
    let grants = crate::sandbox::local::path(root);
    std::fs::create_dir_all(grants.parent().unwrap()).unwrap();
    std::fs::write(
        &grants,
        r#"{"hosts_seen": 5, "vaults_mine": [{"vault": "other", "persona": "devops"}]}"#,
    )
    .unwrap();
    let chat = Asker {
        confines: None,
        ..asker(root, Some("devops"))
    };

    let read = fill_value(&chat, "team", "MISSING").expect("read");

    assert_eq!(
        read.map(|value| value.expose().to_owned()).as_deref(),
        Some("n0t-yours")
    );
}
