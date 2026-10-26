// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person's browsers, as the window opens and watches them (#2012, spec #2008).
//!
//! **A browser runs in the session host, never in the window.** The window asks its host to
//! start one in a project ([`open_browser`]), and draws it in a view tab: a browser view whose
//! whole frames the window decodes on a canvas ([`watch_browser`]). Frames are images handed
//! over as bytes, never an embedded page, so the window's content policy is unchanged.
//!
//! **Who opens one.** The person, from the window: a browser is a program started as the
//! person, so these commands are the window's alone (`ui::WINDOW_ONLY`). It reaches what the
//! project's browsers reach (`crate::chatbrowsers::person_reach`, #2176), and goes to an address
//! the person types in its view tab ([`go_browser`]). A chat's browsers are its own (#2013).
//!
//! **Watching is cheap.** The view the person looks at is the focused one, sent every frame; a
//! thumbnail is sent about one a second; with no view attached, the browser makes no frames at
//! all (`purlis_host`'s browsers).
//!
//! **Stop all ends every browser** ([`stop_every_browser`]): the kill switch, from the title bar
//! or `purlis stop --all`, stops each browser the window's host runs, which ends its engine's
//! whole process group. While stopped, no browser starts.
//!
//! **The person takes a browser over from its view** (#2014): [`take_over_browser`], then their
//! own clicks and keys ([`browser_input`]), each aimed at what the frame they saw showed, and
//! [`hand_back_browser`] on Done. The lease is the session host's: a view opened again, after
//! a crash of the window too, says whether the person still drives ([`BrowserWatching`]).
//! These are the window's alone (`ui::WINDOW_ONLY`): what the person types is theirs.
//!
//! **The event log says who drives, and that someone typed, never what** (#2293). Each change
//! of a browser's driver the window makes (the person taking it over, handing it back, purlis
//! performing a login and handing it back to the agent) and the first key its driver types
//! after it are told to whoever writes the log (`WindowBrowsers::saying`), by the browser's
//! id alone.
//!
//! **A browser view drawn from a runner says it is reconnecting** while the link to that runner
//! is down (#2542, as a runner's chats do, #2496): its view rides the window's own link, so
//! the window follows what that link hears of the runner, and tells each view drawn from it
//! ([`BrowserFrame::reconnection`]). A view is reconnected once the link is back, once it draws
//! again from a snapshot, or once the window can no longer tell.

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

/// What a browser view is to the person: the view they are looking at, or a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BrowserViewRole {
    Focused,
    Thumbnail,
}

/// A browser view the window has open: its number, the size of the page its frames are
/// painted at, in CSS pixels, and whether the person drives the browser (#2014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct BrowserWatching {
    pub view: u32,
    pub width: u16,
    pub height: u16,
    pub person_drives: bool,
}

/// One frame of a browser view, as the window is sent it (#2014): what it shows (`p1:5`, the
/// page and its document), which the person's input names back, and the JPEG in base64. An
/// empty `jpeg` says the view has ended for good.
///
/// Or, with `reconnection` and no image, where the view stands while the link to the runner it
/// is drawn from is down and back (#2542): its last frame stays, dimmed, until it is
/// reconnected.
///
/// Or, with `page` and no image, where the browser's page is (#2300): its origin alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BrowserFrame {
    pub aim: String,
    pub jpeg: String,
    /// Where the view stands while its runner's link is down and back (#2542): said on its
    /// own, with no image; `null` on every frame.
    pub reconnection: Option<crate::reconnecting::Reconnection>,
    /// Where the browser's page is (#2300): said on its own, with no image, where it changed
    /// and before the first frame of each snapshot; `null` on every frame.
    pub page: Option<PageAt>,
}

/// **Where a browser's page is** (#2300): the origin of the document it has loaded
/// (`http://localhost:5173`), from the engine's own record, or `null` for no page or a page
/// with no origin of its own (`about:blank`). Never its path, query, title or text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct PageAt {
    pub origin: Option<String>,
}

/// What the person's mouse did, in a browser they drive (#2014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MouseAct {
    Moved,
    Pressed,
    Released,
    Wheel,
}

/// A mouse button (#2014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    None,
    Left,
    Middle,
    Right,
}

/// What the person's key did (#2014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyAct {
    Down,
    Up,
}

/// One event of the person's in a browser they drive (#2014): aimed at what a frame showed
/// (its `aim`), at a point in CSS pixels of the page; `modifiers` are bits (Alt 1, Control 2,
/// Meta 4, Shift 8).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserInput {
    Mouse {
        aim: String,
        action: MouseAct,
        x: i32,
        y: i32,
        button: MouseButton,
        clicks: u8,
        modifiers: u8,
        delta_x: i32,
        delta_y: i32,
    },
    Key {
        aim: String,
        action: KeyAct,
        key: String,
        physical: String,
        key_code: u32,
        modifiers: u8,
    },
}

/// What became of one event of the person's (#2014): it reached the page; it was dropped, the
/// page having moved on from the frame it was aimed at; or it was refused, in words to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum InputOutcome {
    Taken,
    Missed,
    Refused { why: String },
}

/// **Where the person's address took their browser** (#2176): the page is at `address` now, or
/// the browser held back what it would have opened on `sites`, each new to the project, and the
/// person is asked whether their browsers may go there, with `answers`, the sites ask's own
/// (`crate::asking::browser_site_answers`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Went {
    Went {
        address: Option<String>,
    },
    Asks {
        sites: Vec<String>,
        answers: Vec<crate::asking::Offered>,
    },
}

/// What a frame shows, as the window holds it: `p1:5`, the page and its document.
pub(crate) fn aim_said(aim: &purlis_session_protocol::browsing::Aim) -> String {
    format!("{}:{}", aim.page, aim.document)
}

/// What `said` (as [`aim_said`] wrote it) names, if it reads.
#[cfg(unix)]
fn aim_read(said: &str) -> Option<purlis_session_protocol::browsing::Aim> {
    let (page, document) = said.rsplit_once(':')?;
    Some(purlis_session_protocol::browsing::Aim {
        page: page.to_owned(),
        document: document.parse().ok()?,
    })
}

#[cfg(unix)]
impl BrowserInput {
    /// The event in the session protocol's words, or `None` where its aim does not read.
    fn wire(&self) -> Option<purlis_session_protocol::browsing::Input> {
        use purlis_session_protocol::browsing::{self as wire, Input};
        Some(match self {
            BrowserInput::Mouse {
                aim,
                action,
                x,
                y,
                button,
                clicks,
                modifiers,
                delta_x,
                delta_y,
            } => Input::Mouse {
                aim: aim_read(aim)?,
                action: match action {
                    MouseAct::Moved => wire::MouseAction::Moved,
                    MouseAct::Pressed => wire::MouseAction::Pressed,
                    MouseAct::Released => wire::MouseAction::Released,
                    MouseAct::Wheel => wire::MouseAction::Wheel,
                },
                x: *x,
                y: *y,
                button: match button {
                    MouseButton::None => wire::Button::None,
                    MouseButton::Left => wire::Button::Left,
                    MouseButton::Middle => wire::Button::Middle,
                    MouseButton::Right => wire::Button::Right,
                },
                clicks: *clicks,
                modifiers: *modifiers,
                delta_x: *delta_x,
                delta_y: *delta_y,
            },
            BrowserInput::Key {
                aim,
                action,
                key,
                physical,
                key_code,
                modifiers,
            } => Input::Key {
                aim: aim_read(aim)?,
                action: match action {
                    KeyAct::Down => wire::KeyAction::Down,
                    KeyAct::Up => wire::KeyAction::Up,
                },
                key: key.clone(),
                physical: physical.clone(),
                key_code: *key_code,
                modifiers: *modifiers,
            },
        })
    }
}

#[cfg(unix)]
pub(crate) use hosted::WindowBrowsers;

#[cfg(unix)]
mod hosted {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use purlis_core::eventlog::browsing::{Driver as Drives, Said, Typist};
    use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach, Driver};
    use purlis_session_protocol::session::{
        self, Answer, Command, ProgramKind, StartProgram, ViewKind, ViewRole,
    };

    use super::{BrowserInput, BrowserViewRole, BrowserWatching, InputOutcome, KeyAct, Went};
    use crate::chatbrowsers::NotStarted;
    use crate::hostedsessions::{Connection, FrameSink, HostLink, OriginSink, Reconnect};
    use crate::reconnecting::Reconnection;

    /// **The window's line for the end of browser `program`** (#2745): how the session host said
    /// it ended. The host's own line, in the same log, says who ended it and why.
    pub(crate) fn ended_said(program: &str, ending: &crate::hostedsessions::Ending) -> String {
        use crate::hostedsessions::Ending;
        use purlis_session_protocol::session::Exit;
        let how = match ending {
            Ending::Exited(Exit::Code(code)) => format!("its engine's exit code {code}"),
            Ending::Exited(Exit::Signal(signal)) => format!("its engine's signal {signal}"),
            Ending::WithTheHost => "with its session host, whose link ended".to_owned(),
        };
        format!("purlis: browser {program} ended, {how}")
    }

    /// What the person is told of a browser that has gone when they send it an address.
    const GONE: &str = "This browser has ended: open a new one from the palette.";

    /// How long a browser command waits for the window's link to its host.
    const LINK_WITHIN: Duration = Duration::from_secs(30);

    /// How long a chat's act waits for its browser (#2013): a wait for text of a minute, and
    /// the page settling after it.
    const AN_ACT_TAKES_AT_MOST: Duration = Duration::from_secs(85);

    /// How long a login purlis performs waits for its browser (#2016): its page, its form
    /// showing, and its password field going, each within its own limit.
    const A_LOGIN_TAKES_AT_MOST: Duration = Duration::from_secs(150);

    /// **Why a browser's start was refused**, from the host's refusal: the device's budget,
    /// `budget` here, where the host said `device_full` (1.32, #2611); the engine's sandbox
    /// where it said `no_engine_sandbox` (1.36, #2899), or its words for it from any host (one
    /// before 1.36, or a relaying host that passed the words on without the code); else in the
    /// host's words, as for any code this window does not know.
    pub(super) fn not_started(refused: session::Refusal, budget: usize) -> NotStarted {
        if refused.code == session::DEVICE_FULL {
            NotStarted::Full { most: budget }
        } else if refused.code == session::NO_ENGINE_SANDBOX
            || crate::enginesandbox::is_no_sandbox(&refused.why)
        {
            NotStarted::NoSandbox
        } else if refused.why.contains(purlis_host::ENGINE_DOWNLOAD_CANCELLED) {
            NotStarted::Cancelled
        } else {
            NotStarted::Failed(refused.why)
        }
    }

    /// A chat's browsers run here (#2013): the window's session host.
    impl crate::chatbrowsers::Driver for WindowBrowsers {
        fn start(
            &self,
            root: &Path,
            reach: BrowserReach,
            kept: Option<purlis_session_protocol::browsing::KeptLogin>,
        ) -> Result<String, crate::chatbrowsers::NotStarted> {
            self.open_reaching(root, Some(reach), kept)
        }

        fn act(
            &self,
            program: &str,
            call_id: &str,
            act: Act,
            reach: BrowserReach,
        ) -> Result<Browsed, crate::chatbrowsers::Refused> {
            use crate::chatbrowsers::Refused;
            let link = self.link().map_err(Refused::Said)?;
            let command = Command::Browse {
                program: program.to_owned(),
                call_id: call_id.to_owned(),
                act,
                reach: Some(reach),
            };
            match link.call_refused_within(command, AN_ACT_TAKES_AT_MOST) {
                Ok(Answer::Browsed(browsed)) => Ok(*browsed),
                Ok(other) => Err(Refused::Said(format!(
                    "the session host answered a browser's act with {other:?}"
                ))),
                Err(refusal)
                    if refusal.code == session::NO_SUCH_CHAT
                        || refusal.code == session::BROWSER_ENDED =>
                {
                    Err(Refused::Gone)
                }
                Err(refusal) => Err(Refused::Said(refusal.why)),
            }
        }

        fn log_in(
            &self,
            program: &str,
            login: purlis_session_protocol::browsing::LogIn,
        ) -> Result<Browsed, crate::chatbrowsers::Refused> {
            use crate::chatbrowsers::Refused;
            let link = self.link().map_err(Refused::Said)?;
            let command = Command::LogIn {
                program: program.to_owned(),
                login,
            };
            // purlis drives it while it logs in (#2293), unless the person does: the host
            // refuses the login then. Who drives it is the host's word where this window has
            // none yet: a window started again may log in before it watches the browser.
            self.seed_from_the_host(&link, program);
            let purlis_drives = self.driver_said(program) != Drives::Person;
            if purlis_drives {
                self.drove(program, Drives::Purlis);
            }
            let answered = match link.call_refused_within(command, A_LOGIN_TAKES_AT_MOST) {
                Ok(Answer::Browsed(browsed)) => Ok(*browsed),
                Ok(_) => Err(Refused::Said(
                    "the session host answered a login with something else".into(),
                )),
                Err(refusal)
                    if refusal.code == session::NO_SUCH_CHAT
                        || refusal.code == session::BROWSER_ENDED =>
                {
                    Err(Refused::Gone)
                }
                Err(refusal) => Err(Refused::Said(refusal.why)),
            };
            // Back with the agent once it answers, unless the person took it over meanwhile;
            // a login that went through typed what it filled.
            if purlis_drives && self.driver_said(program) == Drives::Purlis {
                if answered.is_ok() {
                    self.typed(program, Typist::Purlis);
                }
                self.drove(program, Drives::Agent);
            }
            answered
        }

        fn close(&self, program: &str) -> Result<(), String> {
            WindowBrowsers::close(self, program)
        }

        fn stop(&self, program: &str) -> Result<(), String> {
            match self.link()?.call_refused_within(
                Command::Stop {
                    chat: program.to_owned(),
                },
                LINK_WITHIN,
            ) {
                Err(refusal) if refusal.code != session::NO_SUCH_CHAT => Err(refusal.why),
                _ => Ok(()),
            }
        }

        fn note(&self, root: &Path, program: &str, note: &crate::chatbrowsers::Note) {
            WindowBrowsers::note(self, root, program, note);
        }

        fn taken_back(&self, root: &Path, program: &str) {
            WindowBrowsers::taken_back(self, root, program);
        }

        fn driven(&self) -> Result<Vec<String>, String> {
            match self.link()?.call(Command::List)? {
                Answer::Chats(listed) => Ok(listed
                    .into_iter()
                    .filter(|one| one.kind == Some(ProgramKind::Browser) && one.driver.is_some())
                    .map(|one| one.chat)
                    .collect()),
                other => Err(format!("the session host answered a list with {other:?}")),
            }
        }
    }

    /// The window's browsers, in its session host: the link they go over, and each project the
    /// window has held for them, by its root, with the browsers it started there. A project is
    /// held on its root's own link, which its chats share (#2178).
    pub(crate) struct WindowBrowsers {
        connection: Arc<Connection>,
        projects: Mutex<HashMap<PathBuf, Held>>,
        ended: Ended,
        /// The most browsers the device runs at once (#2023), read at each start, and the lock
        /// one start holds from its count to its start, so two never both take the last.
        budget: Arc<dyn Fn() -> usize + Send + Sync>,
        starting: Mutex<()>,
        /// How many acts of the person's this window sent (#2176): part of each one's id.
        acts: std::sync::atomic::AtomicU64,
        /// Each view the window watches, and whether it is dimmed while its runner's link is
        /// down (#2542).
        dimming: Arc<Mutex<Dimming>>,
        /// Told what the window says of a browser in the event log (#2293).
        says: Says,
        /// Who drives each browser, as the window last said it, by program (#2293).
        logged: Arc<Mutex<HashMap<String, Logged>>>,
        /// Where each browser's note goes to be kept in the host, in the order given (#2025):
        /// one thread sends them, started with the first.
        notes: Mutex<Option<std::sync::mpsc::Sender<Noted>>>,
    }

    /// **Told what the window says of a browser in the event log** (#2293): the project the
    /// window started it in, where it did, and what it says. Whoever is told writes it; nothing
    /// in it is a key, a value, an address or a page's text.
    pub(crate) type Says = Arc<dyn Fn(Option<&Path>, Said) + Send + Sync + 'static>;

    /// Who drives a browser, as the window last said it in the event log, and whether its
    /// driver typed since (#2293): each change of driver is said once, and the first typing
    /// after it.
    #[derive(Debug, Clone, Copy)]
    struct Logged {
        driver: Drives,
        typed: bool,
    }

    /// What every browser starts as: the agent's, nothing typed.
    const STARTS: Logged = Logged {
        driver: Drives::Agent,
        typed: false,
    };

    /// Told where a browser view stands while the link to the runner it is drawn from comes and
    /// goes (#2542).
    pub(crate) type LinkSaid = Arc<dyn Fn(Reconnection) + Send + Sync + 'static>;

    /// The views the window watches, by the link each rides and its number there, and the links
    /// heard already.
    #[derive(Default)]
    struct Dimming {
        views: HashMap<(u64, u32), Followed>,
        heard: std::collections::HashSet<u64>,
    }

    /// One view: who is told where it stands, and whether it is dimmed now.
    struct Followed {
        tell: LinkSaid,
        dimmed: bool,
    }

    /// A window note on its way to the host: the project's root, the browser, the note.
    type Noted = (
        PathBuf,
        String,
        purlis_session_protocol::session::ProgramNote,
    );

    /// Told the program of each browser the window started whose engine ended (#2385), on the
    /// link's own thread: whoever is told does its work elsewhere.
    pub(crate) type Ended = Arc<dyn Fn(String) + Send + Sync + 'static>;

    /// A project the window holds for its browsers: its id, and the browsers started in it.
    struct Held {
        project: String,
        browsers: Vec<String>,
    }

    impl WindowBrowsers {
        pub(crate) fn new(connection: Arc<Connection>) -> Self {
            WindowBrowsers {
                connection,
                projects: Mutex::new(HashMap::new()),
                ended: Arc::new(|_| {}),
                budget: Arc::new(|| purlis_core::browserlimits::budget_for(None)),
                starting: Mutex::new(()),
                dimming: Arc::default(),
                acts: std::sync::atomic::AtomicU64::new(0),
                says: Arc::new(|_, _| {}),
                logged: Arc::default(),
                notes: Mutex::new(None),
            }
        }

        /// **Keeps `note` as this window's note of browser `program`, of the project at `root`,
        /// in its host** (#2025), on that project's own link: sent after every note given before
        /// it, never waited on. A host before 1.39 keeps none, and is sent none.
        pub(crate) fn note(&self, root: &Path, program: &str, note: &crate::chatbrowsers::Note) {
            let Some(note) = note.kept() else {
                return;
            };
            let mut notes = lock(&self.notes);
            let sender = notes.get_or_insert_with(|| {
                let (sender, noted) = std::sync::mpsc::channel::<Noted>();
                let connection = Arc::clone(&self.connection);
                let spawned = std::thread::Builder::new()
                    .name("purlis-browser-notes".into())
                    .spawn(move || {
                        for (root, program, note) in noted {
                            let sent = connection.of_project(&root, LINK_WITHIN).and_then(|link| {
                                if link.version() < session::KNOWS_BROWSERS_TAKEN_BACK {
                                    return Ok(());
                                }
                                link.call(Command::NoteProgram {
                                    program: program.clone(),
                                    note,
                                })
                                .map(|_| ())
                            });
                            if let Err(why) = sent {
                                tracing::warn!(
                                    "purlis: the session host kept no note of browser {program}: \
                                     {why}"
                                );
                            }
                        }
                    });
                if let Err(why) = spawned {
                    tracing::warn!("purlis: browsers' notes cannot be kept in the host: {why}");
                }
                sender
            });
            let _ = sender.send((root.to_path_buf(), program.to_owned(), note));
        }

        /// **Browser `program`, left in the host by a window before this one, is this window's
        /// again** in the project at `root` (#2025): listed among the browsers started there, and
        /// its end told as for one it started.
        pub(crate) fn taken_back(&self, root: &Path, program: &str) {
            let link = match self.held(root) {
                Ok((link, _)) => link,
                Err(why) => {
                    tracing::warn!("purlis: browser {program} could not be taken back: {why}");
                    return;
                }
            };
            self.follow_its_end(&link, program);
            if let Some(held) = self.projects().get_mut(root)
                && !held.browsers.iter().any(|one| one == program)
            {
                held.browsers.push(program.to_owned());
            }
            // Who drives it, as the host holds the lease: kept, not said again.
            self.seed_from_the_host(&link, program);
        }

        /// Its end, as the host says it, told to whoever is told (#2385).
        fn follow_its_end(&self, link: &HostLink, program: &str) {
            let ended = Arc::clone(&self.ended);
            let logged = Arc::clone(&self.logged);
            let named = program.to_owned();
            link.when_it_ends(
                program,
                Box::new(move |ending| {
                    lock(&logged).remove(&named);
                    // Who ended it and why is in the session host's own line (#2745).
                    tracing::info!("{}", ended_said(&named, &ending));
                    ended(named);
                }),
            );
        }

        /// **`says` is told who drives each browser and that someone typed, never what**
        /// (#2293), for the event log: the person taking a browser over and handing it back,
        /// purlis performing a login, and the first key the person or purlis types while
        /// driving.
        pub(crate) fn saying(self, says: Says) -> Self {
            WindowBrowsers { says, ..self }
        }

        /// The project the window started `program` in, where it did.
        fn root_of(&self, program: &str) -> Option<PathBuf> {
            self.projects()
                .iter()
                .find(|(_, held)| held.browsers.iter().any(|one| one == program))
                .map(|(root, _)| root.clone())
        }

        /// **Who drives `program`, as the host lists it**, kept where this window has not said
        /// it yet: the person's take-over a window before this one said. Kept, not said again.
        fn seed_from_the_host(&self, link: &HostLink, program: &str) {
            if lock(&self.logged).contains_key(program) {
                return;
            }
            let person = matches!(
                link.call(Command::List),
                Ok(Answer::Chats(listed)) if listed.iter().any(|one| {
                    one.chat == program && one.driver == Some(Driver::Person)
                })
            );
            if person {
                lock(&self.logged)
                    .entry(program.to_owned())
                    .or_insert(Logged {
                        driver: Drives::Person,
                        typed: false,
                    });
            }
        }

        /// Who drives `program`, as the window last said it.
        fn driver_said(&self, program: &str) -> Drives {
            lock(&self.logged)
                .get(program)
                .map_or(Drives::Agent, |logged| logged.driver)
        }

        /// **`driver` drives `program` now** (#2293): said where the window did not say so last.
        fn drove(&self, program: &str, driver: Drives) {
            {
                let mut logged = lock(&self.logged);
                let now = logged.entry(program.to_owned()).or_insert(STARTS);
                if now.driver == driver {
                    return;
                }
                *now = Logged {
                    driver,
                    typed: false,
                };
            }
            self.say(Said::DriverChanged {
                browser: program.to_owned(),
                driver,
            });
        }

        /// **`by` typed into `program`, which it drives** (#2293): said once for each stretch it
        /// drives, never what.
        fn typed(&self, program: &str, by: Typist) {
            {
                let mut logged = lock(&self.logged);
                let now = logged.entry(program.to_owned()).or_insert(STARTS);
                if now.typed {
                    return;
                }
                now.typed = true;
            }
            self.say(Said::Typed {
                browser: program.to_owned(),
                by,
            });
        }

        fn say(&self, said: Said) {
            let root = self.root_of(said.browser());
            (self.says)(root.as_deref(), said);
        }

        /// **At most `budget` browsers run on this device at once** (#2023): every browser the
        /// session host runs counts, whoever started it.
        #[cfg(test)]
        pub(crate) fn budgeted(self, budget: usize) -> Self {
            self.budgeted_by(Arc::new(move || budget))
        }

        /// **At most what `budget` reads at each start** (#2023): the person's setting over the
        /// budget this device's memory gives.
        pub(crate) fn budgeted_by(self, budget: Arc<dyn Fn() -> usize + Send + Sync>) -> Self {
            WindowBrowsers { budget, ..self }
        }

        /// **`ended` is told each browser this window starts whose engine ends** (#2385): a
        /// crash, the host stopping it, the host itself ending. The host's word of a program's
        /// end tells it, so nothing waits for the browser's next act.
        pub(crate) fn telling_ended(self, ended: Ended) -> Self {
            WindowBrowsers { ended, ..self }
        }

        pub(crate) fn link(&self) -> Result<Arc<HostLink>, String> {
            self.connection.wait(LINK_WITHIN)
        }

        /// The project at `root`'s own link, with the project held on it, and the project's id.
        fn held(&self, root: &Path) -> Result<(Arc<HostLink>, String), String> {
            let link = self.connection.of_project(root, LINK_WITHIN)?;
            let mut projects = self.projects();
            if let Some(held) = projects.get(root) {
                return Ok((link, held.project.clone()));
            }
            let project = purlis_core::scaffold::planefile::id_to_hold_by(root).map_err(|why| {
                format!("this project has no id the session host can name it by: {why}")
            })?;
            link.call(Command::HoldProject {
                project: project.clone(),
                project_path: root.display().to_string(),
            })?;
            projects.insert(
                root.to_path_buf(),
                Held {
                    project: project.clone(),
                    browsers: Vec::new(),
                },
            );
            Ok((link, project))
        }

        /// Starts a browser in the project at `root` reaching nothing listed, and answers its
        /// id: what a test of the window's browsers starts.
        #[cfg(test)]
        pub(crate) fn open(&self, root: &Path) -> Result<String, String> {
            self.open_for_the_person(root, None)
        }

        /// **Starts a browser the person opens** in the project at `root`, reaching `reach`
        /// (#2176), and answers its id, or why it did not start in the person's words.
        pub(crate) fn open_for_the_person(
            &self,
            root: &Path,
            reach: Option<BrowserReach>,
        ) -> Result<String, String> {
            self.open_reaching(root, reach, None)
                .map_err(|not| match not {
                    NotStarted::Full { most } => purlis_core::browserlimits::device_full_told(most),
                    NotStarted::NoSandbox => {
                        crate::enginesandbox::refused();
                        crate::enginesandbox::NOT_STARTED_FOR_THE_PERSON.to_owned()
                    }
                    NotStarted::Cancelled => {
                        crate::enginedownload::CANCELLED_FOR_THE_PERSON.to_owned()
                    }
                    NotStarted::Failed(why) => why,
                })
        }

        /// Starts a browser in the project at `root` reaching `reach` (#2013), or with none,
        /// nothing listed and every site new to the project refused; opened from the kept login
        /// `kept` where one is named (#2021); answers its id. Refused where the device runs its
        /// budget of browsers already (#2023): the session host holds every client to it and
        /// refuses a start past it `device_full` (1.32, #2611); this window's own count first
        /// keeps its starts within it on a host before 1.32.
        pub(crate) fn open_reaching(
            &self,
            root: &Path,
            reach: Option<BrowserReach>,
            kept: Option<purlis_session_protocol::browsing::KeptLogin>,
        ) -> Result<String, NotStarted> {
            let (link, project) = self.held(root).map_err(NotStarted::Failed)?;
            let _one_at_a_time = self.starting.lock().unwrap_or_else(PoisonError::into_inner);
            let running = match link.call(Command::List).map_err(NotStarted::Failed)? {
                Answer::Chats(listed) => listed
                    .iter()
                    .filter(|one| one.kind == Some(ProgramKind::Browser) && one.state != "ended")
                    .count(),
                other => {
                    return Err(NotStarted::Failed(format!(
                        "the session host answered a list with {other:?}"
                    )));
                }
            };
            let budget = (self.budget)();
            if running >= budget {
                return Err(NotStarted::Full { most: budget });
            }
            // Named here, so the engine download this window is told of is known to be this
            // start's, and waited for as long as the download may take (#2207).
            let named = purlis_core::browsers::a_browsers_id();
            // Waited on before it is asked for, so no word of its download comes too early.
            crate::enginedownload::here().started(&named);
            let start = Command::StartProgram(Box::new(StartProgram {
                program: Some(named.clone()),
                reach,
                kept,
                ..StartProgram::of(project, ProgramKind::Browser)
            }));
            // The host's first start downloads the engine inside it: waited for as long as that
            // may take, not the 30 seconds any other answer gets.
            let answered =
                link.call_refused_within(start, purlis_core::browsers::A_START_TAKES_AT_MOST);
            crate::enginedownload::here().finished(&named);
            match answered.map_err(|refused| not_started(refused, budget))? {
                Answer::ProgramStarted { program, .. } => {
                    self.follow_its_end(&link, &program);
                    if let Some(held) = self.projects().get_mut(root) {
                        held.browsers.push(program.clone());
                    }
                    Ok(program)
                }
                other => Err(NotStarted::Failed(format!(
                    "the session host answered a browser's start with {other:?}"
                ))),
            }
        }

        /// Attaches a view of `program` as `role`, its whole frames handed to `frames`, its
        /// page's origin to `origins` (#2300), and `link_said` told where it stands while the
        /// link to the runner it is drawn from is down and back (#2542).
        pub(crate) fn watch(
            &self,
            program: &str,
            role: BrowserViewRole,
            frames: FrameSink,
            origins: OriginSink,
            link_said: LinkSaid,
        ) -> Result<BrowserWatching, String> {
            let link = self.link()?;
            let role = match role {
                BrowserViewRole::Focused => ViewRole::Focused,
                BrowserViewRole::Thumbnail => ViewRole::Thumbnail,
            };
            let attached = match link.call(Command::AttachView {
                program: program.to_owned(),
                kind: ViewKind::Browser,
                role: Some(role),
                rate: None,
            })? {
                Answer::Attached(attached) => attached,
                other => {
                    return Err(format!(
                        "the session host answered a browser view with {other:?}"
                    ));
                }
            };
            // Followed before it draws, so its first frame finds it.
            self.follow(&link, attached.view, link_said);
            link.draw_frames(attached.view, frames, origins);
            // The lease is the host's: a view opened again says whether the person still drives.
            let person_drives = matches!(
                link.call(Command::List),
                Ok(Answer::Chats(listed)) if listed.iter().any(|one| {
                    one.chat == program && one.driver == Some(Driver::Person)
                })
            );
            // The person still drives it, as a window before this one said: kept, not said
            // again.
            if person_drives && self.driver_said(program) != Drives::Person {
                lock(&self.logged).insert(
                    program.to_owned(),
                    Logged {
                        driver: Drives::Person,
                        typed: false,
                    },
                );
            }
            Ok(BrowserWatching {
                view: attached.view,
                width: attached.cols,
                height: attached.rows,
                person_drives,
            })
        }

        /// **The person cancels the engine's download for `program`'s start** (#2207): nothing
        /// of it is kept, and the start is refused in words that say so.
        pub(crate) fn cancel_download(&self, program: &str) -> Result<(), String> {
            self.link()?
                .call(Command::CancelEngineDownload {
                    program: program.to_owned(),
                })
                .map(|_| ())
        }

        /// The person takes `program` over (#2014): the chat's acts are refused until they
        /// hand it back, and one in flight is cut.
        pub(crate) fn take_over(&self, program: &str) -> Result<(), String> {
            self.link()?.call(Command::TakeOver {
                program: program.to_owned(),
            })?;
            self.drove(program, Drives::Person);
            Ok(())
        }

        /// The person hands `program` back to its chat (#2014).
        pub(crate) fn hand_back(&self, program: &str) -> Result<(), String> {
            self.link()?.call(Command::HandBack {
                program: program.to_owned(),
            })?;
            self.drove(program, Drives::Agent);
            Ok(())
        }

        /// One event of the person's in `program`, which they drive (#2014).
        pub(crate) fn input(
            &self,
            program: &str,
            input: BrowserInput,
        ) -> Result<InputOutcome, String> {
            // A key pressed is typing, whatever it was: only that is said (#2293).
            let keyed = matches!(
                &input,
                BrowserInput::Key {
                    action: KeyAct::Down,
                    ..
                }
            );
            let Some(input) = input.wire() else {
                return Ok(InputOutcome::Missed);
            };
            let sent = self.link()?.call_refused_within(
                Command::BrowserInput {
                    program: program.to_owned(),
                    input,
                },
                LINK_WITHIN,
            );
            Ok(match sent {
                Ok(_) => {
                    if keyed {
                        self.typed(program, Typist::Person);
                    }
                    InputOutcome::Taken
                }
                Err(refusal) if refusal.code == session::INPUT_MISSED => InputOutcome::Missed,
                Err(refusal) => InputOutcome::Refused { why: refusal.why },
            })
        }

        /// **The person sends `program` to `address`** (#2176), an http or https address read
        /// before ([`purlis_core::browsers::typed`]), the browser reaching `reach` from then on.
        /// It is judged as an address the agent writes is: a site new to the project is held
        /// back, and the answer names it. A browser the person drives is handed back first,
        /// since a browser takes an address only from its driver's lane; their next click takes
        /// it over again. The current page goes there, or a new one where it has none.
        pub(crate) fn go(
            &self,
            program: &str,
            address: &str,
            reach: &BrowserReach,
        ) -> Result<Went, String> {
            let link = self.link()?;
            match link.call_refused_within(
                Command::HandBack {
                    program: program.to_owned(),
                },
                LINK_WITHIN,
            ) {
                Err(refusal) if refusal.code == session::NO_SUCH_CHAT => {
                    return Err(GONE.to_owned());
                }
                Ok(_) => self.drove(program, Drives::Agent),
                // Not driven by the person: nothing to hand back.
                Err(_) => {}
            }
            let act = |act: Act| -> Result<Browsed, String> {
                let command = Command::Browse {
                    program: program.to_owned(),
                    call_id: self.call_id(),
                    act,
                    reach: Some(reach.clone()),
                };
                match link.call_refused_within(command, AN_ACT_TAKES_AT_MOST) {
                    Ok(Answer::Browsed(browsed)) => Ok(*browsed),
                    Ok(other) => Err(format!(
                        "the session host answered a browser's act with {other:?}"
                    )),
                    Err(refusal)
                        if refusal.code == session::NO_SUCH_CHAT
                            || refusal.code == session::BROWSER_ENDED =>
                    {
                        Err(GONE.to_owned())
                    }
                    Err(refusal) => Err(refusal.why),
                }
            };
            let current = act(Act::Pages)?
                .pages
                .into_iter()
                .find(|page| page.current)
                .map(|page| page.page);
            let address = address.to_owned();
            let browsed = match current {
                Some(page) => act(Act::Navigate {
                    page: Some(page),
                    address,
                })?,
                None => act(Act::Open { address })?,
            };
            Ok(if browsed.asked.is_empty() {
                Went::Went {
                    address: browsed
                        .page
                        .map(|page| page.address)
                        .or_else(|| browsed.opened.last().map(|page| page.address.clone())),
                }
            } else {
                Went::Asks {
                    sites: browsed.asked,
                    answers: Vec::new(),
                }
            })
        }

        /// An id for one of the person's acts no other act of this window's has, nor one of
        /// an earlier window's on a browser it reattached: the host answers a repeated id as a
        /// retry, doing nothing again.
        fn call_id(&self) -> String {
            let n = self.acts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let since = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            format!("person-{since}-{n}")
        }

        /// The person sets `program`'s pages to `width` by `height` CSS pixels (#2014).
        pub(crate) fn size(&self, program: &str, width: u16, height: u16) -> Result<(), String> {
            self.link()?
                .call(Command::SizeBrowser {
                    program: program.to_owned(),
                    width,
                    height,
                })
                .map(|_| ())
        }

        /// Closes view `view` of `program`: its frames stop with the last view.
        pub(crate) fn unwatch(&self, program: &str, view: u32) -> Result<(), String> {
            let link = self.link()?;
            lock(&self.dimming).views.remove(&(link.id(), view));
            link.undraw(view);
            link.call(Command::DetachView {
                program: program.to_owned(),
                view,
            })
            .map(|_| ())
        }

        /// View `view` on `link` is told where it stands as `link` hears of its runner, from
        /// now on (#2542).
        fn follow(&self, link: &HostLink, view: u32, tell: LinkSaid) {
            let id = link.id();
            let first = {
                let mut dimming = lock(&self.dimming);
                // A view on a link the window's has replaced ended with that link.
                dimming.views.retain(|(on, _), _| *on == id);
                dimming.views.insert(
                    (id, view),
                    Followed {
                        tell,
                        dimmed: false,
                    },
                );
                dimming.heard.insert(id)
            };
            if first {
                let dimming = Arc::clone(&self.dimming);
                link.when_reconnecting(Arc::new(move |heard| hear(&dimming, id, heard)));
            }
        }

        /// What link `link` heard of the runner its views are drawn from (#2542).
        #[cfg(test)]
        pub(crate) fn heard(&self, link: u64, heard: Reconnect) {
            hear(&self.dimming, link, heard);
        }

        fn projects(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, Held>> {
            self.projects.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// Whether `program` is a browser this window started in the project at `root` (#2022).
        pub(crate) fn started_in(&self, root: &Path, program: &str) -> bool {
            self.projects()
                .get(root)
                .is_some_and(|held| held.browsers.iter().any(|one| one == program))
        }

        /// Ends `program` and forgets it.
        pub(crate) fn close(&self, program: &str) -> Result<(), String> {
            // Forgotten only once the host has ended it: one it could not be told of stays
            // listed, so letting go of its project and Stop all still end it.
            let closed = self.link()?.call_refused_within(
                Command::Close {
                    program: program.to_owned(),
                },
                LINK_WITHIN,
            );
            match closed {
                // Gone already: there is nothing left to end.
                Err(refusal) if refusal.code == session::NO_SUCH_CHAT => {}
                Err(refusal) => return Err(refusal.why),
                Ok(_) => {}
            }
            for held in self.projects().values_mut() {
                held.browsers.retain(|one| one != program);
            }
            lock(&self.logged).remove(program);
            Ok(())
        }

        /// The project at `root` is let go of (#2012): every browser the window started in it is
        /// ended, and the window's hold on it let go of. Answers how many browsers it ended.
        pub(crate) fn let_go(&self, root: &Path) -> usize {
            let Some(held) = self.projects().remove(root) else {
                return 0;
            };
            let Ok(link) = self.connection.of_project(root, Duration::ZERO) else {
                return 0;
            };
            let ended = held
                .browsers
                .into_iter()
                .filter(|program| {
                    link.call(Command::Close {
                        program: program.clone(),
                    })
                    .is_ok()
                })
                .count();
            if let Err(why) = link.call(Command::LetGoOfProject {
                project: held.project,
            }) {
                tracing::warn!("purlis: the session host kept a project its browsers left: {why}");
            }
            ended
        }

        /// Stops every browser the host runs, and answers how many: the kill switch's part.
        pub(crate) fn stop_every_browser(&self) -> usize {
            let Ok(link) = self.connection.wait(Duration::ZERO) else {
                return 0;
            };
            let Ok(Answer::Chats(programs)) = link.call(Command::List) else {
                return 0;
            };
            programs
                .into_iter()
                .filter(|program| program.kind == Some(ProgramKind::Browser))
                .filter(|browser| {
                    link.call(Command::Stop {
                        chat: browser.chat.clone(),
                    })
                    .is_ok()
                })
                .count()
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Each view drawn from `link` follows what it heard of its runner** (#2542): every one is
    /// dimmed while the link is down, every dimmed one is reconnected once it is back or the
    /// window cannot tell, and one alone once it draws again from a snapshot. Each is told once
    /// for each change, outside the hold.
    fn hear(dimming: &Mutex<Dimming>, link: u64, heard: Reconnect) {
        let (dim, now) = match heard {
            Reconnect::Started | Reconnect::KeyChanged => (true, Reconnection::Reconnecting),
            Reconnect::Over | Reconnect::Drawn(_) => (false, Reconnection::Reconnected),
        };
        let told: Vec<LinkSaid> = lock(dimming)
            .views
            .iter_mut()
            .filter(|((on, view), followed)| {
                *on == link
                    && followed.dimmed != dim
                    && !matches!(heard, Reconnect::Drawn(drawn) if drawn != *view)
            })
            .map(|(_, followed)| {
                followed.dimmed = dim;
                Arc::clone(&followed.tell)
            })
            .collect();
        for tell in told {
            tell(now);
        }
    }

    #[cfg(test)]
    mod tests;
}

/// The window's browsers, from the app's state.
#[cfg(unix)]
pub(crate) fn browsers(app: &tauri::AppHandle) -> Result<std::sync::Arc<WindowBrowsers>, String> {
    use tauri::Manager;
    app.try_state::<std::sync::Arc<WindowBrowsers>>()
        .map(|state| std::sync::Arc::clone(&state))
        .ok_or_else(|| "this window has no session host to run a browser in".to_owned())
}

/// Starts a browser in project `plane`, and answers its id. Refused while Stop all holds.
#[tauri::command]
#[specta::specta]
pub async fn open_browser(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<String, String> {
    if planes.is_stopped() {
        return Err(crate::sessions::STOPPED.to_owned());
    }
    let held = planes.held(&plane)?;
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("starting a browser", move || {
            // It reaches what the project's browsers reach, and asks of a new site (#2176).
            let reach = crate::chatbrowsers::person_reach(&held);
            let program = browsers.open_for_the_person(held.root(), Some(reach))?;
            // The person's own, noted so (#2025).
            let listing = held.browsers().listing();
            held.browsers().opened_by_the_person(&program);
            held.browsers_listed_since(listing);
            Ok(program)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, held);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// **The person sends browser `program` of project `plane` to the address they typed** (#2176),
/// from its view tab: http and https only, any other scheme refused in words, and an address
/// the browser's reach refuses outright said so before it loads. A site new to the project is
/// held back as an address the agent writes is, and the answer names it to ask about. Never a
/// chat's browser, which its chat drives. The window's alone (`ui::WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn go_browser(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    program: String,
    address: String,
) -> Result<Went, String> {
    let held = planes.held(&plane)?;
    // Never a chat's browser, in this project or any other the window holds.
    if planes
        .every_held()
        .iter()
        .any(|one| one.browsers().is_in_use(&program))
    {
        return Err(A_CHATS_BROWSER.to_owned());
    }
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        // Only one the window started in this project goes with this project's reach.
        if !browsers.started_in(held.root(), &program) {
            return Err(NOT_THIS_PROJECTS.to_owned());
        }
        crate::off_the_window("sending a browser to an address", move || {
            let address = purlis_core::browsers::typed::address_of(&address)?;
            let reach = crate::chatbrowsers::person_reach(&held);
            if let Some(why) = purlis_core::browsers::typed::refused_by(&reach, &address) {
                return Err(why);
            }
            let went = browsers.go(&program, &address, &reach)?;
            // What it held back waits on the person, and only what the ask names whole, with
            // the answers the scopes policy leaves open offer (#2176).
            Ok(match went {
                Went::Asks { sites, .. } => {
                    let (named, answers) =
                        crate::chatbrowsers::theirs_asked(&held, &program, &sites);
                    Went::Asks {
                        sites: named,
                        answers,
                    }
                }
                went @ Went::Went { .. } => {
                    held.browsers().theirs_asked(&program, &[]);
                    went
                }
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, held, program, address);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// What sending a browser the window did not start in this project to an address is refused
/// with (#2176).
pub const NOT_THIS_PROJECTS: &str = "This browser was not opened in this project from this \
                                     window: open one here from the palette to go somewhere.";

/// What sending a chat's browser to an address from the window is refused with (#2176).
pub const A_CHATS_BROWSER: &str = "This browser is a chat's, and goes where its chat sends it: \
                                   open a browser of your own from the palette to go somewhere.";

/// Attaches a browser view of `program` as `role`: each whole frame, a JPEG, is sent on
/// `frames` in base64 with what it shows (#2014); an empty one says the view has ended.
#[tauri::command]
#[specta::specta]
pub async fn watch_browser(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    program: String,
    role: BrowserViewRole,
    frames: Channel<BrowserFrame>,
) -> Result<BrowserWatching, String> {
    let projects = planes.every_held();
    #[cfg(unix)]
    {
        use base64::Engine as _;
        let browsers = browsers(&app)?;
        let said = frames.clone();
        let paged = frames.clone();
        crate::off_the_window("opening a browser view", move || {
            browsers
                .watch(
                    &program,
                    role,
                    Box::new(move |aim, image| {
                        // A window that has gone drops what it cannot take; the browser runs on.
                        let _ = frames.send(BrowserFrame {
                            aim: aim.unwrap_or_default(),
                            jpeg: base64::engine::general_purpose::STANDARD.encode(&image),
                            reconnection: None,
                            page: None,
                        });
                    }),
                    // Where its page is (#2300).
                    Box::new(move |origin| {
                        let _ = paged.send(BrowserFrame {
                            aim: String::new(),
                            jpeg: String::new(),
                            reconnection: None,
                            page: Some(PageAt { origin }),
                        });
                    }),
                    // Where it stands while its runner's link is down and back (#2542).
                    std::sync::Arc::new(move |reconnection| {
                        let _ = said.send(BrowserFrame {
                            aim: String::new(),
                            jpeg: String::new(),
                            reconnection: Some(reconnection),
                            page: None,
                        });
                    }),
                )
                .inspect(|watching| {
                    // The host's word that the person still drives it, after a crash of the window
                    // too: its rows say so (#2300).
                    if watching.person_drives {
                        crate::chatbrowsers::person_drove(&projects, &program, true);
                    }
                })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, role, frames, projects);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// Closes a browser view the window had open. The browser runs on.
#[tauri::command]
#[specta::specta]
pub async fn unwatch_browser(
    app: tauri::AppHandle,
    program: String,
    view: u32,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("closing a browser view", move || {
            browsers.unwatch(&program, view)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, view);
        Ok(())
    }
}

/// The person takes browser `program` over (#2014): its chat's browser tools are refused in
/// words until [`hand_back_browser`], and a call in flight is cut.
#[tauri::command]
#[specta::specta]
pub async fn take_over_browser(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    program: String,
) -> Result<(), String> {
    // An Ask that asked the person to take it over says from now on that they drive it (#2015).
    let projects = planes.every_held();
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("taking a browser over", move || {
            browsers.take_over(&program)?;
            crate::browserneedsyou::taken_over(&projects, &program);
            crate::chatbrowsers::person_drove(&projects, &program, true);
            Ok(())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, projects);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// The person hands browser `program` back to its chat (#2014): Done. An Ask that asked them
/// to take it over ends, and its chat is told they finished, never what they did (#2015).
#[tauri::command]
#[specta::specta]
pub async fn hand_back_browser(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    program: String,
) -> Result<(), String> {
    // The Ask that asked the person to take it over ends, and its chat is woken (#2015).
    let projects = planes.every_held();
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("handing a browser back", move || {
            browsers.hand_back(&program)?;
            crate::chatbrowsers::person_drove(&projects, &program, false);
            crate::browserneedsyou::handed_back(&projects, &program);
            Ok(())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, projects);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// One click, move, wheel or key of the person's in browser `program`, which they drive (#2014).
#[tauri::command]
#[specta::specta]
pub async fn browser_input(
    app: tauri::AppHandle,
    program: String,
    input: BrowserInput,
) -> Result<InputOutcome, String> {
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("sending input to a browser", move || {
            browsers.input(&program, input)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, input);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// The person sets browser `program`'s pages to `width` by `height` CSS pixels (#2014).
#[tauri::command]
#[specta::specta]
pub async fn size_browser(
    app: tauri::AppHandle,
    program: String,
    width: u16,
    height: u16,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("sizing a browser", move || {
            browsers.size(&program, width, height)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, width, height);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// Ends browser `program`: its engine and everything it started. Never a browser a chat uses,
/// which ends with its chat (#2024).
#[tauri::command]
#[specta::specta]
pub async fn close_browser(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    program: String,
) -> Result<(), String> {
    let held = planes.every_held();
    if let Some(why) =
        crate::chatbrowsers::closing_refused(held.iter().map(|one| one.browsers()), &program)
    {
        return Err(why);
    }
    // What it asked about waits no more (#2176).
    for one in &held {
        one.browsers().theirs_asked(&program, &[]);
    }
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("ending a browser", move || {
            browsers.close(&program)?;
            // A pinned one whose chat closed leaves its rows (#2022).
            for one in &held {
                let listing = one.browsers().listing();
                one.browsers().stopped_by_the_person(&program);
                one.browsers_listed_since(listing);
            }
            Ok(())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, held);
        Ok(())
    }
}

/// **The person hands browser `program` to chat `session` of project `plane`** (#2022): one
/// pinned there whose chat closed, or one they opened there that no chat uses. Its chat's browser
/// tools use it from their next call; answers what the chat calls it. The window's alone
/// (`ui::WINDOW_ONLY`): only the person gives a chat a browser.
#[tauri::command]
#[specta::specta]
pub async fn hand_browser_to_chat(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    program: String,
    session: u32,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    #[cfg(unix)]
    {
        let opened_here = browsers(&app)
            .map(|browsers| browsers.started_in(held.root(), &program))
            .unwrap_or(false);
        crate::off_the_window("handing a browser to a chat", move || {
            crate::chatbrowsers::handed(&held, &program, session, opened_here)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, held, program, session);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// The project at `root` is let go of: every browser the window started in it ends, and the
/// window's hold on it in the session host is let go of (#2012).
pub fn let_go_of(app: &tauri::AppHandle, root: &std::path::Path) {
    #[cfg(unix)]
    if let Ok(browsers) = browsers(app) {
        browsers.let_go(root);
    }
    #[cfg(not(unix))]
    let _ = (app, root);
}

/// Stop all's part for browsers: every browser the window's host runs is stopped. Answers how
/// many.
pub fn stop_every_browser(app: &tauri::AppHandle) -> usize {
    #[cfg(unix)]
    {
        use tauri::Manager;
        let stopped = browsers(app).map_or(0, |browsers| browsers.stop_every_browser());
        // The chats' browsers leave their rows and tabs (#2024), and every Ask about one ends
        // with it, each with an Update (#2015).
        if let Some(planes) = app.try_state::<crate::planes::Planes>() {
            planes.chat_browsers_stopped();
            crate::browserneedsyou::all_ended(&planes.every_held());
        }
        stopped
    }
    #[cfg(not(unix))]
    {
        let _ = app;
        0
    }
}

// A chat's browser end to end on a real engine (#2243).
#[cfg(all(test, unix))]
mod tests;
