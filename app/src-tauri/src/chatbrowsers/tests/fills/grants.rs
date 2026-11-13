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
        self.a_chat_holding(None)
    }

    /// A chat started as persona qa that a handoff narrowed to `held`'s grants, where it says.
    fn a_chat_holding(&self, held: Option<purlis_core::reopen::HeldGrants>) -> u32 {
        self.started(Some("qa"), held)
    }

    /// A chat that runs as no persona.
    fn a_chat_of_no_persona(&self) -> u32 {
        self.started(None, None)
    }

    /// A chat started as `persona`, narrowed to `held`'s grants where it says.
    fn started(&self, persona: Option<&str>, held: Option<purlis_core::reopen::HeldGrants>) -> u32 {
        let chat = Chat {
            program: "/bin/sh".to_owned(),
            name: "1".to_owned(),
            persona: persona.map(str::to_owned),
            held,
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
    fills_into(project, browsers, hosted, chat, field(kind))
}

/// [`fills`], of a field the browser reads as `said`.
fn fills_into(
    project: &Project,
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    chat: u32,
    said: FieldSaid,
) -> Answer {
    fills_into_while(project, browsers, hosted, chat, said, &|| false)
}

/// [`fills_into`], its asker `gone` where that says.
fn fills_into_while(
    project: &Project,
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    chat: u32,
    said: FieldSaid,
    gone: &dyn Fn() -> bool,
) -> Answer {
    *hosted.field.lock().unwrap() = Some(said);
    let held = &project.held;
    answered_filling(
        browsers,
        hosted,
        &ask(chat, fill("e12")),
        None,
        &|asked: &FillAsked| crate::secretfills::asked(held, chat, asked, 1000, gone),
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

/// Whether each fill the browser was handed went in under a grant, with no notice for it.
fn handed_under_a_grant(hosted: &Hosted) -> Vec<bool> {
    hosted
        .fills
        .lock()
        .unwrap()
        .iter()
        .map(|(_, fill)| fill.under_a_grant)
        .collect()
}

/// A one-line field whose form sends it by GET where `by_get` says.
fn sent(by_get: Option<bool>) -> FieldSaid {
    FieldSaid {
        by_get,
        ..field(FieldKind::OneLine)
    }
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

/// The person's ruling of 2026-11-07: two Keep blocked in one chat of a persona end the fill
/// notices of its other chats, so the next one's call is told so and nothing is put to the
/// person; a chat that runs as no persona keeps a count of its own and is still asked.
#[test]
fn two_kept_blocked_in_one_chat_of_a_persona_end_its_other_chats_fill_notices() {
    let project = Project::holding(&the_key_held());
    let (first, second) = (project.a_chat(), project.a_chat());
    let nobodys = project.a_chat_of_no_persona();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    for chat in [first, second, nobodys] {
        opens_a_browser(&browsers, &hosted, chat);
    }
    let keep = Says::Presses(Press::Keep);
    let person = Person::saying(&project, &[keep.clone(), keep.clone(), keep]);

    let kept = ["https://a.example.test", "https://b.example.test"].map(|origin| {
        refusal(fills_into(
            &project,
            &browsers,
            &hosted,
            first,
            one_line_on(origin),
        ))
    });
    let next = refusal(fills_into(
        &project,
        &browsers,
        &hosted,
        second,
        one_line_on(ORIGIN),
    ));
    let nobodys_told = refusal(fills_into(
        &project,
        &browsers,
        &hosted,
        nobodys,
        one_line_on(ORIGIN),
    ));

    let came_up = person.came_up();
    assert_eq!(
        came_up
            .notices
            .iter()
            .map(|notice| notice.session)
            .collect::<Vec<_>>(),
        [first, first, nobodys],
        "the second chat of qa was never asked"
    );
    for told in kept.iter().chain([&next, &nobodys_told]) {
        assert!(told.contains(&format!("kept {KEY} blocked")), "{told}");
    }
    assert!(handed(&hosted).is_empty());
}

/// A fill whose call ends while the person is asked takes its notice with it: nothing is left
/// for them to press, and nothing is filled.
#[test]
fn a_fill_whose_call_ends_while_its_notice_waits_takes_the_notice_with_it() {
    let project = Project::holding(&the_key_held());
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    // Where the notice outlives its call, the person keeps it blocked after a while, so the
    // call ends.
    let held = Arc::clone(&project.held);
    let done = Arc::new(AtomicBool::new(false));
    let done_here = Arc::clone(&done);
    let late = std::thread::spawn(move || {
        let from = std::time::Instant::now() + Duration::from_secs(10);
        let mut kept = Vec::new();
        while !done_here.load(Ordering::SeqCst) {
            let up = held.secret_fills().waiting().into_iter().next();
            if let Some(wait) = up.filter(|_| std::time::Instant::now() >= from) {
                answer(
                    held.root(),
                    held.secret_fills(),
                    &Asked {
                        session: wait.session,
                        id: wait.id,
                        press: Press::Keep,
                        shown: &wait.digest(),
                        reason: None,
                    },
                    &|_, _| Ok(()),
                )
                .expect("the press counts");
                kept.push(wait);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        kept
    });
    // The asker hangs up once the notice is up.
    let fills = project.held.secret_fills();
    let hung_up = || !fills.waiting().is_empty();

    let told = refusal(fills_into_while(
        &project,
        &browsers,
        &hosted,
        chat,
        field(FieldKind::OneLine),
        &hung_up,
    ));

    done.store(true, Ordering::SeqCst);
    let kept = late.join().expect("the person's thread");
    assert!(told.contains("the call ended"), "{told}");
    assert!(kept.is_empty(), "no notice was left: {kept:?}");
    assert!(fills.waiting().is_empty());
    assert!(handed(&hosted).is_empty());
}

/// A fill whose call ends while it waits on a Secret ask for its key stops waiting, and the ask
/// goes from the person's window: nothing is left for them to answer, and nothing is filled.
#[test]
fn a_fill_whose_call_ends_while_its_secret_ask_waits_takes_the_ask_with_it() {
    let project = Project::holding(&serde_json::json!({}));
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    // The person allows the fill once. Where the Secret ask outlives its call, they decline it
    // after a while, so the call ends.
    let held = Arc::clone(&project.held);
    let done = Arc::new(AtomicBool::new(false));
    let done_here = Arc::clone(&done);
    let person = std::thread::spawn(move || {
        let mut declined = Vec::new();
        let mut up_since = None;
        while !done_here.load(Ordering::SeqCst) {
            if let Some(wait) = held.secret_fills().waiting().into_iter().next() {
                answer(
                    held.root(),
                    held.secret_fills(),
                    &Asked {
                        session: wait.session,
                        id: wait.id,
                        press: Press::Once,
                        shown: &wait.digest(),
                        reason: None,
                    },
                    &|_, _| Ok(()),
                )
                .expect("the press counts");
            } else if let Some(open) = held.secret_asks().open().into_iter().next() {
                let since = *up_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= Duration::from_secs(10) {
                    held.secret_asks()
                        .answer(
                            &Ctx::new(held.root(), Env::from_process()),
                            open.id,
                            SecretAnswer::Decline,
                            Answerer::admitted(Admitted::LocalUi).expect("the window"),
                        )
                        .expect("the answer counts");
                    declined.push(open);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        declined
    });
    // The asker hangs up once the Secret ask is up.
    let asks = project.held.secret_asks();
    let hung_up = || !asks.open().is_empty();

    let told = refusal(fills_into_while(
        &project,
        &browsers,
        &hosted,
        chat,
        field(FieldKind::OneLine),
        &hung_up,
    ));

    done.store(true, Ordering::SeqCst);
    let declined = person.join().expect("the person's thread");
    assert!(told.contains("the call ended"), "{told}");
    assert!(declined.is_empty(), "no ask was left: {declined:?}");
    assert!(asks.open().is_empty());
    assert!(handed(&hosted).is_empty());
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

/// The person's ruling of 2026-11-06: under Allow for this chat or Always, a fill into a field
/// whose form sends it by GET is refused, with no notice; a fill the person is asked about goes
/// in, and the browser is told which went in under a grant.
#[test]
fn under_a_grant_a_field_whose_form_sends_it_by_get_is_refused() {
    for grant in [Press::Always, Press::Chat] {
        let project = Project::holding(&the_key_held());
        let chat = project.a_chat();
        let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
        opens_a_browser(&browsers, &hosted, chat);
        let person = Person::saying(&project, &[Says::Presses(grant)]);

        let granted = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
        let by_post = fills_into(&project, &browsers, &hosted, chat, sent(Some(false)));
        let by_get = fills_into(&project, &browsers, &hosted, chat, sent(Some(true)));

        let came_up = person.came_up();
        told_it_filled(&granted);
        told_it_filled(&by_post);
        let told = refusal(by_get);
        assert!(told.contains("by GET"), "{grant:?}: {told}");
        assert!(told.contains("standing Allow"), "{grant:?}: {told}");
        assert_eq!(came_up.notices.len(), 1, "{grant:?}: no notice for it");
        assert_eq!(handed_under_a_grant(&hosted), [false, true], "{grant:?}");
    }
}

/// Asked about, a field whose form sends it by GET takes the value on Allow once.
#[test]
fn a_field_whose_form_sends_it_by_get_fills_on_allow_once() {
    let project = Project::holding(&the_key_held());
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    let person = Person::saying(&project, &[Says::Presses(Press::Once)]);

    let answer = fills_into(&project, &browsers, &hosted, chat, sent(Some(true)));

    let came_up = person.came_up();
    told_it_filled(&answer);
    assert_eq!(came_up.notices.len(), 1);
    assert_eq!(handed_under_a_grant(&hosted), [false]);
}

/// A field whose read doesn't say how its form sends it (a host that doesn't read it) is asked
/// about, whatever was allowed.
#[test]
fn a_field_whose_read_says_nothing_of_its_form_raises_the_notice_under_a_grant() {
    let project = Project::holding(&the_key_held());
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    let person = Person::saying(
        &project,
        &[Says::Presses(Press::Chat), Says::Presses(Press::Once)],
    );

    let granted = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
    let unknown = fills_into(&project, &browsers, &hosted, chat, sent(None));

    let came_up = person.came_up();
    told_it_filled(&granted);
    told_it_filled(&unknown);
    assert_eq!(came_up.notices.len(), 2, "{:?}", came_up.notices);
    assert_eq!(handed_under_a_grant(&hosted), [false, false]);
}

/// The steward's decision of 2026-11-06: a field the page marks a card's or payment one raises
/// the notice under either grant, with Allow once and Keep blocked alone, though its key is no
/// card's; and so does one whose read doesn't say.
#[test]
fn a_payment_field_raises_the_notice_with_allow_once_alone_under_either_grant() {
    for grant in [Press::Always, Press::Chat] {
        for payment in [Some(true), None] {
            let project = Project::holding(&the_key_held());
            let chat = project.a_chat();
            let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
            opens_a_browser(&browsers, &hosted, chat);
            let person = Person::saying(
                &project,
                &[Says::Presses(grant), Says::Presses(Press::Once)],
            );
            let card = FieldSaid {
                payment,
                ..field(FieldKind::OneLine)
            };

            let granted = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
            let asked = fills_into(&project, &browsers, &hosted, chat, card);

            let came_up = person.came_up();
            told_it_filled(&granted);
            told_it_filled(&asked);
            assert_eq!(came_up.notices.len(), 2, "{grant:?}, {payment:?}");
            let notice = &came_up.notices[1];
            assert!(notice.payment && !notice.card, "{grant:?}, {payment:?}");
            assert!(notice.once_only(), "only Allow once and Keep blocked");
            assert_eq!(notice.line, None, "Always isn't offered on it");
            assert_eq!(handed_under_a_grant(&hosted), [false, false]);
        }
    }
}

/// The steward's decision of 2026-11-06: a chat narrowed to another persona's grants fills under
/// that persona's, never under the Always of the persona it was started as, and its notice's
/// Always would write in the persona it was narrowed to (none, where it holds none).
#[test]
fn a_narrowed_chat_fills_under_the_grants_it_was_narrowed_to() {
    let project = Project::holding(&the_key_held());
    let own = project.a_chat();
    let to_devops = project.a_chat_holding(Some(purlis_core::reopen::HeldGrants {
        persona: Some("devops".to_owned()),
    }));
    let to_none = project.a_chat_holding(Some(purlis_core::reopen::HeldGrants { persona: None }));
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    for chat in [own, to_devops, to_none] {
        opens_a_browser(&browsers, &hosted, chat);
    }
    let person = Person::saying(
        &project,
        &[
            Says::Presses(Press::Always),
            Says::Presses(Press::Keep),
            Says::Presses(Press::Keep),
        ],
    );

    let granted = fills(&project, &browsers, &hosted, own, FieldKind::OneLine);
    let again = fills(&project, &browsers, &hosted, own, FieldKind::OneLine);
    let devops = fills(&project, &browsers, &hosted, to_devops, FieldKind::OneLine);
    let none = fills(&project, &browsers, &hosted, to_none, FieldKind::OneLine);

    let came_up = person.came_up();
    told_it_filled(&granted);
    told_it_filled(&again);
    refusal(devops);
    refusal(none);
    let asked: Vec<(u32, Option<&str>)> = came_up
        .notices
        .iter()
        .map(|notice| (notice.session, notice.persona.as_deref()))
        .collect();
    assert_eq!(
        asked,
        [
            (own, Some("qa")),
            (to_devops, Some("devops")),
            (to_none, None)
        ]
    );
    assert_eq!(handed(&hosted).len(), 2, "the narrowed chats' went nowhere");
}

/// #5910 (from #5831): a fill's notice raised for a chat narrowed to another persona's grants, or
/// to none, reads vaults as the persona the chat was started as, the persona its reads use: the
/// persona it was narrowed to only decides whose grants it fills under and whose tools Always
/// writes in.
#[test]
fn a_narrowed_chats_fill_notice_reads_as_the_persona_it_was_started_as() {
    let project = Project::holding(&the_key_held());
    let own = project.a_chat();
    let to_devops = project.a_chat_holding(Some(purlis_core::reopen::HeldGrants {
        persona: Some("devops".to_owned()),
    }));
    let to_none = project.a_chat_holding(Some(purlis_core::reopen::HeldGrants { persona: None }));
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    for chat in [own, to_devops, to_none] {
        opens_a_browser(&browsers, &hosted, chat);
    }
    let person = Person::saying(
        &project,
        &[
            Says::Presses(Press::Keep),
            Says::Presses(Press::Keep),
            Says::Presses(Press::Keep),
        ],
    );

    for chat in [own, to_devops, to_none] {
        refusal(fills(
            &project,
            &browsers,
            &hosted,
            chat,
            FieldKind::OneLine,
        ));
    }

    let came_up = person.came_up();
    let read_as: Vec<(u32, Option<&str>)> = came_up
        .notices
        .iter()
        .map(|notice| (notice.session, notice.reads_as.as_deref()))
        .collect();
    assert_eq!(
        read_as,
        [
            (own, Some("qa")),
            (to_devops, Some("qa")),
            (to_none, Some("qa"))
        ]
    );
}

/// #5242: the person allowed a fill once and its key was then asked for, but the call ended
/// before they added it. Allow once was for that fill, which never went in: the chat's retry of
/// exactly that fill asks only for the key, never for the Allow again.
#[test]
fn allow_once_for_a_fill_whose_call_ended_waiting_on_its_key_carries_its_retry() {
    let project = Project::holding(&serde_json::json!({}));
    let chat = project.a_chat();
    let (browsers, hosted) = (ChatBrowsers::default(), Arc::new(Hosted::default()));
    opens_a_browser(&browsers, &hosted, chat);
    let held = Arc::clone(&project.held);
    let done = Arc::new(AtomicBool::new(false));
    let done_here = Arc::clone(&done);
    // Allows each fill notice once, until the chat retries; then keeps any blocked, so a notice
    // the retry raises ends its call, and adds the key on its Secret ask.
    let retrying = Arc::new(AtomicBool::new(false));
    let retrying_here = Arc::clone(&retrying);
    let person = std::thread::spawn(move || {
        let mut notices = Vec::new();
        while !done_here.load(Ordering::SeqCst) {
            let again = retrying_here.load(Ordering::SeqCst);
            if let Some(wait) = held.secret_fills().waiting().into_iter().next() {
                answer(
                    held.root(),
                    held.secret_fills(),
                    &Asked {
                        session: wait.session,
                        id: wait.id,
                        press: if again { Press::Keep } else { Press::Once },
                        shown: &wait.digest(),
                        reason: None,
                    },
                    &|_, _| Ok(()),
                )
                .expect("the press counts");
                notices.push(wait);
            } else if again && let Some(open) = held.secret_asks().open().into_iter().next() {
                held.secret_asks()
                    .answer(
                        &Ctx::new(held.root(), Env::from_process()),
                        open.id,
                        SecretAnswer::Fill(VALUE.to_owned().into()),
                        Answerer::admitted(Admitted::LocalUi).expect("the window"),
                    )
                    .expect("the answer counts");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        notices
    });
    // The first call hangs up once the key is asked for, as a harness's call that timed out.
    let asks = project.held.secret_asks();
    let hung_up = || !asks.open().is_empty();
    let first = refusal(fills_into_while(
        &project,
        &browsers,
        &hosted,
        chat,
        field(FieldKind::OneLine),
        &hung_up,
    ));
    assert!(first.contains("the call ended"), "{first}");

    retrying.store(true, Ordering::SeqCst);
    let retried = fills(&project, &browsers, &hosted, chat, FieldKind::OneLine);
    // Used by the fill it was for: the next fill of that key asks, and is kept blocked.
    let next = refusal(fills(
        &project,
        &browsers,
        &hosted,
        chat,
        FieldKind::OneLine,
    ));
    done.store(true, Ordering::SeqCst);
    let notices = person.join().expect("the person's thread");

    assert_eq!(
        notices.len(),
        2,
        "asked once for the retry, once after: {notices:?}"
    );
    assert!(next.contains("blocked"), "{next}");
    told_it_filled(&retried);
    assert_eq!(handed(&hosted), [(VALUE.to_owned(), false)]);
    assert!(
        handed_under_a_grant(&hosted).iter().all(|grant| !grant),
        "it went in on the person's notice, under no standing grant"
    );
}
