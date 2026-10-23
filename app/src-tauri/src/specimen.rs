// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme specimen, drawn** (#2330, spec #2313): a fixed page of sample content
//! (`app/specimen.html`, `app/src/specimen/`) in a hidden window of its own, photographed by the
//! platform's webview and handed back as PNG. Two callers: a chat's `theme_preview` tool, whose
//! ask reaches the app on the chat's hook socket ([`answered`]), and the window itself
//! ([`theme_specimen`]), for Settings › Theme's cards and the scenario tests.
//!
//! **A chat never sees the person's window, and three things make that so**:
//!
//! - **the page is handed the theme and nothing else** ([`creation_script`]): the built-in
//!   themes, compiled into the core, the one theme asked for, the colour it is tinted with, and
//!   the bytes of the theme's own fonts (#2536), which the page makes its faces from, since it
//!   may ask purlis for nothing. No project, chat, path, layout or setting of the person's is in
//!   it;
//! - **its window is granted no command** ([`label`] matches no capability in `capabilities/`),
//!   so the page cannot ask the core for anything of the person's even if it tried;
//! - **every word, line and colour on it is the page's own fixture**, drawn from the app's
//!   real components.
//!
//! **Where a chat's specimen is resolved** (#2536): where that chat works, by this app's record
//! of it ([`ChatAt`]): its plane, its workspace and its id, never what the chat sent. The theme
//! engine looks the theme's id up there ([`themepreview::resolved`]), as the window resolves a
//! choice of it there, and the page tints it with that workspace's colour, as the window is
//! tinted there.
//!
//! **How it is drawn.** A hidden window of [`themepreview::WIDTH`] × [`themepreview::HEIGHT`]
//! loads the page, which draws itself and then sets its title to [`READY`]; the webview is then
//! photographed (WebKit's `takeSnapshotWithConfiguration` on macOS, `webkit_web_view_get_snapshot`
//! on Linux, each of which paints the page in the web process rather than reading the screen,
//! so a hidden window is drawn whole) and the window destroyed. One specimen at a time, each
//! within [`themepreview::BUDGET`]; past it the window is closed and the ask answered no.
//!
//! **A chat goes first** (#2614): whose turn the painter is, is [`themepreview::turns`]'s. A
//! chat's `theme_preview` asked while one of Settings › Theme's pictures is drawn waits for it,
//! and that picture gives way and is answered [`BUSY`], so the chat is drawn next; the window
//! asks for the picture again within a second. After a chat's turn the window asked during, the
//! gallery is owed one picture, drawn whole before any chat goes again (#2751), so chats asking
//! again and again still leave the cards on screen their pictures.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock, mpsc};
use std::time::Instant;

use purlis_core::hookwire::{Answer, ThemePreviewAsk};
use purlis_core::themepreview::spacing::Spacing;
use purlis_core::themepreview::turns::{Turn, Turns, Waited, Who};
use purlis_core::themepreview::{self, Painted, Reached, Specimen};

use crate::extensions::WindowTheme;

/// The title the specimen page sets once it is drawn and may be photographed.
pub const READY: &str = "purlis-specimen-drawn";

/// The title the specimen page sets when it could not draw itself, before why.
pub const FAILED: &str = "purlis-specimen-failed: ";

/// The page the hidden window loads.
pub const PAGE: &str = "specimen.html";

/// The global the page reads what it is to draw from.
pub const GIVEN: &str = "__PURLIS_SPECIMEN__";

/// A specimen's picture: PNG, and its size in pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// **What the specimen's page is handed** (#2536): the theme, drawn by the engine with its floors
/// held; the workspace colour the window is tinted with where the chat works, `None` for none;
/// and the bytes of each face of the theme's fonts, by its digest.
#[derive(Debug, Clone, PartialEq)]
pub struct Handed {
    pub theme: WindowTheme,
    pub tint: Option<String>,
    pub faces: Vec<(String, Vec<u8>)>,
}

impl Handed {
    /// `theme`, untinted, with the faces its fonts name, or why they cannot be handed.
    pub fn of(theme: WindowTheme) -> Result<Self, String> {
        let faces = themepreview::faces_of(
            [&theme.fonts.ui, &theme.fonts.mono, &theme.fonts.terminal]
                .into_iter()
                .flat_map(|font| font.faces.iter().map(|face| face.asset.as_str())),
        )?;
        Ok(Self {
            theme,
            tint: None,
            faces,
        })
    }
}

/// **What draws a specimen**: the app's hidden window ([`InTheApp`]), or a test's stand-in.
pub trait Painter: Send + Sync {
    /// Whose turn it is to be drawn: one at a time, a chat ahead of the gallery.
    fn turns(&self) -> &Turns;

    /// How often each chat is drawn: at most once a gap ([`themepreview::spacing`]), so a chat
    /// asking again and again leaves Settings › Theme its pictures.
    fn spacing(&self) -> &Spacing;

    /// Draws the specimen of what `handed` holds on `turn`, within the budget, or says why it
    /// could not: [`BUSY`] when the turn gave way to a chat ([`Turn::wait`]).
    fn paint(&self, handed: &Handed, turn: &Turn<'_>) -> Result<Picture, String>;
}

/// **Where a chat works, as this app knows it** (#2536): the plane it was started in, the
/// workspace it works in (`None` at the plane root), and its id, whose theme drafts are its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatAt {
    pub plane: PathBuf,
    pub workspace: Option<String>,
    pub id: Option<String>,
}

impl ChatAt {
    /// What the chat is spaced by: its id, else the plane and its number here.
    fn key(&self, chat: u32) -> String {
        self.id
            .clone()
            .unwrap_or_else(|| format!("{}#{chat}", self.plane.display()))
    }
}

/// The painter this app draws with, once its window machinery is up ([`install`]).
static PAINTER: OnceLock<Arc<dyn Painter>> = OnceLock::new();

/// The painter in force, if the app has one.
pub fn painter() -> Option<Arc<dyn Painter>> {
    PAINTER.get().cloned()
}

/// Puts the app's own painter in force: called once, from setup.
pub fn install(app: &tauri::AppHandle) {
    let _ = PAINTER.set(Arc::new(InTheApp {
        app: app.clone(),
        turns: Turns::new(),
        spacing: Spacing::new(),
    }));
}

/// **Answers a chat's `theme_preview`** ([`purlis_core::hookwire::Ask::ThemePreview`]): each
/// specimen asked for, looked up where the chat works (`chat_at`, this app's record of the chat
/// the line's token is of; `None` for one it does not have open), drawn by `painter` in the
/// theme the engine resolves there, in order. `config` is this machine's config home.
pub fn answered(
    chat_at: &dyn Fn(u32) -> Option<ChatAt>,
    config: Option<PathBuf>,
    painter: Option<&dyn Painter>,
    asks: &ThemePreviewAsk,
) -> Answer {
    let no = |why: String| Answer::No { why };
    let Some(at) = chat_at(asks.chat) else {
        return no(format!(
            "chat {} is not one this purlis window has open, and only such a chat is drawn the \
             theme specimen",
            asks.chat
        ));
    };
    // The tool's bounds, held again here: a chat may write the line itself.
    if let Err(why) = themepreview::checked(&asks.specimens) {
        return no(why);
    }
    let key = at.key(asks.chat);
    if let Err(left) = painter.map_or(Ok(()), |one| one.spacing().may(&key, Instant::now())) {
        return no(Spacing::said(left));
    }
    let drawing = drawn(
        painter,
        config.map(|config| (config, at)),
        &asks.specimens,
        Who::Chat,
    );
    // A turn is spaced from its end once it began a picture (#2751): an ask answered busy, or a
    // theme refused by its id, drew nothing, and the chat may ask again at once.
    if let Some(one) = painter
        && drawing.began
    {
        one.spacing().ended(&key, Instant::now());
    }
    match drawing.answer {
        Ok(painted) => Answer::Previewed { painted },
        Err(why) => no(why),
    }
}

/// Each of `specimens` drawn by `painter` on one turn of `who`'s, or why not: [`BUSY`] when it is
/// not `who`'s turn ([`themepreview::turns`]). Looked up where the chat at `at` works, with this
/// machine's config home; the built-in themes alone with no `at`, as for a chat on a machine with
/// no config home. The turn is taken first, so the lookup, the font read, the tint and the
/// pictures are all drawn on it, within [`themepreview::TURN`].
pub fn painted(
    painter: Option<&dyn Painter>,
    at: Option<(PathBuf, ChatAt)>,
    specimens: &[Specimen],
    who: Who,
) -> Result<Vec<Painted>, String> {
    drawn(painter, at, specimens, who).answer
}

/// What one ask's turn came to: the answer, and whether a picture was begun on it.
struct Drawing {
    answer: Result<Vec<Painted>, String>,
    began: bool,
}

impl Drawing {
    fn no(why: String) -> Self {
        Self {
            answer: Err(why),
            began: false,
        }
    }
}

/// [`painted`], saying whether a picture was begun.
fn drawn(
    painter: Option<&dyn Painter>,
    at: Option<(PathBuf, ChatAt)>,
    specimens: &[Specimen],
    who: Who,
) -> Drawing {
    if specimens.is_empty() || specimens.len() > themepreview::AT_MOST {
        return Drawing::no(format!(
            "ask for one theme specimen, or {} side by side",
            themepreview::AT_MOST
        ));
    }
    let Some(painter) = painter else {
        return Drawing::no(NO_PAINTER.to_owned());
    };
    on_turn(painter, specimens, who, themepreview::TURN, move || {
        let tint = at.as_ref().and_then(|(_, chat)| colour_of(chat));
        let place = at.map(|(config, chat)| {
            let place = crate::themegallery::Place::read(
                config.clone(),
                Some(chat.plane.clone()),
                chat.workspace.clone(),
            );
            (config, chat, place)
        });
        move |specimen: &Specimen| {
            // Drawn as the window draws it there: the consent floors held (ADR 0097 §6).
            let drawn = match &place {
                None => themepreview::resolved(&Reached::nowhere(), specimen)?,
                Some((config, chat, place)) => themepreview::resolved(
                    &Reached {
                        config: Some(config),
                        plane: Some(&chat.plane),
                        workspace: chat.workspace.as_deref(),
                        chat: chat.id.as_deref(),
                        shelf: place.shelf(),
                    },
                    specimen,
                )?,
            };
            let theme = drawn.theme.ok_or_else(|| NO_THEME.to_owned())?;
            Ok(Looked {
                handed: Handed {
                    tint: tint.clone(),
                    ..Handed::of(WindowTheme::from(&theme))?
                },
                appearance: theme.appearance,
                high_contrast: theme.high_contrast,
            })
        }
    })
}

/// One specimen, looked up: what its page is handed, and the appearance it is drawn in.
struct Looked {
    handed: Handed,
    appearance: purlis_core::theme::Appearance,
    high_contrast: bool,
}

/// **`specimens` drawn on one turn of `who`'s, `turn` long**: the turn taken first, then the
/// lookup `looking` makes (read once the turn is held), then each specimen looked up and drawn,
/// none begun that could end past the turn.
fn on_turn<L>(
    painter: &dyn Painter,
    specimens: &[Specimen],
    who: Who,
    turn: std::time::Duration,
    looking: impl FnOnce() -> L,
) -> Drawing
where
    L: Fn(&Specimen) -> Result<Looked, String>,
{
    let held = match turn_of(painter, who) {
        Ok(held) => held,
        Err(why) => return Drawing::no(why),
    };
    let until = Instant::now() + turn;
    let look = looking();
    let mut began = false;
    let answer = specimens
        .iter()
        .map(|specimen| {
            let started = Instant::now();
            let looked = look(specimen)?;
            // No picture is begun that could end past the turn's time: the caller has stopped
            // waiting by then.
            if Instant::now() + themepreview::BUDGET > until {
                return Err(over_budget());
            }
            began = true;
            let picture = painter.paint(&looked.handed, &held)?;
            Ok(Painted {
                preview: purlis_core::theme::is_preview(&specimen.theme),
                theme: specimen.theme.clone(),
                appearance: looked.appearance,
                high_contrast: looked.high_contrast,
                width: picture.width,
                height: picture.height,
                millis: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
                png: picture.png,
            })
        })
        .collect();
    Drawing { answer, began }
}

/// **The colour the window is tinted with where `chat` works**: its workspace's, as its
/// manifest holds it ([`themepreview::workspace_colour`]); `None` at the plane root and for a
/// workspace with none.
fn colour_of(chat: &ChatAt) -> Option<String> {
    themepreview::workspace_colour(&chat.plane, chat.workspace.as_deref()?)
}

/// **`who`'s turn on `painter`**, or [`BUSY`]: a chat waits for a gallery picture to give way,
/// one budget at most.
fn turn_of(painter: &dyn Painter, who: Who) -> Result<Turn<'_>, String> {
    match who {
        Who::Chat => painter.turns().chat(themepreview::BUDGET),
        Who::Gallery => painter.turns().gallery(),
    }
    .ok_or_else(|| BUSY.to_owned())
}

const NO_THEME: &str = "purlis has no theme to draw this specimen in";

const NO_PAINTER: &str = "this purlis window cannot draw the theme specimen yet: it is still \
                          starting, or this platform's webview takes no picture of a page";

/// **What the specimen page is handed**, as the script that runs before any of its own: the
/// built-in themes and the default (`__CHARTER_AT_CREATION__`'s `built_in` and `default_theme`,
/// which the window's theme module reads at its start, `windowprefs::HANDED_AT_LOAD`), and on [`GIVEN`] the one theme to draw, the size to lay it out at, the
/// colour to tint it with (`null` for none) and its fonts' faces, each by its digest as base64
/// (#2536). **Nothing else**: this is the whole of what the page knows, and it is built from
/// `handed` and the compiled-in built-ins alone.
pub fn creation_script(handed: &Handed) -> String {
    use base64::Engine as _;
    let (built_in, default_theme) =
        crate::windowprefs::handed_with_default(&purlis_core::theme::Layer::default());
    let at_creation = serde_json::json!({
        "built_in": built_in,
        "default_theme": default_theme,
    });
    let faces: serde_json::Map<String, serde_json::Value> = handed
        .faces
        .iter()
        .map(|(asset, bytes)| {
            (
                asset.clone(),
                base64::engine::general_purpose::STANDARD
                    .encode(bytes)
                    .into(),
            )
        })
        .collect();
    let given = serde_json::json!({
        "theme": handed.theme,
        "width": themepreview::WIDTH,
        "height": themepreview::HEIGHT,
        "tint": handed.tint,
        "faces": faces,
    });
    format!(
        "Object.defineProperty(window, \"__CHARTER_AT_CREATION__\", {{ value: Object.freeze({at_creation}) }});\n\
         Object.defineProperty(window, \"{GIVEN}\", {{ value: Object.freeze({given}) }});\n"
    )
}

/// The label of the `n`th specimen window: one no capability grants anything to, and none of
/// purlis's own windows ([`crate::windows::is_charter_window`]).
pub fn label(n: u64) -> String {
    format!("theme-specimen-{n}")
}

/// **The window draws the specimen itself** (Settings › Theme's cards, and the scenario tests):
/// `theme` by id and `appearance` (`dark`, `light`, or none for the theme's first), as a chat's
/// `theme_preview` is drawn. With `at`, or for a theme that is not Purlis Classic, `theme` is a
/// card of the gallery at the level `at` names (the machine's with none, #2331) and
/// `appearance` one of its appearances by id, and `from` the level of the theme folder it is
/// (#2516): the engine resolves it there, with the floors held, and the page is handed that
/// theme alone.
#[tauri::command]
#[specta::specta]
pub async fn theme_specimen(
    planes: tauri::State<'_, crate::planes::Planes>,
    theme: String,
    appearance: Option<String>,
    at: Option<crate::extensions::ThemeAt>,
    from: Option<String>,
) -> Result<SpecimenPicture, String> {
    let tagged = match appearance.as_deref() {
        None => Some(None),
        Some("dark") => Some(Some(purlis_core::theme::Appearance::Dark)),
        Some("light") => Some(Some(purlis_core::theme::Appearance::Light)),
        Some(_) => None,
    };
    let built_in = tagged.filter(|appearance| {
        at.is_none()
            && from.is_none()
            && themepreview::chosen(&Specimen {
                theme: theme.clone(),
                appearance: *appearance,
                high_contrast: false,
                options: purlis_core::theme::Values::new(),
            })
            .is_ok()
    });
    let specimen = match built_in {
        Some(appearance) => Ask::BuiltIn(Specimen {
            theme,
            appearance,
            high_contrast: false,
            options: purlis_core::theme::Values::new(),
        }),
        None => {
            let place = match at {
                Some(crate::extensions::ThemeAt { plane, workspace }) => {
                    Some((planes.held(&plane)?.root().to_path_buf(), workspace))
                }
                None => None,
            };
            Ask::Card {
                place,
                theme,
                from,
                appearance,
            }
        }
    };
    let one = tauri::async_runtime::spawn_blocking(move || match specimen {
        Ask::BuiltIn(specimen) => painted(
            painter().as_deref(),
            None,
            std::slice::from_ref(&specimen),
            Who::Gallery,
        )
        .map(|mut drawn| {
            let one = drawn.remove(0);
            (one.png, one.width, one.height, one.millis, one.appearance)
        }),
        Ask::Card {
            place,
            theme,
            from,
            appearance,
        } => {
            let theme = crate::themegallery::specimen_theme(
                place,
                &theme,
                from.as_deref(),
                appearance.as_deref(),
            )?;
            let painter = painter().ok_or_else(|| NO_PAINTER.to_owned())?;
            let turn = turn_of(painter.as_ref(), Who::Gallery)?;
            let started = Instant::now();
            // A card is drawn as the gallery draws it: untinted, in its own fonts.
            let picture = painter.paint(&Handed::of(theme.clone())?, &turn)?;
            let appearance = if theme.appearance == "light" {
                purlis_core::theme::Appearance::Light
            } else {
                purlis_core::theme::Appearance::Dark
            };
            Ok((
                picture.png,
                picture.width,
                picture.height,
                u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
                appearance,
            ))
        }
    })
    .await
    .map_err(|e| format!("the theme specimen failed inside purlis: {e}"))??;
    let (png, width, height, millis, appearance) = one;
    use base64::Engine as _;
    Ok(SpecimenPicture {
        png: base64::engine::general_purpose::STANDARD.encode(&png),
        width,
        height,
        millis,
        appearance: themepreview::word(appearance).to_owned(),
    })
}

/// What the window asked a specimen of: a built-in by id, as a chat asks, or a gallery's card,
/// resolved by the engine.
enum Ask {
    BuiltIn(Specimen),
    Card {
        /// The plane's root and the workspace in it; `None` for the machine's level.
        place: Option<(std::path::PathBuf, Option<String>)>,
        theme: String,
        /// The level of the theme folder it is, as the window names it (#2516).
        from: Option<String>,
        appearance: Option<String>,
    },
}

/// What [`theme_specimen`] answers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct SpecimenPicture {
    /// The picture, PNG, in base64.
    pub png: String,
    pub width: u32,
    pub height: u32,
    /// From asking for the window to holding the picture.
    pub millis: u32,
    /// The appearance it was drawn in: `dark` or `light`.
    pub appearance: String,
}

/// **The app's own painter**: a hidden window per specimen, one turn at a time.
struct InTheApp {
    app: tauri::AppHandle,
    turns: Turns,
    spacing: Spacing,
}

/// How many specimen windows this app has made, for each one's label.
static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Painter for InTheApp {
    fn turns(&self) -> &Turns {
        &self.turns
    }

    fn spacing(&self) -> &Spacing {
        &self.spacing
    }

    fn paint(&self, handed: &Handed, turn: &Turn<'_>) -> Result<Picture, String> {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        // One at a time, and no queue: `turn` is the painter's, and an ask that found it taken
        // was answered at once ([`themepreview::turns`]).
        let until = Instant::now() + themepreview::BUDGET;
        let n = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (drawn, is_drawn) = mpsc::channel::<Result<(), String>>();
        let window = WebviewWindowBuilder::new(&self.app, label(n), WebviewUrl::App(PAGE.into()))
            .title("purlis theme specimen")
            .inner_size(
                f64::from(themepreview::WIDTH),
                f64::from(themepreview::HEIGHT),
            )
            .resizable(false)
            .decorations(false)
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .initialization_script(creation_script(handed))
            .on_new_window(crate::navguard::no_new_window)
            .on_document_title_changed(move |_, title| {
                if title == READY {
                    let _ = drawn.send(Ok(()));
                } else if let Some(why) = title.strip_prefix(FAILED) {
                    let _ = drawn.send(Err(why.to_owned()));
                }
            })
            .build()
            .map_err(|e| format!("purlis could not open the theme specimen's window: {e}"))?;
        // Destroyed however this ends: an answer, a refusal, or a panic.
        let window = Hidden(window);
        match turn.wait(&is_drawn, until) {
            Ok(Ok(())) => {}
            Ok(Err(why)) => return Err(format!("the theme specimen could not draw: {why}")),
            Err(waited) => return Err(not_drawn(waited)),
        }
        let (pictured, picture) = mpsc::channel::<Result<Picture, String>>();
        window
            .0
            .with_webview(move |webview| photograph::take(webview, pictured))
            .map_err(|e| format!("purlis could not reach the specimen's webview: {e}"))?;
        turn.wait(&picture, until)
            .unwrap_or_else(|waited| Err(not_drawn(waited)))
    }
}

/// What a specimen not drawn on its turn is answered: busy when it gave way to a chat.
fn not_drawn(waited: Waited) -> String {
    match waited {
        Waited::GaveWay => BUSY.to_owned(),
        Waited::Over => over_budget(),
    }
}

/// A specimen's hidden window, destroyed when this is dropped. The destroy is queued on the
/// window's event loop after the photograph it was asked for.
struct Hidden(tauri::WebviewWindow);

impl Drop for Hidden {
    fn drop(&mut self) {
        let _ = self.0.destroy();
    }
}

/// What an ask is told while another specimen is being drawn.
pub const BUSY: &str = "purlis is drawing another theme specimen, and draws one at a time: ask \
                        again in a moment";

fn over_budget() -> String {
    format!(
        "the theme specimen was not drawn within its budget of {} s: ask for it again",
        themepreview::BUDGET.as_secs()
    )
}

/// **The platform's own picture of a webview**, taken on the main thread, answered on `done`.
mod photograph {
    use std::sync::mpsc::Sender;

    use super::Picture;

    #[cfg(target_os = "macos")]
    pub fn take(webview: tauri::webview::PlatformWebview, done: Sender<Result<Picture, String>>) {
        let Some(main) = objc2::MainThreadMarker::new() else {
            let _ = done.send(Err(
                "the specimen was photographed off the main thread".into()
            ));
            return;
        };
        snapshot(&webview, main, done);
    }

    /// **WebKit's snapshot of the specimen's webview**, made a PNG and answered on `done`: one of
    /// the workspace's audited blocks (see the workspace `Cargo.toml`).
    #[cfg(target_os = "macos")]
    #[allow(
        unsafe_code,
        reason = "the fourth audited block, WebKit's snapshot of the specimen; see the SAFETY comment"
    )]
    fn snapshot(
        webview: &tauri::webview::PlatformWebview,
        main: objc2::MainThreadMarker,
        done: Sender<Result<Picture, String>>,
    ) {
        use objc2::rc::Retained;
        use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
        use objc2_foundation::{NSDictionary, NSError, NSNumber};
        use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};

        // SAFETY:
        // - `webview` is Tauri's platform webview, which on macOS is wry's `WKWebView`. The
        //   `&WKWebView` made from it is used only here, synchronously, inside the
        //   `with_webview` closure Tauri runs on the main thread (`main`), and is not kept: the
        //   window's destroy is queued after this closure on the same event loop, so the view
        //   is alive for every call made through the borrow.
        // - Every WebKit and AppKit call here is made on the main thread, as they require.
        // - The completion block captures only `done`, a `Sender`. WebKit copies the block. If
        //   WebKit calls it late, after the window is gone, or never, it only sends into a
        //   receiver that has been dropped, and that send's failure is ignored.
        // - The block's two pointers are WebKit's: each is null or a valid object for the call,
        //   and each is retained before it is read.
        // - `representationUsingType:properties:` is handed an empty dictionary of the key type
        //   it reads.
        unsafe {
            let view: &WKWebView = &*webview.inner().cast::<WKWebView>();
            let configuration = WKSnapshotConfiguration::new(main);
            // The page has said it is drawn; a hidden window has no screen update to wait for,
            // and WebKit paints the snapshot in the web process from the page as it is.
            configuration.setAfterScreenUpdates(false);
            // The picture's width, pinned in points: the page's own, whatever the hidden window
            // was given.
            configuration.setSnapshotWidth(Some(&NSNumber::new_f64(f64::from(
                purlis_core::themepreview::WIDTH,
            ))));
            let handler = block2::RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
                let picture = match Retained::retain(image) {
                    None => Err(Retained::retain(error).map_or_else(
                        || "WebKit took no picture of the specimen".to_owned(),
                        |error| {
                            format!(
                                "WebKit took no picture of the specimen: {}",
                                error.localizedDescription()
                            )
                        },
                    )),
                    Some(image) => image
                        .TIFFRepresentation()
                        .and_then(|tiff| NSBitmapImageRep::imageRepWithData(&tiff))
                        .and_then(|rep| {
                            rep.representationUsingType_properties(
                                NSBitmapImageFileType::PNG,
                                &NSDictionary::new(),
                            )
                            .map(|png| Picture {
                                png: png.to_vec(),
                                width: u32::try_from(rep.pixelsWide()).unwrap_or(0),
                                height: u32::try_from(rep.pixelsHigh()).unwrap_or(0),
                            })
                        })
                        .ok_or_else(|| "the specimen's picture could not be made a PNG".to_owned()),
                };
                let _ = done.send(picture);
            });
            view.takeSnapshotWithConfiguration_completionHandler(Some(&configuration), &handler);
        }
    }

    #[cfg(target_os = "linux")]
    pub fn take(webview: tauri::webview::PlatformWebview, done: Sender<Result<Picture, String>>) {
        use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
        // The whole document: the page lays itself out at the specimen's own size, so it is
        // drawn whole whether or not the hidden window was ever given one.
        webview.inner().snapshot(
            SnapshotRegion::FullDocument,
            SnapshotOptions::NONE,
            None::<&webkit2gtk::gio::Cancellable>,
            move |taken| {
                let picture = taken
                    .map_err(|e| format!("WebKitGTK took no picture of the specimen: {e}"))
                    .and_then(|surface| {
                        let image = cairo::ImageSurface::try_from(surface).map_err(|_| {
                            "WebKitGTK's picture of the specimen is not an image".to_owned()
                        })?;
                        let mut png = Vec::new();
                        image
                            .write_to_png(&mut png)
                            .map_err(|e| format!("the specimen's picture is not a PNG: {e}"))?;
                        Ok(Picture {
                            png,
                            width: u32::try_from(image.width()).unwrap_or(0),
                            height: u32::try_from(image.height()).unwrap_or(0),
                        })
                    });
                let _ = done.send(picture);
            },
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn take(_: tauri::webview::PlatformWebview, done: Sender<Result<Picture, String>>) {
        let _ = done.send(Err(
            "this platform's webview takes no picture of the theme specimen yet".to_owned(),
        ));
    }
}

#[cfg(test)]
mod tests;
