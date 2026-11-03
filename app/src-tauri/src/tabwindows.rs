// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Tab windows** (#4768, ruled on #4701): a tab torn off into a bare window of its project,
//! holding the strip and panes but no sidebar. This is the Tauri side; what each window holds is
//! the core's (`Chats`, `purlis_core::reopen::layout`) and which window is which is
//! [`Showing`]'s.
//!
//! - **Move to new window** and **Move to window ▸** move a tab or a pane in one step of the
//!   core's ([`move_to_window`]), so a chat is never shown twice, and every window of the
//!   project is told what it holds now ([`LAYOUT_MOVED`]).
//! - **Closing a tab window** sends its tabs back to the project window, behind its front tab,
//!   and every chat goes on running ([`closing`]). Closing the project closes its tab windows
//!   and keeps their tabs, so they come back with it ([`project_closed`]).
//! - **A minimised or hidden window counts as nothing** on screen ([`looked_at_again`]).
//! - **Tab windows come back after a relaunch** ([`tab_windows_back`]), each in its place on
//!   screen, which is this machine's (`machine::Place`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::machine::{self, Place};
use purlis_core::reopen::layout::Side;
use tauri::{AppHandle, Manager, Runtime};

use crate::layout::LaidOut;
use crate::planes::{PlaneId, Planes, Showing, TabWindow, WrittenBehind};

/// The event a project's whole window set is sent when tabs moved between its windows: every
/// window draws its own share of the layout it carries ([`LayoutMoved`]).
pub const LAYOUT_MOVED: &str = "layout-moved";

/// What [`LAYOUT_MOVED`] carries: the project, and every tab of it as the core holds them now.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub(crate) struct LayoutMoved {
    pub plane: PlaneId,
    pub layout: LaidOut,
}

/// Where a move landed: the window's number in its project, and its label.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MovedTo {
    pub window: u32,
    pub label: String,
}

/// **Move to new window** (`to` null) or **Move to window ▸** (`to` a window of the project,
/// `0` the project window): the `tab`th tab of the asking window, or the pane at `pane` in it
/// (`a` and `b` from the root), goes there as that window's front tab. A new window is made for
/// it; a window there already comes forward. Every window of the project is told what it holds
/// now.
///
/// **Refused**, saying why and changing nothing, from a window that is not one of the
/// project's, for a tab or pane that is not there, an empty pane, or a window that is not the
/// project's. A window only ever moves its own tabs. Async, as [`crate::windows::move_projects`]
/// is, for its reason: making a window from a synchronous command deadlocks on Windows.
#[tauri::command]
#[specta::specta]
pub async fn move_to_window(
    app: AppHandle,
    window: tauri::Window,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    tab: u32,
    pane: Option<String>,
    to: Option<u32>,
) -> Result<MovedTo, String> {
    let showing = app.state::<Showing>();
    let from = showing
        .window_of(window.label(), &plane)
        .ok_or("This window does not show that project.")?;
    let held = planes.held(&plane)?;
    let nth = usize::try_from(tab).map_err(|_| "That tab is not open here.".to_owned())?;
    let moved = match pane.as_deref() {
        Some(path) => held
            .chats()
            .move_pane(from, nth, &Side::path_of(path), to)?,
        None => held.chats().move_tab(from, nth, to)?,
    };
    let label = match window_label(&showing, &plane, moved) {
        Some(label) => {
            crate::windows::raise(&app, &label);
            label
        }
        None => open_tab_window(
            &app,
            &showing,
            &planes,
            &plane,
            moved,
            window.current_monitor().ok().flatten(),
        )?,
    };
    tell_moved(&app, &plane, &held);
    close_unnamed(&app, &plane, &held);
    crate::dispatched::front_moved(&held);
    Ok(MovedTo {
        window: moved,
        label,
    })
}

/// The event a window is sent to show a chat it has in a pane: a press in another window of
/// the project landed on it ([`press_elsewhere`]).
pub const PRESS_HERE: &str = "press-here";

/// What [`PRESS_HERE`] carries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PressHere {
    pub plane: PlaneId,
    pub chat: u32,
}

/// **A press on chat `chat` in a project with tab windows** (#4768, ruled on #4701): where
/// another of the project's windows has it in a pane, that window comes forward and shows it
/// there, and its label is answered; a chat is never shown in two windows. Null where no other
/// window has it, and the press is the asking window's.
#[tauri::command]
#[specta::specta]
pub fn press_elsewhere(
    app: AppHandle,
    window: tauri::Window,
    showing: tauri::State<'_, Showing>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    chat: u32,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    let Some(number) = held.chats().window_naming(chat) else {
        return Ok(None);
    };
    if showing.window_of(window.label(), &plane) == Some(number) {
        return Ok(None);
    }
    let Some(label) = window_label(&showing, &plane, number) else {
        return Ok(None);
    };
    crate::windows::raise(&app, &label);
    let _ = tauri::Emitter::emit_to(
        &app,
        label.as_str(),
        PRESS_HERE,
        PressHere {
            plane: plane.clone(),
            chat,
        },
    );
    Ok(Some(label))
}

/// The label of window `number` of `plane`: the window holding it for `0`, else its tab window.
fn window_label(showing: &Showing, plane: &PlaneId, number: u32) -> Option<String> {
    if number == 0 {
        return showing.holder(plane);
    }
    showing.tab_window_label(plane, number)
}

/// What this window is a tab window of, and its number there: null for a window that is not
/// one. A tab window asks it once, as it first draws, and draws that project's strip and panes
/// with no sidebar.
#[tauri::command]
#[specta::specta]
pub fn tab_window_handed(
    window: tauri::Window,
    showing: tauri::State<'_, Showing>,
) -> Option<TabWindow> {
    showing.tab_window(window.label())
}

/// **The project window put back `plane`'s tabs** (#4768): each tab window its record names
/// and no window draws yet is made again, in its place on screen where this machine kept one.
/// Answers how many were made. Asked by the window holding the project, once it has drawn the
/// project's tabs; a window that does not hold it makes none.
#[tauri::command]
#[specta::specta]
pub async fn tab_windows_back(
    app: AppHandle,
    window: tauri::Window,
    showing: tauri::State<'_, Showing>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<u32, String> {
    if showing.window_of(window.label(), &plane) != Some(0) {
        return Ok(0);
    }
    let held = planes.held(&plane)?;
    let Some(layout) = held.chats().layout() else {
        return Ok(0);
    };
    let mut made = 0;
    for number in layout.tab_windows() {
        if showing.tab_window_label(&plane, number).is_some() {
            continue;
        }
        open_tab_window(
            &app,
            &showing,
            &planes,
            &plane,
            number,
            window.current_monitor().ok().flatten(),
        )?;
        made += 1;
    }
    Ok(made)
}

/// Makes tab window `number` of `plane`, in its place on screen where this machine kept one,
/// and answers its label. Filed in [`Showing`] before it exists, so its first ask finds it.
fn open_tab_window<R: Runtime>(
    app: &AppHandle<R>,
    showing: &Showing,
    planes: &Planes,
    plane: &PlaneId,
    number: u32,
    near: Option<tauri::Monitor>,
) -> Result<String, String> {
    let label = showing.fresh_label();
    showing.tab_window_made(&label, plane, number);
    let made = match crate::windows::make_split(app, &label, near) {
        Ok(made) => made,
        Err(why) => {
            showing.tab_window_gone(&label);
            return Err(why);
        }
    };
    let root = planes
        .held(plane)
        .ok()
        .map(|held| held.root().to_path_buf());
    let kept = planes
        .config()
        .zip(root)
        .and_then(|(config, root)| machine::read(config).store.place_of(&root, number).cloned());
    if let Some(place) = kept {
        let _ = made.set_size(tauri::LogicalSize::new(place.width, place.height));
        let _ = made.set_position(tauri::LogicalPosition::new(place.x, place.y));
    }
    crate::windows::tell_windows_changed(app, None);
    Ok(label)
}

/// Tells every window of `plane` what each holds now.
fn tell_moved<R: Runtime>(app: &AppHandle<R>, plane: &PlaneId, held: &crate::planes::Held) {
    let Some(layout) = held.chats().layout() else {
        return;
    };
    crate::windows::emit_for_plane(
        app,
        plane,
        LAYOUT_MOVED,
        LayoutMoved {
            plane: plane.clone(),
            layout: layout.into(),
        },
    );
}

/// **Window `label` is closing**: where it is a tab window, its tabs go back to its project
/// window, behind the front tab of each one's strip, every chat in them goes on running, and
/// its place is forgotten. Answers whether it was a tab window. Once: a close and the window's
/// end both come here, and only the first finds it.
pub fn closing<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    let Some(showing) = app.try_state::<Showing>() else {
        return false;
    };
    let Some(tab) = showing.tab_window_gone(label) else {
        return false;
    };
    if let Some(planes) = app.try_state::<Planes>()
        && let Ok(held) = planes.held(&tab.plane)
    {
        held.chats().send_back(tab.number);
        let root = held.root().to_path_buf();
        if let Some(places) = app.try_state::<Places>() {
            places
                .now
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&(root.clone(), tab.number));
        }
        if let Some(config) = planes.config() {
            let _ = machine::update(config, |store| store.forget_place(&root, tab.number));
        }
        tell_moved(app, &tab.plane, &held);
        crate::dispatched::front_moved(&held);
    }
    crate::windows::tell_windows_changed(app, Some(label));
    true
}

/// **A tab window goes with its last tab** (#4768): each of `plane`'s tab windows its layout no
/// longer names closes. It has nothing to send back.
pub fn close_unnamed<R: Runtime>(app: &AppHandle<R>, plane: &PlaneId, held: &crate::planes::Held) {
    let Some(showing) = app.try_state::<Showing>() else {
        return;
    };
    let named = held
        .chats()
        .layout()
        .map(|layout| layout.tab_windows())
        .unwrap_or_default();
    for (label, number) in showing.tab_windows_of(plane) {
        if named.contains(&number) {
            continue;
        }
        showing.tab_window_gone(&label);
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
}

/// **`plane` closed**: each of its tab windows closes too, and its tabs stay in the record as
/// they were, so they come back when the project is opened again.
pub fn project_closed<R: Runtime>(app: &AppHandle<R>, plane: &PlaneId) {
    let Some(showing) = app.try_state::<Showing>() else {
        return;
    };
    for (label, _) in showing.tab_windows_of(plane) {
        // Forgotten first, so its end sends nothing back to a project that is gone.
        showing.tab_window_gone(&label);
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
}

/// **Window `window` may have gone from the screen or come back**: minimised, hidden, shown or
/// restored. Each project it draws is told whether that window is away, and where that changes
/// what is on screen, its held tasks are looked at again: a minimised window counts as nothing.
pub fn looked_at_again<R: Runtime>(window: &tauri::Window<R>) {
    let app = window.app_handle();
    let (Some(showing), Some(planes)) = (app.try_state::<Showing>(), app.try_state::<Planes>())
    else {
        return;
    };
    let away = window.is_minimized().unwrap_or(false) || !window.is_visible().unwrap_or(true);
    let drawn: Vec<(PlaneId, u32)> = match showing.tab_window(window.label()) {
        Some(tab) => vec![(tab.plane, tab.number)],
        None => showing
            .holding(window.label())
            .map(|holding| holding.planes.into_iter().map(|plane| (plane, 0)).collect())
            .unwrap_or_default(),
    };
    for (plane, number) in drawn {
        if let Ok(held) = planes.held(&plane)
            && held.chats().window_away(number, away)
        {
            crate::dispatched::front_moved(&held);
        }
    }
}

/// **Where each tab window is on screen, as it moves** (#4768): kept for a relaunch, in this
/// machine's store, written behind so a drag is not a write per frame.
pub struct Places {
    now: Mutex<HashMap<(PathBuf, u32), Place>>,
    written: Arc<WrittenBehind<(PathBuf, Vec<Place>)>>,
}

impl Default for Places {
    fn default() -> Self {
        Self {
            now: Mutex::default(),
            written: WrittenBehind::new(|(config, places): (PathBuf, Vec<Place>)| {
                if let Err(why) = machine::update(&config, move |store| {
                    for place in places {
                        store.keep_place(place);
                    }
                }) && why.kind() != std::io::ErrorKind::Unsupported
                {
                    tracing::warn!(
                        "purlis: where a tab window is was not written down ({why}); the next \
                         launch opens it where a new window opens"
                    );
                }
            }),
        }
    }
}

/// **Tab window `window` moved or changed size**: its place is kept for a relaunch.
pub fn moved<R: Runtime>(window: &tauri::Window<R>) {
    let app = window.app_handle();
    let (Some(showing), Some(planes), Some(places)) = (
        app.try_state::<Showing>(),
        app.try_state::<Planes>(),
        app.try_state::<Places>(),
    ) else {
        return;
    };
    let Some(tab) = showing.tab_window(window.label()) else {
        return;
    };
    let (Some(config), Ok(held)) = (planes.config(), planes.held(&tab.plane)) else {
        return;
    };
    let Ok(scale) = window.scale_factor() else {
        return;
    };
    let (Ok(at), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    let at = at.to_logical::<i32>(scale);
    let size = size.to_logical::<u32>(scale);
    let root = held.root().to_path_buf();
    let place = Place {
        plane: root.clone(),
        window: tab.number,
        x: at.x,
        y: at.y,
        width: size.width,
        height: size.height,
    };
    let all: Vec<Place> = {
        let mut now = places.now.lock().unwrap_or_else(PoisonError::into_inner);
        now.insert((root, tab.number), place);
        now.values().cloned().collect()
    };
    places.written.hand((config.to_path_buf(), all));
}
