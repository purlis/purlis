// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! After a re-link, the window takes back the sandboxes of the chats it started on a runner, its
//! own chats only (#2511, ruled 2026-10-17). On each link the desktop's relay makes to the runner
//! for one window it names that window (`take_back`), and the runner names that link the
//! confinements of the chats the window started there over the links before, whether the window
//! made the confinement or the runner composed it for the window's start. It names it no
//! confinement the runner's own window made, and none made or composed for another window's
//! link. A sandbox the window made on a link a renewal is replacing, before its chat started, is
//! the window's on its new link too, and passes to it as the old link ends (#2624). A port the
//! runner withdrew as the window's last link ended is given back as the window takes the chat
//! back, and one the person withdrew stays withdrawn (#2625); one the runner's policy no longer
//! gives at the re-link stays withdrawn too (#2738). Here the
//! runner's host is spoken to directly, each link of the desktop's admitted as
//! `remote-link`, so the links' ends are the test's to choose; the confinements and programs are
//! stand-ins, so none of this needs a terminal.

#![cfg(unix)]

use std::time::Duration;

use purlis_host::standin::{StandInConfines, StandIns};
use purlis_host::{Compose, Hosting, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session::{
    self, Answer, Beside, Client, Command, Confine, ConfinePlan, Events, Launch, Layer, Listed,
    ProgramKind, Pushed, ReachChange, SandboxTold, StartProgram, Told,
};
use purlis_terminal::session::Spec;
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);
/// The desktop's relay's names for two windows it relays to the runner.
const WINDOW: &str = "01JA0WNDW0000000000000000";
const ANOTHER_WINDOW: &str = "01JA0WNDW0000000000000001";
/// A chat's confinement the runner composes for the window's start.
const COMPOSED: &str = "01J9CNFNMNT0000000000000001";
/// A chat's confinement the window made, and a chat started in it.
const MADE: &str = "01J9CNFNMNT0000000000000002";
/// A chat's confinement composed for another window's start, and for a link that named none.
const OTHERS: &str = "01J9CNFNMNT0000000000000003";
const UNNAMED: &str = "01J9CNFNMNT0000000000000004";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes every start as `/bin/echo`; one started by its names in a confinement of the
/// runner's own, as its binary composes it with its own sandbox.
struct Composes;

impl Compose for Composes {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        if starting.start.launch.is_none() {
            starting.beside.confine(&planned("unused").plan)?;
        }
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

fn planned(id: &str) -> Confine {
    Confine {
        confinement: id.into(),
        project: PROJECT.into(),
        plan: ConfinePlan {
            listed: vec![Listed {
                host: "api.anthropic.com".into(),
                layer: Layer::Open,
            }],
            never: Vec::new(),
            withdrawn: Vec::new(),
            anything: false,
            tunnel_ports: vec![443],
            plain_ports: vec![80],
            any_port: false,
            beside: Beside::Wrapped,
            given: vec![5432],
        },
        asks: true,
    }
}

/// The runner's host, its confinements stand-ins, and the project it holds.
struct Runner {
    hosting: Hosting,
    confines: StandInConfines,
    project: tempfile::TempDir,
}

fn a_runner() -> Runner {
    let confines = StandInConfines::default();
    Runner {
        hosting: Hosting::confining(Composes, confines.clone(), StandIns::default()),
        confines,
        project: tempfile::tempdir().unwrap(),
    }
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

/// A link to `runner` over the link between hosts, which it serves as `remote-link`.
async fn between_hosts(runner: Hosting) -> Link {
    let (a_end, b_end) = duplex(256 * 1024);
    let (desktop, ours) = (
        identity("01JA0DESKT0P00000000000000"),
        identity("01JB0RUNNER0000000000000000"),
    );
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let (peer, runner_pinned) = (pinned(&desktop), pinned(&ours));
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &ours,
            &peer,
            Liveness::default(),
        )
        .await
        .unwrap();
        session::serve(answered.link, runner, None).await
    });
    hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &desktop,
        &runner_pinned,
        Liveness::default(),
    )
    .await
    .unwrap()
}

impl Runner {
    /// A link to the runner admitted as `scope`, holding the project and subscribed: the
    /// runner's own window's for `local-ui`, and one of the desktop's over the link between
    /// hosts for `remote-link`.
    async fn linked(&self, scope: Scope) -> (Client, Events) {
        let linked = if scope == Scope::RemoteLink {
            between_hosts(self.hosting.clone()).await
        } else {
            let (a, b) = duplex(256 * 1024);
            let (client, served) = tokio::join!(
                link::connect(a, session::speaks(), scope, HELD.of(scope)),
                link::serve_any(b, session::speaks(), &HELD)
            );
            tokio::spawn(session::serve(served.unwrap(), self.hosting.clone(), None));
            client.unwrap()
        };
        let (client, _views, events) = Client::new(linked);
        client
            .hold_project(PROJECT, self.project.path().to_str().unwrap())
            .await
            .expect("held");
        client
            .subscribe(Default::default())
            .await
            .expect("subscribed");
        (client, events)
    }

    /// A link the desktop's relay made for `window`, named so before anything else.
    async fn relayed_for(&self, window: &str) -> (Client, Events, Vec<String>) {
        let (client, events) = self.linked(Scope::RemoteLink).await;
        let taken = client.take_back(window).await.expect("named its window");
        (client, events, taken)
    }

    /// Wait until the runner has dealt with the end of the link that last held `id`: its ports
    /// of the machine are withdrawn.
    async fn unheld(&self, id: &str) {
        let deadline = std::time::Instant::now() + PATIENCE;
        while !self.confines.seen().iter().any(|one| {
            one.confine.confinement == id
                && one
                    .changes
                    .contains(&ReachChange::WithdrawPort { port: 5432 })
        }) {
            assert!(
                std::time::Instant::now() < deadline,
                "the runner never heard the link end"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// A chat the runner composes by its names, its confinement named to the start's link.
fn composed(id: &str) -> StartProgram {
    StartProgram {
        confinement: Some(id.into()),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

/// A chat started in confinement `id`, which the link made.
fn running_in(id: &str) -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/echo".into(),
            args: Vec::new(),
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: None,
            confinement: Some(id.into()),
            prompt_key: None,
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

async fn changed(client: &Client, id: &str, change: ReachChange) -> Result<Answer, String> {
    client
        .call(Command::ChangeReach {
            confinement: id.into(),
            change,
        })
        .await
        .map_err(|why| why.refusal().map(|r| r.code.clone()).unwrap_or_default())
}

async fn reach_of(client: &Client, id: &str) -> Result<(), String> {
    client
        .call(Command::ReachOf {
            confinement: id.into(),
        })
        .await
        .map(drop)
        .map_err(|why| why.refusal().map(|r| r.code.clone()).unwrap_or_default())
}

/// Whether `events` is pushed what a confinement's proxy did, soon.
async fn hears_a_sandbox(events: &mut Events) -> bool {
    timeout(Duration::from_millis(300), async {
        loop {
            match events.recv().await {
                Some(Told::Pushed(Pushed::Sandbox { .. })) => return true,
                Some(_) => continue,
                None => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

fn asked() -> SandboxTold {
    SandboxTold::Asked {
        targets: vec!["pypi.example.com:443".into()],
    }
}

#[tokio::test]
async fn after_a_re_link_the_window_answers_its_runner_chats_asks_and_changes_their_reach_again() {
    let runner = a_runner();
    let (first, first_events, taken) = runner.relayed_for(WINDOW).await;
    assert_eq!(taken, Vec::<String>::new(), "nothing to take back yet");
    first
        .start_program(composed(COMPOSED))
        .await
        .expect("composed on the runner");
    first
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    first
        .start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");

    drop((first, first_events));
    runner.unheld(COMPOSED).await;
    runner.unheld(MADE).await;
    let (again, mut events, taken) = runner.relayed_for(WINDOW).await;

    assert_eq!(taken, [COMPOSED, MADE], "both of the window's chats");
    for id in [COMPOSED, MADE] {
        runner.confines.proxy_did(id, asked());
        assert!(hears_a_sandbox(&mut events).await, "{id}: its ask is heard");
        assert!(
            runner.confines.askable(id),
            "{id}: it may hold an ask again"
        );
        assert_eq!(
            changed(
                &again,
                id,
                ReachChange::Allow {
                    host: "pypi.example.com".into(),
                    layer: Layer::Chat,
                },
            )
            .await,
            Ok(Answer::ReachChanged {
                retry: vec!["pypi.example.com:443".into()]
            }),
            "{id}: the window answers its ask"
        );
        assert_eq!(reach_of(&again, id).await, Ok(()), "{id}");
    }
}

#[tokio::test]
async fn after_a_re_link_the_person_forgets_a_runner_chats_allows() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    first
        .start_program(composed(COMPOSED))
        .await
        .expect("composed on the runner");
    let allow = ReachChange::Allow {
        host: "pypi.example.com".into(),
        layer: Layer::Chat,
    };
    changed(&first, COMPOSED, allow).await.expect("allowed");

    drop((first, first_events));
    runner.unheld(COMPOSED).await;
    let (again, _events, _) = runner.relayed_for(WINDOW).await;

    let forget = ReachChange::Forget {
        host: "pypi.example.com".into(),
        layer: Layer::Chat,
    };
    changed(&again, COMPOSED, forget.clone())
        .await
        .expect("forgotten");
    let seen = runner.confines.seen();
    assert_eq!(seen[0].changes.last(), Some(&forget));
}

#[tokio::test]
async fn a_link_renewed_ahead_of_its_limit_is_named_the_windows_chats_while_the_old_one_lasts() {
    let runner = a_runner();
    let (old, _old_events, _) = runner.relayed_for(WINDOW).await;
    old.start_program(composed(COMPOSED))
        .await
        .expect("composed on the runner");

    let (renewed, _events, taken) = runner.relayed_for(WINDOW).await;

    assert_eq!(taken, [COMPOSED]);
    assert_eq!(reach_of(&renewed, COMPOSED).await, Ok(()));
    assert_eq!(
        reach_of(&old, COMPOSED).await,
        Ok(()),
        "the old link still is"
    );
}

#[tokio::test]
async fn a_chat_the_window_starts_on_the_link_a_renewal_replaces_is_named_to_the_new_one_too() {
    let runner = a_runner();
    let (old, _old_events, _) = runner.relayed_for(WINDOW).await;
    let (renewed, _events, _) = runner.relayed_for(WINDOW).await;

    old.start_program(composed(COMPOSED))
        .await
        .expect("composed on the runner");
    old.call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    old.start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");

    for id in [COMPOSED, MADE] {
        assert_eq!(reach_of(&renewed, id).await, Ok(()), "{id}");
    }
}

#[tokio::test]
async fn the_runner_never_names_its_own_windows_confinements_to_a_relayed_window() {
    let runner = a_runner();
    let (own, _own_events) = runner.linked(Scope::LocalUi).await;
    own.start_program(composed(COMPOSED))
        .await
        .expect("composed for the runner's own window");
    own.call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined by the runner's own window");
    own.start_program(running_in(MADE))
        .await
        .expect("runs in its confinement");

    let (relayed, mut events, taken) = runner.relayed_for(WINDOW).await;

    assert_eq!(taken, Vec::<String>::new());
    for id in [COMPOSED, MADE] {
        assert_eq!(
            reach_of(&relayed, id).await,
            Err(session::NO_SUCH_CHAT.into()),
            "{id}"
        );
        runner.confines.proxy_did(id, asked());
        assert!(!hears_a_sandbox(&mut events).await, "{id}: not told");
    }
}

#[tokio::test]
async fn the_runner_never_names_another_windows_confinements_to_a_window_after_its_re_link() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    let (another, _another_events, _) = runner.relayed_for(ANOTHER_WINDOW).await;
    first
        .start_program(composed(COMPOSED))
        .await
        .expect("composed for the first window");
    another
        .start_program(composed(MADE))
        .await
        .expect("composed for another window");

    drop((first, first_events));
    runner.unheld(COMPOSED).await;
    let (again, _events, taken) = runner.relayed_for(WINDOW).await;

    assert_eq!(taken, [COMPOSED]);
    assert_eq!(
        reach_of(&again, MADE).await,
        Err(session::NO_SUCH_CHAT.into()),
        "another window's chat"
    );
    assert_eq!(
        reach_of(&another, MADE).await,
        Ok(()),
        "still that window's"
    );
}

#[tokio::test]
async fn a_window_whose_link_ended_is_not_named_to_another_window_s_next_link() {
    let runner = a_runner();
    let (another, another_events, _) = runner.relayed_for(ANOTHER_WINDOW).await;
    another
        .start_program(composed(COMPOSED))
        .await
        .expect("composed for another window");

    drop((another, another_events));
    runner.unheld(COMPOSED).await;
    let (relayed, _events, taken) = runner.relayed_for(WINDOW).await;

    assert_eq!(taken, Vec::<String>::new());
    assert_eq!(
        reach_of(&relayed, COMPOSED).await,
        Err(session::NO_SUCH_CHAT.into())
    );
}

/// The programs `client` is answered, each by its id.
async fn programs_of(client: &Client) -> Vec<String> {
    client
        .programs_of(PROJECT)
        .await
        .expect("answered")
        .into_iter()
        .map(|program| program.program)
        .collect()
}

#[tokio::test]
async fn a_window_started_again_is_answered_its_own_chats_on_the_runner_alone() {
    let runner = a_runner();
    let (own_window, _own_events) = runner.linked(Scope::LocalUi).await;
    own_window
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined by the runner's window");
    let the_runners = own_window.start_program(running_in(MADE)).await.unwrap();
    let (first, _first_events, _) = runner.relayed_for(WINDOW).await;
    let mine = first.start_program(composed(COMPOSED)).await.unwrap();
    let (other, _other_events, _) = runner.relayed_for(ANOTHER_WINDOW).await;
    let theirs = other.start_program(composed(OTHERS)).await.unwrap();
    let (unnamed, _unnamed_events) = runner.linked(Scope::RemoteLink).await;
    unnamed.start_program(composed(UNNAMED)).await.unwrap();

    drop(first);
    let (back, _back_events, _) = runner.relayed_for(WINDOW).await;

    assert_eq!(programs_of(&back).await, [mine]);
    assert_eq!(programs_of(&other).await, [theirs]);
    assert!(
        programs_of(&unnamed).await.is_empty(),
        "a link that named no window is answered nothing"
    );
    assert!(
        programs_of(&own_window).await.contains(&the_runners),
        "the runner's own window is answered as before"
    );
    assert!(
        reach_of(&back, MADE).await.is_err(),
        "it is named none of the runner's own window's sandboxes"
    );
    assert!(
        reach_of(&back, OTHERS).await.is_err(),
        "it is named none of another window's sandboxes"
    );
}

#[tokio::test]
async fn a_sandbox_composed_for_a_start_that_named_none_stays_unnamed_as_its_chat_is_answered() {
    let runner = a_runner();
    let (first, _first_events, _) = runner.relayed_for(WINDOW).await;
    let chat = first
        .start_program(StartProgram::of(PROJECT, ProgramKind::Harness))
        .await
        .unwrap();

    drop(first);
    let (back, _back_events, _) = runner.relayed_for(WINDOW).await;
    let programs = back.programs_of(PROJECT).await.expect("answered");

    assert_eq!(programs.len(), 1);
    assert_eq!(programs[0].program, chat);
    assert!(programs[0].confined.is_none());
    assert!(programs[0].plan.is_none());
    assert!(reach_of(&back, "unused").await.is_err());
}

#[tokio::test]
async fn a_link_between_hosts_keeps_no_note_of_a_program() {
    let runner = a_runner();
    let (window, _events, _) = runner.relayed_for(WINDOW).await;
    let program = window.start_program(composed(COMPOSED)).await.unwrap();

    let refused = window
        .call(Command::NoteProgram {
            program,
            note: session::ProgramNote(Default::default()),
        })
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
}

#[tokio::test]
async fn a_link_names_one_window() {
    let runner = a_runner();
    let (relayed, _events, _) = runner.relayed_for(WINDOW).await;

    relayed
        .take_back(WINDOW)
        .await
        .expect("the same window again");
    let refused = relayed.take_back(ANOTHER_WINDOW).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::MALFORMED)
    );
}

#[tokio::test]
async fn a_link_that_named_no_window_has_its_chats_taken_back_by_none() {
    let runner = a_runner();
    let (unnamed, unnamed_events) = runner.linked(Scope::RemoteLink).await;
    unnamed
        .start_program(composed(COMPOSED))
        .await
        .expect("composed on the runner");

    drop((unnamed, unnamed_events));
    runner.unheld(COMPOSED).await;
    let (relayed, _events, taken) = runner.relayed_for(WINDOW).await;

    assert_eq!(taken, Vec::<String>::new());
    assert_eq!(
        reach_of(&relayed, COMPOSED).await,
        Err(session::NO_SUCH_CHAT.into())
    );
}

#[tokio::test]
async fn the_window_takes_nothing_back_by_a_relaying_hosts_word() {
    let runner = a_runner();
    let (own, _events) = runner.linked(Scope::LocalUi).await;

    let refused = own.take_back(WINDOW).await.unwrap_err();

    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
}

/// A sandbox the window made on a link a renewal is replacing, before its chat started there:
/// the window starts its chat on the new link (#2624). Another window's link cannot.
#[tokio::test]
async fn a_sandbox_made_on_a_link_a_renewal_replaces_starts_on_the_new_link() {
    let runner = a_runner();
    let (old, _old_events, _) = runner.relayed_for(WINDOW).await;
    old.call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the link being renewed");
    // The renewal: a new link for the same window, while the old one still answers.
    let (new, _new_events, _) = runner.relayed_for(WINDOW).await;
    let (another, _another_events, _) = runner.relayed_for(ANOTHER_WINDOW).await;

    assert!(
        another.start_program(running_in(MADE)).await.is_err(),
        "another window's link starts nothing in it"
    );
    new.start_program(running_in(MADE))
        .await
        .expect("the window's chat starts in it on the new link");
    drop(old);
    assert_eq!(reach_of(&new, MADE).await, Ok(()));
}

/// A sandbox the window made on a link a renewal replaced, which ended before the chat started:
/// it is carried over to the window's new link, not ended with the old one (#2624).
#[tokio::test]
async fn a_sandbox_made_on_a_link_a_renewal_replaced_is_carried_over_as_the_old_link_ends() {
    let runner = a_runner();
    let (old, old_events, _) = runner.relayed_for(WINDOW).await;
    old.call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the link being renewed");
    let (new, _new_events, _) = runner.relayed_for(WINDOW).await;

    drop((old, old_events));
    // However long the runner takes over the old link's end, the sandbox stays.
    let _ = timeout(Duration::from_millis(300), async {
        while !runner.confines.ended(MADE) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(!runner.confines.ended(MADE), "it ended with the old link");
    assert_eq!(reach_of(&new, MADE).await, Ok(()));
    new.start_program(running_in(MADE))
        .await
        .expect("the window's chat starts in it on the new link");
}

/// A sandbox a link made that names no window, or whose window has no other link, still ends
/// with that link, as one nothing runs in yet does (#1999).
#[tokio::test]
async fn a_sandbox_whose_window_has_no_other_link_ends_with_its_link() {
    let runner = a_runner();
    let (only, only_events, _) = runner.relayed_for(WINDOW).await;
    only.call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined");
    let (another, _another_events, _) = runner.relayed_for(ANOTHER_WINDOW).await;

    drop((only, only_events));
    timeout(PATIENCE, async {
        while !runner.confines.ended(MADE) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("it ends with its link");
    assert!(another.start_program(running_in(MADE)).await.is_err());
}

/// Whether the runner gives `id` back port 5432 within `within`.
async fn given_back(runner: &Runner, id: &str, within: Duration) -> bool {
    timeout(within, async {
        while !runner
            .confines
            .seen()
            .iter()
            .any(|one| one.confine.confinement == id && one.given_back == [5432])
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}

/// A chat the window started, with port 5432 of the runner given: the runner withdraws the port
/// when the window's last link ends, and gives it back once the window takes the chat back
/// (#2625), for each kind of the window's runner chats.
#[tokio::test]
async fn after_a_re_link_a_runner_chats_withdrawn_port_is_given_again_as_the_person_gave_it() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    first
        .start_program(composed(COMPOSED))
        .await
        .expect("composed on the runner");
    first
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    first
        .start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");

    drop((first, first_events));
    runner.unheld(COMPOSED).await;
    runner.unheld(MADE).await;
    // Another window's link gives nothing back.
    let (_another, _another_events, _) = runner.relayed_for(ANOTHER_WINDOW).await;
    assert!(!given_back(&runner, MADE, Duration::from_millis(200)).await);

    let (_again, _events, _) = runner.relayed_for(WINDOW).await;
    for id in [COMPOSED, MADE] {
        assert!(given_back(&runner, id, PATIENCE).await, "{id}: given back");
    }
}

/// A port the person withdrew from the chat, before its window's last link ended, stays
/// withdrawn after the window takes the chat back (#2625).
#[tokio::test]
async fn a_port_the_person_withdrew_stays_withdrawn_after_a_re_link() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    first
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    first
        .start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");
    changed(&first, MADE, ReachChange::WithdrawPort { port: 5432 })
        .await
        .expect("the person withdraws the port");

    drop((first, first_events));
    let (_again, _events, taken) = runner.relayed_for(WINDOW).await;
    assert_eq!(taken, [MADE]);
    assert!(
        !given_back(&runner, MADE, Duration::from_millis(300)).await,
        "a port the person withdrew was given back"
    );
}

/// The person withdraws a port while the runner is giving it back after a re-link: the port
/// ends withdrawn, whichever lands first (#2625).
#[tokio::test]
async fn a_port_the_person_withdraws_while_it_is_given_back_ends_withdrawn() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    first
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    first
        .start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");
    drop((first, first_events));
    runner.unheld(MADE).await;
    // Each give-back now waits a while for the person's withdrawal to land first.
    runner
        .confines
        .give_backs_wait_for_a_withdrawal(Duration::from_millis(500));

    let (again, _events, _) = runner.relayed_for(WINDOW).await;
    changed(&again, MADE, ReachChange::WithdrawPort { port: 5432 })
        .await
        .expect("the person withdraws the port");
    tokio::time::sleep(Duration::from_millis(800)).await;

    let seen = runner.confines.seen();
    let made = seen
        .iter()
        .find(|one| one.confine.confinement == MADE)
        .unwrap();
    let last_withdrawal = made
        .changes
        .iter()
        .rposition(|change| *change == ReachChange::WithdrawPort { port: 5432 })
        .unwrap();
    assert!(
        made.given_back_after
            .last()
            .is_none_or(|given_after| last_withdrawal >= *given_after),
        "given back after the person withdrew it: {:?}, {:?}",
        made.changes,
        made.given_back_after
    );
}

/// The runner's policy stopped giving a port while the window was away: the port the runner
/// withdrew as the window's last link ended is checked against the policy as it is at the
/// re-link, and not given back (#2738). It stays withdrawn for want of a window, so a re-link
/// once the policy gives it again gives it back.
#[tokio::test]
async fn a_port_the_runners_policy_no_longer_gives_is_not_given_back_after_a_re_link() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    first
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    first
        .start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");
    drop((first, first_events));
    runner.unheld(MADE).await;
    runner.confines.no_longer_gives(5432);

    let (again, again_events, taken) = runner.relayed_for(WINDOW).await;
    assert_eq!(taken, [MADE]);
    assert!(
        !given_back(&runner, MADE, Duration::from_millis(300)).await,
        "a port the runner's policy no longer gives was given back"
    );
    assert!(
        runner
            .confines
            .asked_whether_given()
            .contains(&(runner.project.path().canonicalize().unwrap(), 5432)),
        "the runner's policy was not asked: {:?}",
        runner.confines.asked_whether_given()
    );

    drop((again, again_events));
    runner.confines.gives_again(5432);
    let (_back, _events, _) = runner.relayed_for(WINDOW).await;
    assert!(given_back(&runner, MADE, PATIENCE).await, "given back");
}

/// The proxy refuses the person's withdrawal of a port it carries: the plan is brought back to
/// what the proxy carries, so the port still counts as given (#2744). The runner then withdraws
/// it as the window's last link ends, as it does every given port, and gives it back after a
/// re-link: it is never left carried with nobody to withdraw it.
#[tokio::test]
async fn a_withdrawal_the_proxy_refuses_leaves_the_plan_and_the_proxy_agreeing() {
    let runner = a_runner();
    let (first, first_events, _) = runner.relayed_for(WINDOW).await;
    first
        .call(Command::Confine(Box::new(planned(MADE))))
        .await
        .expect("confined on the runner");
    first
        .start_program(running_in(MADE))
        .await
        .expect("runs in the window's confinement");
    runner.confines.refuse_changes("the proxy did not take it");
    assert!(
        changed(&first, MADE, ReachChange::WithdrawPort { port: 5432 })
            .await
            .is_err(),
        "the refusal is the window's to hear"
    );
    runner.confines.take_changes();

    drop((first, first_events));
    runner.unheld(MADE).await;
    let (_again, _events, _) = runner.relayed_for(WINDOW).await;
    assert!(given_back(&runner, MADE, PATIENCE).await, "given back");
}
