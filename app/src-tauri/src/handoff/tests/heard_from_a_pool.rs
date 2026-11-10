// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's open or dispatch heard from a pool waits for the person** (`crate::runnerdispatches`):
//! whatever the persona, held until their Approve in the window, and a Refuse is told to the
//! chat in words. A chat of this machine's own dispatches as it always did.

use super::*;

const POOL: &str = "builds";

/// A dispatch from `asking` to `to`, heard over pool [`POOL`]'s link.
fn dispatch_from_the_pool(
    held: &Held,
    id: &PlaneId,
    asking: u32,
    to: Option<&str>,
    name: &str,
) -> (Answer, Option<Arrived>) {
    let tickets = Tickets::default();
    crate::hooks::heard_from(Some(POOL.to_owned()), || {
        dispatch(held, id, &tickets, asking, to, name)
    })
}

#[test]
fn a_dispatch_heard_from_a_pool_to_the_chats_own_persona_is_held_and_starts_nothing() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);
    let before = held.chats().open_now().len();

    let (said, told) = dispatch_from_the_pool(&held, &id, asking, Some("devops"), "check prod");

    assert!(
        matches!(
            &said,
            Answer::NeedsGrant { to, waiting: None, held_by: Some(pool), .. }
                if to == "devops" && pool == POOL
        ),
        "held for the person, on the pool's account, not {said:?}"
    );
    assert!(told.is_none(), "nothing arrived");
    assert_eq!(held.chats().open_now().len(), before, "nothing started");
    let waiting = held.runner_dispatches().every();
    assert_eq!(waiting.len(), 1);
    assert_eq!(
        (waiting[0].session, waiting[0].pool.as_deref()),
        (asking, Some(POOL))
    );
    assert!(
        held.dispatch_grants().waiting(asking).is_empty(),
        "held for the person's Approve, not as a grant"
    );
}

#[test]
fn a_handoff_heard_from_a_pool_is_held_too() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);
    let before = held.chats().open_now().len();

    let (said, told) = crate::hooks::heard_from(Some(POOL.to_owned()), || {
        a_handoff(&held, &id, asking, None, None, INTO_ALPHA)
    });

    assert!(
        matches!(&said, Answer::NeedsGrant { waiting: None, held_by: Some(pool), .. } if pool == POOL),
        "{said:?}"
    );
    assert!(told.is_none());
    assert_eq!(held.chats().open_now().len(), before, "nothing opened");
    assert_eq!(held.runner_dispatches().every().len(), 1);
}

#[test]
fn approve_starts_the_held_dispatch_as_it_was_asked() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);
    let (_, _) = dispatch_from_the_pool(&held, &id, asking, Some("devops"), "check prod");
    let waiting = held.runner_dispatches().every()[0].id;

    let arrived = Mutex::new(None);
    crate::runnerdispatches::approved(&held, &id, waiting, &|it| {
        *arrived.lock().unwrap() = Some(it);
    })
    .expect("approved");

    let it = arrived
        .into_inner()
        .unwrap()
        .expect("the window is told it started");
    assert_eq!(it.persona.as_deref(), Some("devops"));
    assert_eq!(it.label.as_deref(), Some("check prod"));
    let from = held.chats().handed_from(it.session).expect("its lineage");
    assert_eq!(from.chat, asking, "a task of the chat that asked");
    assert!(
        held.runner_dispatches().every().is_empty(),
        "it waits no more"
    );
    let told = eventually(|| {
        let told = told_on_its_next_turn(&held, asking);
        (!told.is_empty()).then_some(told)
    })
    .expect("the asking chat is told");
    assert_eq!(
        told[0].answered,
        Some(purlis_core::handback::Answered::Started)
    );
    // Answered once: a second Approve starts nothing.
    assert!(crate::runnerdispatches::approved(&held, &id, waiting, &nothing_opens).is_err());
}

#[test]
fn refuse_starts_nothing_and_tells_the_runner_chat_in_words() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);
    let before = held.chats().open_now().len();
    let (_, _) = dispatch_from_the_pool(&held, &id, asking, Some("devops"), "check prod");
    let waiting = held.runner_dispatches().every()[0].id;

    crate::runnerdispatches::refused(&held, waiting).expect("refused");

    assert_eq!(held.chats().open_now().len(), before, "nothing started");
    assert!(held.runner_dispatches().every().is_empty());
    let told = eventually(|| {
        let told = told_on_its_next_turn(&held, asking);
        (!told.is_empty()).then_some(told)
    })
    .expect("the asking chat is told");
    assert_eq!(
        told[0].answered,
        Some(purlis_core::handback::Answered::KeptBlocked)
    );
    assert_eq!(told[0].from, "check prod");
    assert!(
        told[0].summary.contains("refused") && told[0].summary.contains(POOL),
        "{}",
        told[0].summary
    );
}

#[test]
fn a_dispatch_of_a_chat_on_this_machine_is_not_held() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);

    let (said, told) = dispatch(
        &held,
        &id,
        &Tickets::default(),
        asking,
        Some("devops"),
        "check prod",
    );

    assert!(matches!(said, Answer::Dispatched { .. }), "{said:?}");
    assert!(told.is_some(), "it started");
    assert!(held.runner_dispatches().every().is_empty());
}

#[test]
fn a_dispatch_heard_from_a_pool_to_another_persona_waits_for_approve_and_raises_no_grant() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);
    let before = held.chats().open_now().len();

    let (said, told) = dispatch_from_the_pool(&held, &id, asking, Some("qa"), "check prod");

    assert!(
        matches!(&said, Answer::NeedsGrant { to, waiting: None, .. } if to == "qa"),
        "{said:?}"
    );
    assert!(told.is_none());
    assert_eq!(held.chats().open_now().len(), before, "nothing started");
    assert_eq!(held.runner_dispatches().every().len(), 1);
    assert!(
        held.dispatch_grants().waiting(asking).is_empty(),
        "the person is asked to Approve first, not for a grant"
    );
}

#[test]
fn approve_opens_a_held_handoff() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);
    let (_, _) = crate::hooks::heard_from(Some(POOL.to_owned()), || {
        a_handoff(&held, &id, asking, None, None, INTO_ALPHA)
    });
    let waiting = held.runner_dispatches().every()[0].id;

    let arrived = Mutex::new(None);
    crate::runnerdispatches::approved(&held, &id, waiting, &|it| {
        *arrived.lock().unwrap() = Some(it);
    })
    .expect("approved");

    let it = arrived
        .into_inner()
        .unwrap()
        .expect("the window is told it opened");
    assert_eq!(it.persona.as_deref(), Some("devops"));
    assert_eq!(it.workspace.as_deref(), Some("alpha"));
    assert!(held.runner_dispatches().every().is_empty());
}

#[test]
fn a_dispatch_heard_from_a_pool_with_no_task_name_is_refused_and_not_held() {
    let plane = a_plane_with_personas();
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_as(&held, &plane.root, Some("devops"), &plane.root);

    let (said, _) = dispatch_from_the_pool(&held, &id, asking, Some("devops"), "  ");

    assert!(matches!(said, Answer::No { .. }), "{said:?}");
    assert!(held.runner_dispatches().every().is_empty());
}

/// What the person is asked when chat `chat` names `curl` ahead of its run for `TOKEN` of vault
/// `team`, heard over `pool`'s link or this machine's (`None`): the plain Secret asks listed, the
/// Run grant notices listed, and what the chat heard once the person said no to whichever came.
fn curl_named_ahead(
    pool: Option<&str>,
) -> (
    Vec<purlis_core::secrets::ask::Open>,
    Vec<purlis_core::secrets::ask::run::OpenRun>,
    Answer,
) {
    use purlis_core::harness::asks::{Admitted, Answerer};
    use purlis_core::secrets::ask::run::RunAnswer;
    let plane = Plane::new().a_persona("ops", "");
    let host = Pretend::default();
    let planes = planes_on(&host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    std::fs::write(
        plane.root.join("vaults.json"),
        r#"{"vaults": {"team": {"provider": "plain-file", "config": {"file": "t.json"}, "persona": "ops"}}}"#,
    )
    .expect("a registry");
    let chat = a_chat_as(&held, &plane.root, Some("ops"), &plane.root);
    let line = purlis_core::secrets::ask::SecretAsk {
        chat,
        key: "TOKEN".to_owned(),
        purpose: "the API the chat calls".to_owned(),
        vault: Some("team".to_owned()),
        replace: None,
        program: Some("curl".to_owned()),
    };
    std::thread::scope(|scope| {
        let pool = pool.map(str::to_owned);
        let asking = scope
            .spawn(|| crate::hooks::heard_from(pool, || crate::secretasks::asked(&held, &line)));
        let (plain, runs) = loop {
            let (plain, runs) = (held.secret_asks().open(), held.secret_asks().open_runs());
            if !plain.is_empty() || !runs.is_empty() {
                break (plain, runs);
            }
            assert!(
                !asking.is_finished(),
                "the ask ended without asking the person"
            );
            std::thread::yield_now();
        };
        let window = Answerer::admitted(Admitted::LocalUi).expect("the window");
        for open in &runs {
            held.secret_asks()
                .answer_run(open.id, RunAnswer::Refuse, window)
                .expect("Don't allow");
        }
        let ctx =
            purlis_core::secrets::Ctx::new(&plane.root, purlis_core::secrets::Env::from_process());
        for open in &plain {
            held.secret_asks()
                .answer(
                    &ctx,
                    open.id,
                    purlis_core::secrets::ask::Answer::Decline,
                    window,
                )
                .expect("Not now");
        }
        (plain, runs, asking.join().expect("it ends"))
    })
}

/// **D-5244-4**: a chat on a pool's runner runs its commands there, where no Run grant reaches, so
/// its `secret_ask` naming `curl` ahead is the plain Fill: no Allow is offered, and no grant can
/// come of the person's answer.
#[cfg(unix)]
#[test]
fn a_secret_ask_naming_curl_heard_from_a_pool_is_the_plain_fill_with_no_allow() {
    let (plain, runs, said) = curl_named_ahead(Some(POOL));

    assert_eq!(runs.len(), 0, "no Run grant notice: {runs:?}");
    assert_eq!(
        plain
            .iter()
            .map(|open| open.key.as_str())
            .collect::<Vec<_>>(),
        ["TOKEN"],
        "the plain Fill for the key"
    );
    assert!(
        matches!(
            &said,
            Answer::Secret(purlis_core::secrets::ask::Said::Declined { key, .. }) if key == "TOKEN"
        ),
        "{said:?}"
    );
}

/// The same ask from this machine's chat offers curl's Allow for this chat in its one notice
/// (#5244), so the test above is about where the line came from and nothing else.
#[cfg(unix)]
#[test]
fn the_same_secret_ask_from_this_machine_is_one_notice_offering_the_allow() {
    if super::no_sandboxed_claude_code_stand_in_here() {
        return;
    }
    let (plain, runs, _) = curl_named_ahead(None);

    assert_eq!(plain.len(), 0, "no plain Fill beside it: {plain:?}");
    assert_eq!(runs.len(), 1, "one notice");
    assert!(runs[0].ahead, "asked ahead of its run");
    assert_eq!(
        runs[0].offer,
        purlis_core::secrets::ask::run::Offer::Program,
        "with the program's Allow"
    );
}
