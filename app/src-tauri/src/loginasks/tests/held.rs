// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A login's notice on a project the window holds, its chats real ones on a session host that
//! runs nothing: Keep blocked counts for the persona across its chats, a call that ends while its
//! notice waits takes the notice with it, and a runner chat's notice nobody answered goes and
//! the chat is told.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::askoutcome::{About, Answer as Said, Outcome};
use purlis_core::engine::Size;
use purlis_core::reopen::Chat;

use super::*;

/// A project whose vault `qa`, persona qa's, holds [`QA_HOLDS`], held by the window on a session
/// host that runs nothing: each word a chat is told of an Ask's answer is kept in `told`.
struct Project {
    _dir: tempfile::TempDir,
    _planes: Planes,
    held: Arc<Held>,
    told: Arc<Mutex<Vec<(u32, Outcome)>>>,
}

impl Project {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let root = dir.path().join("plane");
        let persona = root.join("personas").join("qa");
        std::fs::create_dir_all(&persona).expect("a persona");
        std::fs::write(
            persona.join("persona.md"),
            "---\nname: qa\ndescription: checks the shop\n---\n# qa\n",
        )
        .expect("its definition");
        std::fs::write(
            purlis_core::names::manifest(&root),
            "[persona]\ndefault = \"qa\"\n",
        )
        .expect("the project's file");
        std::fs::write(root.join("vaults.json"), VAULTS).expect("a registry");
        std::fs::write(root.join("q.json"), QA_HOLDS).expect("vault qa");
        let host = crate::host::pretend::Pretend::default();
        let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
            .running_sessions_on(Arc::new(move |_, _: &Path| Box::new(host.clone())));
        let id = planes.open(&root);
        let held = planes.held(&id).expect("held");
        let told = Arc::new(Mutex::new(Vec::new()));
        let keeps = Arc::clone(&told);
        held.chats()
            .tell_network_words_to(Arc::new(move |session: u32, outcome: Outcome| {
                keeps.lock().unwrap().push((session, outcome));
            }));
        Self {
            _dir: dir,
            _planes: planes,
            held,
            told,
        }
    }

    /// A chat that runs as persona qa, whose browser the person allowed.
    fn a_chat(&self) -> u32 {
        let chat = self.started(None);
        let persona = self.held.chats().secret_grants_of(chat);
        assert_eq!(persona.as_deref(), Some("qa"), "its persona");
        chat
    }

    /// A chat started as persona qa that a handoff narrowed to `held`'s grants, whose browser the
    /// person allowed.
    fn a_chat_narrowed(&self, held: purlis_core::reopen::HeldGrants) -> u32 {
        let chat = self.started(Some(held.clone()));
        let persona = self.held.chats().secret_grants_of(chat);
        assert_eq!(persona, held.persona, "the persona it was narrowed to");
        chat
    }

    /// A chat started as persona qa, narrowed to `held`'s grants where it says, whose browser the
    /// person allowed.
    fn started(&self, held: Option<purlis_core::reopen::HeldGrants>) -> u32 {
        let chat = Chat {
            program: "/bin/sh".to_owned(),
            name: "1".to_owned(),
            persona: Some("qa".to_owned()),
            held,
            cwd: Some(self.held.root().to_path_buf()),
            unsandboxed: true,
            ..Default::default()
        };
        let size = Size {
            columns: 80,
            rows: 24,
        };
        let chat = self.held.chats().start(&chat, size).expect("it runs");
        self.held.browsers().allowed_now(chat);
        // What a login's notice waits behind, each said where it doesn't hold.
        assert!(self.held.browsers().may_browse(chat, false), "its browser");
        assert!(!self.held.chats().sandboxed_by_its_host(chat), "its host");
        let asker = self.held.chats().login_asker(self.held.root(), chat);
        assert!(asker.is_some(), "who asks for its logins");
        chat
    }

    /// Chat `chat`'s `browser_login` of `login` on `site`, whose form is on that page, its asker
    /// `gone` where that says: what it is answered before its browser hears anything.
    fn logs_in(&self, chat: u32, login: &str, site: &str, gone: &dyn Fn() -> bool) -> Answer {
        let browse = BrowseAsk {
            chat,
            browser: None,
            call_id: "c-1".into(),
            waited: false,
            asked: Browsing::LogIn {
                login: login.to_owned(),
                suggested: Some(suggested(site)),
                start: None,
            },
        };
        asked_about_the_login(&self.held, &browse, None, 1000, gone, &on_its_page)
            .expect("the login waits on the person, not on the normal path")
    }

    /// A notice for chat `chat`'s login shop-login, raised as a runner chat's is: nothing waits
    /// on it.
    fn raised(&self, chat: u32) -> (u32, mpsc::Receiver<Heard>) {
        let root = self.held.root();
        let site = suggested("https://shop.example.test/");
        let judged = super::super::judged(
            root,
            Whose {
                grants: "qa",
                reads_as: "qa",
            },
            "shop-login",
            &site,
            &on_its_page,
            None,
            &[],
        )
        .expect("asked")
        .expect("an ask");
        self.held
            .login_grants()
            .raise(LoginWait {
                id: 0,
                session: chat,
                chat: self.held.chats().id_of(chat),
                proposal: judged.proposal,
                since: 1,
                vault_too: judged.vault_too,
                no_always: judged.no_always,
                pool: Some("ci".to_owned()),
                reads_as: "qa".to_owned(),
            })
            .expect("raised")
    }

    /// What the chats were told of their Asks' answers, once something was or `within` passed.
    fn told_within(&self, within: Duration) -> Vec<(u32, Outcome)> {
        let until = Instant::now() + within;
        while self.told.lock().unwrap().is_empty() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.told.lock().unwrap().clone()
    }
}

fn refusal(answer: Answer) -> String {
    match answer {
        Answer::No { why } => why,
        other => panic!("not a refusal: {other:?}"),
    }
}

/// The person at the window, on a thread of their own while a chat's call waits: once `after`
/// has passed, Keep blocked on each login notice as it comes up.
struct Keeper {
    done: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Vec<(u32, String)>>,
}

impl Keeper {
    fn after(project: &Project, after: Duration) -> Self {
        let held = Arc::clone(&project.held);
        let done = Arc::new(AtomicBool::new(false));
        let done_here = Arc::clone(&done);
        let from = Instant::now() + after;
        let thread = std::thread::spawn(move || {
            let mut came_up = Vec::new();
            while !done_here.load(Ordering::SeqCst) {
                let up = held.login_grants().waiting().into_iter().next();
                if let Some(wait) = up.filter(|_| Instant::now() >= from) {
                    let shown = wait.proposal.digest();
                    let keep = Asked {
                        session: wait.session,
                        id: wait.id,
                        press: Press::Keep,
                        shown: &shown,
                        typed: None,
                        reason: None,
                    };
                    answer(
                        held.root(),
                        held.login_grants(),
                        &keep,
                        &on_its_page,
                        &|_, _| Ok(()),
                    )
                    .expect("the press counts");
                    came_up.push((wait.session, wait.reads_as.clone()));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            came_up
        });
        Self { done, thread }
    }

    /// The chats whose notices they kept blocked, in order.
    fn came_up(self) -> Vec<u32> {
        self.came_up_reading_as()
            .into_iter()
            .map(|(session, _)| session)
            .collect()
    }

    /// [`Keeper::came_up`], each with the persona its notice reads vaults as.
    fn came_up_reading_as(self) -> Vec<(u32, String)> {
        self.done.store(true, Ordering::SeqCst);
        self.thread.join().expect("the person's thread")
    }
}

/// #5910 (from #5831): a login's notice raised for a chat narrowed to another persona's grants
/// reads vaults as the persona the chat was started as, the persona its reads use, never the
/// persona it was narrowed to, which only decides whose grants it reads and whose tools Always
/// writes in.
#[test]
fn a_narrowed_chats_login_notice_reads_as_the_persona_it_was_started_as() {
    let project = Project::new();
    let own = project.a_chat();
    let narrowed = project.a_chat_narrowed(purlis_core::reopen::HeldGrants {
        persona: Some("devops".to_owned()),
    });
    let keeper = Keeper::after(&project, Duration::ZERO);
    let here = || false;

    for chat in [own, narrowed] {
        refusal(project.logs_in(chat, "shop-login", "https://shop.example.test/", &here));
    }

    assert_eq!(
        keeper.came_up_reading_as(),
        [(own, "qa".to_owned()), (narrowed, "qa".to_owned())]
    );
}

/// The person's ruling of 2026-11-07: two Keep blocked on one chat's logins end the login
/// notices of every chat of its persona until purlis restarts, so the next chat's call is told
/// so and nothing is put to the person.
#[test]
fn two_keep_blocked_in_one_chat_end_the_login_notices_of_another_chat_of_its_persona() {
    let project = Project::new();
    let (first, second) = (project.a_chat(), project.a_chat());
    let keeper = Keeper::after(&project, Duration::ZERO);
    let here = || false;

    let kept = [
        ("shop-login", "https://shop.example.test/"),
        ("dash-login", "https://dash.example.test/"),
    ]
    .map(|(login, site)| refusal(project.logs_in(first, login, site, &here)));
    let next = refusal(project.logs_in(second, "mail-login", "https://mail.example.test/", &here));

    let came_up = keeper.came_up();
    assert_eq!(
        came_up,
        [first, first],
        "the second chat's login was never put to the person"
    );
    for told in &kept {
        assert!(told.contains("kept"), "{told}");
    }
    assert!(
        next.contains("the person kept mail-login blocked"),
        "{next}"
    );
}

/// A login call that ends while its notice waits takes the notice with it: nothing is left for
/// the person to press, and the chat's next call is put to them afresh.
#[test]
fn a_login_call_that_ends_while_its_notice_waits_takes_the_notice_with_it() {
    let project = Project::new();
    let chat = project.a_chat();
    // Where the notice outlives its call, the person keeps it blocked, so the call ends.
    let keeper = Keeper::after(&project, Duration::from_secs(10));
    let held = &project.held;
    // The asker hangs up once the notice is up.
    let hung_up = || !held.login_grants().waiting().is_empty();

    let told = refusal(project.logs_in(chat, "shop-login", "https://shop.example.test/", &hung_up));

    let came_up = keeper.came_up();
    assert!(told.contains("the call ended"), "{told}");
    assert!(came_up.is_empty(), "no notice was left: {came_up:?}");
    assert!(held.login_grants().waiting().is_empty());
}

/// The person's ruling of 2026-11-07: a runner chat's login notice, which no call waits on, goes
/// once its time is up, and the chat is told on its next turn that nobody answered. One the
/// person answered in time tells it nothing of the kind.
#[test]
fn a_runner_chats_login_notice_nobody_answered_goes_and_the_chat_is_told() {
    let project = Project::new();
    let (chat, answered) = (project.a_chat(), project.a_chat());
    let (id, heard) = project.raised(answered);
    project.held.login_grants().end(id, Heard::Allowed);
    let_go_unanswered(
        &project.held,
        answered,
        "shop-login",
        (id, heard),
        Duration::from_millis(20),
    );

    let raised = project.raised(chat);
    let_go_unanswered(
        &project.held,
        chat,
        "shop-login",
        raised,
        Duration::from_millis(20),
    );

    let told = project.told_within(Duration::from_secs(10));
    assert_eq!(
        told,
        [(
            chat,
            Outcome {
                about: About::BrowserLogin {
                    login: "shop-login".to_owned()
                },
                answer: Said::NotAnswered,
            }
        )]
    );
    assert!(
        project.held.login_grants().waiting().is_empty(),
        "the notice went"
    );
}

/// A runner chat's login notice refused at the press for a key out of reach is never told to the
/// chat as an Allow (#5831): the press is the person's error, and the notice stays for Keep blocked
/// or its time, as for every press refused.
#[test]
fn a_runner_chats_login_refused_at_the_press_is_never_told_as_allowed() {
    let project = Project::new();
    let chat = project.a_chat();
    let raised = project.raised(chat);
    let id = raised.0;
    let_go_unanswered(
        &project.held,
        chat,
        "shop-login",
        raised,
        Duration::from_secs(60),
    );
    linked_out_of_reach(project.held.root());
    let wait = project.held.login_grants().waiting().into_iter().next();
    let shown = wait.expect("the notice waits").proposal.digest();

    let refused = pressed(
        &project.held,
        None,
        &Asked {
            session: chat,
            id,
            press: Press::Always,
            shown: &shown,
            typed: None,
            reason: None,
        },
    )
    .map(|_| ())
    .expect_err("refused");

    assert!(refused.starts_with("Nothing was allowed: "), "{refused}");
    assert!(
        refused.contains("password in vault qa reads an entry of a vault qa may not use"),
        "{refused}"
    );
    assert!(
        project.told_within(Duration::from_millis(200)).is_empty(),
        "the chat is told nothing yet, and never Allowed"
    );
    assert_eq!(
        project.held.login_grants().waiting().len(),
        1,
        "the notice stays"
    );
}
