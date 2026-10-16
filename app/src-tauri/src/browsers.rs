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

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

/// What a browser view is to the person: the view they are looking at, or a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BrowserViewRole {
    Focused,
    Thumbnail,
}

/// A browser view the window has open: its number, and the size of the page its frames are
/// painted at, in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct BrowserWatching {
    pub view: u32,
    pub width: u16,
    pub height: u16,
}

#[cfg(unix)]
pub(crate) use hosted::WindowBrowsers;

#[cfg(unix)]
mod hosted {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use purlis_session_protocol::browsing::{Act, Browsed, BrowserReach};
    use purlis_session_protocol::session::{
        self, Answer, Command, ProgramKind, StartProgram, ViewKind, ViewRole,
    };

    use super::{BrowserViewRole, BrowserWatching};
    use crate::hostedsessions::{Connection, FrameSink, HostLink};

    /// How long a browser command waits for the window's link to its host.
    const LINK_WITHIN: Duration = Duration::from_secs(30);

    /// How long a chat's act waits for its browser (#2013): a wait for text of a minute, and
    /// the page settling after it.
    const AN_ACT_TAKES_AT_MOST: Duration = Duration::from_secs(85);

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
        ) -> Result<Browsed, crate::chatbrowsers::Refused> {
            use crate::chatbrowsers::Refused;
            let link = self.link().map_err(Refused::Said)?;
            let command = Command::Browse {
                program: program.to_owned(),
                call_id: call_id.to_owned(),
                act,
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

        fn close(&self, program: &str) {
            if let Err(why) = WindowBrowsers::close(self, program) {
                tracing::warn!("purlis: a chat's browser was not ended: {why}");
            }
        }
    }

    /// The window's browsers, in its session host: the link they go over, and each project the
    /// window has held on it for them, by its root, with the browsers it started there.
    pub(crate) struct WindowBrowsers {
        connection: Arc<Connection>,
        projects: Mutex<HashMap<PathBuf, Held>>,
    }

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
            }
        }

        pub(crate) fn link(&self) -> Result<Arc<HostLink>, String> {
            self.connection.wait(LINK_WITHIN)
        }

        /// The link, with the project at `root` held on it, and the project's id.
        fn held(&self, root: &Path) -> Result<(Arc<HostLink>, String), String> {
            let link = self.link()?;
            let mut projects = self.projects();
            if let Some(held) = projects.get(root) {
                return Ok((link, held.project.clone()));
            }
            let project =
                purlis_core::scaffold::planefile::ensure_project_id(root).map_err(|why| {
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
            Ok(BrowserWatching {
                view: attached.view,
                width: attached.cols,
                height: attached.rows,
            })
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
            for held in self.projects().values_mut() {
                held.browsers.retain(|one| one != program);
            }
            self.link()?
                .call(Command::Close {
                    program: program.to_owned(),
                })
                .map(|_| ())
        }

        /// The project at `root` is let go of (#2012): every browser the window started in it is
        /// ended, and the window's hold on it let go of. Answers how many browsers it ended.
        pub(crate) fn let_go(&self, root: &Path) -> usize {
            let Some(held) = self.projects().remove(root) else {
                return 0;
            };
            let Ok(link) = self.connection.wait(Duration::ZERO) else {
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
/// `frames` in base64; an empty one says the view has ended.
#[tauri::command]
#[specta::specta]
pub async fn watch_browser(
    app: tauri::AppHandle,
    program: String,
    role: BrowserViewRole,
    frames: Channel<String>,
) -> Result<BrowserWatching, String> {
    #[cfg(unix)]
    {
        use base64::Engine as _;
        let browsers = browsers(&app)?;
        crate::off_the_window("opening a browser view", move || {
            browsers.watch(
                &program,
                role,
                Box::new(move |image| {
                    // A window that has gone drops what it cannot take; the browser runs on.
                    let _ = frames.send(base64::engine::general_purpose::STANDARD.encode(&image));
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

/// Ends browser `program`: its engine and everything it started.
#[tauri::command]
#[specta::specta]
pub async fn close_browser(app: tauri::AppHandle, program: String) -> Result<(), String> {
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
        browsers(app).map_or(0, |browsers| browsers.stop_every_browser())
    }
    #[cfg(not(unix))]
    {
        let _ = app;
        0
    }
}
