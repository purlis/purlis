// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's layout and the operator's theme, **handed to the page as it is created** and
//! written back when the layout changes (`purlis_core::windowprefs` holds the files and the
//! reasons; `purlis_core::theme` resolves the theme).
//!
//! **Injected, never fetched.** A command is asynchronous, so a layout the window asked for
//! would land after the first paint and the window would paint the default arrangement and then
//! re-lay itself out — the flash ADR 0038 removed. So `lib.rs` builds the main window itself,
//! from its entry in `tauri.conf.json`, with [`at_creation`]'s script as its initialization script:
//! Tauri runs that before the page's own scripts, so `main.tsx` finds the layout, the theme
//! already resolved and the built-in themes in the page, and the first frame is drawn from them.
//! The built-ins are compiled into the core, so handing them over reads nothing from disk.
//!
//! **Main frame only.** `initialization_script` and not `…_for_all_frames`: an extension's panel
//! is a frame of its own, and the operator's arrangement is nothing it needs to be handed.

use std::path::Path;

use purlis_core::theme::{self, Complaint, Contrast, Theirs, TheirsKept};
use purlis_core::windowprefs::{self, Reading};

use crate::extensions::{PersonInForce, WindowTheme};

/// The global the page reads both readings from. `app/src/windowprefs.ts` names it too; the
/// test below holds the script to it, and `windowprefs.test.ts` holds the page to it.
pub(crate) const GLOBAL: &str = "__CHARTER_AT_CREATION__";

/// **How long a window's creation waits for the themes it draws to be measured** (#4337), all told:
/// a theme whose parts pass over its tints takes hundreds of milliseconds per appearance. Past it
/// the frame is drawn with no parts of that theme's, and they land when the window asks.
const FIRST_FRAME_WAITS: std::time::Duration = std::time::Duration::from_millis(50);

/// The themes of one creation, each in the window's shape by the one deadline. **The clock starts
/// at the first theme handed**: what a creation reads before it is not a measure, and must not
/// leave a shipped theme, which measures in under a millisecond, no time to.
struct Handing {
    wait: std::time::Duration,
    deadline: std::cell::OnceCell<std::time::Instant>,
}

impl Handing {
    fn new(wait: std::time::Duration) -> Self {
        Self {
            wait,
            deadline: std::cell::OnceCell::new(),
        }
    }

    /// When the measures of this creation stop being waited for.
    fn deadline(&self) -> std::time::Instant {
        *self
            .deadline
            .get_or_init(|| std::time::Instant::now() + self.wait)
    }

    /// `theme` as the window is handed it, and whether its parts come later.
    fn of(&self, theme: &theme::ResolvedTheme) -> (WindowTheme, bool) {
        let parts = theme::parts::stylesheet_by(theme, self.deadline());
        let later = parts.is_none();
        (
            WindowTheme::with_parts(theme, parts.unwrap_or_default()),
            later,
        )
    }
}

/// What the page receives. Every theme in it is a [`WindowTheme`], the one wire shape a
/// resolved theme has, which `theme_in_force` answers in too.
#[derive(Debug, serde::Serialize)]
struct AtCreation {
    layout: Reading,
    /// The person's own theme, resolved by the core: what the first frame is drawn in when it
    /// is in force.
    theme: TheirTheme,
    /// The built-in appearances a pick names, by the pick ([`Handed`]): what a pick of
    /// one draws without asking the core.
    built_in: std::collections::BTreeMap<String, WindowTheme>,
    /// What the person's colours left out of each of [`Self::built_in`], where any was (#3023):
    /// what the Notice says when a pick of one is drawn without asking the core.
    built_in_mine: std::collections::BTreeMap<String, Vec<String>>,
    /// The default theme's appearance, Purlis's dark (#3034): what the window draws when nothing
    /// else was handed.
    default_theme: WindowTheme,
    /// The person's density and font override, read by the engine from the layout file's
    /// `appearance` (#2326): what the first frame draws over the theme.
    person: PersonInForce,
    /// What the first frame draws (#2602): the theme the levels choose for the project and the
    /// workspace the window is created on, resolved once, the one answer the native window is
    /// created in too.
    first: FirstFrame,
}

/// **What the first frame draws** (#2602, #2582): each frame the levels' answer draws, for the
/// system's appearance when it follows the system, and for the contrast the system asks for.
/// The page draws the one for its own appearance and contrast; the native window is created in
/// the first.
#[derive(Debug, serde::Serialize)]
struct FirstFrame {
    /// Whether the theme drawn follows the system's appearance: the frames are then one for
    /// each, and the page draws the one its `prefers-color-scheme` answers.
    follows_system: bool,
    /// The contrast the core read the system asks for (`syscontrast.rs`), `standard` or `more`;
    /// `null` where the platform says nothing purlis reads, and the frames are then one for
    /// each contrast that draws differently, which the page picks from with `prefers-contrast`.
    contrast: Option<&'static str>,
    frames: Vec<Frame>,
}

/// One frame of [`FirstFrame`].
#[derive(Debug, PartialEq, serde::Serialize)]
struct Frame {
    /// For a light system (when the theme follows it).
    light: bool,
    /// For more contrast.
    more: bool,
    theme: WindowTheme,
    /// The Notice that names the consent floors it breaks, when it breaks one.
    floors: Option<String>,
    /// Which of the person's colours it left out (#3023): what the first frame's Notice says.
    mine: Vec<String>,
    /// What the window asks for its parts by ([`parts_later`]), when it was drawn before they
    /// were measured; `null` when it has them.
    parts_later: Option<u32>,
}

/// **Where a window is created** (#2602): the project in front as it opens, and the workspace
/// it is on there (`None` at the plane root), which the levels are resolved for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatedOn {
    pub(crate) plane: std::path::PathBuf,
    pub(crate) workspace: Option<String>,
    /// The project's font files the creation has hashed already (#2696): the trust check's,
    /// for a remembered project, so its theme is read with no second hashing.
    pub(crate) measured: theme::Measured,
}

impl CreatedOn {
    /// The project at `plane`, on the workspace the window will most likely show it on: the one
    /// its chat in front worked in when its record was last written (`reopen.json`), else the
    /// one its explorer was focused on, else the plane root. A guess and no more: a window that
    /// shows another draws that one's theme once it knows, as it always did.
    pub(crate) fn plane(plane: &Path) -> Self {
        Self::measured(plane, theme::Measured::default())
    }

    /// [`Self::plane`], with the font files `measured` has hashed already.
    fn measured(plane: &Path, measured: theme::Measured) -> Self {
        Self {
            workspace: workspace_in_front(plane),
            plane: plane.to_path_buf(),
            measured,
        }
    }

    /// **The project the main window opens on**: the launch's own (`launched`, the plane the
    /// working directory resolved to), else the one in front in the first window the machine
    /// remembers (`machine.json`), which the restore brings back; `None` with neither.
    ///
    /// **A window's first frame draws a project's theme only once the project is trusted on this
    /// machine.** Running purlis in a project is that yes; a remembered one is drawn only while
    /// the store's approval of it holds for what it contributes now, the check the restore makes
    /// before it opens it (`Planes::open_if_approved`). Otherwise the machine's levels decide.
    ///
    /// **Its font files are hashed once** (#2696): what the check hashed is handed to the read of
    /// its theme folders, which takes a face's digest from it for the same file, unchanged.
    pub(crate) fn main(config_root: Option<&Path>, launched: Option<&Path>) -> Option<Self> {
        Self::main_of(launched, || {
            config_root.map(|root| purlis_core::machine::read(root).store)
        })
    }

    /// [`Self::main`], with what the machine remembers read by `remembered`, only when the
    /// launch named no project.
    fn main_of(
        launched: Option<&Path>,
        remembered: impl FnOnce() -> Option<purlis_core::machine::Store>,
    ) -> Option<Self> {
        if let Some(plane) = launched {
            return Some(Self::plane(plane));
        }
        let remembered = remembered()?;
        let first = remembered.windows.first()?;
        let plane = first.planes.get(first.active)?;
        let measured = theme::Measured::default();
        let contributes = purlis_core::machine::Contribution::of_measured(plane, &measured);
        if remembered.consent(plane, &contributes).must_ask() {
            return None;
        }
        Some(Self::measured(plane, measured))
    }
}

/// The workspace the chat in front of the project at `plane` worked in when its record was last
/// written, or its explorer's focus with no chat in front; `None` outside every workspace.
fn workspace_in_front(plane: &Path) -> Option<String> {
    let record = purlis_core::reopen::read_or_refusal(plane).ok()?;
    if let Some(front) = record.chats.iter().find(|chat| chat.active) {
        let cwd = front.cwd.as_deref()?;
        return purlis_core::active::workspace_of_tree(plane, cwd);
    }
    record.focus.map(|focus| focus.workspace)
}

/// **What a window is created with** (#2602): the creation script, and the native window's
/// appearance and colour, both from one resolution of the levels.
pub(crate) struct Created {
    /// The initialization script ([`at_creation`]).
    pub(crate) script: String,
    /// The appearance and colour the window is created in: the first frame's.
    pub(crate) native: theme::Native,
    /// Whether the first frame follows the system: the native parts are then the system's.
    pub(crate) follows_system: bool,
}

impl FirstFrame {
    /// The frames the levels draw for the window created `on`, with the person's own theme
    /// `theirs` and the system's `contrast` as read natively (`None`: not read).
    fn of(
        config_root: Option<&Path>,
        on: Option<&CreatedOn>,
        theirs: Option<&theme::Theme>,
        contrast: Option<Contrast>,
        person: &theme::Layer,
        handing: &Handing,
    ) -> (Self, theme::Native) {
        // More contrast in purlis is layered on the system's (#2925): it draws as the system's
        // does, so there is one frame for it.
        let contrast = if person.contrast == theme::Least::More {
            Some(Contrast::More)
        } else {
            contrast
        };
        let plane = on.map(|on| on.plane.as_path());
        let workspace = on.and_then(|on| on.workspace.as_deref());
        let levels = theme::Levels::read(config_root, plane, workspace);
        // One read for every frame: a face is hashed once for all of them (#2696).
        let measured = on.map_or_else(theme::Measured::default, |on| on.measured.clone());
        let folders = theme::Folders::of_measured(config_root, plane, workspace, measured);
        let drawn = |system, contrast| {
            theme::first_frame(&theme::LevelInputs {
                levels: &levels,
                folders: &folders,
                theirs,
                on: theme::On::Every,
                system,
                contrast,
            })
            .under(person)
        };
        let contrasts: &[Contrast] = match contrast {
            Some(Contrast::More) => &[Contrast::More],
            Some(Contrast::Standard) => &[Contrast::Standard],
            None => &[Contrast::Standard, Contrast::More],
        };
        let mut frames: Vec<Frame> = Vec::new();
        let mut follows_system = false;
        let mut native = None;
        for &contrast in contrasts {
            let more = contrast == Contrast::More;
            let dark = drawn(theme::Appearance::Dark, contrast);
            follows_system = dark.follows_system;
            let mut each = vec![(false, dark)];
            if follows_system {
                each.push((true, drawn(theme::Appearance::Light, contrast)));
            }
            for (light, one) in each {
                let resolved = one.theme.as_ref().unwrap_or_else(|| theme::default_theme());
                // The window is created in the first: for the contrast read, or the standard
                // one where none was, and for a dark system, where nothing says which.
                native.get_or_insert_with(|| resolved.native());
                let (theme, later) = handing.of(resolved);
                let frame = Frame {
                    light,
                    more,
                    theme,
                    floors: one.held.map(|held| held.said()),
                    mine: said(one.mine),
                    parts_later: later.then(|| parts_later(resolved)),
                };
                // A frame that more contrast draws the same as the standard one is handed once.
                let same = frames.iter().any(|was| {
                    was.light == frame.light
                        && was.theme == frame.theme
                        && was.floors == frame.floors
                        && was.mine == frame.mine
                });
                if !same {
                    frames.push(frame);
                }
            }
        }
        let first = Self {
            follows_system,
            contrast: contrast.map(|contrast| match contrast {
                Contrast::More => "more",
                Contrast::Standard => "standard",
            }),
            frames,
        };
        (
            first,
            native.unwrap_or_else(|| theme::default_theme().native()),
        )
    }
}

/// [`Theirs`] with its theme in the window's shape.
#[derive(Debug, serde::Serialize)]
struct TheirTheme {
    path: String,
    found: bool,
    trouble: Option<String>,
    theme: Option<WindowTheme>,
    complaints: Vec<Complaint>,
    /// The Notice that names the consent floors the theme breaks, when it breaks one: what is
    /// drawn has purlis's own values for those tokens.
    floors: Option<String>,
    /// Which of the person's colours it left out (#3023), as [`Self::floors`] says its floors.
    mine: Vec<String>,
    /// Whether what is handed was drawn before its parts were measured (#4337): the window then
    /// asks the core for it instead of drawing what it holds.
    parts_later: bool,
    /// Whether it is a format 1 file in force, which the window offers, once, to move into a
    /// theme folder (#2323).
    format_1: bool,
    /// What is drawn in its place while the system asks for more contrast (#2328): the theme's
    /// appearance tagged high contrast, when it has one of the first's tag; `None` when more
    /// contrast draws the same.
    more: Option<TheirDrawn>,
}

/// One appearance of the person's theme as the first frame draws it, and its floors' Notice.
#[derive(Debug, serde::Serialize)]
struct TheirDrawn {
    theme: WindowTheme,
    floors: Option<String>,
    /// Which of the person's colours this appearance left out (#3023).
    mine: Vec<String>,
    #[serde(skip)]
    parts_later: bool,
}

impl TheirTheme {
    /// The person's own theme with their layer `person` drawn over it (#2925).
    fn of(theirs: Theirs, person: &theme::Layer, handing: &Handing) -> Self {
        // What the window draws of the person's own theme where no level chooses (`handedFor`
        // in `Extensions.tsx`): its first appearance for each contrast, the consent floors held.
        // The page picks with `prefers-contrast`, so both are handed.
        let drawn = |contrast| {
            theirs.theme.as_ref().and_then(|theme| {
                let drawn = theme::draw(theme.first_for(contrast)).under(person);
                let (theme, parts_later) = handing.of(drawn.theme.as_ref()?);
                Some(TheirDrawn {
                    theme,
                    floors: drawn.held.map(|held| held.said()),
                    mine: said(drawn.mine),
                    parts_later,
                })
            })
        };
        let standard = drawn(Contrast::Standard);
        let more = drawn(Contrast::More)
            .filter(|more| Some(&more.theme) != standard.as_ref().map(|it| &it.theme));
        let parts_later = [&standard, &more]
            .into_iter()
            .flatten()
            .any(|it| it.parts_later);
        Self {
            path: theirs.path,
            found: theirs.found,
            trouble: theirs.trouble,
            complaints: theirs.complaints,
            floors: standard.as_ref().and_then(|it| it.floors.clone()),
            mine: standard
                .as_ref()
                .map_or_else(Vec::new, |it| it.mine.clone()),
            theme: standard.map(|it| it.theme),
            more,
            parts_later,
            format_1: theirs.format_1,
        }
    }
}

/// What drawing the person's layer left out, as the Notice says each (#3023).
fn said(mine: Vec<Complaint>) -> Vec<String> {
    mine.into_iter().map(|one| one.said).collect()
}

/// **The person's own theme, read once for this process** (`purlis_core::theme::TheirsKept`):
/// every window's creation script and every `theme_in_force` answer resolve from the same read,
/// so a later window, or a link that never created one, draws what the first did.
pub(crate) static THEIRS: TheirsKept = TheirsKept::new();

/// **The themes a first frame was drawn without the parts of** (#4410): what a frame's `parts_later`
/// names, by its place here. Each is one theme a creation ran past the wait of, so there are as
/// many as themes that ever did in this process, and they stay: a window's retry finds its own.
static PARTS_LATER: std::sync::Mutex<Vec<theme::ResolvedTheme>> = std::sync::Mutex::new(Vec::new());

/// The ticket `theme`'s parts are asked for by, the same for each frame that draws it.
fn parts_later(theme: &theme::ResolvedTheme) -> u32 {
    let mut kept = PARTS_LATER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let at = kept.iter().position(|one| one == theme).unwrap_or_else(|| {
        kept.push(theme.clone());
        kept.len() - 1
    });
    u32::try_from(at).unwrap_or(u32::MAX)
}

/// **The parts of the first frame `ticket` names**, as long as a measure takes: the one its
/// creation left running, shared and not made again. Refused for a ticket no creation handed.
pub(crate) fn first_frame_parts(ticket: u32) -> Result<String, String> {
    let theme = usize::try_from(ticket).ok().and_then(|at| {
        PARTS_LATER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(at)
            .cloned()
    });
    theme
        .map(|theme| theme::parts::stylesheet(&theme))
        .ok_or_else(|| format!("no first frame waits for its parts as {ticket}"))
}

/// **What a window created `on` is created with** (#2602, #2582): the levels resolved once, for
/// the project and workspace it opens on, and the contrast the system asks for read natively
/// (`syscontrast.rs`). The creation script hands the page that answer, which its first frame
/// draws, and the native window is created in that frame's appearance and colour
/// (`nativewindow.rs`), so neither is drawn again when the page asks what is in force.
///
/// **What is read from disk reaches the page as JSON data, never as code.** It is serialised
/// by `serde_json`, which escapes every quote and control character in a string, so nothing in a
/// hand-edited file can end the literal it sits in; and the value is frozen, so nothing that
/// runs later can quietly rewrite what the launch read. A theme's values are the core's
/// resolved ones, each a colour or a number, never the file's text.
///
/// A machine with no config home hands the page an empty layout and no theme of the person's:
/// the default arrangement and the built-in theme, which is what such a machine has always had.
pub(crate) fn at_creation(config_root: Option<&Path>, on: Option<&CreatedOn>) -> Created {
    created_within(
        FIRST_FRAME_WAITS,
        &THEIRS,
        config_root,
        on,
        crate::syscontrast::asked(),
    )
}

/// [`created_within`], waiting as long as a measure takes: what a test reads does not turn on how
/// busy its machine is.
#[cfg(test)]
fn at_creation_from(
    kept: &TheirsKept,
    config_root: Option<&Path>,
    on: Option<&CreatedOn>,
    contrast: Option<Contrast>,
) -> Created {
    let hour = std::time::Duration::from_secs(3600);
    created_within(hour, kept, config_root, on, contrast)
}

/// [`at_creation`], waiting `wait` for its themes to be measured, from the person's theme as `kept`
/// holds it and the contrast `contrast`.
fn created_within(
    wait: std::time::Duration,
    kept: &TheirsKept,
    config_root: Option<&Path>,
    on: Option<&CreatedOn>,
    contrast: Option<Contrast>,
) -> Created {
    created_by(&Handing::new(wait), kept, config_root, on, contrast)
}

/// [`created_within`], its themes handed by `handing`.
fn created_by(
    handing: &Handing,
    kept: &TheirsKept,
    config_root: Option<&Path>,
    on: Option<&CreatedOn>,
    contrast: Option<Contrast>,
) -> Created {
    let layout = config_root.map_or_else(Reading::default, windowprefs::read_layout);
    // A layout file purlis refused has no document: the person's settings are the defaults.
    let appearance = layout
        .document
        .as_ref()
        .and_then(|document| document.get("appearance"));
    let (person, layer) = PersonInForce::read(appearance);
    let theirs = kept.get(config_root);
    // First: it waits for each of its measures, and costs the same whenever it runs. So the
    // default and Purlis Classic are kept before the deadline starts, and a frame of either has
    // its parts however busy the machine is.
    let Handed {
        built_in,
        built_in_mine,
        default_theme,
    } = Handed::under(&layer);
    let (first, native) = FirstFrame::of(
        config_root,
        on,
        theirs.theme.as_ref(),
        contrast,
        &layer,
        handing,
    );
    let follows_system = first.follows_system;
    let at = AtCreation {
        theme: TheirTheme::of(theirs, &layer, handing),
        built_in,
        built_in_mine,
        default_theme,
        person,
        layout,
        first,
    };
    let json = serde_json::to_string(&at).expect("a reading is plain data serde can always write");
    Created {
        script: format!(
            "Object.defineProperty(window, {GLOBAL:?}, {{ value: Object.freeze({json}), \
             writable: false, configurable: false }});"
        ),
        native,
        follows_system,
    }
}

/// **What `theme.ts` reads from the creation global as its module loads**, and throws without:
/// every page that loads it is handed each (`creation_script_from` here, `specimen.rs`'s), and a
/// test holds both to this list.
#[cfg(test)]
pub(crate) const HANDED_AT_LOAD: [&str; 2] = ["built_in", "default_theme"];

/// **The built-in appearances a window is handed, by the pick that names each** (#3034):
/// `purlis-dark`, the default's appearance, and `purlis-classic`, Purlis Classic's first, each
/// under the person's `layer`. What a pick of one draws in the frame it is made, without asking
/// the core.
///
/// **Only what costs the first frame nothing more**: the default is drawn anyway, and Classic
/// draws no parts. Purlis's light is not handed: its parts are measured across every workspace
/// tint before they are drawn (`theme::parts`), which is a pass a launch should not make for a
/// pick that is not there, so a pick of `purlis-light` asks the core as any other theme's does
/// (ADR 0026).
#[cfg(test)]
pub(crate) fn handed_built_ins(
    layer: &theme::Layer,
) -> std::collections::BTreeMap<String, WindowTheme> {
    Handed::under(layer).built_in
}

/// The picks a window is handed the appearance of ([`Handed`]).
pub(crate) const HANDED_PICKS: [&str; 2] = [purlis_core::names::THEME_DARK.write, theme::CLASSIC];

/// The built-ins a window is handed ([`Handed`]) and the default's appearance, `default_theme`.
pub(crate) fn handed_with_default(
    layer: &theme::Layer,
) -> (std::collections::BTreeMap<String, WindowTheme>, WindowTheme) {
    let handed = Handed::under(layer);
    (handed.built_in, handed.default_theme)
}

/// **The built-in themes a window is handed, each drawn once** (#3023): as it draws under the
/// person's layer, what that left out, and the default's appearance, which is one of them.
struct Handed {
    built_in: std::collections::BTreeMap<String, WindowTheme>,
    built_in_mine: std::collections::BTreeMap<String, Vec<String>>,
    default_theme: WindowTheme,
}

impl Handed {
    fn under(layer: &theme::Layer) -> Self {
        let mut built_in = std::collections::BTreeMap::new();
        let mut built_in_mine = std::collections::BTreeMap::new();
        for pick in HANDED_PICKS {
            let Some(one) = theme::built_in(pick) else {
                continue;
            };
            let (drawn, mine) = built_in_under(one, layer);
            built_in.insert(pick.to_owned(), drawn);
            if !mine.is_empty() {
                built_in_mine.insert(pick.to_owned(), mine);
            }
        }
        let default_theme = built_in
            .get(purlis_core::names::THEME_DARK.write)
            .filter(|_| {
                theme::built_in(purlis_core::names::THEME_DARK.write)
                    == Some(theme::default_theme())
            })
            .cloned()
            .unwrap_or_else(|| built_in_under(theme::default_theme(), layer).0);
        Self {
            built_in,
            built_in_mine,
            default_theme,
        }
    }
}

/// A built-in theme as the window is handed it: as it is, or with the person's colours and
/// contrast drawn over it, the consent floors held, when their layer changes colours (#2925);
/// and which of their colours that left out (#3023).
fn built_in_under(one: &theme::ResolvedTheme, person: &theme::Layer) -> (WindowTheme, Vec<String>) {
    if !person.colours_anything() {
        return (WindowTheme::from(one), Vec::new());
    }
    let drawn = theme::draw(one).under(person);
    let handed = WindowTheme::from(drawn.theme.as_ref().unwrap_or(one));
    (handed, said(drawn.mine))
}

/// [`at_creation_from`]'s script, for a window on no project and the standard contrast.
#[cfg(test)]
fn creation_script_from(kept: &TheirsKept, config_root: Option<&Path>) -> String {
    at_creation_from(kept, config_root, None, Some(Contrast::Standard)).script
}

/// Keeps the window's layout, replacing what the file held.
///
/// `text` is the document as JSON text; the core parses it and writes it back out itself, so
/// what lands on disk is always a document charter wrote.
#[tauri::command]
#[specta::specta]
pub async fn write_layout(text: String) -> Result<(), String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || windowprefs::write_layout(&root, &text))
        .await
        .map_err(|err| format!("keeping the layout did not finish: {err}"))?
        .map_err(|err| err.to_string())
}

/// Moves the arrangement web storage held into the layout file — **only when there is no file
/// yet**. Answers whether it did, so the window knows the old key can go.
#[tauri::command]
#[specta::specta]
pub async fn adopt_layout(text: String) -> Result<bool, String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || windowprefs::adopt_layout(&root, &text))
        .await
        .map_err(|err| format!("moving the layout did not finish: {err}"))?
        .map_err(|err| err.to_string())
}

/// **Use built-in** (NO-6): moves the operator's theme file aside to `theme.aside.json`, never
/// over another file, so what is in force without it is drawn from here on. Answers where the
/// file went.
#[tauri::command]
#[specta::specta]
pub async fn use_built_in_theme() -> Result<String, String> {
    let root = config_root()?;
    let moved =
        tauri::async_runtime::spawn_blocking(move || windowprefs::use_built_in_theme(&root))
            .await
            .map_err(|err| format!("moving the theme aside did not finish: {err}"))??;
    // From here on there is no theme of the person's to draw: the next ask reads again, and
    // finds the file gone.
    THEIRS.forget();
    Ok(moved)
}

/// **Use the default layout** (#1289): moves the layout file aside to `layout.aside.json`,
/// never over another file, so the next launch draws the default arrangement. What else the
/// file keeps goes with it: text sizes, editor, Notices dismissed. Answers where it went.
#[tauri::command]
#[specta::specta]
pub async fn use_default_layout() -> Result<String, String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || windowprefs::use_default_layout(&root))
        .await
        .map_err(|err| format!("moving the layout aside did not finish: {err}"))?
}

/// Keeps the Notices dismissed in one project until their cause changes (NO-2, V91j):
/// `causes` replaces that project's list in the layout file and nothing else, so two windows
/// dismissing in different projects both keep theirs. An empty list takes the project out.
#[tauri::command]
#[specta::specta]
pub async fn set_dismissed(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    causes: Vec<String>,
) -> Result<(), String> {
    // A project this window holds keeps any list: the key is its root as the registry resolved
    // it, the same string the window keys its dismissals by. A remembered project that is gone
    // (#1291), which no window can hold, keeps only the dismissal of its own line, so that line
    // stays dismissed past a relaunch (`may_keep_unheld`), asked of the disk off the thread
    // that draws. (Said here and not in the doc comment, which the bindings carry.)
    if let Err(not_held) = planes.held(&plane) {
        let remembered = planes.remembered();
        let project = plane.as_str().to_owned();
        let causes = causes.clone();
        tauri::async_runtime::spawn_blocking(move || {
            may_keep_unheld(&project, &causes, &remembered, |path| {
                purlis_core::machine::still_a_plane(path).is_ok()
            })
        })
        .await
        .map_err(|err| format!("checking the project did not finish: {err}"))?
        .map_err(|why| format!("{not_held} ({why})"))?;
    }
    let root = config_root()?;
    let project = plane.as_str().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        windowprefs::set_dismissed(&root, &project, &causes)
    })
    .await
    .map_err(|err| format!("keeping what was dismissed did not finish: {err}"))?
    .map_err(|err| err.to_string())
}

/// **Keeps `cause` as seen once on this machine** (#1501): added, under the layout file's lock,
/// to what is kept under no project (`windowprefs::ON_THIS_MACHINE`), so two windows each
/// seeing it keep it once. Only a cause `windowprefs::ONCE_ON_THIS_MACHINE` lists is kept.
///
/// The window asks it on the person's press. It is not one of the window-only commands
/// (`session_protocol::ui::WINDOW_ONLY`): like `set_dismissed`, it is also served to a
/// connection that holds the window's own credential (a `local-ui` peer of the same build),
/// and to no chat.
#[tauri::command]
#[specta::specta]
pub async fn see_on_this_machine(cause: String) -> Result<(), String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || windowprefs::see_on_this_machine(&root, &cause))
        .await
        .map_err(|err| format!("keeping what you have seen did not finish: {err}"))?
        .map_err(|err| err.to_string())
}

/// **Whether `cause` was seen once on this machine**, as the layout file says now (#1501): what
/// the window asks before it shows such a Notice, since another window may have seen it after
/// this one launched. No, where purlis keeps no layout or cannot read it.
#[tauri::command]
#[specta::specta]
pub async fn seen_on_this_machine(cause: String) -> Result<bool, String> {
    let Ok(root) = config_root() else {
        return Ok(false);
    };
    tauri::async_runtime::spawn_blocking(move || windowprefs::seen_on_this_machine(&root, &cause))
        .await
        .map_err(|err| format!("reading what you have seen did not finish: {err}"))
}

/// The family of a gone project's line (`GoneProjectNotice.tsx`): `project-gone:<path>`.
const PROJECT_GONE: &str = "project-gone:";

/// **Whether `causes` may be kept under `project`, a path no window holds** (#1291).
///
/// - A path this machine remembers as a project that is gone (`there` says no) keeps its own
///   `project-gone:<path>` cause, and nothing else: no other cause, and no other path's.
/// - An empty list takes a key out, which can only show a Notice again. It is taken for a
///   remembered project that is gone, and for an absolute path the machine no longer
///   remembers (forgotten, or located elsewhere), so what was kept for it goes with it.
/// - A remembered project that is there is written only by a window that holds it, as before,
///   and a key that is no absolute path (what is seen once on this machine) never is.
fn may_keep_unheld(
    project: &str,
    causes: &[String],
    remembered: &purlis_core::machine::Loaded,
    there: impl Fn(&Path) -> bool,
) -> Result<(), String> {
    if !Path::new(project).is_absolute() {
        return Err("it is not a project's path".to_owned());
    }
    let entry = remembered
        .store
        .recents
        .iter()
        .find(|entry| entry.plane.display().to_string() == project);
    let gone = match entry {
        Some(entry) if there(&entry.plane) => {
            return Err("only a window that holds it keeps what was dismissed there".to_owned());
        }
        Some(_) => true,
        None => false,
    };
    if causes.is_empty() {
        return Ok(());
    }
    if !gone {
        return Err("this machine does not remember it".to_owned());
    }
    let own = format!("{PROJECT_GONE}{project}");
    if causes.iter().all(|cause| *cause == own) {
        Ok(())
    } else {
        Err("a project that is gone keeps only the dismissal of its own line".to_owned())
    }
}

/// The config home, or the reason charter keeps no layout on this machine.
fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root().ok_or_else(|| {
        "this machine has no config home, so purlis cannot keep the layout".to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The object the script defines, read back the way a page would get it.
    fn defined(script: &str) -> serde_json::Value {
        let start = script.find("Object.freeze(").expect("frozen") + "Object.freeze(".len();
        let end = script.rfind("), writable").expect("closed");
        serde_json::from_str(&script[start..end]).expect("the value is JSON")
    }

    #[test]
    fn the_page_is_handed_the_layout_and_the_theme_on_disk() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(windowprefs::LAYOUT),
            r#"{"version":1,"regions":[{"id":"aside","side":"left","order":0}]}"#,
        )
        .unwrap();
        std::fs::write(dir.join(windowprefs::THEME), r#"{"name":"Mine"}"#).unwrap();

        let script = creation_script_from(&TheirsKept::new(), Some(home.path()));
        assert!(script.starts_with(&format!("Object.defineProperty(window, \"{GLOBAL}\"")));
        let at = defined(&script);
        assert_eq!(at["layout"]["found"], true);
        assert_eq!(at["layout"]["document"]["regions"][0]["side"], "left");
        assert_eq!(at["theme"]["theme"]["name"], "Mine");
        // Resolved: every token, filled from the built-in the file did not mention.
        assert_eq!(
            at["theme"]["theme"]["values"]["surface.base"],
            theme::built_in_for(theme::Appearance::Dark).value("surface.base")
        );
        let built: Vec<&str> = at["built_in"]
            .as_object()
            .expect("the built-ins")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(built, ["purlis-classic", "purlis-dark"]);
    }

    #[test]
    fn the_person_s_theme_is_handed_with_the_consent_floors_held_and_the_notice_for_them() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let lost = theme::built_in_for(theme::Appearance::Dark).value("surface.base");
        std::fs::write(
            dir.join(windowprefs::THEME),
            format!(r#"{{"name":"Mine","tokens":{{"state.failed":"{lost}"}}}}"#),
        )
        .unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(
            at["theme"]["theme"]["values"]["state.failed"],
            theme::built_in_for(theme::Appearance::Dark).value("state.failed")
        );
        let floors = at["theme"]["floors"]
            .as_str()
            .expect("the floors are named");
        assert!(
            floors.starts_with("Mine breaks the consent floors"),
            "{floors}"
        );

        std::fs::write(dir.join(windowprefs::THEME), r#"{"name":"Mine"}"#).unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(at["theme"]["floors"], serde_json::Value::Null);
    }

    #[test]
    fn the_first_frame_and_the_built_ins_wear_the_person_s_colours_and_contrast() {
        // #2925: the person's colours and more contrast, read from the layout file, are drawn
        // into every theme the page is handed as it is created.
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let layout = |appearance: &str| {
            std::fs::write(
                dir.join(windowprefs::LAYOUT),
                format!(r#"{{"version":2,"regions":[],"appearance":{appearance}}}"#),
            )
            .unwrap();
            at_creation_from(&TheirsKept::new(), Some(home.path()), None, None)
        };
        let accent = "#9a86ff";
        let created = layout(&format!(
            r#"{{"motion":"reduced","colours":{{"accent.base":"{accent}"}}}}"#
        ));
        let at = defined(&created.script);
        assert_eq!(at["person"]["motion"], "reduced");
        assert_eq!(at["person"]["colours"]["accent.base"], accent);
        for frame in at["first"]["frames"].as_array().expect("frames") {
            assert_eq!(frame["theme"]["values"]["accent.base"], accent);
        }
        // A light accent reads on the dark built-ins; on a light one it would put words under
        // their floor, so the theme's own is drawn there.
        for one in at["built_in"].as_object().expect("the built-ins").values() {
            let drawn = &one["values"]["accent.base"];
            if one["appearance"] == "dark" {
                assert_eq!(drawn, accent, "{}", one["name"]);
            } else {
                assert_ne!(drawn, accent, "{}", one["name"]);
            }
        }
        // Nothing was left out: a built-in that drew every colour has no line of its own.
        assert_eq!(at["built_in_mine"], serde_json::json!({}));
        for frame in at["first"]["frames"].as_array().expect("frames") {
            assert_eq!(frame["mine"], serde_json::json!([]));
        }

        // More contrast in purlis is the system's more: one frame, for it, drawn raised.
        let created = layout(r#"{"contrast":"more"}"#);
        let at = defined(&created.script);
        assert_eq!(at["person"]["contrast"], "more");
        assert_eq!(at["first"]["contrast"], "more");
        for frame in at["first"]["frames"].as_array().expect("frames") {
            assert_eq!(frame["more"], true);
        }
        let classic = &at["built_in"]["purlis-classic"]["values"];
        assert_ne!(
            classic["text.muted"],
            theme::classic().first().value("text.muted"),
            "muted words are raised to 7:1"
        );
    }

    /// **What a window is handed says what the core would** (#3023): a colour that would put
    /// words under their floor is left out of the first frame, of the person's own theme and of
    /// each built-in handed, in the core's own sentence.
    #[test]
    fn what_a_window_is_handed_names_the_person_s_colours_it_left_out() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        // A dark theme of the person's, so the words below are under its floor too.
        std::fs::write(dir.join(windowprefs::THEME), SHARP).unwrap();
        // The words in their own ground's colour: under the floor on every dark ground.
        let ground = theme::default_theme().value("surface.base");
        std::fs::write(
            dir.join(windowprefs::LAYOUT),
            format!(
                r#"{{"version":2,"regions":[],"appearance":{{"colours":{{"text.primary":"{ground}"}}}}}}"#
            ),
        )
        .unwrap();
        let created = at_creation_from(
            &TheirsKept::new(),
            Some(home.path()),
            None,
            Some(Contrast::Standard),
        );
        let at = defined(&created.script);
        let the_dark = at["built_in_mine"][purlis_core::names::THEME_DARK.write]
            .as_array()
            .expect("what the dark built-in left out");
        assert_eq!(the_dark.len(), 1);
        assert!(
            the_dark[0].as_str().unwrap().contains("text.primary"),
            "{the_dark:?}"
        );
        let frames = at["first"]["frames"].as_array().expect("frames");
        assert!(!frames.is_empty());
        for frame in frames {
            assert!(
                !frame["mine"].as_array().expect("mine").is_empty(),
                "{frame}"
            );
        }
        assert!(
            !at["theme"]["mine"].as_array().expect("mine").is_empty(),
            "{}",
            at["theme"]
        );
    }

    /// A theme file whose parts pass over its tints when measured.
    const MEASURED_IN_TINTS: &str = r##"{"name":"Heavy","extends":"purlis-classic",
        "appearances":[{"id":"dark","tokens":{"chart.4":"#ff000040","chart.3":"#ffffff"}}],
        "tint":{"tokens":["surface.base","control.base"]},
        "parts":{"button.root.fill":"chart.4","button.root.text":"chart.3"}}"##;

    #[test]
    fn a_first_frame_past_its_wait_is_drawn_with_no_parts_and_the_window_asks_for_them() {
        // #4337: the frame carries the theme's colours and no parts: each part its family default,
        // what a part that falls is drawn as. The reading says the window must ask.
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(windowprefs::THEME), MEASURED_IN_TINTS).unwrap();
        let standard = Some(Contrast::Standard);
        let root = Some(home.path());
        let zero = std::time::Duration::ZERO;
        let at = defined(&created_within(zero, &TheirsKept::new(), root, None, standard).script);
        assert_eq!(at["theme"]["parts_later"], true);
        assert_eq!(at["theme"]["theme"]["parts"], "");
        assert_eq!(at["first"]["frames"][0]["theme"]["name"], "Heavy");
        assert_eq!(at["first"]["frames"][0]["theme"]["parts"], "");
        let ticket = at["first"]["frames"][0]["parts_later"]
            .as_u64()
            .expect("the frame says its parts are coming");

        // With the measure done, the same creation is handed the parts, and nothing to ask for.
        let at = defined(&at_creation_from(&TheirsKept::new(), root, None, standard).script);
        assert_eq!(at["theme"]["parts_later"], false);
        assert_eq!(
            at["first"]["frames"][0]["parts_later"],
            serde_json::Value::Null
        );
        let parts = at["first"]["frames"][0]["theme"]["parts"].as_str().unwrap();
        assert!(parts.contains("@layer parts"), "{parts}");
        // And kept, so a later window waits for nothing.
        let again = defined(&created_within(zero, &TheirsKept::new(), root, None, standard).script);
        assert_eq!(again["first"]["frames"][0]["theme"]["parts"], parts);
        assert_eq!(again["theme"]["parts_later"], false);
        assert_eq!(
            again["first"]["frames"][0]["parts_later"],
            serde_json::Value::Null
        );

        // The window's ask is answered what the creation would have drawn.
        let ticket = u32::try_from(ticket).unwrap();
        assert_eq!(first_frame_parts(ticket).as_deref(), Ok(parts));
    }

    #[test]
    fn the_window_asks_for_a_frame_s_parts_with_no_project_known_and_waits_for_the_measure() {
        // #4410: nothing here names a plane, the restore's to bring back. The theme is one nothing
        // else in this process measures, so the ask finds its creation's pass still running or
        // makes it, and answers the sheet either way.
        let written = MEASURED_IN_TINTS.replace("#ff000040", "#ff000041");
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(windowprefs::THEME), &written).unwrap();
        let at = defined(
            &created_within(
                std::time::Duration::ZERO,
                &TheirsKept::new(),
                Some(home.path()),
                None,
                Some(Contrast::Standard),
            )
            .script,
        );
        let frame = &at["first"]["frames"][0];
        assert_eq!(frame["theme"]["parts"], "");
        let ticket = u32::try_from(frame["parts_later"].as_u64().unwrap()).unwrap();
        let parts = first_frame_parts(ticket).expect("the frame's parts");
        assert!(parts.contains("@layer parts"), "{parts}");
    }

    #[test]
    fn two_frames_of_one_theme_are_asked_for_by_one_ticket() {
        let theme = theme::load_text(MEASURED_IN_TINTS).theme.first().clone();
        assert_eq!(parts_later(&theme), parts_later(&theme));
    }

    #[test]
    fn a_ticket_no_creation_handed_is_refused() {
        assert!(first_frame_parts(u32::MAX).is_err());
    }

    #[test]
    fn the_wait_starts_at_the_first_theme_handed_not_when_the_creation_starts() {
        // Review of #4337: reading the layout, the levels and the folders comes first, and none of
        // it is a measure, so none of it spends the wait. No clock race: what is read is when the
        // deadline was set.
        let wait = std::time::Duration::from_millis(20);
        let handing = Handing::new(wait);
        std::thread::sleep(wait * 2);
        let reached = std::time::Instant::now();
        let deadline = handing.deadline();
        assert!(deadline >= reached + wait, "the setup spent the wait");
        // One deadline for the whole creation, however many themes it hands.
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert_eq!(handing.deadline(), deadline);
    }

    #[test]
    fn a_slow_setup_leaves_a_shipped_theme_its_parts_on_the_first_frame() {
        // Review of #4337: past its deadline before its first theme, a creation would draw Purlis
        // with none of its parts, and they would pop in. A second is far longer than Purlis takes.
        let wait = std::time::Duration::from_secs(1);
        let handing = Handing::new(wait);
        std::thread::sleep(wait + std::time::Duration::from_millis(100));
        let standard = Some(Contrast::Standard);
        let at = defined(&created_by(&handing, &TheirsKept::new(), None, None, standard).script);
        let frame = &at["first"]["frames"][0]["theme"];
        assert_eq!(frame["name"], theme::default_theme().name.as_str());
        let parts = frame["parts"].as_str().expect("parts are written");
        assert!(parts.contains("@layer parts"), "drawn without: {parts:?}");
        assert_eq!(
            at["first"]["frames"][0]["parts_later"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn the_default_theme_has_its_parts_on_a_first_frame_that_waits_for_nothing() {
        // #4410: the window asks for the parts of a frame that says they are coming, but the
        // default never comes to that: what is handed is measured before the deadline starts.
        let handing = Handing::new(std::time::Duration::ZERO);
        let standard = Some(Contrast::Standard);
        let at = defined(&created_by(&handing, &TheirsKept::new(), None, None, standard).script);
        let frame = &at["first"]["frames"][0];
        assert_eq!(frame["theme"]["name"], theme::default_theme().name.as_str());
        let parts = frame["theme"]["parts"].as_str().expect("parts are written");
        assert!(parts.contains("@layer parts"), "drawn without: {parts:?}");
        assert_eq!(frame["parts_later"], serde_json::Value::Null);
    }

    #[test]
    fn a_first_frame_with_no_parts_still_holds_the_consent_floors_and_the_text_tiers() {
        // Review of #4337: what keeps the frame without parts readable is the holds on its tokens,
        // not the family defaults. This theme breaks a consent floor and a tier as it is written.
        let lost = theme::built_in_for(theme::Appearance::Dark).value("surface.base");
        let written = MEASURED_IN_TINTS.replace(
            r##""chart.3":"#ffffff""##,
            &format!(r##""chart.3":"#ffffff","state.failed":"{lost}","text.muted":"{lost}""##),
        );
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(windowprefs::THEME), &written).unwrap();
        let standard = Some(Contrast::Standard);
        let root = Some(home.path());
        let zero = std::time::Duration::ZERO;
        let at = defined(&created_within(zero, &TheirsKept::new(), root, None, standard).script);
        let frame = &at["first"]["frames"][0]["theme"];
        assert_eq!(frame["parts"], "", "this frame is the one without parts");

        // The frame's own colours, read back onto the theme, as the check reads a theme.
        let mut drawn = theme::load_text(&written).theme.first().clone();
        let unheld = under_floors(&drawn);
        assert!(
            ["state.failed", "text.muted"]
                .iter()
                .all(|token| unheld.iter().any(|pair| pair.starts_with(token))),
            "as written it breaks both, or this holds nothing: {unheld:?}"
        );
        for (token, value) in &mut drawn.values {
            *value = frame["values"][*token].as_str().unwrap().to_owned();
        }
        assert_eq!(under_floors(&drawn), Vec::<String>::new());
    }

    /// The consent pairs and text tiers `theme` draws under their floors, untinted.
    fn under_floors(theme: &theme::ResolvedTheme) -> Vec<String> {
        theme::check(theme)
            .findings
            .iter()
            .filter_map(|finding| match finding {
                theme::Finding::Contrast {
                    tint: None, pair, ..
                } if pair.front.starts_with("state.")
                    || pair.front.starts_with("danger.")
                    || pair.front.starts_with("needs-you.")
                    || pair.front == theme::FAINT
                    || theme::TEXT_TIERS.contains(&pair.front) =>
                {
                    Some(format!("{} on {}", pair.front, pair.back))
                }
                _ => None,
            })
            .collect()
    }

    /// **What a creation takes with a heavy theme** (#4337), printed, in this process's cold state:
    /// `cargo test -p purlis-app --lib creation_with_a_heavy_theme_timed -- --ignored --nocapture`.
    /// Everything but opening the window and loading the page; each run measures nothing kept.
    #[test]
    #[ignore = "a timing to read, not a check"]
    fn creation_with_a_heavy_theme_timed() {
        let recipe = r#"["surface.base","surface.sunken","surface.deep","surface.raised",
            "surface.overlay","surface.hover","control.base","control.hover","tab.active",
            "list.selected"]"#;
        let written = MEASURED_IN_TINTS.replace(r#"["surface.base","control.base"]"#, recipe);
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(windowprefs::THEME), &written).unwrap();
        let loaded = theme::load_text(&written).theme;

        let started = std::time::Instant::now();
        let drawn = theme::draw(loaded.first());
        let holds = started.elapsed();
        assert!(drawn.theme.is_some());

        let started = std::time::Instant::now();
        let created = created_within(
            FIRST_FRAME_WAITS,
            &TheirsKept::new(),
            Some(home.path()),
            None,
            None,
        );
        let took = started.elapsed();
        let at = defined(&created.script);
        let frames = at["first"]["frames"].as_array().unwrap();
        println!(
            "one draw (both holds) {holds:?}; created_within {took:?}; {} frames; parts later: {}",
            frames.len(),
            at["theme"]["parts_later"],
        );
    }

    #[test]
    fn with_nothing_of_the_person_s_the_built_ins_are_handed_as_they_are() {
        let at = defined(&creation_script_from(&TheirsKept::new(), None));
        let handed = handed_built_ins(&theme::Layer::default());
        assert_eq!(at["built_in"], serde_json::to_value(&handed).unwrap());
        let classic = WindowTheme::from(theme::classic().first());
        assert_eq!(handed["purlis-classic"], classic);
        assert_eq!(at["person"]["contrast"], "standard");
        assert_eq!(at["person"]["motion"], "system");
    }

    #[test]
    fn the_window_is_handed_the_default_and_each_built_in_by_the_pick_that_names_it() {
        // #3034: Purlis is the default, handed as `default_theme` and by `purlis-dark`; Classic
        // by `purlis-classic`. Purlis's light is not handed: its parts would be measured on the
        // way to the first frame, so a pick of it asks the core.
        let at = defined(&creation_script_from(&TheirsKept::new(), None));
        let of = |one: &theme::ResolvedTheme| serde_json::to_value(WindowTheme::from(one)).unwrap();
        assert_eq!(at["default_theme"], of(theme::purlis().first()));
        assert_eq!(at["built_in"]["purlis-dark"], of(theme::purlis().first()));
        assert_eq!(
            at["built_in"]["purlis-classic"],
            of(theme::classic().first())
        );
        assert_eq!(at["built_in"].as_object().unwrap().len(), 2);
    }

    /// **Every page that loads the window's theme module is handed what it reads at load**
    /// (#3034): the module throws without one, so the window and the theme specimen's page are
    /// each held to [`HANDED_AT_LOAD`], and the list to what `theme.ts` reads and throws without.
    #[test]
    fn every_creation_script_carries_each_key_the_theme_module_reads_at_load() {
        let module = include_str!("../../src/theme/theme.ts");
        assert_eq!(
            module.matches("was opened without").count(),
            HANDED_AT_LOAD.len(),
            "theme.ts throws at load without a key HANDED_AT_LOAD does not name"
        );
        for key in HANDED_AT_LOAD {
            assert!(module.contains(&format!("given?.{key}")), "{key}");
        }
        let window = defined(&creation_script_from(&TheirsKept::new(), None));
        let specimen = crate::specimen::creation_script(&crate::specimen::Handed {
            theme: WindowTheme::from(theme::default_theme()),
            tint: None,
            faces: Vec::new(),
        });
        let specimen: serde_json::Value = {
            let line = specimen
                .lines()
                .find(|line| line.contains(&format!("\"{GLOBAL}\"")))
                .expect("the creation global");
            let from = line.find("Object.freeze(").expect("frozen") + "Object.freeze(".len();
            let to = line.rfind(") })").expect("closed");
            serde_json::from_str(&line[from..to]).expect("JSON")
        };
        for key in HANDED_AT_LOAD {
            assert!(
                window.get(key).is_some_and(|it| !it.is_null()),
                "window {key}"
            );
            assert!(
                specimen.get(key).is_some_and(|it| !it.is_null()),
                "specimen {key}"
            );
        }
    }

    #[test]
    fn every_window_is_handed_the_one_read_of_the_person_s_theme() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(windowprefs::THEME), r#"{"name":"First"}"#).unwrap();
        let kept = TheirsKept::new();
        let first = defined(&creation_script_from(&kept, Some(home.path())));
        std::fs::write(dir.join(windowprefs::THEME), r#"{"name":"Second"}"#).unwrap();
        let second = defined(&creation_script_from(&kept, Some(home.path())));
        assert_eq!(second["theme"], first["theme"]);
        assert_eq!(second["theme"]["theme"]["name"], "First");
    }

    #[test]
    fn a_resolved_theme_is_handed_in_one_shape_whatever_hands_it() {
        // The creation script's built-ins are `WindowTheme`s, as `theme_in_force`'s answer is.
        let at = defined(&creation_script_from(&TheirsKept::new(), None));
        let one = serde_json::to_value(WindowTheme::from(theme::default_theme())).unwrap();
        assert_eq!(at["built_in"]["purlis-dark"], one);
        assert_eq!(at["default_theme"], one);
    }

    #[test]
    fn a_hand_edited_file_reaches_the_page_as_data_and_never_as_code() {
        // Whatever the operator — or anybody who can write the file — puts in a string, it is a
        // string when it arrives.
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let hostile = r#"\"}); alert(1); ({\" </script>"#;
        std::fs::write(
            dir.join(windowprefs::THEME),
            format!(r#"{{"name":"{hostile}"}}"#),
        )
        .unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(
            at["theme"]["theme"]["name"],
            "\"}); alert(1); ({\"\u{2028}</script>"
        );
    }

    #[test]
    fn a_broken_layout_reaches_the_page_as_a_reason_and_no_document() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(windowprefs::LAYOUT), "{ nope").unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(at["layout"]["found"], true);
        assert_eq!(at["layout"]["document"], serde_json::Value::Null);
        assert!(
            at["layout"]["trouble"]
                .as_str()
                .is_some_and(|why| why.contains("is not JSON"))
        );
    }

    /// A store remembering `paths`, as `machine::read` would hand it over.
    fn remembering(paths: &[&str]) -> purlis_core::machine::Loaded {
        let mut store = purlis_core::machine::Store::default();
        for (age, path) in paths.iter().enumerate() {
            store.remember(Path::new(path), 100 - u64::try_from(age).unwrap_or(0));
        }
        purlis_core::machine::Loaded {
            store,
            ..purlis_core::machine::Loaded::default()
        }
    }

    const GONE: &str = "/media/disk/gone";
    const THERE: &str = "/home/dev/there";

    /// `GONE` is not on the disk any more; every other path is a project that is there.
    fn on_disk(path: &Path) -> bool {
        path != Path::new(GONE)
    }

    fn keeping(project: &str, causes: &[&str]) -> Result<(), String> {
        let causes: Vec<String> = causes.iter().map(|one| (*one).to_owned()).collect();
        may_keep_unheld(project, &causes, &remembering(&[THERE, GONE]), on_disk)
    }

    #[test]
    fn a_remembered_gone_project_keeps_its_own_dismissal_past_a_relaunch() {
        // #1291: a gone project is never one the window holds, so its line's Dismiss is kept
        // under its remembered path, with its own cause and nothing else.
        assert_eq!(keeping(GONE, &["project-gone:/media/disk/gone"]), Ok(()));
        // And let go again, the same way.
        assert_eq!(keeping(GONE, &[]), Ok(()));
    }

    #[test]
    fn a_gone_project_keeps_no_other_cause() {
        for causes in [
            &["pin-dormant:ide"][..],
            &["project-gone:/media/disk/other"],
            &["project-gone:/media/disk/gone", "chat-fresh:3"],
            &["project-gone:/media/disk/gone/"],
        ] {
            assert!(keeping(GONE, causes).is_err(), "{causes:?} was kept");
        }
    }

    #[test]
    fn a_remembered_project_that_is_there_is_not_written_unless_the_window_holds_it() {
        assert!(keeping(THERE, &["project-gone:/home/dev/there"]).is_err());
        assert!(keeping(THERE, &[]).is_err());
    }

    #[test]
    fn a_path_the_machine_does_not_remember_keeps_nothing() {
        assert!(keeping("/home/dev/unknown", &["project-gone:/home/dev/unknown"]).is_err());
        // What is seen once on this machine is never a project's to write.
        assert!(keeping(windowprefs::ON_THIS_MACHINE, &[]).is_err());
        assert!(keeping("relative/gone", &[]).is_err());
    }

    #[test]
    fn a_project_forgotten_or_located_away_lets_its_dismissal_go() {
        // Forget and Locate… take the path out of the store, then the window lets go of what it
        // kept there: an empty list only takes the key out, so it shows a Notice again at most.
        assert_eq!(keeping("/media/disk/forgotten", &[]), Ok(()));
    }

    #[test]
    fn the_native_window_is_created_in_the_first_frame_s_appearance_and_background() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let standard = Some(Contrast::Standard);
        // No theme of the person's: Purlis's, as the page draws it.
        let none = at_creation_from(&TheirsKept::new(), Some(home.path()), None, standard);
        assert_eq!(none.native, theme::default_theme().native());
        std::fs::write(
            dir.join(windowprefs::THEME),
            r##"{"name":"Mine","appearance":"light","tokens":{"window.background":"#abcdef"}}"##,
        )
        .unwrap();
        let kept = TheirsKept::new();
        let created = at_creation_from(&kept, Some(home.path()), None, standard);
        assert_eq!(created.native.appearance, theme::Appearance::Light);
        assert_eq!(created.native.background, [0xab, 0xcd, 0xef]);
        assert!(!created.follows_system);
        // The same answer the page is handed: the theme its first frame draws.
        let at = defined(&created.script);
        assert_eq!(at["first"]["frames"][0]["theme"]["appearance"], "light");
        assert_eq!(
            at["first"]["frames"][0]["theme"]["values"]["window.background"],
            "#abcdef"
        );
        assert_eq!(at["theme"]["theme"]["name"], "Mine");
    }

    /// A config home, and a plane with a stable id and a workspace `alpha`.
    struct Places {
        _home: tempfile::TempDir,
        config: std::path::PathBuf,
        plane: std::path::PathBuf,
    }

    const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

    impl Places {
        fn new(manifest: &str) -> Self {
            let home = tempfile::tempdir().unwrap();
            let config = home.path().join("config");
            let plane = home.path().join("plane");
            std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
            std::fs::create_dir_all(purlis_core::machine::dir(&config)).unwrap();
            std::fs::write(
                purlis_core::names::manifest(&plane),
                format!("[project]\nid = \"{PROJECT}\"\n{manifest}"),
            )
            .unwrap();
            Self {
                _home: home,
                config,
                plane,
            }
        }

        /// Puts the theme folder `name` holding `document` under `themes`.
        fn put(themes: &Path, name: &str, document: &str) {
            let dir = themes.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(theme::THEME_JSON), document).unwrap();
        }

        fn on(&self, workspace: Option<&str>) -> CreatedOn {
            CreatedOn {
                plane: self.plane.clone(),
                workspace: workspace.map(str::to_owned),
                measured: theme::Measured::default(),
            }
        }

        fn created(&self, on: Option<&CreatedOn>, contrast: Option<Contrast>) -> Created {
            at_creation_from(&TheirsKept::new(), Some(&self.config), on, contrast)
        }
    }

    const TEAM: &str = r##"{"name":"Team","appearances":[{"id":"day","appearance":"light",
        "tokens":{"window.background":"#abcdef"}}]}"##;

    const SHARP: &str = r##"{"name":"Sharp","appearances":[
        {"id":"dark","appearance":"dark","tokens":{"window.background":"#202020"}},
        {"id":"contrast","appearance":"dark","high-contrast":true,
         "tokens":{"window.background":"#000000"}},
        {"id":"light","appearance":"light","tokens":{"window.background":"#f4f4f4"}}]}"##;

    #[test]
    fn a_window_on_a_project_whose_level_chooses_is_created_in_that_theme() {
        let places = Places::new("[theme]\nuse = \"team\"\n");
        Places::put(&places.plane.join("themes"), "team", TEAM);
        // The person's own file is beneath the project's committed default.
        std::fs::write(
            purlis_core::machine::dir(&places.config).join(windowprefs::THEME),
            r#"{"name":"Mine"}"#,
        )
        .unwrap();
        let created = places.created(Some(&places.on(None)), Some(Contrast::Standard));
        assert_eq!(created.native.appearance, theme::Appearance::Light);
        assert_eq!(created.native.background, [0xab, 0xcd, 0xef]);
        let at = defined(&created.script);
        assert_eq!(at["first"]["frames"].as_array().map(Vec::len), Some(1));
        assert_eq!(at["first"]["frames"][0]["theme"]["name"], "Team");
        assert_eq!(at["first"]["contrast"], "standard");
        // What the window is answered once it asks is the frame it drew: nothing is drawn again.
        let levels = theme::Levels::read(Some(&places.config), Some(&places.plane), None);
        let folders = theme::Folders::of(Some(&places.config), Some(&places.plane), None);
        let answered = crate::extensions::theme_in_force_of(
            &levels,
            &folders,
            None,
            None,
            false,
            Contrast::Standard,
            || unreachable!("no extension is named"),
            |_| None,
        );
        assert_eq!(
            serde_json::to_value(answered.theme).unwrap(),
            at["first"]["frames"][0]["theme"]
        );
        // The person's own theme is still handed, for what is drawn where no level chooses.
        assert_eq!(at["theme"]["theme"]["name"], "Mine");
    }

    #[test]
    fn a_window_is_created_in_the_machine_default_theme_folder() {
        let places = Places::new("");
        Places::put(
            &purlis_core::machine::dir(&places.config).join("themes"),
            "mine",
            TEAM,
        );
        theme::choices::set(
            &places.config,
            theme::choices::Slot::Machine,
            theme::Choice::new("mine", None),
        )
        .unwrap();
        // On no project, and on one that chooses nothing of its own.
        for on in [None, Some(places.on(None))] {
            let created = places.created(on.as_ref(), Some(Contrast::Standard));
            assert_eq!(created.native.background, [0xab, 0xcd, 0xef]);
            assert_eq!(
                defined(&created.script)["first"]["frames"][0]["theme"]["name"],
                "Team"
            );
        }
    }

    #[test]
    fn a_window_on_a_workspace_is_created_in_the_theme_its_level_chooses() {
        let places = Places::new("[theme]\nuse = \"purlis-classic\"\n");
        Places::put(&places.plane.join("workspaces/alpha/themes"), "team", TEAM);
        theme::choices::set(
            &places.config,
            theme::choices::Slot::Workspace(PROJECT, "alpha"),
            theme::Choice::new("team", None),
        )
        .unwrap();
        let on = places.on(Some("alpha"));
        let created = places.created(Some(&on), Some(Contrast::Standard));
        assert_eq!(created.native.background, [0xab, 0xcd, 0xef]);
        // At the plane root the project's own is drawn: its pick of Purlis Classic.
        let root = places.created(Some(&places.on(None)), Some(Contrast::Standard));
        let classic = theme::classic().first();
        assert_eq!(root.native, classic.native());
        assert_ne!(root.native, theme::default_theme().native());
    }

    #[test]
    fn with_more_contrast_asked_a_window_is_created_in_the_high_contrast_appearance() {
        let places = Places::new("[theme]\nuse = \"sharp\"\n");
        Places::put(&places.plane.join("themes"), "sharp", SHARP);
        let on = places.on(None);
        let more = places.created(Some(&on), Some(Contrast::More));
        assert_eq!(more.native.background, [0, 0, 0]);
        let at = defined(&more.script);
        assert_eq!(at["first"]["contrast"], "more");
        assert_eq!(at["first"]["frames"].as_array().map(Vec::len), Some(1));
        assert_eq!(at["first"]["frames"][0]["more"], true);
        assert_eq!(
            at["first"]["frames"][0]["theme"]["values"]["window.background"],
            "#000000"
        );
        let standard = places.created(Some(&on), Some(Contrast::Standard));
        assert_eq!(standard.native.background, [0x20, 0x20, 0x20]);
        // Where the platform says nothing: both, for the page's `prefers-contrast`, and the
        // window created for the standard one.
        let unread = places.created(Some(&on), None);
        assert_eq!(unread.native.background, [0x20, 0x20, 0x20]);
        let at = defined(&unread.script);
        assert_eq!(at["first"]["contrast"], serde_json::Value::Null);
        let frames: Vec<(bool, &str)> = at["first"]["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| {
                (
                    one["more"].as_bool().unwrap(),
                    one["theme"]["values"]["window.background"]
                        .as_str()
                        .unwrap(),
                )
            })
            .collect();
        assert_eq!(frames, [(false, "#202020"), (true, "#000000")]);
    }

    #[test]
    fn a_first_frame_that_follows_the_system_is_handed_for_each_appearance() {
        let places = Places::new("[theme]\nuse = \"sharp\"\nappearance = \"system\"\n");
        Places::put(&places.plane.join("themes"), "sharp", SHARP);
        let created = places.created(Some(&places.on(None)), Some(Contrast::Standard));
        assert!(created.follows_system);
        let at = defined(&created.script);
        assert_eq!(at["first"]["follows_system"], true);
        let frames: Vec<(bool, &str)> = at["first"]["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| {
                (
                    one["light"].as_bool().unwrap(),
                    one["theme"]["appearance"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(frames, [(false, "dark"), (true, "light")]);
    }

    #[test]
    fn the_first_frame_holds_the_consent_floors_and_names_them() {
        let lost = theme::built_in_for(theme::Appearance::Dark).value("surface.base");
        let places = Places::new("[theme]\nuse = \"breaks\"\n");
        Places::put(
            &places.plane.join("themes"),
            "breaks",
            &format!(
                r#"{{"name":"Breaks","appearances":[{{"id":"dark","appearance":"dark",
                "tokens":{{"state.failed":"{lost}"}}}}]}}"#
            ),
        );
        let at = defined(&places.created(Some(&places.on(None)), None).script);
        let frame = &at["first"]["frames"][0];
        assert_eq!(
            frame["theme"]["values"]["state.failed"],
            theme::built_in_for(theme::Appearance::Dark).value("state.failed")
        );
        assert!(
            frame["floors"]
                .as_str()
                .is_some_and(|said| said.starts_with("Breaks breaks the consent floors")),
            "{frame}"
        );
    }

    #[test]
    fn a_window_is_created_on_the_workspace_its_chat_in_front_worked_in() {
        let places = Places::new("");
        let record = |cwd: &str| purlis_core::reopen::Record {
            chats: vec![purlis_core::reopen::Chat {
                program: "claude".to_owned(),
                name: "one".to_owned(),
                cwd: Some(places.plane.join(cwd)),
                active: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(CreatedOn::plane(&places.plane).workspace, None);
        std::fs::create_dir_all(places.plane.join("workspaces/alpha/repo")).unwrap();
        purlis_core::reopen::write(&places.plane, &record("workspaces/alpha/repo")).unwrap();
        assert_eq!(
            CreatedOn::plane(&places.plane).workspace.as_deref(),
            Some("alpha")
        );
        purlis_core::reopen::write(&places.plane, &record(".")).unwrap();
        assert_eq!(CreatedOn::plane(&places.plane).workspace, None);
    }

    #[test]
    fn the_main_window_is_created_on_the_launch_s_project_else_the_one_remembered_in_front() {
        let places = Places::new("");
        let other = places.plane.with_file_name("other");
        std::fs::create_dir_all(&other).unwrap();
        assert_eq!(CreatedOn::main(Some(&places.config), None), None);
        let remembered = || {
            let mut store = purlis_core::machine::Store::default();
            store.approve(&other, 1, purlis_core::machine::Contribution::of(&other));
            store.windows = vec![purlis_core::machine::Window {
                planes: vec![places.plane.clone(), other.clone()],
                active: 1,
            }];
            Some(store)
        };
        assert_eq!(
            CreatedOn::main_of(Some(&places.plane), remembered).map(|on| on.plane),
            Some(places.plane.clone())
        );
        assert_eq!(
            CreatedOn::main_of(None, remembered).map(|on| on.plane),
            Some(other.clone())
        );
    }

    #[test]
    fn a_remembered_project_not_trusted_now_chooses_nothing_of_the_first_frame() {
        let places = Places::new("[theme]\nuse = \"team\"\n");
        let fonts = places.plane.join("themes/team/fonts");
        std::fs::create_dir_all(&fonts).unwrap();
        let face = b"wOF2 a face of the team's own, never served before it is trusted";
        std::fs::write(fonts.join("Team.woff2"), face).unwrap();
        Places::put(
            &places.plane.join("themes"),
            "team",
            r##"{"name":"Team","appearances":[{"id":"day","appearance":"light",
                "tokens":{"window.background":"#abcdef"}}],
                "fonts":{"ui":{"family":"Team","faces":[{"file":"fonts/Team.woff2"}]}}}"##,
        );
        let window = |store: &mut purlis_core::machine::Store| {
            store.windows = vec![purlis_core::machine::Window {
                planes: vec![places.plane.clone()],
                active: 0,
            }];
        };
        // Never approved, and approved for less than it contributes now.
        let never = {
            let mut store = purlis_core::machine::Store::default();
            store.remember(&places.plane, 1);
            window(&mut store);
            store
        };
        // It now names a program to run that it did not when it was approved.
        purlis_core::reopen::write(
            &places.plane,
            &purlis_core::reopen::Record {
                chats: vec![purlis_core::reopen::Chat {
                    program: "claude".to_owned(),
                    name: "one".to_owned(),
                    active: true,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let stale = {
            let mut store = purlis_core::machine::Store::default();
            store.approve(
                &places.plane,
                1,
                purlis_core::machine::Contribution::default(),
            );
            window(&mut store);
            store
        };
        for store in [never, stale] {
            let on = CreatedOn::main_of(None, || Some(store));
            assert_eq!(on, None);
            let created = places.created(on.as_ref(), Some(Contrast::Standard));
            assert_eq!(created.native, theme::default_theme().native());
        }
        let digest: String = {
            use sha2::Digest;
            sha2::Sha256::digest(face)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        };
        assert!(
            theme::served(&digest).is_err(),
            "no face of it is registered"
        );
        // And a window created on it while this machine's own store holds the stale approval
        // (the store `open_if_approved` reads) is not handed its faces either: a project's face
        // is read only as that approval holds it.
        purlis_core::machine::update(&places.config, |store| {
            store.approve(
                &places.plane,
                1,
                purlis_core::machine::Contribution::default(),
            );
        })
        .unwrap();
        let _ = places.created(Some(&places.on(None)), Some(Contrast::Standard));
        assert!(
            theme::served(&digest).is_err(),
            "no face of it is registered"
        );
        // Approved for what it contributes now: its theme is drawn.
        let mut approved_now = purlis_core::machine::Store::default();
        approved_now.approve(
            &places.plane,
            1,
            purlis_core::machine::Contribution::of(&places.plane),
        );
        window(&mut approved_now);
        let on = CreatedOn::main_of(None, || Some(approved_now));
        assert_eq!(on.as_ref().map(|on| &on.plane), Some(&places.plane));
        purlis_core::machine::update(&places.config, |store| {
            store.approve(
                &places.plane,
                2,
                purlis_core::machine::Contribution::of(&places.plane),
            );
        })
        .unwrap();
        let created = places.created(on.as_ref(), Some(Contrast::Standard));
        assert_eq!(created.native.background, [0xab, 0xcd, 0xef]);
        assert!(
            theme::served(&digest).is_ok(),
            "its face is served once it is trusted"
        );
    }

    #[test]
    fn the_page_is_handed_the_person_s_high_contrast_appearance_for_more_contrast() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(windowprefs::THEME),
            r##"{"name":"Mine","appearances":[{"id":"dark","appearance":"dark"},
                {"id":"sharp","appearance":"dark","high-contrast":true,
                 "tokens":{"surface.base":"#000000"}}]}"##,
        )
        .unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(
            at["theme"]["more"]["theme"]["values"]["surface.base"],
            "#000000"
        );
        assert_ne!(at["theme"]["theme"]["values"]["surface.base"], "#000000");
        assert_eq!(at["theme"]["more"]["floors"], serde_json::Value::Null);

        // A theme with none tagged high contrast is handed once.
        std::fs::write(dir.join(windowprefs::THEME), r#"{"name":"Mine"}"#).unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(at["theme"]["more"], serde_json::Value::Null);
    }

    #[test]
    fn a_machine_with_no_config_home_is_handed_nothing_to_draw_from() {
        let at = defined(&creation_script_from(&TheirsKept::new(), None));
        assert_eq!(at["layout"]["found"], false);
        assert_eq!(at["theme"]["found"], false);
        assert_eq!(at["theme"]["theme"], serde_json::Value::Null);
        assert_eq!(at["built_in"].as_object().map(|it| it.len()), Some(2));
    }

    /// **A project at the font budget**: 64 font files, 32 MiB, in three theme folders, the one
    /// chosen naming 24 of them (8 for each slot), with both appearances and a high-contrast one
    /// of each, so the first frame resolves four times. Approved on this machine and remembered
    /// in front of the first window.
    fn at_the_font_budget() -> Places {
        const FILE: usize = 512 * 1024;
        let places = Places::new("[theme]\nuse = \"team\"\nappearance = \"system\"\n");
        let themes = places.plane.join("themes");
        let mut seed: u64 = 0x2696;
        let mut faces = Vec::new();
        for (folder, files) in [("team", 24), ("two", 24), ("three", 16)] {
            let fonts = themes.join(folder).join("fonts");
            std::fs::create_dir_all(&fonts).unwrap();
            for one in 0..files {
                let mut bytes = b"wOF2".to_vec();
                while bytes.len() < FILE {
                    seed = seed
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    bytes.extend_from_slice(&seed.to_le_bytes());
                }
                bytes.truncate(FILE);
                let name = format!("Face{one:02}.woff2");
                std::fs::write(fonts.join(&name), bytes).unwrap();
                if folder == "team" {
                    faces.push(name);
                }
            }
            if folder != "team" {
                Places::put(&themes, folder, r#"{"name":"Other"}"#);
            }
        }
        let slot = |at: usize| {
            let listed: Vec<String> = faces[at * 8..at * 8 + 8]
                .iter()
                .enumerate()
                .map(|(n, file)| format!(r#"{{"file":"fonts/{file}","weight":{}}}"#, 100 * (n + 1)))
                .collect();
            format!(r#"{{"family":"Team","faces":[{}]}}"#, listed.join(","))
        };
        Places::put(
            &themes,
            "team",
            &format!(
                r##"{{"name":"Team","appearances":[
                    {{"id":"dark","appearance":"dark","tokens":{{"window.background":"#202020"}}}},
                    {{"id":"dark-more","appearance":"dark","high-contrast":true,
                     "tokens":{{"window.background":"#000000"}}}},
                    {{"id":"light","appearance":"light","tokens":{{"window.background":"#f4f4f4"}}}},
                    {{"id":"light-more","appearance":"light","high-contrast":true,
                     "tokens":{{"window.background":"#ffffff"}}}}],
                    "fonts":{{"ui":{},"mono":{},"terminal":{}}}}}"##,
                slot(0),
                slot(1),
                slot(2)
            ),
        );
        let contributes = purlis_core::machine::Contribution::of(&places.plane);
        assert_eq!(
            contributes.fonts.len(),
            64,
            "the budget's 64 files are listed"
        );
        // As a project's files are by the time a window opens on it: settled.
        std::thread::sleep(theme::SETTLED + std::time::Duration::from_millis(100));
        purlis_core::machine::update(&places.config, |store| {
            store.approve(&places.plane, 1, contributes);
            store.windows = vec![purlis_core::machine::Window {
                planes: vec![places.plane.clone()],
                active: 0,
            }];
        })
        .unwrap();
        places
    }

    #[test]
    fn a_remembered_project_s_fonts_are_hashed_once_as_the_main_window_is_created() {
        // #2696: the trust check hashes each font file the project brings, and the first
        // frame's read takes those digests for the same files, unchanged.
        let places = Places::new("[theme]\nuse = \"team\"\nappearance = \"system\"\n");
        let fonts = places.plane.join("themes/team/fonts");
        std::fs::create_dir_all(&fonts).unwrap();
        let face = b"wOF2 the team's face, hashed once as the window is created";
        std::fs::write(fonts.join("Team.woff2"), face).unwrap();
        Places::put(
            &places.plane.join("themes"),
            "team",
            r#"{"name":"Team","appearances":[{"id":"dark","appearance":"dark"},
                {"id":"light","appearance":"light"}],
                "fonts":{"ui":{"family":"Team","faces":[{"file":"fonts/Team.woff2"}]}}}"#,
        );
        purlis_core::machine::update(&places.config, |store| {
            store.approve(
                &places.plane,
                1,
                purlis_core::machine::Contribution::of(&places.plane),
            );
            store.windows = vec![purlis_core::machine::Window {
                planes: vec![places.plane.clone()],
                active: 0,
            }];
        })
        .unwrap();
        std::thread::sleep(theme::SETTLED + std::time::Duration::from_millis(100));
        let on = CreatedOn::main(Some(&places.config), None).expect("the project is trusted");
        assert_eq!(on.measured.hashed(), 1);
        // Two frames for each contrast, one per appearance: the face is not hashed again.
        let created = places.created(Some(&on), None);
        assert!(created.follows_system);
        assert_eq!(on.measured.hashed(), 1);
        let at = defined(&created.script);
        assert_eq!(at["first"]["frames"][0]["theme"]["name"], "Team");
        let digest: String = {
            use sha2::Digest;
            sha2::Sha256::digest(face)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        };
        assert!(
            theme::served(&digest).is_ok(),
            "its face is drawn and served"
        );
    }

    #[test]
    #[ignore = "a measurement: run it in a release build, `--ignored --nocapture`"]
    // The number is the measurement's whole point, read with `--nocapture`.
    #[allow(clippy::disallowed_macros)]
    fn creating_the_main_window_on_a_project_at_the_font_budget_is_timed() {
        // #2696: what the main window's creation reads on the main thread, against ADR 0026's
        // 2 s start budget, for a remembered project at the font budget.
        let places = at_the_font_budget();
        let mut took = Vec::new();
        for _ in 0..7 {
            let started = std::time::Instant::now();
            let on = CreatedOn::main(Some(&places.config), None);
            let checked = started.elapsed();
            let created =
                at_creation_from(&TheirsKept::new(), Some(&places.config), on.as_ref(), None);
            took.push((started.elapsed(), checked));
            // Each of the 64 files hashed once: the check's, which the four frames' reads take.
            assert_eq!(on.as_ref().map(|on| on.measured.hashed()), Some(64));
            assert_eq!(on.map(|on| on.plane), Some(places.plane.clone()));
            let at = defined(&created.script);
            assert_eq!(at["first"]["frames"].as_array().map(Vec::len), Some(4));
            assert_eq!(at["first"]["frames"][0]["theme"]["name"], "Team");
        }
        took.sort();
        let (median, checked) = took[took.len() / 2];
        eprintln!(
            "creation at the font budget: median {median:?} (the trust check {checked:?}), \
             fastest {:?}, slowest {:?}",
            took[0].0,
            took[took.len() - 1].0
        );
        assert!(median < std::time::Duration::from_secs(2), "{median:?}");
    }
}
