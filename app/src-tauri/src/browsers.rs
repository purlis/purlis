// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person's browsers, as the window opens and watches them (#2012, spec #2008).
//!
//! **A browser runs in the session host, never in the window.** The window asks its host to
//! start one in a project ([`open_browser`]), and draws it in a view tab: a browser view whose
//! whole frames the window decodes on a canvas ([`watch_browser`]). Frames are images handed
//! over as bytes, never an embedded page, so the window's content policy is unchanged.
//!
//! **Who opens one.** In this version the person, from the window: a browser is a program
//! started as the person, so these commands are the window's alone (`ui::WINDOW_ONLY`). Chats
//! get browser tools later (#2013).
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BrowserFrame {
    pub aim: String,
    pub jpeg: String,
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

    use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach, Driver};
    use purlis_session_protocol::session::{
        self, Answer, Command, ProgramKind, StartProgram, ViewKind, ViewRole,
    };

    use super::{BrowserInput, BrowserViewRole, BrowserWatching, InputOutcome};
    use crate::hostedsessions::{Connection, FrameSink, HostLink};

    /// How long a browser command waits for the window's link to its host.
    const LINK_WITHIN: Duration = Duration::from_secs(30);

    /// How long a chat's act waits for its browser (#2013): a wait for text of a minute, and
    /// the page settling after it.
    const AN_ACT_TAKES_AT_MOST: Duration = Duration::from_secs(85);

    /// How long a login purlis performs waits for its browser (#2016): its page, its form
    /// showing, and its password field going, each within its own limit.
    const A_LOGIN_TAKES_AT_MOST: Duration = Duration::from_secs(150);

    /// A chat's browsers run here (#2013): the window's session host.
    impl crate::chatbrowsers::Driver for WindowBrowsers {
        fn start(&self, root: &Path, reach: BrowserReach) -> Result<String, String> {
            self.open_reaching(root, Some(reach))
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
            match link.call_refused_within(command, A_LOGIN_TAKES_AT_MOST) {
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
            }
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
    }

    /// The window's browsers, in its session host: the link they go over, and each project the
    /// window has held for them, by its root, with the browsers it started there. A project is
    /// held on its root's own link, which its chats share (#2178).
    pub(crate) struct WindowBrowsers {
        connection: Arc<Connection>,
        projects: Mutex<HashMap<PathBuf, Held>>,
        ended: Ended,
    }

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
            }
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

        /// Starts a browser in the project at `root`, and answers its id.
        pub(crate) fn open(&self, root: &Path) -> Result<String, String> {
            self.open_reaching(root, None)
        }

        /// Starts a browser in the project at `root` reaching `reach` (#2013), or with none,
        /// nothing listed and every site new to the project refused; answers its id.
        pub(crate) fn open_reaching(
            &self,
            root: &Path,
            reach: Option<BrowserReach>,
        ) -> Result<String, String> {
            let (link, project) = self.held(root)?;
            match link.call(Command::StartProgram(Box::new(StartProgram {
                reach,
                ..StartProgram::of(project, ProgramKind::Browser)
            })))? {
                Answer::ProgramStarted { program, .. } => {
                    let ended = Arc::clone(&self.ended);
                    let named = program.clone();
                    link.when_it_ends(&program, Box::new(move |_| ended(named)));
                    if let Some(held) = self.projects().get_mut(root) {
                        held.browsers.push(program.clone());
                    }
                    Ok(program)
                }
                other => Err(format!(
                    "the session host answered a browser's start with {other:?}"
                )),
            }
        }

        /// Attaches a view of `program` as `role`, its whole frames handed to `frames`.
        pub(crate) fn watch(
            &self,
            program: &str,
            role: BrowserViewRole,
            frames: FrameSink,
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
            link.draw_frames(attached.view, frames);
            // The lease is the host's: a view opened again says whether the person still drives.
            let person_drives = matches!(
                link.call(Command::List),
                Ok(Answer::Chats(listed)) if listed.iter().any(|one| {
                    one.chat == program && one.driver == Some(Driver::Person)
                })
            );
            Ok(BrowserWatching {
                view: attached.view,
                width: attached.cols,
                height: attached.rows,
                person_drives,
            })
        }

        /// The person takes `program` over (#2014): the chat's acts are refused until they
        /// hand it back, and one in flight is cut.
        pub(crate) fn take_over(&self, program: &str) -> Result<(), String> {
            self.link()?
                .call(Command::TakeOver {
                    program: program.to_owned(),
                })
                .map(|_| ())
        }

        /// The person hands `program` back to its chat (#2014).
        pub(crate) fn hand_back(&self, program: &str) -> Result<(), String> {
            self.link()?
                .call(Command::HandBack {
                    program: program.to_owned(),
                })
                .map(|_| ())
        }

        /// One event of the person's in `program`, which they drive (#2014).
        pub(crate) fn input(
            &self,
            program: &str,
            input: BrowserInput,
        ) -> Result<InputOutcome, String> {
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
                Ok(_) => InputOutcome::Taken,
                Err(refusal) if refusal.code == session::INPUT_MISSED => InputOutcome::Missed,
                Err(refusal) => InputOutcome::Refused { why: refusal.why },
            })
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
            link.undraw(view);
            link.call(Command::DetachView {
                program: program.to_owned(),
                view,
            })
            .map(|_| ())
        }

        fn projects(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, Held>> {
            self.projects.lock().unwrap_or_else(PoisonError::into_inner)
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

    #[cfg(test)]
    mod tests;
}

/// The window's browsers, from the app's state.
#[cfg(unix)]
fn browsers(app: &tauri::AppHandle) -> Result<std::sync::Arc<WindowBrowsers>, String> {
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
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("starting a browser", move || browsers.open(&root)).await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, root);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

/// Attaches a browser view of `program` as `role`: each whole frame, a JPEG, is sent on
/// `frames` in base64 with what it shows (#2014); an empty one says the view has ended.
#[tauri::command]
#[specta::specta]
pub async fn watch_browser(
    app: tauri::AppHandle,
    program: String,
    role: BrowserViewRole,
    frames: Channel<BrowserFrame>,
) -> Result<BrowserWatching, String> {
    #[cfg(unix)]
    {
        use base64::Engine as _;
        let browsers = browsers(&app)?;
        crate::off_the_window("opening a browser view", move || {
            browsers.watch(
                &program,
                role,
                Box::new(move |aim, image| {
                    // A window that has gone drops what it cannot take; the browser runs on.
                    let _ = frames.send(BrowserFrame {
                        aim: aim.unwrap_or_default(),
                        jpeg: base64::engine::general_purpose::STANDARD.encode(&image),
                    });
                }),
            )
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program, role, frames);
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
    #[cfg(unix)]
    {
        let browsers = browsers(&app)?;
        crate::off_the_window("ending a browser", move || browsers.close(&program)).await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program);
        Ok(())
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
