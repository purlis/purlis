// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A held CI-file push reaches the person** (#7685): a dispatched task's route holds a push that
//! changes CI files, and its ask is in the project's Asks registry, which the ✋, the window's
//! Asks and the notifier all read. The person's press performs the push, and nothing before it
//! does. Where the window cannot show the ask, the chat is told that nobody was asked.
//!
//! The forge is a stand-in that records what it was asked to do; the project, the steward chat
//! and its task are the app's own.

use std::sync::{Arc, Mutex};

use purlis_core::forge::Kind;
use purlis_core::forge::proxy::{ChatId, HeldSink};
use purlis_core::heldact::{
    Forked, HeldContext, HeldForge, OwnedWorld, PushError, Recorded, Requested, TagName, Trees,
    Unavailable, WorkflowCredential,
};
use purlis_core::mergehold::{Base, HeadSha, Repo, Request};
use purlis_core::secrets::keyring::Secret;

use super::*;

fn sha(c: char) -> HeadSha {
    HeadSha::try_from(c.to_string().repeat(40)).unwrap()
}

const WORKFLOW: &str = ".github/workflows/ci.yml";

/// The person's forge: `main` is at `1…`, the task's branch is not there yet, and every push
/// it is asked to make is recorded.
#[derive(Default)]
struct Forge {
    pushed: Mutex<Vec<String>>,
}

impl HeldForge for Forge {
    fn kind(&self) -> Kind {
        Kind::GitHub
    }
    fn host(&self) -> String {
        "github.com".to_owned()
    }
    fn rev_commit(&self, _: &Repo, _: &str) -> Result<HeadSha, String> {
        Ok(sha('2'))
    }
    fn default_branch(&self, _: &Repo) -> Result<(Base, HeadSha), String> {
        Ok((Base::try_from("main").unwrap(), sha('1')))
    }
    fn branch_tip(&self, _: &Repo, _: &Base) -> Result<Option<HeadSha>, String> {
        Ok(None)
    }
    fn on_branch(&self, _: &Repo, _: &Base, _: &HeadSha) -> Result<bool, String> {
        Ok(false)
    }
    fn tag_at(&self, _: &Repo, _: &TagName) -> Result<Option<HeadSha>, String> {
        Ok(None)
    }
    fn ci_config_path(&self, _: &Repo) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn source_id(&self, _: &Repo) -> Result<String, String> {
        Ok("id-1".to_owned())
    }
    fn account(&self) -> Result<String, String> {
        Ok("aaron".to_owned())
    }
    fn may_fork(&self, _: &Repo) -> Result<bool, String> {
        Ok(false)
    }
    fn create_tag(&self, _: &Repo, _: &TagName, _: &HeadSha) -> Result<(), String> {
        Err("no tags here".to_owned())
    }
    fn create_release(
        &self,
        _: &Repo,
        _: &TagName,
        _: &str,
        _: &str,
        _: bool,
        _: bool,
    ) -> Result<(), String> {
        Err("no releases here".to_owned())
    }
    fn make_fork(&self, _: &Repo, _: &str) -> Result<Forked, String> {
        Err("no forks here".to_owned())
    }
    fn push_tip(
        &self,
        repo: &Repo,
        branch: &Base,
        tip: &HeadSha,
        _: &HeadSha,
        _: bool,
        second: Option<&Secret>,
    ) -> Result<(), PushError> {
        self.pushed.lock().unwrap().push(format!(
            "{} {} {} second:{}",
            repo.as_str(),
            branch.as_str(),
            tip.as_str(),
            second.is_some()
        ));
        Ok(())
    }
}

/// The push's objects, as the app's quarantine reads them: its tip changes the workflow.
struct Quarantined;

impl Trees for Quarantined {
    fn changed(&self, _: &HeadSha, _: &HeadSha) -> Result<Vec<String>, String> {
        Ok(vec![WORKFLOW.to_owned()])
    }
    fn patch(&self, _: &HeadSha, _: &HeadSha, _: &[String]) -> Result<String, String> {
        Ok(format!(
            "--- a/{WORKFLOW}\n+++ b/{WORKFLOW}\n@@ -1 +1 @@\n-name: ci\n+name: ci2\n"
        ))
    }
    fn message(&self, _: &HeadSha) -> Result<String, String> {
        Ok("Rename the CI workflow".to_owned())
    }
    /// The tip is on its base, as a branch the quarantine fetched up to date is (#7690).
    fn missing(&self, _: &HeadSha, _: &HeadSha) -> Result<usize, String> {
        Ok(0)
    }
}

/// The person's second sign-in, with the workflow permission.
struct Second;

impl WorkflowCredential for Second {
    fn available(&self, _: &str) -> Result<(), Unavailable> {
        Ok(())
    }
    fn secret(&self, _: &str) -> Result<Secret, Unavailable> {
        Ok(Secret::new("workflow-token".to_owned()))
    }
}

struct Recorded1;

impl Recorded for Recorded1 {
    fn resolve(&self, _: Kind, id: &str) -> Option<Repo> {
        Repo::try_from(id).ok()
    }
    fn holds(&self, _: &Repo) -> bool {
        true
    }
}

/// What the forge route knows: the run `task-run` is chat `session`, and the forge as the person.
struct Context {
    session: u32,
    forge: Arc<Forge>,
}

impl HeldContext for Context {
    fn session(&self, chat: &ChatId) -> Option<u32> {
        (chat.as_str() == "task-run").then_some(self.session)
    }
    fn attended(&self) -> bool {
        true
    }
    fn now(&self) -> u64 {
        50
    }
    fn world(&self, _: u32, _: &Repo) -> Result<OwnedWorld, String> {
        Ok(OwnedWorld {
            forge: self.forge.clone(),
            trees: Arc::new(Quarantined),
            workflow: Arc::new(Second),
            recorded: Arc::new(Recorded1),
        })
    }
    fn merge(&self, _: &ChatId, _: &Request) -> String {
        String::new()
    }
}

/// The CI-file push the task's route held.
fn the_push() -> Requested {
    Requested::CiPush {
        repo: Repo::try_from("acme/app").unwrap(),
        branch: Base::try_from("7685-nextest").unwrap(),
        tip: sha('3'),
    }
}

/// A steward chat, the task it dispatched, and the forge route's sink for project `id`'s chats,
/// which knows the task's run as session `run`.
struct Scene {
    _plane: Plane,
    _planes: Planes,
    id: PlaneId,
    held: Arc<Held>,
    task: u32,
    forge: Arc<Forge>,
    sink: Box<dyn HeldSink>,
}

/// A steward chat and the task it dispatched; the route knows the task's run as the session
/// `run` gives (the task's own, unless a test says otherwise).
fn a_task_whose_route_holds_acts(run: impl FnOnce(&Held, u32) -> u32) -> Scene {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let task = a_task_of(&held, &id, steward, "push the nextest config");
    let forge = Arc::new(Forge::default());
    held.held_acts().set_context(Arc::new(Context {
        session: run(&held, task),
        forge: forge.clone(),
    }));
    let sink = crate::forgewiring::sink_of(&held).expect("held acts are on");
    Scene {
        _plane: plane,
        _planes: planes,
        id,
        held,
        task,
        forge,
        sink,
    }
}

/// The task's own run.
fn the_task(_: &Held, task: u32) -> u32 {
    task
}

#[test]
fn a_held_ci_file_push_from_a_dispatched_task_is_in_asks_and_the_persons_press_performs_it() {
    let Scene {
        held,
        task,
        forge,
        sink,
        ..
    } = a_task_whose_route_holds_acts(the_task);

    let said = sink.ask(&ChatId::new("task-run"), &the_push());

    assert!(
        said.contains("It has asked the person") && said.contains("Ask 1: CI-file push"),
        "{said}"
    );
    // The ask is in the registry the ✋, Asks and the notifier read, as the task's own.
    let asks = crate::asking::counted(&held).asks;
    let listed = asks
        .iter()
        .find(|one| one.session == task && one.ask.starts_with("held-act:"))
        .unwrap_or_else(|| panic!("the held push is not in Asks: {asks:?}"));
    assert!(listed.says.contains("acme/app"), "{}", listed.says);
    // Nothing was pushed before the person pressed.
    assert!(forge.pushed.lock().unwrap().is_empty());

    // The person presses twice, as Asks draws it.
    let waiting = held.held_acts().waiting();
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    let first = held
        .held_acts()
        .press(waiting[0].ask, &waiting[0].digest, None)
        .expect("the first press asks once more");
    let crate::heldacts::HeldPress::AsksOnceMore { shown, nonce } = first else {
        panic!("the first press performs nothing: {first:?}");
    };
    assert!(forge.pushed.lock().unwrap().is_empty());
    let done = held
        .held_acts()
        .press(shown.ask, &shown.digest, Some(&nonce))
        .expect("the second press pushes");

    assert!(
        matches!(done, crate::heldacts::HeldPress::Done { .. }),
        "{done:?}"
    );
    assert_eq!(
        *forge.pushed.lock().unwrap(),
        vec![format!(
            "acme/app 7685-nextest {} second:true",
            sha('3').as_str()
        )]
    );
    assert!(
        !crate::asking::counted(&held)
            .asks
            .iter()
            .any(|one| one.ask.starts_with("held-act:")),
        "a push made is asked no more"
    );
}

#[test]
fn a_held_push_the_window_cannot_show_tells_the_chat_nobody_was_asked() {
    // The route knows the run as a session the window has no chat open for.
    let Scene {
        held, forge, sink, ..
    } = a_task_whose_route_holds_acts(|_, _| 999);

    let said = sink.ask(&ChatId::new("task-run"), &the_push());

    assert!(!said.contains("It has asked the person"), "{said}");
    assert!(!said.contains("Ask 1"), "{said}");
    assert!(said.contains("Nobody was asked"), "{said}");
    assert!(
        held.held_acts().waiting().is_empty(),
        "an ask nobody can see does not wait"
    );
    assert!(forge.pushed.lock().unwrap().is_empty());
}

#[test]
fn a_held_push_of_a_graded_chat_the_window_draws_nowhere_is_shown_nowhere_and_nothing_waits() {
    // A grade's chat cannot start in this plane (a grade runs only in the full sandbox), so
    // this asks one level down: a chat whose record is a grade's, open but drawn nowhere.
    let Scene { held, .. } = a_task_whose_route_holds_acts(the_task);
    let start = purlis_core::start::Start {
        profile: Some("work".to_owned()),
        name: "1".to_owned(),
        cwd: Some(held.root().join("workspaces").join("alpha")),
        ..Default::default()
    };
    let ready = purlis_core::start::ready(&start, held.root()).expect("the chat starts");
    let graded = held
        .chats()
        .start_ready(
            &Chat {
                program: ready.program.clone(),
                cwd: ready.cwd.clone(),
                name: "1".to_owned(),
                resume: ready.session.clone(),
                profile: Some("work".to_owned()),
                graded: true,
                ..Default::default()
            },
            &ready,
            STARTING,
        )
        .expect("it runs");

    let shown = crate::heldacts::shown_in(&held, graded);

    let why = shown.expect_err("a grade's chat has no Asks to show it in");
    assert!(why.contains("not drawn in the window"), "{why}");
    assert!(
        held.held_acts().waiting().is_empty(),
        "an ask nobody can see does not wait"
    );
    assert!(crate::asking::counted(&held).asks.is_empty());
}

#[test]
fn a_held_push_raised_tells_the_window_its_asks_moved_and_the_notifier_tells_the_person_of_it() {
    // #7685's cause: the ask was kept, and nothing told the window or the notifier that it was,
    // so neither read the asks again. Here the window's listener is the notifier's own read.
    use crate::asknotify::{Act, Heard, Rules};
    let Scene {
        id,
        held,
        task,
        sink,
        ..
    } = a_task_whose_route_holds_acts(the_task);
    let rules = Arc::new(Mutex::new(Rules::default()));
    let sent: Arc<Mutex<Vec<Act>>> = Arc::default();
    held.chats().tell_asks_moved_to({
        let held = Arc::downgrade(&held);
        let (rules, sent) = (rules.clone(), sent.clone());
        Arc::new(move || {
            let Some(held) = held.upgrade() else { return };
            let asks = crate::asking::counted(&held).asks;
            let acts = rules.lock().unwrap().heard(&Heard {
                plane: &id,
                asks: &asks,
                looking: &|_| false,
                quiet: &|| false,
                now: std::time::Instant::now(),
            });
            sent.lock().unwrap().extend(acts);
        })
    });

    let said = sink.ask(&ChatId::new("task-run"), &the_push());

    assert!(said.contains("It has asked the person"), "{said}");
    let sent = sent.lock().unwrap();
    assert!(
        sent.iter().any(|act| matches!(
            act,
            Act::Show(notice) if notice.chats.iter().any(|(_, session)| *session == task)
        )),
        "the person is told the task waits: {sent:?}"
    );
}
