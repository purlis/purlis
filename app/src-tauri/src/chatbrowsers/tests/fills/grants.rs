// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The secret fill grants end to end (#5146), through the window's browser call with the session
//! host stood in: the real [`crate::secretfills::asked`] on a held project, whose vault is a file,
//! and a person who presses what each test says on whichever notice comes up. Each says what the
//! chat is told, which notices came up, and what the browser was handed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use purlis_core::engine::Size;
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::reopen::Chat;
use purlis_core::secrets::ask::{Answer as SecretAnswer, Open};
use purlis_core::secrets::{Ctx, Env};
use purlis_session_protocol::browsing::{FieldKind, FieldSaid};

use super::*;
use crate::host::pretend::Pretend;
use crate::loginasks::Press;
use crate::planes::{Held, Planes};
use crate::secretfills::{Asked, FillWait, answer};

const ORIGIN: &str = "https://dash.example.test";
const KEY: &str = "dash-api-key";

/// A project whose vault `qa`, persona qa's, is a file holding `file`, and is held by the window
/// on a session host that runs nothing.
struct Project {
    _dir: tempfile::TempDir,
    _planes: Planes,
    held: Arc<Held>,
}

impl Project {
    fn holding(file: &serde_json::Value) -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let root = dir.path().join("plane");
        let persona = root.join("personas").join("qa");
        std::fs::create_dir_all(&persona).expect("a persona");
        std::fs::write(
            persona.join("persona.md"),
            "---\nname: qa\ndescription: checks the dashboard\n---\n# qa\n",
        )
        .expect("its definition");
        std::fs::write(
            purlis_core::names::manifest(&root),
            "[persona]\ndefault = \"qa\"\n[sandbox]\nmode = \"on\"\negress = []\n",
        )
        .expect("the project's file");
        std::fs::write(
            root.join("vaults.json"),
            r#"{"vaults":{"qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"}}}"#,
        )
        .expect("a registry");
        std::fs::write(root.join("q.json"), file.to_string()).expect("the vault's file");
        let host = Pretend::default();
        let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
            .running_sessions_on(Arc::new(move |_, _: &Path| Box::new(host.clone())));
        let id = planes.open(&root);
        let held = planes.held(&id).expect("held");
        Self {
            _dir: dir,
            _planes: planes,
            held,
        }
    }

    /// A chat that runs as persona qa.
    fn a_chat(&self) -> u32 {
        let chat = Chat {
            program: "/bin/sh".to_owned(),
            name: "1".to_owned(),
            persona: Some("qa".to_owned()),
            cwd: Some(self.held.root().to_path_buf()),
            unsandboxed: true,
            ..Default::default()
        };
        let size = Size {
            columns: 80,
            rows: 24,
        };
        self.held.chats().start(&chat, size).expect("it runs")
    }
}

/// What the person does with the next thing that comes up.
#[derive(Debug, Clone)]
enum Says {
    /// Presses this on the notice of a fill.
    Presses(Press),
    /// Adds this value on a Secret ask.
    Adds(&'static str),
    /// Chooses Not now on a Secret ask.
    Declines,
}

/// What came up for the person, in order.
#[derive(Debug, Default)]
struct CameUp {
    notices: Vec<FillWait>,
    secret_asks: Vec<Open>,
}

/// The person at the window, answering on a thread of their own while the chat's call waits.
struct Person {
    done: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<CameUp>,
}

impl Person {
    /// Says `script` as things come up. Anything that comes up beyond it is kept blocked or
    /// declined, so that the chat's call ends, and is there in [`CameUp`] for the test to refuse.
    fn saying(project: &Project, script: &[Says]) -> Self {
        let held = Arc::clone(&project.held);
        let done = Arc::new(AtomicBool::new(false));
        let done_here = Arc::clone(&done);
        let mut script: std::collections::VecDeque<Says> = script.iter().cloned().collect();
        let thread = std::thread::spawn(move || {
            let mut came_up = CameUp::default();
            while !done_here.load(Ordering::SeqCst) {
                if let Some(wait) = held.secret_fills().waiting().into_iter().next() {
                    let press = match script.pop_front() {
                        Some(Says::Presses(press)) => press,
                        None => Press::Keep,
                        Some(other) => panic!("a notice came up where {other:?} was next"),
                    };
                    answer(
                        held.root(),
                        held.secret_fills(),
                        &Asked {
                            session: wait.session,
                            id: wait.id,
                            press,
                            shown: &wait.digest(),
                            reason: None,
                        },
                        &|_, _| Ok(()),
                    )
                    .expect("the press counts");
                    came_up.notices.push(wait);
                } else if let Some(open) = held.secret_asks().open().into_iter().next() {
                    let said = match script.pop_front() {
                        Some(Says::Adds(value)) => SecretAnswer::Fill(value.to_owned().into()),
                        Some(Says::Declines) | None => SecretAnswer::Decline,
                        Some(other) => panic!("a Secret ask came up where {other:?} was next"),
                    };
                    held.secret_asks()
                        .answer(
                            &Ctx::new(held.root(), Env::from_process()),
                            open.id,
                            said,
                            Answerer::admitted(Admitted::LocalUi).expect("the window"),
                        )
                        .expect("the answer counts");
                    came_up.secret_asks.push(open);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(script.is_empty(), "{script:?} never came up");
            came_up
        });
        Self { done, thread }
    }

    /// What came up for them.
    fn came_up(self) -> CameUp {
        self.done.store(true, Ordering::SeqCst);
        self.thread.join().expect("the person's thread")
    }
}

fn field(kind: FieldKind) -> FieldSaid {
    FieldSaid {
        kind,
        ..one_line_on(ORIGIN)
    }
}

/// A browser's fill of a field of `kind` by chat `chat`, through the window's call and the real
/// [`crate::secretfills::asked`]: what the chat is told.
fn fills(
    project: &Project,
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    chat: u32,
    kind: FieldKind,
) -> Answer {
    *hosted.field.lock().unwrap() = Some(field(kind));
    let held = &project.held;
    answered_filling(
        browsers,
        hosted,
        &ask(chat, fill("e12")),
        None,
        &|asked: &FillAsked| crate::secretfills::asked(held, chat, asked, 1000),
    )
}

/// Chat `chat` is told its fill went in, and never the value.
fn told_it_filled(answer: &Answer) {
    match answer {
        Answer::Browsed { browsed, .. } => {
            assert_eq!(browsed.note.as_deref(), Some("purlis filled the key"));
        }
        other => panic!("not filled: {other:?}"),
    }
    assert!(!format!("{answer:?}").contains(VALUE), "{answer:?}");
}

/// What the browser was handed, in order: each value, and whether it may go in a multi-line field.
fn handed(hosted: &Hosted) -> Vec<(String, bool)> {
    hosted
        .fills
        .lock()
        .unwrap()
        .iter()
        .map(|(_, fill)| (fill.value.expose().to_owned(), fill.multi_line))
        .collect()
}

fn the_key_held() -> serde_json::Value {
    serde_json::json!({ KEY: VALUE })
}

#[test]
fn after_always_the_next_fill_of_that_key_on_that_site_raises_no_notice() {
    let project = Project::holding(&the_key_held());
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    let person = Person::saying(&project, &[Says::Presses(Press::Always)]);

    let first = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
    let next = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);

    let came_up = person.came_up();
    told_it_filled(&first);
    told_it_filled(&next);
    assert_eq!(came_up.notices.len(), 1, "{:?}", came_up.notices);
    let notice = &came_up.notices[0];
    assert_eq!((notice.key.as_str(), notice.origin.as_str()), (KEY, ORIGIN));
    let shown = notice.line.as_deref().expect("it showed the line");
    let file = std::fs::read_to_string(purlis_core::names::manifest(project.held.root()))
        .expect("the project's file");
    assert!(shown.contains(ORIGIN), "{shown}");
    assert!(file.contains(&format!(r#"fill = "{ORIGIN}""#)), "{file}");
    assert!(came_up.secret_asks.is_empty());
    assert_eq!(
        handed(&hosted),
        [(VALUE.to_owned(), false), (VALUE.to_owned(), false)]
    );
}

#[test]
fn allow_for_this_chat_raises_no_notice_in_that_chat_and_one_in_another() {
    let project = Project::holding(&the_key_held());
    let (chat, another) = (project.a_chat(), project.a_chat());
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    opens_a_browser(&browsers, &hosted, another);
    let person = Person::saying(
        &project,
        &[Says::Presses(Press::Chat), Says::Presses(Press::Keep)],
    );

    let first = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
    let next = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
    let elsewhere = fills(&project, &browsers, &hosted, another, FieldKind::OneLine);

    let came_up = person.came_up();
    told_it_filled(&first);
    told_it_filled(&next);
    let told = refusal(elsewhere);
    assert!(told.contains(&format!("kept {KEY} blocked")), "{told}");
    assert_eq!(
        came_up
            .notices
            .iter()
            .map(|notice| notice.session)
            .collect::<Vec<_>>(),
        [chat, another]
    );
    assert_eq!(handed(&hosted).len(), 2, "the other chat's went nowhere");
}

#[test]
fn a_multi_line_field_raises_the_notice_under_either_grant() {
    for grant in [Press::Always, Press::Chat] {
        for kind in [FieldKind::MultiLine, FieldKind::Editable] {
            let project = Project::holding(&the_key_held());
            let chat = project.a_chat();
            let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
            opens_a_browser(&browsers, &hosted, chat);
            let person = Person::saying(
                &project,
                &[Says::Presses(grant), Says::Presses(Press::Once)],
            );

            let granted = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
            let asked_again = fills(&project, &browsers, &hosted, chat, kind);

            let came_up = person.came_up();
            told_it_filled(&granted);
            told_it_filled(&asked_again);
            assert_eq!(came_up.notices.len(), 2, "{grant:?}, {kind:?}");
            let notice = &came_up.notices[1];
            assert!(notice.multi_line, "{grant:?}, {kind:?}");
            assert!(notice.once_only(), "only Allow once and Keep blocked");
            assert_eq!(notice.line, None, "Always isn't offered on it");
            assert_eq!(
                handed(&hosted),
                [(VALUE.to_owned(), false), (VALUE.to_owned(), true)],
                "{grant:?}, {kind:?}"
            );
        }
    }
}

/// The person allows a fill of a key the vault doesn't hold: the Secret ask that follows, asked
/// of them for this fill.
fn asked_for_the_key(came_up: &CameUp) {
    assert_eq!(came_up.notices.len(), 1, "{:?}", came_up.notices);
    assert_eq!(came_up.secret_asks.len(), 1, "{:?}", came_up.secret_asks);
    let asked = &came_up.secret_asks[0];
    assert_eq!((asked.vault.as_str(), asked.key.as_str()), ("qa", KEY));
    assert_eq!(
        asked.tool.as_deref(),
        Some(format!("a secret fill on {ORIGIN}").as_str())
    );
}

#[test]
fn a_key_the_vault_lacks_raises_a_secret_ask_and_goes_in_once_the_person_adds_it() {
    let project = Project::holding(&serde_json::json!({}));
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    let person = Person::saying(&project, &[Says::Presses(Press::Once), Says::Adds(VALUE)]);

    let answer = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);

    let came_up = person.came_up();
    told_it_filled(&answer);
    asked_for_the_key(&came_up);
    assert_eq!(handed(&hosted), [(VALUE.to_owned(), false)]);
}

#[test]
fn a_key_the_person_declines_to_add_fills_nothing_and_says_declined() {
    let project = Project::holding(&serde_json::json!({}));
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    let person = Person::saying(&project, &[Says::Presses(Press::Once), Says::Declines]);

    let told = refusal(fills(
        &project,
        &browsers,
        &hosted,
        chat,
        FieldKind::OneLine,
    ));

    let came_up = person.came_up();
    asked_for_the_key(&came_up);
    assert!(told.contains("declined"), "{told}");
    assert!(told.contains("nothing was filled"), "{told}");
    assert!(handed(&hosted).is_empty());
}
