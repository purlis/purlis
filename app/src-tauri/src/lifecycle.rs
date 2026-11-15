// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the app does when its window closes, when something asks it to quit, and when it is
//! started a second time.
//!
//! The shape of it: closing the window only hides it, because every session is a child of
//! this process and dies with it (ADR 0025) — a close that ended them would end the day's
//! work. Quitting goes to the window first, so the operator sees what is about to be ended,
//! and only the window's answer exits.
//!
//! **While something keeps the session host running, Quit goes to the background** (FD-17,
//! #655; ADR 0068 §2, X24): a trigger or a paired device needs the host, and the person needs to
//! see that it runs and how to stop it. So Quit ends nothing: every window is hidden, the Dock
//! icon goes, and the tray says in plain words why purlis keeps running, with **Open purlis**
//! and **Stop purlis**, which is a quit as it always was ([`what_quit_does`], [`Lifetime`]).
//! With nothing keeping the host, Quit is what it always was.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use std::sync::PoisonError;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use tauri::{AppHandle, Emitter, Manager, Runtime, Wry};

/// The event the window is sent when something asks the app to quit. The window answers by
/// calling the `quit` command, or by not calling it.
pub const QUIT_ASKED: &str = "quit-asked";

/// The event the window is sent when the menu's Settings… is chosen (SE-16; charter-app#283 as
/// Preferences…). The window opens its Settings tab, which is the palette row's verb too.
pub const SETTINGS_ASKED: &str = "settings-asked";

/// The main window: the one the tray and the dock bring back, and the one a quit is asked in.
/// A split window (`windows.rs`) is never hidden — its close hands its projects back here — so
/// this is the only window this module ever has to bring back.
pub const WINDOW: &str = crate::windows::MAIN;

/// The ids of the things that can be clicked. Written once here so the handler and the menu
/// cannot drift apart.
const QUIT: &str = "quit";
const SHOW: &str = "show";
const SETTINGS: &str = "settings";
/// The tray's Stop, while purlis runs in the background: a quit as it always was.
const STOP: &str = "stop";
/// The tray's line saying why purlis runs in the background. Never clicked.
const KEPT_FOR: &str = "kept-for";

/// The argument the login item starts the app with: in the background, with no window shown
/// (`purlis_core::loginitem::app_plist`, `app_autostart`).
pub const IN_THE_BACKGROUND: &str = "--in-the-background";

/// The tray's tooltip while purlis runs in the background.
const IN_THE_BACKGROUND_TIP: &str = "purlis is running in the background";

/// What a quit does this time (FD-17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuitDoes {
    /// Every chat ends and the app exits, as it always has.
    Exit,
    /// Something keeps the session host running, which `kept_for` says in the window's words:
    /// purlis goes to the background and ends nothing.
    ToTheBackground { kept_for: String },
    /// Something keeps the host running, but no tray shows here (a desktop with no status
    /// area, such as GNOME without an extension for one): hiding every window would leave no
    /// way back and no way to stop. The window stays, and says why, with Stop.
    StaysOpen { kept_for: String },
}

/// **What a quit does**: to the background while something keeps the host running
/// (`kept_for`, `purlis_core::loginitem::kept_for`) and a tray shows to say so and to stop it;
/// the window stays where no tray shows; and a quit whenever the person chose Stop, whatever
/// keeps the host.
pub fn what_quit_does(kept_for: Option<String>, stopping: bool, tray: bool) -> QuitDoes {
    match kept_for {
        Some(kept_for) if !stopping && tray => QuitDoes::ToTheBackground { kept_for },
        Some(kept_for) if !stopping => QuitDoes::StaysOpen { kept_for },
        _ => QuitDoes::Exit,
    }
}

/// **What an exit asked of the app's event loop does** (FD-17): only an exit nobody named a
/// code for (every window gone) goes to the background. An exit with a code is the app's own
/// and always goes ahead: Stop, restart to update (`updates.rs`, Tauri's `RESTART_EXIT_CODE`),
/// the restart onto the session bus (`portal.rs`). A logout or a shutdown on macOS ends the
/// event loop without asking (`applicationWillTerminate`), so nothing here can hold one up.
/// Whether the exit is held off: purlis went to the background instead.
pub fn on_exit_requested<R: Runtime>(app: &AppHandle<R>, code: Option<i32>) -> bool {
    if code.is_some() {
        return false;
    }
    match app
        .try_state::<Lifetime>()
        .map(|lifetime| lifetime.quit_does())
    {
        Some(QuitDoes::ToTheBackground { kept_for }) => {
            to_the_background(app, kept_for);
            true
        }
        // With no tray, nothing may be left hidden, so the exit goes ahead.
        Some(QuitDoes::StaysOpen { .. } | QuitDoes::Exit) | None => false,
    }
}

/// One entry of the tray's menu.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayItem {
    id: &'static str,
    label: String,
    enabled: bool,
}

/// **The tray's menu**: Show and Quit while the app runs as it always has; while it runs in the
/// background, why it does, in plain words, then Open purlis and Stop purlis, which says what
/// stopping costs.
fn tray_items(background: Option<&str>) -> Vec<TrayItem> {
    if let Some(kept_for) = background {
        return vec![
            TrayItem {
                id: KEPT_FOR,
                label: format!("Running in the background for {kept_for}"),
                enabled: false,
            },
            TrayItem {
                id: SHOW,
                label: "Open purlis".into(),
                enabled: true,
            },
            TrayItem {
                id: STOP,
                label: "Stop purlis and end every chat".into(),
                enabled: true,
            },
        ];
    }
    vec![
        TrayItem {
            id: SHOW,
            label: "Show purlis".into(),
            enabled: true,
        },
        TrayItem {
            id: QUIT,
            label: "Quit purlis".into(),
            enabled: true,
        },
    ]
}

/// **Whether purlis runs in the background, and what keeps it there** (FD-17): managed state.
pub struct Lifetime {
    /// The person chose Stop: the next exit is a quit, whatever keeps the host running.
    stopping: std::sync::atomic::AtomicBool,
    /// Whether a tray shows here, to say purlis runs in the background and to stop it. None
    /// until the tray is made and found shown.
    tray: std::sync::atomic::AtomicBool,
    /// Why purlis runs in the background, while it does.
    background: Mutex<Option<String>>,
    /// What keeps the host running now, in the window's words.
    kept_for: Box<dyn Fn() -> Option<String> + Send + Sync>,
}

impl Lifetime {
    pub fn new(kept_for: impl Fn() -> Option<String> + Send + Sync + 'static) -> Lifetime {
        Lifetime {
            stopping: std::sync::atomic::AtomicBool::new(false),
            tray: std::sync::atomic::AtomicBool::new(false),
            background: Mutex::new(None),
            kept_for: Box::new(kept_for),
        }
    }

    /// What a quit does now.
    pub fn quit_does(&self) -> QuitDoes {
        what_quit_does(
            (self.kept_for)(),
            self.stopping.load(std::sync::atomic::Ordering::SeqCst),
            self.tray.load(std::sync::atomic::Ordering::SeqCst),
        )
    }

    /// Whether a tray shows here: said once the tray is made.
    pub fn tray_shown(&self, shown: bool) {
        self.tray.store(shown, std::sync::atomic::Ordering::SeqCst);
    }

    fn stop(&self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn background(&self) -> Option<String> {
        self.background
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set_background(&self, kept_for: Option<String>) {
        *self
            .background
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = kept_for;
    }
}

/// **purlis goes to the background** (FD-17): every window hidden, no Dock icon, and the tray
/// saying why it keeps running. Nothing ends.
pub fn to_the_background<R: Runtime>(app: &AppHandle<R>, kept_for: String) {
    tracing::info!("purlis: quit goes to the background: it keeps running for {kept_for}");
    for (label, window) in app.windows() {
        if crate::windows::is_charter_window(&label) {
            let _ = window.hide();
        }
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    if let Some(lifetime) = app.try_state::<Lifetime>() {
        lifetime.set_background(Some(kept_for.clone()));
    }
    // So the host starts at the next login, and the app in the background with it.
    #[cfg(unix)]
    if let Some(host) = app.try_state::<crate::sessionhost::SessionHost>() {
        host.keep_login_items();
    }
    redraw_tray(app, Some(&kept_for));
}

/// **purlis comes back from the background**: the Dock icon again, the window shown, and the
/// tray as it always is.
pub fn from_the_background<R: Runtime>(app: &AppHandle<R>) {
    let was = app
        .try_state::<Lifetime>()
        .and_then(|lifetime| lifetime.background());
    if was.is_some() {
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        if let Some(lifetime) = app.try_state::<Lifetime>() {
            lifetime.set_background(None);
        }
        redraw_tray(app, None);
    }
}

/// The tray's menu and tooltip, for running in the background or not.
fn redraw_tray<R: Runtime>(app: &AppHandle<R>, background: Option<&str>) {
    let Some(tray) = app.tray_by_id(TRAY) else {
        return;
    };
    match tray_menu(app, background) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(why) => tracing::warn!("purlis: the tray's menu was not drawn again: {why}"),
    }
    let _ = tray.set_tooltip(Some(if background.is_some() {
        IN_THE_BACKGROUND_TIP
    } else {
        "purlis"
    }));
}

fn tray_menu<R: Runtime>(app: &AppHandle<R>, background: Option<&str>) -> tauri::Result<Menu<R>> {
    let items = tray_items(background)
        .into_iter()
        .map(|item| MenuItem::with_id(app, item.id, item.label, item.enabled, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;
    let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<R>)
        .collect();
    Menu::with_items(app, &refs)
}

/// The tray icon's id.
const TRAY: &str = "charter";

/// **Stop**: a quit, whatever keeps the host running. Every chat ends, and the host with them.
/// The window is not asked: what offers Stop says what it ends.
fn stop<R: Runtime>(app: &AppHandle<R>) {
    if let Some(lifetime) = app.try_state::<Lifetime>() {
        lifetime.stop();
    }
    app.exit(0);
}

/// What the window says when a quit cannot go to the background for want of a tray: its title,
/// what happened and what to do, Stop's label and the label that keeps purlis open.
fn stays_open_words(kept_for: &str) -> [String; 4] {
    [
        "purlis can't run in the background here".into(),
        format!(
            "purlis keeps running for {kept_for}, but this desktop has no tray to show it in, \
             so its window stays open. To end every chat and stop purlis, choose Stop purlis."
        ),
        "Stop purlis".into(),
        "Keep purlis open".into(),
    ]
}

/// **A quit with no tray to go to** (FD-17): nothing is hidden, since there would be no way
/// back and no way to stop. The window is shown and says why, offering Stop.
fn stays_open<R: Runtime>(app: &AppHandle<R>, kept_for: &str) {
    use tauri_plugin_dialog::{DialogExt as _, MessageDialogButtons, MessageDialogKind};
    tracing::info!("purlis: no tray shows here, so a quit keeps the window open for {kept_for}");
    show(app);
    let [title, said, stop_label, keep_label] = stays_open_words(kept_for);
    let parent = app.get_webview_window(WINDOW);
    let mut dialog = app
        .dialog()
        .message(said)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(stop_label, keep_label));
    if let Some(parent) = parent.as_ref() {
        dialog = dialog.parent(parent);
    }
    let app = app.clone();
    dialog.show(move |stopping| {
        if stopping {
            stop(&app);
        }
    });
}

/// **Whether a tray shows on this desktop**, once the tray is made. macOS always draws one in
/// the menu bar. On Linux the tray is a status notifier item, which only shows where something
/// on the session bus watches for them (`org.kde.StatusNotifierWatcher`): KDE, most other
/// desktops, and GNOME only with an extension.
pub fn a_tray_shows_here() -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(bus) = zbus::blocking::Connection::session() else {
            return false;
        };
        let Ok(dbus) = zbus::blocking::fdo::DBusProxy::new(&bus) else {
            return false;
        };
        let Ok(name) = zbus::names::BusName::try_from("org.kde.StatusNotifierWatcher") else {
            return false;
        };
        dbus.name_has_owner(name).unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// How long an unanswered ask keeps the next one armed to quit outright.
///
/// It expires, and that is the point. Without a deadline one `quit-asked` the window never
/// received — pressed while it was still starting, say — would leave the app armed for the
/// rest of the day, and the next Cmd-Q would end every session with no warning at all.
/// Pressing quit twice in a few seconds is someone insisting; pressing it again an hour
/// later is someone quitting.
const INSISTING: Duration = Duration::from_secs(5);

/// Whether the window has already been asked to quit, and when.
///
/// A second ask while the first is unanswered exits without waiting. This is the way out of
/// a window that cannot answer — a wedged webview, a renderer that crashed — which would
/// otherwise leave an app that can only be killed. Pressing quit twice is a thing people
/// already do to an app that seems not to have heard.
#[derive(Default)]
pub struct Quitting {
    asked: Mutex<Option<Instant>>,
}

/// What an ask to quit means this time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// The window is asked, and answers by calling `quit` or by leaving it.
    AskTheWindow,
    /// It has been asked once and has not answered. Go.
    QuitAnyway,
}

impl Quitting {
    pub fn ask(&self) -> Ask {
        self.asked_at(Instant::now())
    }

    /// `ask`, at a moment a test can choose.
    fn asked_at(&self, now: Instant) -> Ask {
        let mut asked = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        let insisting = asked.is_some_and(|before| now.duration_since(before) < INSISTING);
        *asked = Some(now);
        if insisting {
            Ask::QuitAnyway
        } else {
            Ask::AskTheWindow
        }
    }

    /// The window answered "not now", so the next ask is a first ask again.
    pub fn never_mind(&self) {
        *self.asked.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// Brings the window back: shown, unminimised, and in front.
///
/// Each step can fail on its own and none of them is worth refusing to continue over — a
/// window that is shown but not focused is still the window back.
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    from_the_background(app);
    if let Some(window) = app.get_window(WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Asks the window to quit, or quits.
pub fn ask_to_quit<R: Runtime>(app: &AppHandle<R>) {
    // Nothing ends while something keeps the host running, so nothing is asked.
    match app
        .try_state::<Lifetime>()
        .map(|lifetime| lifetime.quit_does())
    {
        Some(QuitDoes::ToTheBackground { kept_for }) => {
            to_the_background(app, kept_for);
            return;
        }
        Some(QuitDoes::StaysOpen { kept_for }) => {
            stays_open(app, &kept_for);
            return;
        }
        Some(QuitDoes::Exit) | None => {}
    }
    match app.state::<Quitting>().ask() {
        Ask::AskTheWindow => {
            // Shown first: the ask is a dialog in the window, and a hidden window would put
            // the question somewhere nobody is looking.
            show(app);
            let _ = app.emit_to(WINDOW, QUIT_ASKED, ());
        }
        Ask::QuitAnyway => app.exit(0),
    }
}

/// The menu-bar glyph: charter's mark redrawn as a macOS template image, and NOT the app
/// icon.
///
/// A template is drawn from its alpha channel alone — macOS throws the colours away and
/// paints the shape black on a light menu bar, white on a dark one, white again when the
/// menu is open. The app icon is full-bleed by design (#159, so the small sizes are not
/// shrunk by a margin only macOS wants), which means its alpha is the whole rectangle, so
/// handing it to a template is how the tray became a solid black square (#202).
///
/// `icons/tray-template.png` is the mark's winged quill in opaque black on transparent,
/// 42x36, redrawn from `icons/source-1024.png` for the size it is drawn at rather than
/// resized to it. tray-icon gives the tray's image a height of 18pt whatever its pixels are
/// (tray-icon 0.24.2, `platform_impl/macos`), so 36px is the Retina pixel size and the
/// glyph inside it lands at 15pt, the air Apple's own menu-bar icons leave.
///
/// The brackets around the quill are dropped, which is the one thing here that is a taste
/// call. Keeping them costs the bird a fifth of its height and takes the wing's feather
/// slots — 18px of a 1024 source — to 1.3px on a Retina menu bar and 0.7px off one, which
/// is a grey smudge inside a box. A menu bar wants a glyph, not a logo.
///
/// It is compiled in rather than read from disk: `include_image!` decodes it at build time,
/// so there is no file for an installer to lose and no runtime failure to handle.
const MENU_BAR_GLYPH: Image<'static> = tauri::include_image!("icons/tray-template.png");

/// What the tray is drawn from, and whether macOS is to treat it as a template.
struct TrayIcon {
    image: Image<'static>,
    as_template: bool,
}

/// Which image the tray gets. `macos` is passed in rather than read from `cfg!` inside, so
/// both answers can be tested from whichever platform the suite happens to run on.
///
/// macOS gets the template glyph and nothing else: a menu bar recolours what it draws, and
/// a coloured icon there is either invisible or a rectangle. Every other platform gets the
/// app's own icon in its own colours, which is what their trays have always shown and what
/// `icon_as_template` means nothing to. `None` is a tray without an icon, not a panic: the
/// app icon is bundled, but a build that lost it still has sessions to give back.
fn tray_icon(macos: bool, app_icon: Option<Image<'static>>) -> Option<TrayIcon> {
    if macos {
        return Some(TrayIcon {
            image: MENU_BAR_GLYPH,
            as_template: true,
        });
    }
    app_icon.map(|image| TrayIcon {
        image,
        as_template: false,
    })
}

/// The tray icon: what the app is while its window is hidden, and what brings it back.
pub fn tray(app: &AppHandle) -> tauri::Result<()> {
    let background = app
        .try_state::<Lifetime>()
        .and_then(|lifetime| lifetime.background());
    let menu = tray_menu(app, background.as_deref())?;

    let mut tray = TrayIconBuilder::with_id(TRAY)
        .tooltip(if background.is_some() {
            IN_THE_BACKGROUND_TIP
        } else {
            "purlis"
        })
        .menu(&menu)
        // The menu is for the right button. A left click is "give me the window back", which
        // is what the icon is mostly there for.
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show(tray.app_handle());
            }
        });
    // Owned, because the tray outlives this call while the bundled icon is borrowed from
    // the app handle.
    let app_icon = app
        .default_window_icon()
        .map(|icon| icon.clone().to_owned());
    match tray_icon(cfg!(target_os = "macos"), app_icon) {
        Some(icon) => tray = tray.icon(icon.image).icon_as_template(icon.as_template),
        // Said out loud, because a tray with no icon is a tray that is hard to find, and
        // reaching the window from the dock is easier than hunting for an empty slot.
        None => tracing::warn!("purlis: the tray has no icon; its menu is still on the click"),
    }
    tray.build(app)?;
    Ok(())
}

/// One entry of the application menu, before it is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    /// charter's own Quit, with the accelerator it carries.
    Quit(&'static str),
    /// Settings… (SE-16; charter-app#283 as Preferences…), with the accelerator it carries: the
    /// Settings tab, which opens at the focused level (SE-23) — the focused workspace's, else
    /// the project's, else You.
    Settings(&'static str),
    Hide,
    Separator,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// The application menu's submenus, by title, for macOS or for everything else.
///
/// **On macOS it is the `⌘` menu every Mac app has**, and it has to be: without an Edit menu
/// carrying the predefined items, `⌘C` and `⌘V` do nothing in a text field. No `⌘` chord is a
/// key a terminal encodes, so nothing here is taken from a chat.
///
/// **Everywhere else, only Quit, on `Ctrl+Shift+Q` (#187).** muda's predefined items there are
/// `Ctrl` chords it does not let charter change, and each is a key the chat's shell owns: Copy
/// is `Ctrl-C`, the interrupt; Undo `Ctrl-Z`, suspend; Select all `Ctrl-A`, beginning of line;
/// Hide `Ctrl-H`, backspace. A terminal app keeps its own keys off them by adding `Shift`
/// (GNOME Terminal and Konsole: `Ctrl+Shift+C`, `V`, `Q`), which xterm encodes as nothing. The
/// Edit items could not be moved there — a predefined item's accelerator is fixed — so they are
/// left out rather than left on the chat's keys. A text field in the webview keeps its own
/// `Ctrl+C` and `Ctrl+V`, which the webview handles without any menu.
///
/// **Settings… is on both, on `⌘,` and `Ctrl+,`** (charter-app#283, SE-16) — the key every Mac app
/// opens its settings on, and VS Code's everywhere else. Neither takes anything from a chat:
/// xterm.js 6.0.0 sends nothing for a `⌘` chord but `⌘A`, and with `Ctrl` it encodes a letter,
/// space, `3`–`8`, `[`, `\` and `]` — not `,`.
///
/// What leaving them out costs, **read from muda 0.19.3's source and not measured in a window**
/// (nobody here has a Linux or Windows desktop to press the key in): on Windows the Edit items
/// were real accelerators in the window's accelerator table, so there they really did take the
/// key before the webview saw it, and the menu loses Undo to Select all. On Linux
/// (`platform_impl/gtk`) Copy, Cut, Paste and Select all draw the chord as a label only and act
/// through `libxdo` when clicked, a feature this build does not enable, and Undo, Redo and Hide
/// are not drawn at all — so there the Edit menu did little but claim keys it did not use.
fn layout(macos: bool) -> Vec<(&'static str, Vec<Item>)> {
    if !macos {
        return vec![(
            "purlis",
            vec![
                Item::Settings("Ctrl+,"),
                Item::Separator,
                Item::Quit("Ctrl+Shift+Q"),
            ],
        )];
    }
    vec![
        (
            "purlis",
            vec![
                Item::Settings("CmdOrCtrl+,"),
                Item::Separator,
                Item::Hide,
                Item::Separator,
                Item::Quit("CmdOrCtrl+Q"),
            ],
        ),
        // Copy, paste and select-all are the predefined items a text field needs to behave;
        // on macOS nothing works without an Edit menu carrying them.
        (
            "Edit",
            vec![
                Item::Undo,
                Item::Redo,
                Item::Separator,
                Item::Cut,
                Item::Copy,
                Item::Paste,
                Item::SelectAll,
            ],
        ),
    ]
}

/// The application menu, which is where Quit lives on every platform.
///
/// Tauri's own default menu has a predefined Quit that ends the process where it is clicked.
/// This replaces it with one of charter's own, so that quitting goes through the window and
/// the operator is told what is about to be ended.
pub fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let mut submenus = Vec::new();
    for (title, items) in layout(cfg!(target_os = "macos")) {
        let mut built: Vec<Box<dyn tauri::menu::IsMenuItem<Wry>>> = Vec::new();
        for item in items {
            built.push(match item {
                Item::Quit(accelerator) => Box::new(MenuItem::with_id(
                    app,
                    QUIT,
                    "Quit purlis",
                    true,
                    Some(accelerator),
                )?),
                Item::Settings(accelerator) => Box::new(MenuItem::with_id(
                    app,
                    SETTINGS,
                    "Settings…",
                    true,
                    Some(accelerator),
                )?),
                Item::Hide => Box::new(PredefinedMenuItem::hide(app, None)?),
                Item::Separator => Box::new(PredefinedMenuItem::separator(app)?),
                Item::Undo => Box::new(PredefinedMenuItem::undo(app, None)?),
                Item::Redo => Box::new(PredefinedMenuItem::redo(app, None)?),
                Item::Cut => Box::new(PredefinedMenuItem::cut(app, None)?),
                Item::Copy => Box::new(PredefinedMenuItem::copy(app, None)?),
                Item::Paste => Box::new(PredefinedMenuItem::paste(app, None)?),
                Item::SelectAll => Box::new(PredefinedMenuItem::select_all(app, None)?),
            });
        }
        let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = built.iter().map(|b| &**b).collect();
        submenus.push(Submenu::with_items(app, title, true, &refs)?);
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = submenus
        .iter()
        .map(|s| s as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    Menu::with_items(app, &refs)
}

/// What a click on one of charter's own menu items does.
pub fn clicked<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        QUIT => ask_to_quit(app),
        SHOW => show(app),
        // Stop is a quit, whatever keeps the host running: every chat ends, and the host with
        // them. The window is not asked: the menu item says what it ends.
        STOP => stop(app),
        // Shown first, for Quit's reason: the tab opens in the window, and a hidden window
        // would put it where nobody is looking.
        //
        // To the window the operator is in when there is one: with a project split into a
        // window of its own, `⌘,` pressed there is about that window.
        SETTINGS => {
            let focused = app
                .windows()
                .into_iter()
                .find(|(label, window)| {
                    crate::windows::is_charter_window(label) && window.is_focused().unwrap_or(false)
                })
                .map(|(label, _)| label);
            match focused {
                Some(label) => {
                    let _ = app.emit_to(label.as_str(), SETTINGS_ASKED, ());
                }
                None => {
                    show(app);
                    let _ = app.emit_to(WINDOW, SETTINGS_ASKED, ());
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_nothing_keeping_the_host_quit_ends_every_chat_as_it_always_has() {
        assert_eq!(what_quit_does(None, false, true), QuitDoes::Exit);
        assert_eq!(what_quit_does(None, false, false), QuitDoes::Exit);
    }

    #[test]
    fn with_a_trigger_or_a_device_quit_goes_to_the_background() {
        assert_eq!(
            what_quit_does(Some("your trigger “nightly”".into()), false, true),
            QuitDoes::ToTheBackground {
                kept_for: "your trigger “nightly”".into()
            }
        );
    }

    #[test]
    fn with_no_tray_to_show_it_quit_never_hides_every_window() {
        assert_eq!(
            what_quit_does(Some("your trigger “nightly”".into()), false, false),
            QuitDoes::StaysOpen {
                kept_for: "your trigger “nightly”".into()
            }
        );
    }

    #[test]
    fn stop_is_a_quit_whatever_keeps_the_host_running() {
        assert_eq!(
            what_quit_does(Some("your trigger “nightly”".into()), true, true),
            QuitDoes::Exit
        );
        assert_eq!(
            what_quit_does(Some("your trigger “nightly”".into()), true, false),
            QuitDoes::Exit
        );
    }

    #[test]
    fn with_no_tray_the_window_says_why_it_stays_and_offers_stop() {
        let [title, said, stop, keep] = stays_open_words("your trigger “nightly”");
        assert_eq!(title, "purlis can't run in the background here");
        assert_eq!(
            said,
            "purlis keeps running for your trigger “nightly”, but this desktop has no tray to \
             show it in, so its window stays open. To end every chat and stop purlis, choose \
             Stop purlis."
        );
        assert_eq!(
            (stop.as_str(), keep.as_str()),
            ("Stop purlis", "Keep purlis open")
        );
    }

    /// An app with a [`Lifetime`] whose host something keeps running, and a tray shown.
    fn an_app_kept_running() -> tauri::App<tauri::test::MockRuntime> {
        let app = tauri::test::mock_builder()
            .build(tauri_context!(test = true))
            .expect("the app builds");
        let lifetime = Lifetime::new(|| Some("your trigger “nightly”".into()));
        lifetime.tray_shown(true);
        app.manage(lifetime);
        app
    }

    #[test]
    fn an_exit_with_a_code_is_never_held_off_to_go_to_the_background() {
        let app = an_app_kept_running();
        for code in [tauri::RESTART_EXIT_CODE, 0] {
            assert!(
                !on_exit_requested(app.handle(), Some(code)),
                "an exit with code {code} (restart to update, the restart onto the session \
                 bus, Stop) goes ahead"
            );
            assert_eq!(app.state::<Lifetime>().background(), None, "{code}");
        }
    }

    #[test]
    fn an_exit_nobody_named_a_code_for_goes_to_the_background_while_the_host_is_kept() {
        let app = an_app_kept_running();
        assert!(on_exit_requested(app.handle(), None));
        assert_eq!(
            app.state::<Lifetime>().background().as_deref(),
            Some("your trigger “nightly”")
        );
    }

    #[test]
    fn an_exit_with_no_tray_to_go_to_goes_ahead() {
        let app = an_app_kept_running();
        app.state::<Lifetime>().tray_shown(false);
        assert!(!on_exit_requested(app.handle(), None));
        assert_eq!(app.state::<Lifetime>().background(), None);
    }

    #[test]
    fn a_window_raised_from_the_dock_finder_or_a_notification_brings_purlis_out_of_the_background()
    {
        let app = an_app_kept_running();
        assert!(on_exit_requested(app.handle(), None));

        crate::windows::raise(app.handle(), WINDOW);

        assert_eq!(app.state::<Lifetime>().background(), None);
    }

    #[test]
    fn a_lifetime_reads_what_keeps_the_host_running_at_each_quit() {
        let kept = std::sync::Arc::new(Mutex::new(None::<String>));
        let lifetime = Lifetime::new({
            let kept = std::sync::Arc::clone(&kept);
            move || kept.lock().unwrap().clone()
        });
        assert_eq!(lifetime.quit_does(), QuitDoes::Exit);
        *kept.lock().unwrap() = Some("your paired device “phone”".into());
        assert_ne!(lifetime.quit_does(), QuitDoes::Exit);
        lifetime.stop();
        assert_eq!(lifetime.quit_does(), QuitDoes::Exit);
    }

    #[test]
    fn the_tray_offers_show_and_quit_while_purlis_runs_as_it_always_has() {
        let items = tray_items(None);
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, ["Show purlis", "Quit purlis"]);
        assert!(items.iter().all(|item| item.enabled));
    }

    #[test]
    fn in_the_background_the_tray_says_why_purlis_runs_and_how_to_stop_it() {
        let items = tray_items(Some("your trigger “nightly”"));
        assert_eq!(
            items,
            [
                TrayItem {
                    id: KEPT_FOR,
                    label: "Running in the background for your trigger “nightly”".into(),
                    enabled: false,
                },
                TrayItem {
                    id: SHOW,
                    label: "Open purlis".into(),
                    enabled: true,
                },
                TrayItem {
                    id: STOP,
                    label: "Stop purlis and end every chat".into(),
                    enabled: true,
                },
            ]
        );
    }

    /// A full-bleed app icon, the shape of the one #159 generated: opaque corner to corner.
    fn full_bleed_icon() -> Image<'static> {
        Image::new_owned(vec![255; 4 * 8 * 8], 8, 8)
    }

    fn alpha(image: &Image<'_>) -> Vec<u8> {
        image.rgba().iter().skip(3).step_by(4).copied().collect()
    }

    /// The property that failed in #202, and the only one macOS reads: a template image is
    /// its alpha channel, so it has to have both a background that is not there and a shape
    /// that is. The app icon passes the second half and fails the first.
    fn reads_as_a_template(image: &Image<'_>) -> bool {
        let alpha = alpha(image);
        let clear = alpha.iter().filter(|&&a| a == 0).count();
        let opaque = alpha.iter().filter(|&&a| a == u8::MAX).count();
        clear * 2 > alpha.len() && opaque * 10 > alpha.len()
    }

    /// The accelerator `item` carries once built, written the way muda parses one.
    ///
    /// A predefined item's is not charter's to choose: muda 0.19.3
    /// (`src/items/predefined.rs::accelerator`) gives each a fixed one, `CmdOrCtrl` on every
    /// platform, and that table is copied here so a test can hold the menu to it.
    fn accelerator(item: Item, macos: bool) -> Option<String> {
        let chord = |key: &str| Some(format!("CmdOrCtrl+{key}"));
        match item {
            Item::Quit(accelerator) | Item::Settings(accelerator) => Some(accelerator.to_owned()),
            Item::Separator => None,
            Item::Hide => chord("H"),
            Item::Undo => chord("Z"),
            Item::Redo if macos => Some("CmdOrCtrl+Shift+Z".to_owned()),
            Item::Redo => chord("Y"),
            Item::Cut => chord("X"),
            Item::Copy => chord("C"),
            Item::Paste => chord("V"),
            Item::SelectAll => chord("A"),
        }
    }

    /// Whether a pane's terminal turns `accelerator` into bytes for the shell, on a platform
    /// that is macOS or not — so a menu that claims it takes a key from the chat.
    ///
    /// xterm (`@xterm/xterm` 6.0.0, `Keyboard.ts`) encodes `Ctrl` with a letter, space, `3`–`8`,
    /// `[`, `\` or `]` as a control character only when neither `Shift`, `Alt` nor `Meta` is
    /// held; with `Shift` it sends nothing, which is the room GNOME Terminal and Konsole put
    /// their own keys in. `CmdOrCtrl` is `Ctrl` off macOS and `⌘` on it, and a `⌘` chord xterm
    /// sends nothing for either.
    fn a_terminal_encodes(accelerator: &str, macos: bool) -> bool {
        let words: Vec<String> = accelerator.split('+').map(str::to_uppercase).collect();
        let (key, modifiers) = words.split_last().expect("an accelerator has a key");
        let ctrl = modifiers.iter().any(|m| {
            m == "CTRL"
                || m == "CONTROL"
                || (!macos && (m == "CMDORCTRL" || m == "COMMANDORCONTROL"))
        });
        let other = modifiers
            .iter()
            .any(|m| m == "SHIFT" || m == "ALT" || m == "OPTION" || m == "SUPER" || m == "CMD");
        let encoded = match key.as_str() {
            "SPACE" | "[" | "\\" | "]" | "BRACKETLEFT" | "BRACKETRIGHT" | "BACKSLASH" => true,
            one if one.len() == 1 => one
                .chars()
                .all(|c| c.is_ascii_alphabetic() || ('3'..='8').contains(&c)),
            _ => false,
        };
        ctrl && !other && encoded
    }

    fn accelerators(macos: bool) -> Vec<(Item, String)> {
        layout(macos)
            .into_iter()
            .flat_map(|(_, items)| items)
            .filter_map(|item| accelerator(item, macos).map(|a| (item, a)))
            .collect()
    }

    #[test]
    fn a_ctrl_letter_is_the_terminals_and_a_ctrl_shift_letter_or_a_cmd_chord_is_not() {
        assert!(a_terminal_encodes("CmdOrCtrl+C", false));
        assert!(a_terminal_encodes("Ctrl+Q", false));
        assert!(!a_terminal_encodes("CmdOrCtrl+C", true));
        assert!(!a_terminal_encodes("Ctrl+Shift+Q", false));
        assert!(!a_terminal_encodes("Alt+F4", false));
        // Not only letters: xterm sends `^[` for `Ctrl+[` and `^\` for `Ctrl+\`.
        assert!(a_terminal_encodes("Ctrl+[", false));
        assert!(a_terminal_encodes("Ctrl+5", false));
        // And not a comma, which is why Settings can be on `Ctrl+,` (charter-app#283).
        assert!(!a_terminal_encodes("Ctrl+,", false));
        assert!(!a_terminal_encodes("Ctrl+0", false));
    }

    #[test]
    fn off_macos_no_menu_accelerator_is_a_key_the_chats_terminal_encodes() {
        // #187: off macOS muda's predefined Edit items are `Ctrl` chords, and every one of
        // them is a key a shell owns — Copy was `Ctrl-C`, the interrupt.
        let taken: Vec<(Item, String)> = accelerators(false)
            .into_iter()
            .filter(|(_, a)| a_terminal_encodes(a, false))
            .collect();

        assert_eq!(taken, Vec::new());
    }

    #[test]
    fn off_macos_quit_is_ctrl_shift_q_as_it_is_in_a_terminal_app() {
        assert_eq!(
            accelerators(false),
            vec![
                (Item::Settings("Ctrl+,"), "Ctrl+,".to_owned()),
                (Item::Quit("Ctrl+Shift+Q"), "Ctrl+Shift+Q".to_owned())
            ]
        );
    }

    #[test]
    fn on_macos_the_menu_is_the_cmd_menu_it_always_was() {
        assert_eq!(
            layout(true),
            vec![
                (
                    "purlis",
                    vec![
                        Item::Settings("CmdOrCtrl+,"),
                        Item::Separator,
                        Item::Hide,
                        Item::Separator,
                        Item::Quit("CmdOrCtrl+Q")
                    ]
                ),
                (
                    "Edit",
                    vec![
                        Item::Undo,
                        Item::Redo,
                        Item::Separator,
                        Item::Cut,
                        Item::Copy,
                        Item::Paste,
                        Item::SelectAll,
                    ]
                ),
            ]
        );
        assert!(
            accelerators(true)
                .iter()
                .all(|(_, a)| !a_terminal_encodes(a, true))
        );
    }

    #[test]
    fn the_menu_bar_glyph_is_a_shape_in_alpha_and_not_a_filled_rectangle() {
        // #202: the tray was the app icon drawn as a template, and a template is painted
        // from its alpha alone, so an opaque image is a black rectangle in the menu bar.
        // Nothing noticed, because the PNG looks right in any viewer.
        let glyph = MENU_BAR_GLYPH;

        let alpha = alpha(&glyph);
        assert_eq!(
            alpha.len(),
            (glyph.width() * glyph.height()) as usize,
            "the glyph is RGBA, so there is one alpha byte per pixel"
        );
        assert!(
            reads_as_a_template(&glyph),
            "the glyph must be transparent around an opaque shape, not opaque throughout"
        );
        let corners = [
            0,
            (glyph.width() - 1) as usize,
            (glyph.width() * (glyph.height() - 1)) as usize,
            (glyph.width() * glyph.height() - 1) as usize,
        ];
        for corner in corners {
            assert_eq!(alpha[corner], 0, "a menu bar shows through every corner");
        }
    }

    #[test]
    fn macos_gets_the_template_glyph_however_opaque_the_app_icon_is() {
        let icon = tray_icon(true, Some(full_bleed_icon())).expect("macOS carries its glyph");

        assert!(icon.as_template, "a menu bar recolours what it draws");
        assert!(
            reads_as_a_template(&icon.image),
            "only an image with a real alpha channel may be drawn as a template (#202)"
        );
    }

    #[test]
    fn every_other_platform_gets_the_app_icon_in_its_own_colours() {
        // `icon_as_template` is a macOS concept. Linux and Windows draw the image as it is,
        // and theirs was never the thing that broke.
        let app_icon = full_bleed_icon();

        let icon = tray_icon(false, Some(app_icon.clone())).expect("the app icon is bundled");

        assert!(!icon.as_template);
        assert_eq!(icon.image.rgba(), app_icon.rgba());
    }

    #[test]
    fn a_build_that_lost_its_app_icon_still_gets_a_tray() {
        // The tray was built with `.expect("the app is bundled with its icon")` while the
        // code around it says a tray is never worth the app: "the sessions are the work".
        assert!(tray_icon(false, None).is_none(), "and no panic");
        assert!(
            tray_icon(true, None).is_some(),
            "macOS never asks the bundle for it — the glyph is compiled in"
        );
    }

    #[test]
    fn the_first_ask_to_quit_goes_to_the_window() {
        let quitting = Quitting::default();

        assert_eq!(quitting.ask(), Ask::AskTheWindow);
    }

    #[test]
    fn a_second_ask_quits_without_waiting_for_a_window_that_has_not_answered() {
        // The way out of a webview that cannot draw the dialog: press quit again.
        let quitting = Quitting::default();
        quitting.ask();

        assert_eq!(quitting.ask(), Ask::QuitAnyway);
    }

    #[test]
    fn an_ask_the_window_never_answered_stops_arming_the_next_one() {
        // A `quit-asked` the window never received — pressed while it was still starting —
        // would otherwise leave the app armed for the rest of the day, and the next Cmd-Q
        // would end every session with no warning at all.
        let quitting = Quitting::default();
        let when = Instant::now();
        quitting.asked_at(when);

        let later = quitting.asked_at(when + INSISTING + Duration::from_millis(1));

        assert_eq!(later, Ask::AskTheWindow);
    }

    #[test]
    fn asking_twice_in_a_moment_is_someone_insisting() {
        let quitting = Quitting::default();
        let when = Instant::now();
        quitting.asked_at(when);

        let again = quitting.asked_at(when + Duration::from_millis(500));

        assert_eq!(again, Ask::QuitAnyway);
    }

    #[test]
    fn an_ask_the_window_said_no_to_is_asked_again_from_the_start() {
        // Otherwise cancelling once would arm the next Cmd-Q to quit with no warning at all.
        let quitting = Quitting::default();
        quitting.ask();

        quitting.never_mind();

        assert_eq!(quitting.ask(), Ask::AskTheWindow);
    }
}
