// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The extension registry, as the window asks about it.
//!
//! `purlis_core::extension` is the whole of the thinking; this is the wire. Four commands:
//! what has contributed what ([`installed_extensions`]), pick a directory
//! ([`pick_extension`]), read one and ask about it ([`install_extension`]), and record the
//! yes ([`approve_extension`]).
//!
//! **There is no executor here, and that is still deliberate.** ADR 0041 staged it —
//! the registry first, the subprocess afterwards — so that the first extension runtime was not
//! also the thing that invented the list it runs against. The runtime now exists
//! (`purlis_core::executor`, drawn by `crate::views`) and it was built against this list: it
//! re-reads the record and re-takes the fingerprint at every press, and nothing in this file
//! starts anything.
//!
//! **Installing is two clicks, and it has to be**, for the reason `approve_plane` is two: the
//! button that chooses a directory and the button that says yes to what was found in it are
//! different answers to different questions, and collapsing them means the operator approved
//! something before it was drawn. [`install_extension`] writes a row with no approval on it
//! and hands back the question; [`approve_extension`] takes the answer, carrying back the
//! fingerprint that was on screen (charter-app#123's shape).

use std::sync::{Arc, Mutex, OnceLock};

use purlis_core::extension;

/// Where this app's bundle keeps its built-in extensions, relative to its resource directory.
pub(crate) const BUILT_IN_DIR: &str = extension::bundle::BUILT_IN_DIR;

/// The running app's built-in extensions, kept once at launch ([`keep_built_in`]).
static BUILT_IN: OnceLock<extension::BuiltIn> = OnceLock::new();

/// This app's built-in extensions, in its resources (charter-app#339).
///
/// **From the app's resource path and from nothing else** — `Contents/Resources/extensions` in
/// a macOS bundle, `/usr/lib/purlis/extensions` in a `.deb` and an AppImage. That path is the
/// whole of what makes an extension built in (`extension::BuiltIn`), so it is taken from where
/// the running app is and never from a file, a setting or the environment. A build with none in
/// its resources — a development build, a test — has none, and every built-in is simply absent.
pub(crate) fn find_built_in(resources: Option<std::path::PathBuf>) -> extension::BuiltIn {
    resources
        .map(|dir| dir.join(BUILT_IN_DIR))
        .filter(|dir| dir.is_dir())
        .map_or_else(extension::BuiltIn::none, extension::BuiltIn::at)
}

/// Keep what [`find_built_in`] found for the life of the process: `setup`'s, once. A second
/// call changes nothing, so the answer cannot move while the app runs.
pub(crate) fn keep_built_in(found: extension::BuiltIn) {
    let _ = BUILT_IN.set(found);
}

/// The running app's built-in extensions, as [`keep_built_in`] kept them; none before it ran.
pub(crate) fn built_in() -> extension::BuiltIn {
    BUILT_IN.get().cloned().unwrap_or_default()
}

/// The question charter asks before an extension contributes anything.
///
/// A mirror of [`extension::Prompt`] rather than the thing itself, because `purlis-core` never
/// depends on the app and the app's wire types are generated into TypeScript.
///
/// **charter's own sentences travel with it rather than being written in the dialog.** That is
/// the point of carrying them: a window that composed its own words about what an extension can
/// reach could drift kinder than the truth one edit at a time, and the truth here is
/// uncomfortable enough that kinder is the likely direction. Since charter-app#152 there are
/// three of them, because the fingerprint note acquired an exception and an exception the window
/// worded itself would be the same drift through a smaller door.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ExtensionAsk {
    /// The id the record is keyed by, and the id the yes is recorded against.
    pub id: String,
    /// What to call it, as its own manifest does.
    pub name: String,
    /// Where it is. Shown and not merely carried: an extension is approved at a path, and one
    /// re-installed from another directory is a different question.
    pub path: String,
    /// Every contribution it declares, in charter's own words, one line each.
    pub declares: Vec<String>,
    /// The fingerprint of the bytes that were read to build this question. It goes back to
    /// [`approve_extension`] untouched, so the yes is for what was shown.
    pub fingerprint: String,
    /// Whether this is a first ask rather than a re-ask.
    pub first: bool,
    /// `extension::RUNS_AS_YOU` — that a plugin runs with the operator's own access and that
    /// the list above is a declaration, not a limit.
    pub runs_as_you: String,
    /// `extension::FINGERPRINTED` — that the fingerprint catches a change and is not a
    /// boundary.
    pub fingerprint_note: String,
    /// `extension::state_note` — which one directory charter does NOT read, when this
    /// extension declares one, and `null` when it declares none (charter-app#152).
    ///
    /// It travels for the same reason the other two do. The fingerprint note says charter read
    /// every file in the directory; the exception to that sentence belongs on the same screen
    /// as the sentence, in the core's words, or the wording has the defect #152 was opened over
    /// one carve-out later.
    pub state_note: Option<String>,
}

impl From<extension::Prompt> for ExtensionAsk {
    fn from(asked: extension::Prompt) -> Self {
        Self {
            id: asked.id,
            name: asked.name,
            path: asked.path,
            declares: asked.declares,
            fingerprint: asked.fingerprint,
            first: asked.first,
            runs_as_you: asked.runs_as_you,
            fingerprint_note: asked.fingerprint_note,
            state_note: asked.state_note,
        }
    }
}

/// One row of "what has contributed what to this window".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ExtensionRow {
    pub id: String,
    /// What to call it: its manifest's name when charter could read one, else its id.
    pub name: String,
    pub path: String,
    /// `approved`, `new` or `changed`.
    pub standing: String,
    /// `installed`, or `app` for a built-in extension: one that ships inside charter, is
    /// trusted through the app, and is turned off rather than removed (charter-app#339).
    pub source: String,
    /// Whether it is on, on this machine. Only a built-in is ever off.
    pub on: bool,
    /// The themes it is contributing **right now** — empty unless it is approved, so the row
    /// says what is in force rather than what was asked for.
    pub themes_in_force: Vec<String>,
    /// What is wrong with each colour theme it is contributing right now, as the theme engine
    /// read it: one entry per theme with something to say, beside the row's other complaints.
    pub theme_trouble: Vec<ThemeTrouble>,
    /// Every contribution it declares, whether or not any of it is in force.
    pub declares: Vec<String>,
    /// Why charter could not read it, when it could not. A row that contributes nothing says
    /// why rather than going quiet.
    pub refused: Option<String>,
    /// The question to ask about it, when there is one to ask.
    pub ask: Option<ExtensionAsk>,
}

/// What has contributed what to this window — ADR 0041's item 2, and the thing every
/// later decision about extensions is read off.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct InstalledExtensions {
    /// charter's own themes, which are compiled into the window and are never asked about.
    /// Listed so that the answer to "what is contributing" is one list rather than one per
    /// source.
    pub built_in_themes: Vec<String>,
    pub extensions: Vec<ExtensionRow>,
    /// Why the record could not be read, when it could not — in which case nothing an
    /// extension declares is in force and `extensions` is empty.
    pub unreadable: Option<String>,
    /// Rows the record held and charter dropped, with the reason for each.
    pub dropped: Vec<String>,
}

/// What is wrong with one colour theme an extension is contributing (#2321), in the theme
/// engine's words: why it is refused, or what it left out of a theme it draws.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ThemeTrouble {
    /// The theme's name, as the manifest gives it.
    pub name: String,
    /// The engine's sentences, one each: why it is refused, or what it left out.
    pub complaints: Vec<String>,
    /// Whether the engine refused the theme, so Purlis is drawn in its place.
    pub refused: bool,
}

/// One theme an approved extension is contributing, as the window receives it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ExtensionTheme {
    /// The extension it came from, so the window can say whose theme it is drawing.
    pub extension: String,
    /// What to call it, as the manifest does.
    pub name: String,
    /// The file's **text**, unparsed: an icon theme's, which `theme/icons.ts` parses against its
    /// closed vocabulary (path data and `icon.*` tokens). A colour theme never reaches the window
    /// as text: the core's theme engine resolves it ([`theme_in_force`]).
    ///
    /// It is the text the fingerprint was taken over, handed on rather than fetched again.
    pub text: String,
}

/// **The theme the window draws** (spec #2313): the core's theme engine resolves it from the six
/// theme levels ([`purlis_core::theme::resolve_levels`], #2323) and the window only applies what
/// comes back.
///
/// `at` is the project and workspace in front ([`ThemeAt`]); the engine reads their levels and
/// theme folders itself. With no project in front, `pick` is what the window was told to draw,
/// and the person's machine default is beneath it. `on` is the extensions the project has on
/// (`extensions_on`), `None` with no project in front, when every approved extension's theme may
/// be drawn. `light` is the operating system's appearance, which `system` follows; the window
/// asks again when it changes.
///
/// The themes approved extensions contribute come from a survey, which reads and fingerprints
/// every extension's files, so it is taken only when the answer needs one and kept until
/// `reread` (an approval or a removal changes them). An extension that is new, changed or
/// unreadable contributes nothing: [`extension::Surveyed::themes_in_force`] is what answers.
///
/// **And the theme folders it is read from are watched for this window** (#2324,
/// [`crate::themewatch`]): from the config home and the plane root held here, never from a path
/// the window or a chat names. A save in one tells the window to ask again.
///
/// **A theme draft of a chat working where the window is draws in place of every level**
/// (#2332, [`crate::themedrafts`]): the chats are this app's own record of where each works, and
/// the draft is drawn only while it holds every hard floor; its last good version stays
/// otherwise. `kept` asks for the theme as kept, without any draft: what Compare shows while
/// it is held. The drafts are watched with the folders.
///
/// The window's alone (#2697): a link is served [`theme_in_force_at`], the same answer read the
/// same way, which watches nothing for anyone.
#[tauri::command]
#[specta::specta]
// The window and the state are Tauri's to hand in; the rest is the window's question.
#[allow(clippy::too_many_arguments)]
pub async fn theme_in_force(
    window: tauri::WebviewWindow,
    planes: tauri::State<'_, crate::planes::Planes>,
    pick: Option<String>,
    on: Option<Vec<String>>,
    light: bool,
    more_contrast: bool,
    reread: bool,
    at: Option<ThemeAt>,
    kept: Option<bool>,
    person: Option<String>,
) -> Result<ThemeInForce, String> {
    let asked = ThemeAsked::of(&planes, pick, on, light, more_contrast, reread, at, true)?
        .drawn_for(person.as_deref());
    tauri::async_runtime::spawn_blocking(move || {
        crate::themewatch::follow(&window, asked.target());
        asked.read(kept)
    })
    .await
    .map_err(|err| format!("reading the theme in force did not finish: {err}"))
}

/// **The theme in force at a project and workspace, with no window** (#2697): what
/// [`theme_in_force`] answers for the same question, read the same way, and served on a link.
/// It takes no window and points no theme watch anywhere, so a link's call never changes what
/// any window watches.
///
/// **It draws the theme as kept, never a chat's draft** (#2332): a draft is drawn in the
/// person's own window, for the chats this app records working where it is, and is watched with
/// that window's folders. A link is answered the theme the levels choose.
#[tauri::command]
#[specta::specta]
// The state is Tauri's to hand in; the rest is the link's question.
#[allow(clippy::too_many_arguments)]
pub async fn theme_in_force_at(
    planes: tauri::State<'_, crate::planes::Planes>,
    pick: Option<String>,
    on: Option<Vec<String>>,
    light: bool,
    more_contrast: bool,
    reread: bool,
    at: Option<ThemeAt>,
    person: Option<String>,
) -> Result<ThemeInForce, String> {
    let asked = ThemeAsked::of(&planes, pick, on, light, more_contrast, reread, at, false)?
        .drawn_for(person.as_deref());
    tauri::async_runtime::spawn_blocking(move || asked.for_a_link())
        .await
        .map_err(|err| format!("reading the theme in force did not finish: {err}"))
}

/// **One ask of the theme in force**, from the roots purlis holds: the config home, and the plane
/// root `Planes` holds the project at, never a path the asker names.
struct ThemeAsked {
    config: std::path::PathBuf,
    plane: Option<std::path::PathBuf>,
    workspace: Option<String>,
    /// The chats drafting where the window is, from this app's own record (#2332): none for a
    /// link's ask.
    chats: Vec<crate::chats::Drafting>,
    pick: Option<String>,
    on: Option<Vec<String>>,
    light: bool,
    more_contrast: bool,
    reread: bool,
    /// The person's layer the asker draws (#2925): its colours and contrast, drawn into the
    /// answer. Each window and each link asks with its own.
    person: purlis_core::theme::Layer,
}

impl ThemeAsked {
    /// The ask, answered with the person's layer `person` names ([`person_layer`]).
    fn drawn_for(self, person: Option<&str>) -> Self {
        Self {
            person: person_layer(person),
            ..self
        }
    }

    /// The ask, its project found among the ones held; with `drafts`, the chats drafting there.
    // The window's question, as it asks it.
    #[allow(clippy::too_many_arguments)]
    fn of(
        planes: &crate::planes::Planes,
        pick: Option<String>,
        on: Option<Vec<String>>,
        light: bool,
        more_contrast: bool,
        reread: bool,
        at: Option<ThemeAt>,
        drafts: bool,
    ) -> Result<Self, String> {
        let config = config_root()?;
        let (plane, workspace, chats) = match at {
            Some(ThemeAt { plane, workspace }) => {
                let held = planes.held(&plane)?;
                let chats = if drafts {
                    crate::themedrafts::chats_at(&held, workspace.as_deref())
                } else {
                    Vec::new()
                };
                (Some(held.root().to_path_buf()), workspace, chats)
            }
            None => (None, None, Vec::new()),
        };
        Ok(Self {
            config,
            plane,
            workspace,
            chats,
            pick,
            on,
            light,
            more_contrast,
            reread,
            person: purlis_core::theme::Layer::default(),
        })
    }

    /// Where the theme is read from: what the window that asks watches.
    fn target(&self) -> crate::themewatch::Target {
        crate::themewatch::Target {
            config: self.config.clone(),
            plane: self.plane.clone(),
            workspace: self.workspace.clone(),
            drafts: self.chats.iter().map(|one| one.id.clone()).collect(),
        }
    }

    /// **The answer a link is given** (D-2697-3): the theme as kept, with no chat's draft and no
    /// chat's record read, whatever the ask holds.
    fn for_a_link(self) -> ThemeInForce {
        Self {
            chats: Vec::new(),
            ..self
        }
        .read(Some(true))
    }

    /// **The answer**, read and resolved by the core's engine; `kept` asks for the theme as
    /// kept, without any draft. Nothing here watches.
    fn read(self, kept: Option<bool>) -> ThemeInForce {
        let root = self.config;
        if self.reread {
            *offered_kept() = None;
        }
        let at = self.plane.as_deref();
        let workspace = self.workspace.as_deref();
        let mut levels = purlis_core::theme::Levels::read(Some(&root), at, workspace);
        if at.is_none() {
            levels.project_default = self
                .pick
                .as_deref()
                .and_then(|pick| purlis_core::theme::Choice::new(pick, None));
        }
        let drafted = match (at, kept) {
            (Some(plane), None | Some(false)) => crate::themedrafts::drafted_here(
                &root,
                plane,
                workspace,
                &self.chats,
                &levels,
                self.light,
                contrast_of(self.more_contrast),
            ),
            _ => None,
        };
        let folders = purlis_core::theme::Folders::of(Some(&root), at, workspace);
        let theirs = crate::windowprefs::THEIRS.get(Some(&root));
        let offered = || {
            offered_kept()
                .get_or_insert_with(|| offered_of(&extension::survey(&root, &built_in())))
                .clone()
        };
        if let Some((Some(drawn), draft)) = drafted.clone() {
            // The theme as kept, asked only whether it is refused: its Notice waits while the
            // draft is drawn, and the strip says so (#2708).
            let kept = theme_in_force_of(
                &levels,
                &folders,
                self.on.as_deref(),
                theirs.theme.as_ref(),
                self.light,
                contrast_of(self.more_contrast),
                offered,
                |_| None,
            );
            return ThemeInForce {
                draft: Some(crate::themedrafts::WindowDraft {
                    kept_refused: kept.refused.is_some(),
                    ..draft
                }),
                // A draft drawn is not refused: there is nothing to review.
                ..answer_of(drawn.under(&self.person), &levels, |_| None)
            };
        }
        let answer = theme_in_force_under(
            &levels,
            &folders,
            self.on.as_deref(),
            theirs.theme.as_ref(),
            self.light,
            contrast_of(self.more_contrast),
            &self.person,
            offered,
            |faces| {
                at.and_then(|plane| {
                    review_kept(plane, faces, || crate::planes::theme_review(&root, plane))
                })
            },
        );
        ThemeInForce {
            draft: drafted.map(|(_, draft)| draft),
            ..answer
        }
    }
}

/// **Maps today's in-plane theme picks into the person's choices, once** (#2323,
/// [`purlis_core::theme::levels::adopt`]): a sentence for each pick copied, which the window says
/// in a Notice; none when there was nothing to copy.
#[tauri::command]
#[specta::specta]
pub async fn theme_levels_adopt(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        purlis_core::theme::levels::adopt(&config, &root, workspace.as_deref())
            .map_err(|why| format!("purlis could not keep this project's theme choice: {why}"))
    })
    .await
    .map_err(|err| format!("keeping this project's theme choice did not finish: {err}"))?
}

/// **Moves the person's format 1 `theme.json` into a theme folder of their own** and chooses it
/// as the machine default where none is chosen (#2323,
/// [`purlis_core::theme::levels::move_into_a_folder`]), on the person's press of the Notice that
/// offers it, all or nothing. Answers what it did, in a sentence.
#[tauri::command]
#[specta::specta]
pub async fn theme_move_into_a_folder() -> Result<String, String> {
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let moved = purlis_core::theme::levels::move_into_a_folder(&config);
        // The file moved: the next ask reads the folder, not the file.
        crate::windowprefs::THEIRS.forget();
        moved.map(|moved| moved_said(&moved))
    })
    .await
    .map_err(|err| format!("moving the theme did not finish: {err}"))?
}

/// What a move into a theme folder says it did.
fn moved_said(moved: &purlis_core::theme::levels::Moved) -> String {
    if moved.chosen {
        format!(
            "Your theme is in the theme folder {}, and it is your machine default.",
            moved.name
        )
    } else {
        format!(
            "Your theme is in the theme folder {}. Your machine default stays the theme you \
             chose before.",
            moved.name
        )
    }
}

/// **The person's own theme choice** for the project at `plane`, or for `workspace` in it
/// (#2323, [`purlis_core::theme::choices`]): `theme` and `appearance` as a pick names them, and
/// `from`, the level of the theme folder it picks (`workspace`, `project` or `machine`, #2516),
/// kept in machine config by the project's id and never in a project file; `theme` of `None`
/// takes the choice away. A choice of the theme already chosen there keeps its option values
/// (#2527). What Settings › Theme writes at the Project and Workspace levels.
#[tauri::command]
#[specta::specta]
pub async fn theme_choose(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    theme: Option<String>,
    appearance: Option<String>,
    from: Option<String>,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        theme_choose_of(
            &config,
            &root,
            workspace.as_deref(),
            theme.as_deref(),
            appearance.as_deref(),
            from.as_deref(),
        )
    })
    .await
    .map_err(|err| format!("keeping your theme choice did not finish: {err}"))?
}

/// [`theme_choose`] without a runtime.
fn theme_choose_of(
    config: &std::path::Path,
    root: &std::path::Path,
    workspace: Option<&str>,
    theme: Option<&str>,
    appearance: Option<&str>,
    from: Option<&str>,
) -> Result<(), String> {
    use purlis_core::theme::{Folders, choices, levels};
    let key = levels::project_key(root).ok_or_else(|| {
        "purlis cannot tell which project this is, so it keeps no choice".to_owned()
    })?;
    let reach = if workspace.is_some() {
        levels::Reach::Workspace
    } else {
        levels::Reach::Project
    };
    let folders = Folders::of(Some(config), Some(root), workspace);
    let choice = choice_of(theme, appearance, from, &folders, reach)?;
    let named = choice
        .as_ref()
        .and_then(|one| crate::themegallery::theme_named(&folders, reach, one));
    let slot = match workspace {
        Some(name) => choices::Slot::Workspace(&key, name),
        None => choices::Slot::Project(&key),
    };
    choices::choose(config, slot, choice, &folders, reach, named.as_ref())
        .map_err(|why| why.to_string())
}

/// **The choice the window asks for** at a level that reaches `reach`: `theme` in `appearance`,
/// of the theme folder of the level `from` names, or, with none, the nearest level within reach
/// that has it (#2516). `None` for no theme; refused for a theme or a level a choice cannot name.
pub(crate) fn choice_of(
    theme: Option<&str>,
    appearance: Option<&str>,
    from: Option<&str>,
    folders: &purlis_core::theme::Folders,
    reach: purlis_core::theme::levels::Reach,
) -> Result<Option<purlis_core::theme::Choice>, String> {
    use purlis_core::theme::{Choice, levels::Reach};
    let Some(theme) = theme else {
        return Ok(None);
    };
    let choice = Choice::new(theme, appearance)
        .ok_or_else(|| format!("{theme} is not a theme a choice can name, so nothing was kept"))?;
    let choice = match from {
        None => choice,
        Some(word) => match Reach::of_word(word) {
            Some(level) if level >= reach => choice.at(level),
            _ => {
                return Err(format!(
                    "{word} is not a level whose theme folders a choice here reaches, so nothing \
                     was kept"
                ));
            }
        },
    };
    Ok(Some(choice.placed(folders, reach)))
}

/// **Where the window is**, for [`theme_in_force`] (#2323): the project in front and the
/// workspace it is on, `None` at its plane root, which resolves from the project's levels down.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct ThemeAt {
    pub plane: crate::planes::PlaneId,
    pub workspace: Option<String>,
}

/// The extension themes the last survey found, until an approval or a removal asks again.
fn offered_kept() -> std::sync::MutexGuard<'static, Option<Arc<[purlis_core::theme::Offered]>>> {
    static KEPT: Mutex<Option<Arc<[purlis_core::theme::Offered]>>> = Mutex::new(None);
    KEPT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// **The themes approved extensions contribute**, from the survey [`theme_in_force`] keeps
/// until an approval or a removal asks again: Settings › Theme's gallery reads the same (#2331).
pub(crate) fn offered_now(root: &std::path::Path) -> Arc<[purlis_core::theme::Offered]> {
    offered_kept()
        .get_or_insert_with(|| offered_of(&extension::survey(root, &built_in())))
        .clone()
}

/// Each theme in force, resolved from the text its fingerprint was taken over.
pub(crate) fn offered_of(seen: &extension::Survey) -> Arc<[purlis_core::theme::Offered]> {
    in_force(seen)
        .into_iter()
        .map(|one| purlis_core::theme::Offered::of(one.extension, one.name, &one.text))
        .collect()
}

/// What the window says the operating system asks for: more contrast when
/// `prefers-contrast: more` matches (#2328).
pub(crate) fn contrast_of(more: bool) -> purlis_core::theme::Contrast {
    if more {
        purlis_core::theme::Contrast::More
    } else {
        purlis_core::theme::Contrast::Standard
    }
}

/// [`theme_in_force`] without a runtime. `review` is asked only for a theme refused for a
/// project font file its approval does not hold (#2647): what its Notice may offer.
// The command's own inputs, and two reads taken only when the answer needs them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn theme_in_force_of(
    levels: &purlis_core::theme::Levels,
    folders: &purlis_core::theme::Folders,
    on: Option<&[String]>,
    theirs: Option<&purlis_core::theme::Theme>,
    light: bool,
    contrast: purlis_core::theme::Contrast,
    offered: impl FnOnce() -> Arc<[purlis_core::theme::Offered]>,
    review: impl FnOnce(&[&str]) -> Option<ThemeReview>,
) -> ThemeInForce {
    theme_in_force_under(
        levels,
        folders,
        on,
        theirs,
        light,
        contrast,
        &purlis_core::theme::Layer::default(),
        offered,
        review,
    )
}

/// [`theme_in_force_of`], with the person's layer `person` drawn into the answer (#2925).
// The command's own inputs, and two reads taken only when the answer needs them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn theme_in_force_under(
    levels: &purlis_core::theme::Levels,
    folders: &purlis_core::theme::Folders,
    on: Option<&[String]>,
    theirs: Option<&purlis_core::theme::Theme>,
    light: bool,
    contrast: purlis_core::theme::Contrast,
    person: &purlis_core::theme::Layer,
    offered: impl FnOnce() -> Arc<[purlis_core::theme::Offered]>,
    review: impl FnOnce(&[&str]) -> Option<ThemeReview>,
) -> ThemeInForce {
    use purlis_core::theme;
    let drawn = theme::resolve_levels(
        &theme::LevelInputs {
            levels,
            folders,
            theirs,
            on: on.map_or(theme::On::Every, theme::On::Only),
            system: if light {
                theme::Appearance::Light
            } else {
                theme::Appearance::Dark
            },
            contrast,
        },
        offered,
    )
    .under(person);
    answer_of(drawn, levels, review)
}

/// What the window is told of `drawn`, the theme the levels (or a draft) chose. `review` is
/// asked only for a theme refused for a project font file its approval does not hold (#2647).
pub(crate) fn answer_of(
    drawn: purlis_core::theme::Drawn,
    levels: &purlis_core::theme::Levels,
    review: impl FnOnce(&[&str]) -> Option<ThemeReview>,
) -> ThemeInForce {
    let review = {
        let faces = drawn.awaiting_faces();
        if faces.is_empty() {
            None
        } else {
            review(&faces)
        }
    };
    let refused = drawn.refused.then(|| ThemeTrouble {
        name: drawn
            .level
            .and_then(|level| levels.each().find(|(at, _)| *at == level))
            .map_or_else(|| "chosen".to_owned(), |(_, choice)| choice.theme.clone()),
        complaints: drawn
            .complaints
            .iter()
            .map(|one| one.said.clone())
            .collect(),
        refused: true,
    });
    ThemeInForce {
        options: drawn
            .theme
            .as_ref()
            .map(|theme| {
                theme
                    .options
                    .iter()
                    .map(|one| WindowOption::from(one).chosen_at(drawn.level))
                    .collect()
            })
            .unwrap_or_default(),
        refused,
        review,
        theme: drawn.theme.as_ref().map(WindowTheme::from),
        complaints: drawn.complaints.into_iter().map(|one| one.said).collect(),
        follows_system: drawn.follows_system,
        floors: drawn.held.map(|held| held.said()),
        draft: None,
        mine: drawn.mine.into_iter().map(|one| one.said).collect(),
    }
}

/// One option of the theme drawn (#2325), as Settings shows it: what it is called, its values,
/// the value drawn and whether a choice made it (`false`: the theme's default).
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowOption {
    pub id: String,
    pub label: String,
    /// One line on what it changes; empty when the theme says none.
    pub description: String,
    /// `toggle` or `choice`.
    pub kind: String,
    /// Its values in the theme's order, each with what a person calls it: a toggle's are `off`
    /// and `on`.
    pub values: Vec<WindowOptionValue>,
    /// The value drawn.
    pub value: String,
    /// The theme's default.
    pub default: String,
    /// Whether a choice made the value drawn; `false` when it is the theme's default.
    pub chosen: bool,
    /// **The level whose choice made the value drawn** (#2527: `workspace-choice`,
    /// `project-choice` or `machine-default`), as the source badge names it; `null` when it is
    /// the theme's default.
    pub level: Option<String>,
}

impl WindowOption {
    /// The option as the choice of `level` drew it: a chosen value names that level.
    pub(crate) fn chosen_at(self, level: Option<purlis_core::theme::Level>) -> Self {
        let level = level
            .filter(|_| self.chosen)
            .and_then(|level| serde_json::to_value(level).ok())
            .and_then(|it| it.as_str().map(str::to_owned));
        Self { level, ..self }
    }
}

/// One value of a theme option.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowOptionValue {
    pub id: String,
    pub label: String,
}

impl From<&purlis_core::theme::OptionDrawn> for WindowOption {
    fn from(drawn: &purlis_core::theme::OptionDrawn) -> Self {
        let option = &drawn.option;
        Self {
            id: option.id.clone(),
            label: option.label.clone(),
            description: option.description.clone(),
            kind: match option.kind {
                purlis_core::theme::Kind::Toggle => "toggle",
                purlis_core::theme::Kind::Choice => "choice",
            }
            .to_owned(),
            values: option
                .values
                .iter()
                .map(|one| WindowOptionValue {
                    id: one.id.clone(),
                    label: one.label.clone(),
                })
                .collect(),
            value: drawn.value.clone(),
            default: option.default.clone(),
            chosen: drawn.chosen,
            level: None,
        }
    }
}

/// What [`theme_in_force`] answers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct ThemeInForce {
    /// The theme to draw; `null` keeps the one the window draws (an extension theme that is not
    /// JSON at all, which nothing picked).
    pub theme: Option<WindowTheme>,
    /// What resolving it had to put right.
    pub complaints: Vec<String>,
    /// Whether it follows the operating system's appearance.
    pub follows_system: bool,
    /// The Notice that names the consent floors the theme breaks, when it breaks one: what is
    /// drawn has purlis's own values for those tokens.
    pub floors: Option<String>,
    /// The options of the theme drawn, each with the value it is drawn with (#2325): empty for
    /// a theme with none.
    pub options: Vec<WindowOption>,
    /// The theme chosen, when the engine refused it and Purlis is drawn in its place, with why
    /// (#2324): what the window says in a Notice, so a save that is not a theme, or a project's
    /// font file changed since it was approved, is never a silent fallback.
    pub refused: Option<ThemeTrouble>,
    /// Whether the project's trust ask can let the refused theme be drawn (#2647): a font file
    /// of the project's or a workspace's theme folder that this machine's approval does not
    /// hold. `fonts`: the project asks about font files alone, and the refused Notice offers
    /// Review and allow (`review_plane`). `more`: it asks for more than font files, and the
    /// Notice offers no button. `unapproved`: this machine never approved the project, and the
    /// Notice offers no button. `null` otherwise.
    pub review: Option<ThemeReview>,
    /// The theme draft of a chat working where the window is (#2332): drawn in place of what
    /// the levels choose while it holds every hard floor, and named on the draft strip either
    /// way. `None` when no chat there has one, or the theme as kept was asked for.
    pub draft: Option<crate::themedrafts::WindowDraft>,
    /// What drawing the person's own colours over this theme left out (#2925): each colour that
    /// would put a pair under its floor here, said once. Empty when none was.
    pub mine: Vec<String>,
}

/// What a refused theme's Notice may offer (#2647, [`ThemeInForce::review`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ThemeReview {
    /// The project asks again about font files alone: Review and allow.
    Fonts,
    /// It asks for more than font files: no button, and where it is reviewed instead.
    More,
    /// This machine never approved it: no button, and where it is reviewed instead.
    Unapproved,
}

/// The review state last found for each project, with the faces its refused theme waited on.
type ReviewsKept =
    std::collections::BTreeMap<std::path::PathBuf, (Vec<String>, Option<ThemeReview>)>;

/// At most this many projects' review states are kept; past it, all are found again.
const MOST_REVIEWS_KEPT: usize = 32;

fn reviews_kept() -> std::sync::MutexGuard<'static, ReviewsKept> {
    static KEPT: Mutex<ReviewsKept> = Mutex::new(std::collections::BTreeMap::new());
    KEPT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// **What a refused theme's Notice may offer for the project at `root`, found once per set of
/// faces it waits on** (#2647): `find` reads and hashes what the project contributes, so it is
/// asked again only when `faces` (each `<path> <sha-256>`, [`purlis_core::theme::Drawn::awaiting_faces`])
/// differ from the last asked for that project. A redraw under live reload that changed no face
/// reads nothing. What it answers is only which Notice to say: a press asks `review_plane`,
/// which reads the project again and refuses an ask about more than font files.
pub(crate) fn review_kept(
    root: &std::path::Path,
    faces: &[&str],
    find: impl FnOnce() -> Option<ThemeReview>,
) -> Option<ThemeReview> {
    if let Some((kept, review)) = reviews_kept().get(root)
        && kept.iter().map(String::as_str).eq(faces.iter().copied())
    {
        return *review;
    }
    // Found with the lock let go: it reads the project's files.
    let review = find();
    let mut kept = reviews_kept();
    if kept.len() >= MOST_REVIEWS_KEPT && !kept.contains_key(root) {
        kept.clear();
    }
    kept.insert(
        root.to_path_buf(),
        (faces.iter().map(|one| (*one).to_owned()).collect(), review),
    );
    review
}

/// A resolved theme as the window applies it: the window's `Theme`, every token and every
/// motion value present.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowTheme {
    pub name: String,
    /// `dark` or `light`.
    pub appearance: String,
    pub values: std::collections::BTreeMap<String, String>,
    pub motion: WindowMotion,
    /// The theme's icon set, by view id: empty when the rails draw the window's own marks.
    pub icons: std::collections::BTreeMap<String, WindowIcon>,
    /// The harness glyphs of its icon set, by name (#2922): empty when the window draws its own.
    pub glyphs: std::collections::BTreeMap<String, WindowIcon>,
    /// The geometry's custom properties, written by the engine, for each density (#2326).
    pub geometry: WindowGeometry,
    /// The UI, mono and terminal fonts (#2326).
    pub fonts: WindowFonts,
    /// The tokens that are not colours (#2327).
    pub typed: WindowTyped,
    /// How a workspace's colour tints it (#2329, #2393): the window keeps only the arithmetic.
    pub tint: WindowTint,
    /// The stylesheet of its component parts (#2903), written by the engine
    /// (`purlis_core::theme::parts::stylesheet`): empty for a theme that names none.
    pub parts: String,
}

/// A resolved theme's tokens that are not colours (#2327), by token: flags, choices, numbers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowTyped {
    pub flags: std::collections::BTreeMap<String, bool>,
    pub choices: std::collections::BTreeMap<String, String>,
    pub numbers: std::collections::BTreeMap<String, f64>,
}

/// A theme's tint recipe as the window draws it (`purlis_core::theme::Tint`): both lists are
/// empty for a theme a workspace's colour does not tint.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowTint {
    /// The tokens the colour of the workspace in front turns on the whole window.
    pub window: Vec<String>,
    /// The tokens a workspace's colour turns on its own tab and its chat strip.
    pub tabs: Vec<String>,
    /// Each workspace colour's name and the OKLCH hue it gives, in the palette's order.
    pub hues: Vec<WindowHue>,
    /// The rules every tint holds, whatever the hue.
    pub apart: Vec<WindowApart>,
}

/// A tint holds `token`, tinted, at least `distance` from `from` in OKLab, `from` as the tint
/// draws it (`purlis_core::theme::apart_chroma`), giving less colour at that hue when it must.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowApart {
    pub token: String,
    pub from: String,
    pub distance: f64,
    /// Whether the rule is held alone: only its own sides give way to it (#3130).
    pub alone: bool,
}

/// One workspace colour's hue in a theme, in degrees.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowHue {
    pub name: String,
    pub hue: f64,
}

/// One icon of a theme's set (#2337): path data on a 24 grid, which the window strokes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowIcon {
    pub paths: Vec<String>,
}

/// One map of a theme's icon set as the window is handed it, by name.
fn window_icons(
    symbols: &std::collections::BTreeMap<&'static str, purlis_core::theme::Symbol>,
) -> std::collections::BTreeMap<String, WindowIcon> {
    symbols
        .iter()
        .map(|(name, symbol)| {
            (
                (*name).to_owned(),
                WindowIcon {
                    paths: symbol.paths.clone(),
                },
            )
        })
        .collect()
}

/// A theme's geometry as the custom properties the window writes, for each density: the
/// person's density picks one ([`purlis_core::theme::Layer`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowGeometry {
    pub comfortable: std::collections::BTreeMap<String, String>,
    pub compact: std::collections::BTreeMap<String, String>,
}

/// A theme's three fonts.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowFonts {
    pub ui: WindowFont,
    pub mono: WindowFont,
    pub terminal: WindowFont,
}

/// One font: the family list the window draws with, as the engine wrote it, and the faces it
/// asks purlis for by digest (`theme_font`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowFont {
    /// The theme's label for it; `null` for core's.
    pub family: Option<String>,
    pub stack: String,
    pub faces: Vec<WindowFace>,
}

/// One face of a font.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowFace {
    /// The sha-256 of its bytes, which `theme_font` answers.
    pub asset: String,
    pub weight: u16,
    /// `normal` or `italic`.
    pub style: String,
}

impl From<&purlis_core::theme::Font> for WindowFont {
    fn from(font: &purlis_core::theme::Font) -> Self {
        Self {
            family: font.family.clone(),
            stack: font.stack.clone(),
            faces: font
                .faces
                .iter()
                .map(|face| WindowFace {
                    asset: face.asset.clone(),
                    weight: face.weight,
                    style: match face.style {
                        purlis_core::theme::Style::Normal => "normal",
                        purlis_core::theme::Style::Italic => "italic",
                    }
                    .to_owned(),
                })
                .collect(),
        }
    }
}

/// `geometry` as the window writes it.
fn written(geometry: &purlis_core::theme::Geometry) -> std::collections::BTreeMap<String, String> {
    purlis_core::theme::geometry::properties(geometry)
        .into_iter()
        .map(|(property, value)| (property.to_owned(), value))
        .collect()
}

/// A resolved theme's motion: milliseconds, and the four numbers of each cubic Bézier.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct WindowMotion {
    pub durations: std::collections::BTreeMap<String, u32>,
    pub easings: std::collections::BTreeMap<String, [f64; 4]>,
}

impl From<&purlis_core::theme::ResolvedTheme> for WindowTheme {
    fn from(theme: &purlis_core::theme::ResolvedTheme) -> Self {
        Self::with_parts(theme, purlis_core::theme::parts::stylesheet(theme))
    }
}

impl WindowTheme {
    /// `theme` in the window's shape, drawing its parts from `parts`.
    pub(crate) fn with_parts(theme: &purlis_core::theme::ResolvedTheme, parts: String) -> Self {
        let owned = |map: &std::collections::BTreeMap<&'static str, String>| {
            map.iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect()
        };
        Self {
            name: theme.name.clone(),
            appearance: match theme.appearance {
                purlis_core::theme::Appearance::Dark => "dark",
                purlis_core::theme::Appearance::Light => "light",
            }
            .to_owned(),
            values: owned(&theme.values),
            motion: WindowMotion {
                durations: theme
                    .motion
                    .durations
                    .iter()
                    .map(|(key, ms)| ((*key).to_owned(), *ms))
                    .collect(),
                easings: theme
                    .motion
                    .easings
                    .iter()
                    .map(|(key, curve)| ((*key).to_owned(), *curve))
                    .collect(),
            },
            icons: window_icons(&theme.icons.views),
            glyphs: window_icons(&theme.icons.glyphs),
            geometry: WindowGeometry {
                comfortable: written(&theme.geometry),
                compact: written(&theme.compact),
            },
            fonts: WindowFonts {
                ui: (&theme.fonts.ui).into(),
                mono: (&theme.fonts.mono).into(),
                terminal: (&theme.fonts.terminal).into(),
            },
            typed: WindowTyped {
                flags: theme
                    .typed
                    .flags
                    .iter()
                    .map(|(key, on)| ((*key).to_owned(), *on))
                    .collect(),
                choices: theme
                    .typed
                    .choices
                    .iter()
                    .map(|(key, chosen)| ((*key).to_owned(), (*chosen).to_owned()))
                    .collect(),
                numbers: theme
                    .typed
                    .numbers
                    .iter()
                    .map(|(key, number)| ((*key).to_owned(), *number))
                    .collect(),
            },
            tint: WindowTint {
                window: owned_names(&theme.tint.window()),
                tabs: owned_names(theme.tint.tabs()),
                hues: theme
                    .tint
                    .hues
                    .iter()
                    .map(|one| WindowHue {
                        name: one.name.to_owned(),
                        hue: one.hue,
                    })
                    .collect(),
                apart: purlis_core::theme::APART
                    .iter()
                    .map(|rule| WindowApart {
                        token: rule.token.to_owned(),
                        from: rule.from.to_owned(),
                        distance: rule.distance,
                        alone: rule.alone,
                    })
                    .collect(),
            },
            parts,
        }
    }
}

/// **The bytes of one font a theme in force brings** (#2326): the face whose digest is `asset`,
/// read again by the core's theme engine from inside the theme's folder, under its cap, and only
/// while it still has that digest (`purlis_core::theme::served`). As base64, as a file tab's
/// image is (FM-2); the window makes a `FontFace` from the bytes, so no path and no address ever
/// reach it.
#[tauri::command]
#[specta::specta]
pub async fn theme_font(asset: String) -> Result<String, String> {
    use base64::Engine as _;
    tauri::async_runtime::spawn_blocking(move || purlis_core::theme::served(&asset))
        .await
        .map_err(|err| format!("reading the font did not finish: {err}"))?
        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// What [`theme_person`] answers: the person's layer, and what reading it had to put right.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct PersonInForce {
    /// `comfortable` or `compact`: which of a theme's geometries is drawn.
    pub density: String,
    /// The family list for each font the person named (`ui`, `mono`, `terminal`), as the engine
    /// wrote it.
    pub fonts: std::collections::BTreeMap<String, String>,
    /// `system` or `reduced` (#2925): `reduced` turns every motion off, whatever the system says.
    pub motion: String,
    /// `standard` or `more` (#2925): `more` draws a theme's high-contrast appearance and raises
    /// the floors, which the engine does to every theme it answers the window with.
    pub contrast: String,
    /// The person's colour for each token they set (#2925), as the engine read it: what Settings
    /// shows. The engine draws them over each theme, held to its floors.
    pub colours: std::collections::BTreeMap<String, String>,
    pub complaints: Vec<String>,
}

impl PersonInForce {
    /// The person's settings as the layout file's `appearance` holds them, read by the engine,
    /// and the layer the engine draws over every theme for them.
    pub(crate) fn read(
        appearance: Option<&serde_json::Value>,
    ) -> (Self, purlis_core::theme::Layer) {
        let (person, complaints) = purlis_core::theme::Person::read(appearance);
        let layer = person.layer();
        let answered = Self {
            density: match layer.density {
                purlis_core::theme::Density::Comfortable => "comfortable",
                purlis_core::theme::Density::Compact => "compact",
            }
            .to_owned(),
            fonts: layer
                .fonts
                .iter()
                .map(|(slot, stack)| ((*slot).to_owned(), stack.clone()))
                .collect(),
            motion: if layer.reduce_motion {
                "reduced"
            } else {
                "system"
            }
            .to_owned(),
            contrast: match layer.contrast {
                purlis_core::theme::Least::Standard => "standard",
                purlis_core::theme::Least::More => "more",
            }
            .to_owned(),
            colours: layer
                .colours
                .iter()
                .map(|(token, colour)| ((*token).to_owned(), colour.clone()))
                .collect(),
            complaints: complaints.into_iter().map(|one| one.said).collect(),
        };
        (answered, layer)
    }
}

/// **The person's layer an asker draws** (#2925): `person` is the JSON text of what the asking
/// window or link draws of the layout file's `appearance` (its `contrast` and `colours`), read by
/// the engine. Nothing, or text that is not that, is the layer that draws every theme as it is.
pub(crate) fn person_layer(person: Option<&str>) -> purlis_core::theme::Layer {
    let value = person.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
    purlis_core::theme::Person::read(value.as_ref()).0.layer()
}

/// **The person's density, font override, motion, contrast and colours, read by the core's theme
/// engine** (#2326, #2925): what the window draws over every theme once Settings changed them.
/// The colours and the contrast are drawn by the engine into each theme the window then asks
/// for: the window hands them with each ask. The window keeps them in its
/// layout file beside the text sizes, and is handed the same at creation. `appearance` is the
/// layout's `appearance` as JSON text.
#[tauri::command]
#[specta::specta]
pub async fn theme_person(appearance: Option<String>) -> PersonInForce {
    // As JSON text, as the layout itself crosses (`write_layout`): what is not JSON is read as
    // a value of no setting's type, and said.
    let value = appearance.map(|text| {
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or(serde_json::Value::String(text))
    });
    PersonInForce::read(value.as_ref()).0
}

fn owned_names(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| (*token).to_owned()).collect()
}

/// Every icon theme in force (FM-3, #1106): the ones approved extensions contribute, on
/// the colour themes' terms and in [`ExtensionTheme`]'s shape. The text is parsed by `theme/icons.ts`
/// against its closed vocabulary — path data and `icon.*` tokens — and never by this crate.
#[tauri::command]
#[specta::specta]
pub async fn extension_icon_themes() -> Result<Vec<ExtensionTheme>, String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        icons_in_force(&extension::survey(&root, &built_in()))
    })
    .await
    .map_err(|err| format!("reading this machine's icon themes did not finish: {err}"))
}

/// Every panel an approved extension contributes to this window's side region.
///
/// **Its own command, asked once per window and never per workspace focus.** A survey reads
/// every installed extension's whole directory to re-take the fingerprint — ADR 0041's named
/// cost of fingerprinting code — and folding that into `workspace_panels` would put it on the
/// path that has 100 ms to draw, once per click on the workspace strip.
///
/// It is the same shape as [`extension_icon_themes`] one row up, and for the same reason: an
/// extension that is new, changed or unreadable contributes nothing here. That is the registry
/// doing its one job, and a contributed panel rests on it entirely.
#[tauri::command]
#[specta::specta]
pub async fn extension_panels() -> Result<Vec<crate::panels::PanelView>, String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        panels_in_force(&extension::survey(&root, &built_in()))
    })
    .await
    .map_err(|err| format!("reading this machine's panels did not finish: {err}"))
}

/// [`extension_panels`] with the survey already taken, so the shaping is testable without a
/// Tauri runtime to run it on.
fn panels_in_force(seen: &extension::Survey) -> Vec<crate::panels::PanelView> {
    let mut found: Vec<purlis_core::panel::Panel> = seen
        .installed
        .iter()
        .flat_map(|row| row.panels_in_force().iter().cloned())
        .collect();
    // Sorted here rather than in the window, so that two extensions declaring the same `order`
    // land in the same place at every launch instead of the order the record was read in.
    purlis_core::panel::Panel::sort(&mut found);
    found.iter().map(crate::panels::PanelView::from).collect()
}

/// The colour themes approved extensions contribute, with the survey already taken.
fn in_force(seen: &extension::Survey) -> Vec<ExtensionTheme> {
    texts_in_force(seen, extension::Surveyed::themes_in_force)
}

/// [`extension_icon_themes`] with the survey already taken.
fn icons_in_force(seen: &extension::Survey) -> Vec<ExtensionTheme> {
    texts_in_force(seen, extension::Surveyed::icon_themes_in_force)
}

/// Each theme `which` says a row has in force, with the text its fingerprint was taken over.
fn texts_in_force(
    seen: &extension::Survey,
    which: fn(&extension::Surveyed) -> &[extension::Theme],
) -> Vec<ExtensionTheme> {
    let mut themes = Vec::new();
    for row in &seen.installed {
        let Some(found) = &row.found else { continue };
        for theme in which(row) {
            // A theme whose text is somehow not there is dropped rather than sent empty: an
            // empty theme would load as the built-in with a complaint for every token, which
            // looks like a theme that did nothing wrong.
            if let Some(text) = found.theme_text(theme) {
                themes.push(ExtensionTheme {
                    extension: row.id.clone(),
                    name: theme.name.clone(),
                    text: text.to_owned(),
                });
            }
        }
    }
    themes
}

/// The config home, or the reason charter has no machine-level state.
pub(crate) fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "this machine has no config home, so purlis keeps no extensions".to_owned())
}

/// What has contributed what to this window.
///
/// It reads the disk — every installed extension's whole directory, to re-take the fingerprint
/// — because an approval is of bytes and the bytes are what may have changed since. ADR 0041
/// names that cost: *the fingerprint is a hash of code, checked at each launch*, and
/// charter-app#152 widened it from the declared list to the tree. Off the UI thread for exactly
/// that reason, and the cost is measured in #152's PR body rather than left as an estimate.
#[tauri::command]
#[specta::specta]
pub async fn installed_extensions() -> Result<InstalledExtensions, String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || listed(&extension::survey(&root, &built_in())))
        .await
        .map_err(|err| format!("reading this machine's extensions did not finish: {err}"))
}

/// [`installed_extensions`] with the survey already taken, so the shaping is testable without
/// a Tauri runtime to run it on.
fn listed(seen: &extension::Survey) -> InstalledExtensions {
    InstalledExtensions {
        built_in_themes: seen.built_in_themes.clone(),
        extensions: seen
            .installed
            .iter()
            .map(|row| {
                let asked = row.found.as_ref().and_then(|found| {
                    // A row that is already approved has nothing to ask, and a question with
                    // no risk in it is one an operator learns to answer yes to without
                    // reading.
                    (!row.standing.may_contribute())
                        .then(|| ExtensionAsk::from(extension::prompt(found, row.standing)))
                });
                ExtensionRow {
                    id: row.id.clone(),
                    name: row
                        .found
                        .as_ref()
                        .map_or_else(|| row.id.clone(), |found| found.manifest.name.clone()),
                    path: row.path.display().to_string(),
                    standing: row.standing.as_str().to_owned(),
                    source: row.source.as_str().to_owned(),
                    on: row.on,
                    themes_in_force: row
                        .themes_in_force()
                        .iter()
                        .map(|theme| theme.name.clone())
                        .collect(),
                    theme_trouble: theme_trouble_of(row),
                    declares: asked.as_ref().map_or_else(
                        || {
                            row.found.as_ref().map_or_else(Vec::new, |found| {
                                extension::prompt(found, row.standing).declares
                            })
                        },
                        |ask| ask.declares.clone(),
                    ),
                    refused: row.refused.clone(),
                    ask: asked,
                }
            })
            .collect(),
        unreadable: seen.unreadable.clone(),
        dropped: seen.dropped.clone(),
    }
}

/// What is wrong with each colour theme `row` contributes right now: the theme engine reads
/// each from the text its fingerprint was taken over, as it does to draw it.
fn theme_trouble_of(row: &extension::Surveyed) -> Vec<ThemeTrouble> {
    let Some(found) = &row.found else {
        return Vec::new();
    };
    row.themes_in_force()
        .iter()
        .filter_map(|theme| {
            let text = found.theme_text(theme)?;
            let offered = purlis_core::theme::Offered::of(row.id.clone(), theme.name.clone(), text);
            let resolved = offered.theme;
            if resolved.complaints.is_empty() && !resolved.refused {
                return None;
            }
            Some(ThemeTrouble {
                name: offered.name,
                complaints: resolved
                    .complaints
                    .into_iter()
                    .map(|one| one.said)
                    .collect(),
                refused: resolved.refused,
            })
        })
        .collect()
}

/// The folder picker, for an extension this machine does not have.
///
/// **By path, by the operator, from nowhere.** ADR 0041 rejects a registry and a fetch by name
/// — *the moment charter resolves an extension name over the network it owns a supply chain* —
/// so this is the only way one arrives, and it is the same picker `pick_project` uses.
#[tauri::command]
#[specta::specta]
pub async fn pick_extension(app: tauri::AppHandle) -> Result<Option<String>, String> {
    crate::opener::pick_folder(&app).await
}

/// Read the extension at `path`, write it into the record **unapproved**, and hand back the
/// question to ask about it — none for an extension that contributes only themes, which is a set
/// of theme folders and needs no approval (ADR 0098 §3).
///
/// An extension charter cannot read is an error here rather than a silent nothing: the
/// operator pointed at a directory and is owed the reason it is not one.
#[tauri::command]
#[specta::specta]
pub async fn install_extension(path: String) -> Result<Option<ExtensionAsk>, String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let at = std::path::PathBuf::from(path);
        let built_in = built_in();
        let found = extension::install(&root, &built_in, &at).map_err(|why| why.to_string())?;
        let standing = extension::read(&root, &built_in).standing(&found);
        Ok((standing != extension::Standing::Themes)
            .then(|| ExtensionAsk::from(extension::prompt(&found, standing))))
    })
    .await
    .map_err(|err| format!("reading that extension did not finish: {err}"))?
}

/// The operator's answer to [`ExtensionAsk`]: yes.
///
/// `fingerprint` is the one the dialog drew, handed straight back — charter-app#123's fix
/// applied before the defect can be copied. What is recorded is the bytes that were on screen,
/// so a write between the drawing and the click is not consented to; it makes the next launch
/// ask again, which is the whole of the mechanism.
///
/// **This is a separate click from the one that installed it, and it has to be.**
#[tauri::command]
#[specta::specta]
pub async fn approve_extension(
    id: String,
    path: String,
    fingerprint: String,
) -> Result<(), String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        extension::approve(&root, &id, std::path::Path::new(&path), &fingerprint)
            .map_err(|why| why.to_string())
    })
    .await
    .map_err(|err| format!("recording that approval did not finish: {err}"))?
}

/// Take an extension out of the record.
///
/// Its own files are not charter's and are not deleted: ADR 0041's *an extension is
/// re-installable by name; its state is its own problem*. What goes is the path and the
/// approval, which is exactly what ADR 0034 says deleting this machine's state must cost.
#[tauri::command]
#[specta::specta]
pub async fn forget_extension(id: String) -> Result<(), String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        extension::forget(&root, &id).map_err(|why| why.to_string())
    })
    .await
    .map_err(|err| format!("forgetting that extension did not finish: {err}"))?
}

/// Turn a built-in extension on or off on this machine: what the Extensions list offers a
/// built-in in the place of Remove (charter-app#339). Off, it contributes nothing to any
/// project; a project or a workspace can still turn it off on its own, as it can any extension.
#[tauri::command]
#[specta::specta]
pub async fn set_extension_on(id: String, on: bool) -> Result<(), String> {
    let root = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        extension::set_on(&root, &built_in(), &id, on).map_err(|why| why.to_string())
    })
    .await
    .map_err(|err| format!("turning that extension on or off did not finish: {err}"))?
}

// The extension record opened in your editor (#1296): the Extensions dialog's way to mend a
// record purlis could not read. The window names the editor and nothing else: the file is this
// machine's own record, placed by the core, never a path the window sent. Window-only
// (`WINDOW_ONLY`): it starts a program of the person's on a machine file, which no link may.
// Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn open_extension_record(
    app: tauri::AppHandle,
    editor: crate::piecefiles::YourEditor,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let root = config_root()?;
    let var = |name: &str| std::env::var(name).ok();
    match extension::record_in_your_editor(&root, editor.into(), &var)? {
        purlis_core::youreditor::Launch::Url(url) => app
            .opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| format!("the system did not open {url}: {e}")),
        purlis_core::youreditor::Launch::Program { program, args } => {
            crate::piecefiles::started(program, args).await
        }
    }
}

// ---------------------------------------------------------------------------------------
// Per project (charter-app#253, ADR 0048)
// ---------------------------------------------------------------------------------------

/// One setting an extension declares, and what this project resolved it to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectExtensionSetting {
    pub key: String,
    pub title: String,
    /// `bool`, `text` or `choice`.
    pub kind: String,
    /// The words a `choice` may be; empty otherwise.
    pub choices: Vec<String>,
    /// What it is when no file sets it, as text (`true`/`false` for a `bool`).
    pub default: String,
    /// What it is in this project, as text.
    pub value: String,
    /// `default`, `shared` or `local`: which file it came from.
    pub source: String,
}

/// One extension in one project, as the Settings tab draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectExtension {
    pub id: String,
    pub name: String,
    /// `on`, `off`, `needs-approval` or `not-installed`.
    pub state: String,
    /// `default`, `shared`, `workspace` or `local`: which file decided `state`.
    pub source: String,
    pub settings: Vec<ProjectExtensionSetting>,
    /// Each value a file set that charter did not use, and why.
    pub ignored: Vec<ProjectExtensionIgnored>,
}

/// Every extension in one project, and why `charter.local.toml` had no say in them when it had
/// none (charter-app#319).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectExtensions {
    pub extensions: Vec<ProjectExtension>,
    /// The ignore check's sentence while git would carry `charter.local.toml` and it sets an
    /// extension — the one the Settings tab says at the Project level — so the Extensions
    /// group says why a value set there is not applied. The core's `Choices::local_left_out`.
    pub local_left_out: Option<String>,
}

/// A value a file set that charter did not use.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectExtensionIgnored {
    /// `charter.toml`, `charter.local.toml` or `workspaces/<ws>/workspace.json`: the section
    /// that says it.
    pub file: String,
    /// The core's sentence.
    pub why: String,
}

/// Every extension this machine has installed, and every one this project's files name, with
/// what each is in this project — `extension::project::resolve`, shaped for the wire. In
/// `workspace`, when one is named, that workspace's settings are a layer too (charter-app#280):
/// what Settings shows at the Workspace level.
///
/// It takes a survey, so it re-hashes every installed extension's directory: an extension that
/// changed since its yes reads as needing approval here, which is the truth the tab is for. It
/// is asked when the tab is opened and after it saves, never on a timer.
#[tauri::command]
#[specta::specta]
pub async fn project_extensions(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<ProjectExtensions, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        project_rows(
            &extension::survey(&config, &built_in()),
            &extension::project::Choices::read_in(&root, workspace.as_deref()),
        )
    })
    .await
    .map_err(|err| format!("reading this project's extensions did not finish: {err}"))
}

/// [`project_extensions`] without a runtime.
fn project_rows(
    seen: &extension::Survey,
    choices: &extension::project::Choices,
) -> ProjectExtensions {
    let installed = extension::project::Installed::from_survey(seen);
    let extensions = extension::project::resolve(&installed, choices)
        .into_iter()
        .map(|it| {
            let declared = installed
                .iter()
                .find(|one| one.id == it.id)
                .map(|one| one.settings.as_slice())
                .unwrap_or_default();
            ProjectExtension {
                settings: it
                    .settings
                    .iter()
                    .filter_map(|resolved| {
                        let setting = declared.iter().find(|d| d.key == resolved.key)?;
                        let (kind, choices) = match &setting.kind {
                            extension::SettingKind::Bool => ("bool", Vec::new()),
                            extension::SettingKind::Text => ("text", Vec::new()),
                            extension::SettingKind::Choice(words) => ("choice", words.clone()),
                        };
                        Some(ProjectExtensionSetting {
                            key: setting.key.clone(),
                            title: setting.title.clone(),
                            kind: kind.to_owned(),
                            choices,
                            default: as_text(&setting.default),
                            value: as_text(&resolved.value),
                            source: resolved.source.as_str().to_owned(),
                        })
                    })
                    .collect(),
                id: it.id,
                name: it.name,
                state: it.state.as_str().to_owned(),
                source: it.source.as_str().to_owned(),
                ignored: it
                    .ignored
                    .into_iter()
                    .map(|one| ProjectExtensionIgnored {
                        file: file_of(one.source, choices.workspace_file()),
                        why: one.why,
                    })
                    .collect(),
            }
        })
        .collect();
    ProjectExtensions {
        extensions,
        local_left_out: choices.local_left_out().map(str::to_owned),
    }
}

/// The file a source is, as a section of a settings tab names it: a workspace's by its path
/// (`workspace_file`, which a reader's `Choices::workspace_file` gives). The extensions and the
/// harness plugins commands both name it here, so their sentences land in the same section.
pub(crate) fn file_of(
    source: extension::project::Source,
    workspace_file: Option<String>,
) -> String {
    match (source, workspace_file) {
        (extension::project::Source::Workspace, Some(file)) => file,
        (source, _) => source.file().unwrap_or_default().to_owned(),
    }
}

/// A plane for a test (charter-app#319): a git repository with `shared` as its `charter.toml`,
/// `local` as its `charter.local.toml` — which `.gitignore` ignores when `ignored` says, and which
/// git would commit otherwise — and a workspace `alpha`. Made from purlis-core's fixture template
/// (charter-app#262), so the developer's global excludes file does not decide what git would do.
#[cfg(test)]
pub(crate) fn test_plane(shared: &str, local: &str, ignored: bool) -> tempfile::TempDir {
    let plane = tempfile::tempdir().expect("a plane");
    let root = plane.path();
    std::fs::write(root.join("charter.toml"), shared).expect("charter.toml");
    std::fs::write(root.join("charter.local.toml"), local).expect("charter.local.toml");
    if ignored {
        std::fs::write(root.join(".gitignore"), "/charter.local.toml\n").expect(".gitignore");
    }
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("the workspace");
    let template = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/purlis-core/tests/support/git-template"
    );
    let init = purlis_core::forklock::output(
        std::process::Command::new("git").arg("-C").arg(root).args([
            "init",
            "-q",
            &format!("--template={template}"),
        ]),
    )
    .expect("git runs in a test");
    assert!(init.status.success(), "{init:?}");
    plane
}

fn as_text(value: &extension::SettingValue) -> String {
    match value {
        extension::SettingValue::Bool(b) => b.to_string(),
        extension::SettingValue::Text(text) => text.clone(),
    }
}

/// The ids of the extensions that are on in this project — in `workspace`, when one is named
/// (charter-app#280): what the window keeps of the panels, views and themes it surveyed once,
/// while this project and that workspace are in front.
///
/// **The record alone, and no extension's directory** — so it is cheap enough to ask for every
/// project a window holds. What it cannot see, an extension that changed since its yes, the
/// survey already left out of what the window holds, and the executor re-takes the fingerprint
/// at every press. The precedence is `extension::project::resolve`'s, as everywhere else.
#[tauri::command]
#[specta::specta]
pub async fn extensions_on(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        on_ids(
            &extension::read(&config, &built_in()),
            &extension::project::Choices::read_in(&root, workspace.as_deref()),
        )
    })
    .await
    .map_err(|err| format!("reading this project's extensions did not finish: {err}"))
}

/// [`extensions_on`] without a runtime.
fn on_ids(loaded: &extension::Loaded, choices: &extension::project::Choices) -> Vec<String> {
    extension::project::resolve(&extension::project::Installed::from_record(loaded), choices)
        .into_iter()
        .filter(extension::project::Effective::is_on)
        .map(|it| it.id)
        .collect()
}

// ---------------------------------------------------------------------------------------
// Badges and repo cells from facts files (charter-app#340)
// ---------------------------------------------------------------------------------------

/// One status-bar badge, as the window draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FactBadge {
    pub extension: String,
    /// What a person calls the extension, for the badge's title.
    pub name: String,
    pub id: String,
    pub label: String,
    pub value: String,
    /// How long ago the extension said it was true.
    pub age_seconds: u32,
    /// Older than it declared fresh: dimmed, with its age.
    pub stale: bool,
}

/// One repo-table cell.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FactCell {
    pub repo: String,
    pub value: String,
    pub age_seconds: u32,
    pub stale: bool,
}

/// One extra column in the repo table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FactColumn {
    pub extension: String,
    pub id: String,
    pub title: String,
    /// One per repo the facts file filled, by repo name. A repo it did not name has none.
    pub cells: Vec<FactCell>,
}

/// What the extensions on in a project (and workspace) show in the window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ExtensionFacts {
    pub badges: Vec<FactBadge>,
    pub columns: Vec<FactColumn>,
    /// What contributed nothing and should have, in the core's words.
    pub notes: Vec<String>,
}

/// The status bar's badges and the repo table's extra columns for `plane`, in `workspace` when
/// one is named — read from each extension's facts file by the core's one reader
/// (`extension::facts::gather`), **which never starts a program**.
///
/// Asked when a workspace is focused and after an extension answers a view, which is when its
/// facts file is refreshed. It re-takes each contributing extension's fingerprint, so it runs
/// off the thread that draws.
#[tauri::command]
#[specta::specta]
pub async fn extension_facts(
    planes: tauri::State<'_, crate::planes::Planes>,
    heard: tauri::State<'_, crate::heard::Heard>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<ExtensionFacts, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    // What events this project reported left behind, drawn with the facts file's own notes:
    // both are an extension that should have shown or heard something and did not
    // (charter-app#343, `heard.rs`).
    let missed = heard.notes_for(&root);
    let mut read = tauri::async_runtime::spawn_blocking(move || {
        facts_in(&config, &root, workspace.as_deref(), chrono::Utc::now())
    })
    .await
    .map_err(|err| format!("reading this project's extension facts did not finish: {err}"))?;
    read.notes.extend(missed);
    Ok(read)
}

/// [`extension_facts`] without a runtime.
fn facts_in(
    config: &std::path::Path,
    root: &std::path::Path,
    workspace: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> ExtensionFacts {
    use extension::facts;
    let read = facts::gather(
        config,
        &built_in(),
        || extension::project::Choices::read_in(root, workspace),
        now,
        facts::Reading::Window,
    );
    let seconds = |age: u64| u32::try_from(age).unwrap_or(u32::MAX);
    ExtensionFacts {
        badges: read
            .badges
            .into_iter()
            .map(|badge| FactBadge {
                extension: badge.extension,
                name: badge.name,
                id: badge.id,
                label: badge.label,
                value: badge.value,
                age_seconds: seconds(badge.age_seconds),
                stale: badge.stale,
            })
            .collect(),
        columns: read
            .columns
            .into_iter()
            .map(|column| FactColumn {
                extension: column.extension,
                id: column.id,
                title: column.title,
                cells: column
                    .cells
                    .into_iter()
                    .map(|(repo, cell)| FactCell {
                        repo,
                        value: cell.value,
                        age_seconds: seconds(cell.age_seconds),
                        stale: cell.stale,
                    })
                    .collect(),
            })
            .collect(),
        notes: read.notes,
    }
}

// ---------------------------------------------------------------------------------------
// A project's theme (charter-app#273, ADR 0048)
// ---------------------------------------------------------------------------------------

/// One theme a project may pick, as the Theme select lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ThemeOption {
    /// What the file holds: a built-in theme's id, `system`, or `<extension>/<theme>`.
    pub value: String,
    /// What the select shows.
    pub label: String,
}

/// A project's theme, as the Settings tab draws it —
/// `extension::project::theme::resolve`, shaped for the wire.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectTheme {
    /// charter's own themes, following the system, and every theme an extension this machine
    /// approved contributes — whether or not this project has that extension on.
    pub options: Vec<ThemeOption>,
    /// What the files pick, in force, as a file holds it; `null` when none picks one.
    pub picked: Option<String>,
    /// `charter.toml`, `charter.local.toml` or `workspaces/<ws>/workspace.json`: the file
    /// `picked` came from; `null` with no pick.
    pub file: Option<String>,
    /// What the window draws while this project is in front; `null` leaves it its own theme.
    pub draws: Option<String>,
    /// Why `draws` is not `picked`, when it is not.
    pub why: Option<String>,
    /// The workspace's colour as its file holds it — a palette name or `#rrggbb` — when it was
    /// asked in a workspace that has one (charter-app#281).
    pub colour: Option<String>,
    /// Each value a file set that charter did not use, and why.
    pub ignored: Vec<ProjectExtensionIgnored>,
    /// Why `charter.local.toml` had no say in the theme when it picks one, as
    /// [`ProjectExtensions::local_left_out`] says it for extensions (charter-app#319): the core's
    /// `Said::local_left_out`.
    pub local_left_out: Option<String>,
    /// Why the file trees do not draw the icon theme the files pick, when they do not: the
    /// core's sentence (`extension::project::theme::resolve_icons`' `why`), as [`Self::why`] is
    /// the drawn theme's (#1145).
    #[specta(optional)]
    pub icons_why: Option<String>,
    /// **What the theme levels draw here** (#2323): the theme's name as the window draws it.
    #[specta(optional)]
    pub drawn: Option<String>,
    /// Which level chose it (`workspace-choice`, `workspace-default`, `project-choice`,
    /// `project-default`, `machine-default`, `purlis`).
    #[specta(optional)]
    pub level: Option<String>,
    /// Where that level is, in a sentence's words: "your choice for this workspace on this
    /// machine", "charter.toml", …
    #[specta(optional)]
    pub from: Option<String>,
    /// The person's own choice here, kept on this machine: for the workspace when asked in
    /// one, else for the project.
    #[specta(optional)]
    pub yours: Option<YourTheme>,
    /// The theme folders a pick here may name, nearest level first.
    #[specta(optional)]
    pub folders: Option<Vec<String>>,
}

/// The person's own theme choice, as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct YourTheme {
    pub theme: String,
    pub appearance: Option<String>,
    /// The level of the theme folder it picked (`workspace`, `project` or `machine`, #2516);
    /// `null` for a choice of no folder, and for one made before choices named a level.
    pub from: Option<String>,
    /// The value of each option it chose, by the option's id (#2527).
    pub options: std::collections::BTreeMap<String, String>,
}

impl From<purlis_core::theme::Choice> for YourTheme {
    fn from(choice: purlis_core::theme::Choice) -> Self {
        Self {
            theme: choice.theme,
            appearance: choice.appearance,
            from: choice.from.map(|level| level.word().to_owned()),
            options: choice.options,
        }
    }
}

/// This project's theme, with every theme it may pick — in `workspace`, when one is named, whose
/// `workspace.json` is a layer too (charter-app#281): what Settings shows at the Workspace level. It takes a survey, as
/// [`project_extensions`] does, so a pick the extension no longer contributes is said here.
#[tauri::command]
#[specta::specta]
pub async fn project_theme(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<ProjectTheme, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = workspace.as_deref();
        let seen = extension::survey(&config, &built_in());
        let choices = extension::project::Choices::read_in(&root, workspace);
        let folders = purlis_core::theme::Folders::of(Some(&config), Some(&root), workspace);
        let reach = purlis_core::theme::levels::Reach::Project;
        let said = extension::project::theme::Said::read_in(&root, workspace).with_folders(
            folders.names(reach),
            folders.names(purlis_core::theme::levels::Reach::Workspace),
        );
        let theme = project_theme_of(&seen, &choices, &said);
        let on = extension::project::resolve(
            &extension::project::Installed::from_survey(&seen),
            &choices,
        );
        let ids: Vec<String> = on
            .iter()
            .filter(|it| it.state == extension::project::State::On)
            .map(|it| it.id.clone())
            .collect();
        with_levels(
            theme,
            &config,
            &root,
            workspace,
            &ids,
            crate::windowprefs::THEIRS.get(Some(&config)).theme.as_ref(),
            offered_of(&seen),
        )
    })
    .await
    .map_err(|err| format!("reading this project's theme did not finish: {err}"))
}

/// `theme` with what the theme levels say here (#2323): what they draw, from which level, the
/// person's own choice, and the theme folders within reach.
fn with_levels(
    theme: ProjectTheme,
    config: &std::path::Path,
    root: &std::path::Path,
    workspace: Option<&str>,
    on: &[String],
    theirs: Option<&purlis_core::theme::Theme>,
    offered: Arc<[purlis_core::theme::Offered]>,
) -> ProjectTheme {
    use purlis_core::theme::{self, Folders, Level, LevelInputs, Levels, choices, levels};
    let levels = Levels::read(Some(config), Some(root), workspace);
    let folders = Folders::of(Some(config), Some(root), workspace);
    let drawn = theme::resolve_levels(
        &LevelInputs {
            levels: &levels,
            folders: &folders,
            theirs,
            on: theme::On::Only(on),
            system: theme::Appearance::Dark,
            // Which level chose, for Settings: the same whatever contrast is asked.
            contrast: theme::Contrast::Standard,
        },
        || offered,
    );
    let level = drawn.level.unwrap_or(Level::Purlis);
    let from = match level {
        Level::WorkspaceChoice => "your choice for this workspace on this machine".to_owned(),
        Level::WorkspaceDefault => workspace
            .map(purlis_core::settings::workspace::named)
            .unwrap_or_default(),
        Level::ProjectChoice => "your choice for this project on this machine".to_owned(),
        Level::ProjectDefault => {
            extension::project::Source::file(extension::project::Source::Shared)
                .unwrap_or_default()
                .to_owned()
        }
        Level::MachineDefault => "your machine default".to_owned(),
        Level::Purlis => "nothing chose one".to_owned(),
    };
    let held = choices::read(config);
    let mine = levels::project_key(root).and_then(|key| held.projects.get(&key).cloned());
    let yours = mine
        .and_then(|mine| match workspace {
            Some(name) => mine.workspaces.get(name).cloned(),
            None => mine.project,
        })
        .map(YourTheme::from);
    let reach = if workspace.is_some() {
        levels::Reach::Workspace
    } else {
        levels::Reach::Project
    };
    ProjectTheme {
        drawn: drawn.theme.as_ref().map(|it| it.name.clone()),
        level: serde_json::to_value(level)
            .ok()
            .and_then(|it| it.as_str().map(str::to_owned)),
        from: Some(from),
        yours,
        folders: Some(folders.names(reach)),
        ..theme
    }
}

/// [`project_theme`] without a runtime.
fn project_theme_of(
    seen: &extension::Survey,
    choices: &extension::project::Choices,
    said: &extension::project::theme::Said,
) -> ProjectTheme {
    use extension::project::theme;
    let installed = extension::project::Installed::from_survey(seen);
    let offered: Vec<theme::Offered> = in_force(seen)
        .into_iter()
        .map(|one| theme::Offered {
            id: one.extension,
            name: one.name,
        })
        .collect();
    let on = extension::project::resolve(&installed, choices);
    let it = theme::resolve(&on, Some(&offered), said);
    // The icon themes in force, as the window's Icons pick offers them.
    let icons: Vec<theme::Offered> = icons_in_force(seen)
        .into_iter()
        .map(|one| theme::Offered {
            id: one.extension,
            name: one.name,
        })
        .collect();
    let icons_why = theme::resolve_icons(&on, Some(&icons), said).why;
    // The built-in themes whole, each drawing its first appearance (#2709), the default first.
    // `purlis-dark` and `purlis-light` are no longer offered: a file that holds one draws
    // Purlis's dark or light, and Settings labels it (#3034).
    let mut options: Vec<ThemeOption> = [purlis_core::theme::PURLIS, purlis_core::theme::CLASSIC]
        .into_iter()
        .map(|name| ThemeOption {
            value: name.to_owned(),
            label: if purlis_core::theme::is_preview(name) {
                format!("{name} (built in, preview)")
            } else {
                format!("{name} (built in)")
            },
        })
        .collect();
    options.push(ThemeOption {
        value: theme::SYSTEM.to_owned(),
        label: "Follow the system".to_owned(),
    });
    options.extend(offered.iter().map(|one| {
        let whose = installed
            .iter()
            .find(|it| it.id == one.id)
            .map_or(one.id.as_str(), |it| it.name.as_str());
        ThemeOption {
            value: theme::Pick::Extension {
                id: one.id.clone(),
                name: one.name.clone(),
            }
            .value(),
            label: format!("{} ({whose})", one.name),
        }
    }));
    let file = |source| match (source, said.workspace_file()) {
        (extension::project::Source::Workspace, Some(file)) => Some(file),
        (source, _) => extension::project::Source::file(source).map(str::to_owned),
    };
    ProjectTheme {
        options,
        picked: it.picked.as_ref().map(theme::Pick::value),
        file: file(it.source),
        draws: it.draws.as_ref().map(theme::Pick::value),
        why: it.why,
        colour: it.colour.as_ref().map(theme::Colour::value),
        ignored: it
            .ignored
            .into_iter()
            .map(|one| ProjectExtensionIgnored {
                file: file(one.source).unwrap_or_default(),
                why: one.why,
            })
            .collect(),
        local_left_out: said.local_left_out().map(str::to_owned),
        icons_why,
        drawn: None,
        level: None,
        from: None,
        yours: None,
        folders: None,
    }
}

/// What the window draws while this project — and `workspace` in it, when one is named
/// (charter-app#281) — is in front, as the window is told it: the choice of the nearest theme
/// level that made one (#2323, [`purlis_core::theme::Levels`]), `null` when none did and the
/// window draws the person's own theme or asks the engine.
///
/// **The record alone**, as [`extensions_on`] is, so it is cheap enough to ask for every project
/// a window holds: an extension's theme the project has off is told as the built-in, and one the
/// extension does not contribute is left to [`theme_in_force`], which reads the levels, their
/// theme folders and the survey.
#[tauri::command]
#[specta::specta]
pub async fn project_theme_drawn(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<Option<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = workspace.as_deref();
        project_theme_drawn_of(
            &extension::read(&config, &built_in()),
            &extension::project::Choices::read_in(&root, workspace),
            &purlis_core::theme::Levels::read(Some(&config), Some(&root), workspace),
        )
    })
    .await
    .map_err(|err| format!("reading this project's theme did not finish: {err}"))
}

/// [`project_theme_drawn`] without a runtime.
fn project_theme_drawn_of(
    loaded: &extension::Loaded,
    choices: &extension::project::Choices,
    levels: &purlis_core::theme::Levels,
) -> Option<String> {
    use extension::project::theme;
    let (_, choice) = levels.chosen()?;
    if let Some(pick @ theme::Pick::Extension { .. }) = theme::Pick::parse(&choice.theme) {
        let on = extension::project::resolve(
            &extension::project::Installed::from_record(loaded),
            choices,
        );
        if !theme::drawable(&on, &pick) {
            return Some(theme::FALLBACK.to_owned());
        }
    }
    Some(choice.told())
}

/// The icon theme the file trees draw while this project — and `workspace` in it — is in front
/// (FM-3, #1106), as a file holds it: `null` draws charter's own. Read as cheaply as
/// [`project_theme_drawn`], from the record and the project's files alone.
#[tauri::command]
#[specta::specta]
pub async fn project_icons_drawn(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<Option<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = workspace.as_deref();
        project_icons_drawn_of(
            &extension::read(&config, &built_in()),
            &extension::project::Choices::read_in(&root, workspace),
            &extension::project::theme::Said::read_in(&root, workspace),
        )
    })
    .await
    .map_err(|err| format!("reading this project's icon theme did not finish: {err}"))
}

/// [`project_icons_drawn`] without a runtime.
fn project_icons_drawn_of(
    loaded: &extension::Loaded,
    choices: &extension::project::Choices,
    said: &extension::project::theme::Said,
) -> Option<String> {
    use extension::project::theme;
    let on =
        extension::project::resolve(&extension::project::Installed::from_record(loaded), choices);
    theme::resolve_icons(&on, None, said)
        .draws
        .as_ref()
        .map(theme::Pick::value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::theme::Contrast;

    fn made() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let at = dir.path().join("ext");
        std::fs::create_dir_all(&at).expect("the extension's directory");
        std::fs::write(
            at.join(extension::MANIFEST),
            r#"{"version":1,"id":"solarized","name":"Solarized",
                "contributes":{"themes":[{"name":"Solarized Dark","file":"dark.json"}],
                               "panels":[{"id":"p","title":"P"}]}}"#,
        )
        .expect("a manifest");
        std::fs::write(at.join("dark.json"), "{}").expect("a theme");
        let config = dir.path().join("config");
        (dir, at, config)
    }

    /// An approved extension with a status-bar badge and a repo column, and its facts file
    /// filled for the repo `svc` — no program, so nothing could be started to draw either.
    fn facts_made() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let at = dir.path().join("ext");
        std::fs::create_dir_all(at.join("state")).expect("the state directory");
        std::fs::write(
            at.join(extension::MANIFEST),
            r#"{"version":1,"id":"prs","name":"Pull requests","state":"state",
                "capabilities":["badges","repo-columns"],
                "contributes":{
                  "badges":[{"id":"open","label":"PRs","surfaces":["status-bar"],"fresh_seconds":600}],
                  "repo-columns":[{"id":"open","title":"PRs","fresh_seconds":600}]}}"#,
        )
        .expect("a manifest");
        let now = chrono::Utc::now().timestamp();
        std::fs::write(
            at.join("state").join(extension::facts::FILE),
            format!(
                r#"{{"badges":{{"open":{{"value":"3","at":{now}}}}},
                    "repo-columns":{{"open":{{"svc":{{"value":"2","at":{}}}}}}}}}"#,
                now - 3600
            ),
        )
        .expect("a facts file");
        let config = dir.path().join("config");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        (dir, config)
    }

    #[test]
    fn the_window_is_handed_badges_and_repo_cells_with_their_age_and_staleness() {
        let (_dir, config) = facts_made();
        let plane = test_plane("", "", true);

        let said = facts_in(&config, plane.path(), Some("alpha"), chrono::Utc::now());
        assert_eq!(said.badges.len(), 1);
        let badge = &said.badges[0];
        assert_eq!(
            (badge.label.as_str(), badge.value.as_str(), badge.stale),
            ("PRs", "3", false)
        );
        let column = &said.columns[0];
        assert_eq!(
            (column.extension.as_str(), column.title.as_str()),
            ("prs", "PRs")
        );
        let cell = &column.cells[0];
        assert_eq!((cell.repo.as_str(), cell.value.as_str()), ("svc", "2"));
        assert!(
            cell.stale,
            "an hour-old value declared fresh for ten minutes"
        );
        assert!(
            (3600..3660).contains(&cell.age_seconds),
            "{}",
            cell.age_seconds
        );
    }

    #[test]
    fn a_workspace_that_turned_the_extension_off_gets_no_column_and_no_badge() {
        let (_dir, config) = facts_made();
        let plane = test_plane("", "", true);
        std::fs::write(
            plane.path().join("workspaces/alpha/workspace.json"),
            r#"{"settings": {"extensions": {"prs": {"enabled": false}}}}"#,
        )
        .expect("a workspace manifest");

        let off = facts_in(&config, plane.path(), Some("alpha"), chrono::Utc::now());
        assert!(off.columns.is_empty() && off.badges.is_empty(), "{off:?}");
        let project = facts_in(&config, plane.path(), None, chrono::Utc::now());
        assert_eq!(project.columns.len(), 1, "the project has it on");
    }

    #[test]
    fn an_extension_of_themes_alone_is_listed_as_themes_drawing_with_nothing_to_ask() {
        let (_dir, at, config) = made();
        std::fs::write(
            at.join(extension::MANIFEST),
            r#"{"version":1,"id":"solarized","name":"Solarized",
                "contributes":{"themes":[{"name":"Solarized Dark","file":"dark.json"}]}}"#,
        )
        .expect("a manifest");
        extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let row = &listed.extensions[0];
        assert_eq!(row.standing, "themes");
        assert_eq!(row.themes_in_force, ["Solarized Dark"]);
        assert_eq!(row.ask, None);
    }

    #[test]
    fn a_row_that_is_not_approved_carries_the_question_to_ask_about_it() {
        let (_dir, at, config) = made();
        extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let row = &listed.extensions[0];
        assert_eq!(row.standing, "new");
        assert!(
            row.themes_in_force.is_empty(),
            "it contributed before it was asked about"
        );
        let ask = row.ask.as_ref().expect("a question");
        assert_eq!(ask.runs_as_you, extension::RUNS_AS_YOU);
        assert_eq!(ask.fingerprint_note, extension::FINGERPRINTED);
        assert_eq!(
            ask.state_note, None,
            "an extension with no state directory has no exception to state"
        );
    }

    #[test]
    fn an_approved_extension_s_refused_theme_says_why_on_its_row() {
        // #2321: an extension theme's complaints reach the person, on the row its other
        // complaints are on, instead of being dropped.
        let (_dir, at, config) = made();
        std::fs::write(
            at.join("dark.json"),
            r#"{"name":"Solarized Dark","appearance":"dark","tokens":{"text.primary":"red"}}"#,
        )
        .expect("a theme");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let trouble = &listed.extensions[0].theme_trouble;
        assert_eq!(trouble.len(), 1, "{trouble:?}");
        assert_eq!(trouble[0].name, "Solarized Dark");
        assert!(
            trouble[0].refused,
            "a value outside its type refuses the theme"
        );
        assert_eq!(trouble[0].complaints.len(), 1, "{trouble:?}");
        assert!(
            trouble[0].complaints[0].starts_with("text.primary is \"red\", which is not #rgb"),
            "{trouble:?}"
        );
    }

    #[test]
    fn an_approved_extension_s_drawn_theme_says_what_was_left_out_on_its_row() {
        let (_dir, at, config) = made();
        std::fs::write(
            at.join("dark.json"),
            r##"{"name":"Solarized Dark","appearance":"dark","tokens":{"text.primry":"#ffffff"}}"##,
        )
        .expect("a theme");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let trouble = &listed.extensions[0].theme_trouble;
        assert_eq!(trouble.len(), 1, "{trouble:?}");
        assert!(
            !trouble[0].refused,
            "an unknown name is left out, not a refusal"
        );
        assert!(
            trouble[0].complaints[0].starts_with("text.primry is not a purlis token"),
            "{trouble:?}"
        );
    }

    #[test]
    fn an_approved_extension_s_theme_that_is_not_json_is_said_to_be_refused() {
        let (_dir, at, config) = made();
        std::fs::write(at.join("dark.json"), "{ not json").expect("a theme");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let trouble = &listed.extensions[0].theme_trouble;
        assert_eq!(trouble.len(), 1, "{trouble:?}");
        assert_eq!(trouble[0].name, "Solarized Dark");
        assert!(trouble[0].refused);
        assert!(
            trouble[0].complaints[0].starts_with("a theme is JSON"),
            "{trouble:?}"
        );
    }

    #[test]
    fn a_theme_the_engine_reads_whole_has_nothing_to_say_and_one_not_approved_is_not_read() {
        let (_dir, at, config) = made();
        std::fs::write(
            at.join("dark.json"),
            r#"{"name":"Solarized Dark","appearance":"dark","tokens":{}}"#,
        )
        .expect("a theme");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        let new = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        assert!(
            new.extensions[0].theme_trouble.is_empty(),
            "not in force, not read"
        );
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        assert!(listed.extensions[0].theme_trouble.is_empty());
    }

    #[test]
    fn a_built_in_is_listed_as_the_app_s_approved_and_on_with_nothing_to_ask() {
        let (dir, at, config) = made();
        // The app's resources, as a bundle lays them out, holding the extension.
        let resources = dir.path().join("Resources");
        let shipped = resources.join(BUILT_IN_DIR).join("solarized");
        std::fs::create_dir_all(&shipped).expect("a bundle");
        for name in [extension::MANIFEST, "dark.json"] {
            std::fs::copy(at.join(name), shipped.join(name)).expect("a copy");
        }
        let built_in = find_built_in(Some(resources));

        let shown = listed(&extension::survey(&config, &built_in));
        let row = &shown.extensions[0];
        assert_eq!(row.source, "app");
        assert_eq!(row.standing, "approved");
        assert!(row.on);
        assert!(row.ask.is_none(), "a built-in was asked about");
        assert_eq!(row.themes_in_force, ["Solarized Dark"]);

        extension::set_on(&config, &built_in, "solarized", false).expect("turned off");
        let shown = listed(&extension::survey(&config, &built_in));
        let row = &shown.extensions[0];
        assert!(!row.on);
        assert!(row.themes_in_force.is_empty(), "it contributed while off");
    }

    #[test]
    fn an_app_with_nothing_in_its_resources_has_no_built_in() {
        let dir = tempfile::tempdir().expect("a directory");
        assert_eq!(
            find_built_in(Some(dir.path().to_path_buf())),
            extension::BuiltIn::none()
        );
        assert_eq!(find_built_in(None), extension::BuiltIn::none());
    }

    #[test]
    fn a_state_directory_is_carried_to_the_window_as_the_core_words_it() {
        // charter-app#152. `fingerprint_note` says charter read every file in the directory;
        // the one directory it did not read has to reach the same screen, in the core's own
        // words, or the window is left to word the exception itself.
        let (_dir, at, config) = made();
        std::fs::write(
            at.join(extension::MANIFEST),
            r#"{"version":1,"id":"solarized","name":"Solarized","state":"cache",
                "contributes":{"themes":[{"name":"Solarized Dark","file":"dark.json"}],
                               "panels":[{"id":"p","title":"P"}]}}"#,
        )
        .expect("a manifest with a state directory");
        extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let ask = listed.extensions[0].ask.as_ref().expect("a question");
        assert_eq!(
            ask.state_note.as_deref(),
            Some(&*extension::state_note("cache"))
        );
    }

    #[test]
    fn an_approved_row_has_nothing_left_to_ask() {
        // A question that never carries risk is one an operator learns to answer without
        // reading, which is `profiletrust::approval_needed`'s reason for never asking about a
        // built-in.
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let row = &listed.extensions[0];
        assert_eq!(row.standing, "approved");
        assert_eq!(row.themes_in_force, vec!["Solarized Dark".to_owned()]);
        assert!(row.ask.is_none());
    }

    #[test]
    fn a_row_charter_could_not_read_says_why_and_still_lists_what_it_declared() {
        let (_dir, at, config) = made();
        extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        std::fs::remove_file(at.join(extension::MANIFEST)).expect("the manifest");

        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));
        let row = &listed.extensions[0];
        assert!(row.refused.is_some(), "it went quiet");
        assert!(row.themes_in_force.is_empty());
        assert!(
            row.ask.is_none(),
            "charter asked about something it could not read"
        );
    }

    #[test]
    fn only_an_approved_extensions_theme_is_ever_in_force() {
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        assert!(
            in_force(&extension::survey(&config, &extension::BuiltIn::none())).is_empty(),
            "an unapproved extension's theme reached the window"
        );

        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let themes = in_force(&extension::survey(&config, &extension::BuiltIn::none()));
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].extension, "solarized");
        assert_eq!(themes[0].name, "Solarized Dark");
        assert_eq!(themes[0].text, "{}", "the text is the file's, unparsed");

        std::fs::write(at.join("dark.json"), r#"{"tokens":{}}"#).expect("a changed theme");
        assert!(
            in_force(&extension::survey(&config, &extension::BuiltIn::none())).is_empty(),
            "a theme that changed after approval was still drawn"
        );
    }

    /// The levels what `said` says make, its workspace taken as LIVE, no choice of the person's.
    fn levels_of(said: &extension::project::theme::Said) -> purlis_core::theme::Levels {
        purlis_core::theme::Levels::of(
            &Default::default(),
            Some(purlis_core::theme::levels::Project {
                key: None,
                said,
                workspace: said.workspace_name().map(|name| (name, true)),
            }),
        )
    }

    #[test]
    fn your_theme_in_settings_is_kept_by_project_id_and_is_what_the_levels_draw() {
        // #2323: Settings › Appearance › Your theme writes the person's choice into machine
        // config, never a project file, and the levels read it back as the nearest level.
        let home = tempfile::tempdir().expect("a directory");
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).expect("a plane");
        std::fs::write(
            purlis_core::names::manifest(&plane),
            "[project]\nid = \"01JABCDEFGHJKMNPQRSTVWXYZ0\"\n",
        )
        .expect("a manifest");
        theme_choose_of(
            &config,
            &plane,
            Some("alpha"),
            Some("purlis-light"),
            None,
            None,
        )
        .expect("kept");
        let levels = purlis_core::theme::Levels::read(Some(&config), Some(&plane), Some("alpha"));
        assert_eq!(
            levels
                .chosen()
                .map(|(level, choice)| (level, choice.theme.as_str())),
            Some((purlis_core::theme::Level::WorkspaceChoice, "purlis-light"))
        );
        assert_eq!(
            std::fs::read_to_string(purlis_core::names::manifest(&plane)).expect("read"),
            "[project]\nid = \"01JABCDEFGHJKMNPQRSTVWXYZ0\"\n",
            "a project file was written"
        );
        // Taken away, and a value that is no pick refused.
        theme_choose_of(&config, &plane, Some("alpha"), None, None, None).expect("taken away");
        assert!(
            purlis_core::theme::Levels::read(Some(&config), Some(&plane), Some("alpha"))
                .workspace_choice
                .is_none()
        );
        assert!(theme_choose_of(&config, &plane, None, Some("Not A Theme"), None, None).is_err());
    }

    /// The levels with no project in front, the window having been told to draw `pick`.
    fn picked(pick: Option<&str>) -> purlis_core::theme::Levels {
        purlis_core::theme::Levels {
            project_default: pick.and_then(|pick| purlis_core::theme::Choice::new(pick, None)),
            ..Default::default()
        }
    }

    #[test]
    fn the_theme_in_force_resolves_the_person_s_theme_with_no_window_created_first() {
        // A link, or a window made after another, asks with no creation script of its own: the
        // one read the process keeps is taken on the first ask.
        let home = tempfile::tempdir().expect("a directory");
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).expect("the config home");
        std::fs::write(
            dir.join(purlis_core::windowprefs::THEME),
            r#"{"name":"Mine","appearance":"light"}"#,
        )
        .expect("a theme");
        let kept = purlis_core::theme::TheirsKept::new();
        let theirs = kept.get(Some(home.path())).theme;
        let drawn = theme_in_force_of(
            &picked(None),
            &Default::default(),
            None,
            theirs.as_ref(),
            false,
            Contrast::Standard,
            || unreachable!(),
            |_| None,
        );
        let theme = drawn.theme.expect("the person's theme");
        assert_eq!(
            (theme.name.as_str(), theme.appearance.as_str()),
            ("Mine", "light")
        );
    }

    #[test]
    fn each_ask_is_drawn_with_the_person_s_layer_it_hands() {
        // #2925: the window hands its own person's colours with each ask, and the answer wears
        // them; an ask with none is drawn as the levels chose it.
        let accent = "#9a86ff";
        let asked = |person: Option<&str>| {
            theme_in_force_under(
                &picked(None),
                &Default::default(),
                None,
                None,
                false,
                Contrast::Standard,
                &person_layer(person),
                || std::sync::Arc::from(Vec::new()),
                |_| None,
            )
        };
        let mine = asked(Some(&format!(
            r#"{{"colours":{{"accent.base":"{accent}"}}}}"#
        )));
        assert_eq!(mine.theme.expect("drawn").values["accent.base"], accent);
        assert!(mine.mine.is_empty());
        let none = asked(None);
        assert_eq!(
            none.theme.expect("drawn").values["accent.base"],
            purlis_core::theme::default_theme().value("accent.base")
        );
        // Text that is not the layer is the layer that draws every theme as it is.
        assert_eq!(
            person_layer(Some("not json")),
            purlis_core::theme::Layer::default()
        );
        // A colour that breaks a floor is left out and said.
        let ground = purlis_core::theme::default_theme().value("surface.base");
        let broken = asked(Some(&format!(
            r#"{{"colours":{{"text.primary":"{ground}"}}}}"#
        )));
        assert_eq!(broken.mine.len(), 1);
    }

    #[test]
    fn more_contrast_asked_for_draws_the_person_s_appearance_tagged_high_contrast() {
        // #2328: the window says `prefers-contrast: more`, and the engine draws the appearance of
        // the drawn tag tagged high contrast.
        let mine = purlis_core::theme::load_text(
            r##"{"name":"Mine","appearances":[{"id":"dark","appearance":"dark"},
                {"id":"sharp","appearance":"dark","high-contrast":true,
                 "tokens":{"surface.base":"#000000"}}]}"##,
        );
        assert!(!mine.refused, "{:?}", mine.complaints);
        let asked = |more| {
            theme_in_force_of(
                &picked(None),
                &Default::default(),
                None,
                Some(&mine.theme),
                false,
                contrast_of(more),
                || unreachable!(),
                |_| None,
            )
            .theme
            .expect("drawn")
        };
        assert_eq!(asked(true).values["surface.base"], "#000000");
        assert_ne!(asked(false).values["surface.base"], "#000000");
    }

    #[test]
    fn the_window_is_handed_the_drawn_theme_s_options_with_the_value_each_is_drawn_with() {
        let mine = purlis_core::theme::load_text(
            r##"{"name":"Mine","appearances":[{"id":"dark","appearance":"dark"}],
                "options":[{"id":"borders","kind":"toggle","label":"Borders",
                    "description":"A rule between panes.","default":true,
                    "on":{"tokens":{"border.subtle":"#777777"}}}]}"##,
        );
        assert!(!mine.refused, "{:?}", mine.complaints);
        let drawn = theme_in_force_of(
            &picked(None),
            &Default::default(),
            None,
            Some(&mine.theme),
            false,
            Contrast::Standard,
            || unreachable!(),
            |_| None,
        );
        assert_eq!(
            drawn.options,
            [WindowOption {
                id: "borders".to_owned(),
                label: "Borders".to_owned(),
                description: "A rule between panes.".to_owned(),
                kind: "toggle".to_owned(),
                values: ["off", "on"]
                    .into_iter()
                    .zip(["Off", "On"])
                    .map(|(id, label)| WindowOptionValue {
                        id: id.to_owned(),
                        label: label.to_owned(),
                    })
                    .collect(),
                value: "on".to_owned(),
                default: "on".to_owned(),
                chosen: false,
                level: None,
            }]
        );
        assert_eq!(
            drawn.theme.expect("drawn").values["border.subtle"],
            "#777777"
        );
        // Purlis Classic has none.
        let classic = theme_in_force_of(
            &picked(Some("purlis-dark")),
            &Default::default(),
            None,
            None,
            false,
            Contrast::Standard,
            || unreachable!(),
            |_| None,
        );
        assert!(classic.options.is_empty());
    }

    #[test]
    fn the_window_is_handed_a_theme_s_icon_set_by_view_and_classic_s_as_none() {
        let classic = WindowTheme::from(&purlis_core::theme::built_ins()[0]);
        assert!(classic.icons.is_empty());

        let dir = tempfile::tempdir().expect("a directory");
        let folder = dir.path().join("dusk");
        std::fs::create_dir_all(&folder).expect("a theme folder");
        std::fs::write(
            folder.join("theme.json"),
            r#"{"name":"Dusk","icons":"icons.json","appearances":[{"id":"dark","appearance":"dark"}]}"#,
        )
        .expect("a theme");
        std::fs::write(
            folder.join("icons.json"),
            r#"{"views":{"chats":{"paths":["M1 1h2"]}}}"#,
        )
        .expect("an icon set");
        let loaded = purlis_core::theme::Library::new().load_folder(dir.path(), "dusk");
        assert!(!loaded.refused, "{:?}", loaded.complaints);
        let handed = WindowTheme::from(loaded.theme.first());
        assert_eq!(
            handed.icons.get("chats").map(|icon| icon.paths.clone()),
            Some(vec!["M1 1h2".to_owned()])
        );
    }

    #[test]
    fn the_window_is_handed_an_approved_extensions_theme_resolved_by_the_core() {
        let (_dir, at, config) = made();
        std::fs::write(
            at.join("dark.json"),
            r##"{"name":"Solarized Dark","appearance":"light","tokens":{"text.primary":"#123"}}"##,
        )
        .expect("a theme");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let offered = || offered_of(&extension::survey(&config, &extension::BuiltIn::none()));

        let drawn = theme_in_force_of(
            &picked(Some("solarized/Solarized Dark")),
            &Default::default(),
            None,
            None,
            false,
            Contrast::Standard,
            offered,
            |_| None,
        );
        let theme = drawn.theme.expect("a theme");
        assert_eq!(theme.name, "Solarized Dark");
        assert_eq!(theme.appearance, "light");
        assert_eq!(theme.values["text.primary"], "#123");
        assert_eq!(
            theme.values.len(),
            purlis_core::theme::vocabulary::TOKENS.len()
        );
        assert!(!drawn.follows_system);

        // Off in the project in front: the default, and no survey needed to say so for a
        // built-in pick or the system's.
        let off = theme_in_force_of(
            &picked(Some("solarized/Solarized Dark")),
            &Default::default(),
            Some(&[]),
            None,
            false,
            Contrast::Standard,
            offered,
            |_| None,
        );
        assert_eq!(off.theme.map(|it| it.name).as_deref(), Some("Purlis"));
        let system = theme_in_force_of(
            &picked(Some("system")),
            &Default::default(),
            None,
            None,
            true,
            Contrast::Standard,
            || unreachable!(),
            |_| None,
        );
        // The old `system` id follows the system with the default, Purlis (#3034).
        assert_eq!(
            system.theme.map(|it| (it.name, it.appearance)),
            Some(("Purlis".to_owned(), "light".to_owned()))
        );
        assert!(system.follows_system);
    }

    #[test]
    fn an_approved_extensions_icon_theme_reaches_the_window_and_a_project_picks_it() {
        // FM-3 (#1106): an icon theme travels on a colour theme's terms, and `[theme] icons`
        // picks it; turned off in the project, charter's own icons are drawn.
        let dir = tempfile::tempdir().expect("a directory");
        let at = dir.path().join("ext");
        std::fs::create_dir_all(&at).expect("the extension's directory");
        std::fs::write(
            at.join(extension::MANIFEST),
            r#"{"version":1,"id":"seti","name":"Seti",
                "contributes":{"icon_themes":[{"name":"Seti","file":"seti.json"}],
                               "panels":[{"id":"p","title":"P"}]}}"#,
        )
        .expect("a manifest");
        std::fs::write(at.join("seti.json"), r#"{"symbols":{}}"#).expect("an icon theme");
        let config = dir.path().join("config");
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        let seen = || extension::survey(&config, &extension::BuiltIn::none());
        assert!(
            icons_in_force(&seen()).is_empty(),
            "unapproved, and in force"
        );
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let icons = icons_in_force(&seen());
        assert_eq!(
            icons,
            [ExtensionTheme {
                extension: "seti".to_owned(),
                name: "Seti".to_owned(),
                text: r#"{"symbols":{}}"#.to_owned(),
            }]
        );
        assert!(
            in_force(&seen()).is_empty(),
            "an icon theme is not a colour theme"
        );

        let loaded = extension::read(&config, &extension::BuiltIn::none());
        let shared = "[theme]\nicons = \"seti/Seti\"\n";
        let drawn = |local: &str| {
            project_icons_drawn_of(
                &loaded,
                &extension::project::Choices::from_text(Some(shared), Some(local)),
                &extension::project::theme::Said::from_text(Some(shared), Some(local)),
            )
        };
        assert_eq!(drawn("").as_deref(), Some("seti/Seti"));
        assert_eq!(
            drawn("[extensions.seti]\nenabled = false\n").as_deref(),
            Some("purlis-icons")
        );
    }

    #[test]
    fn the_window_is_told_charters_own_themes_as_well() {
        let (_dir, _at, config) = made();
        let listed = listed(&extension::survey(&config, &extension::BuiltIn::none()));

        assert_eq!(listed.built_in_themes, extension::BUILT_IN_THEMES.to_vec());
    }

    #[test]
    fn a_project_that_turns_an_approved_extension_off_has_it_off_and_says_which_file() {
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let choices = extension::project::Choices::from_text(
            None,
            Some("[extensions.solarized]\nenabled = false\n"),
        );

        let rows = project_rows(
            &extension::survey(&config, &extension::BuiltIn::none()),
            &choices,
        )
        .extensions;
        assert_eq!(
            rows.iter()
                .map(|row| (row.id.as_str(), row.state.as_str(), row.source.as_str()))
                .collect::<Vec<_>>(),
            vec![("solarized", "off", "local")]
        );
        assert_eq!(
            on_ids(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            on_ids(
                &extension::read(&config, &extension::BuiltIn::none()),
                &extension::project::Choices::default()
            ),
            vec!["solarized".to_owned()],
            "a project that says nothing kept the machine-wide answer"
        );
    }

    #[test]
    fn a_workspace_that_turns_an_extension_off_has_it_off_there_and_names_its_file() {
        // charter-app#280: the focused workspace's settings are a layer of what the window keeps.
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let choices = extension::project::Choices::default().in_workspace(
            "alpha",
            Some(
                r#"{"settings": {"extensions": {"solarized": {"enabled": false, "settings": {"x": true}}}}}"#,
            ),
        );

        let rows = project_rows(
            &extension::survey(&config, &extension::BuiltIn::none()),
            &choices,
        )
        .extensions;
        assert_eq!(
            (rows[0].state.as_str(), rows[0].source.as_str()),
            ("off", "workspace")
        );
        assert_eq!(rows[0].ignored[0].file, "workspaces/alpha/workspace.json");
        assert!(
            on_ids(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices
            )
            .is_empty()
        );
    }

    #[test]
    fn a_project_that_wants_an_unapproved_extension_reads_as_needing_approval_here() {
        let (_dir, at, config) = made();
        extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        let choices = extension::project::Choices::from_text(
            Some("[extensions.solarized]\nenabled = true\n"),
            None,
        );

        let rows = project_rows(
            &extension::survey(&config, &extension::BuiltIn::none()),
            &choices,
        )
        .extensions;
        assert_eq!(
            (rows[0].state.as_str(), rows[0].source.as_str()),
            ("needs-approval", "shared")
        );
        assert!(
            on_ids(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices
            )
            .is_empty()
        );
    }

    #[test]
    fn a_project_file_may_pick_purlis_or_purlis_classic_whole() {
        // #2709: the select offers both built-in themes, and a file that picks one draws it.
        let dir = tempfile::tempdir().expect("a directory");
        let config = dir.path().join("config");
        let seen = extension::survey(&config, &extension::BuiltIn::none());
        for pick in [purlis_core::theme::PURLIS, purlis_core::theme::CLASSIC] {
            let shared = format!("[theme]\nuse = \"{pick}\"\n");
            let choices = extension::project::Choices::from_text(Some(&shared), None);
            let said = extension::project::theme::Said::from_text(Some(&shared), None);
            let theme = project_theme_of(&seen, &choices, &said);
            assert!(
                theme.options.iter().any(|one| one.value == pick),
                "{pick} is offered"
            );
            assert_eq!(theme.picked.as_deref(), Some(pick));
            assert_eq!(theme.draws.as_deref(), Some(pick));
            assert_eq!(theme.why, None);
        }
    }

    #[test]
    fn a_projects_theme_offers_every_approved_theme_and_says_why_one_turned_off_is_not_drawn() {
        // charter-app#273.
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let shared = "[theme]\nuse = \"solarized/Solarized Dark\"\n";
        let local = "[extensions.solarized]\nenabled = false\n";
        let choices = extension::project::Choices::from_text(Some(shared), Some(local));
        let said = extension::project::theme::Said::from_text(Some(shared), Some(local));

        let theme = project_theme_of(
            &extension::survey(&config, &extension::BuiltIn::none()),
            &choices,
            &said,
        );
        assert_eq!(
            theme
                .options
                .iter()
                .map(|one| (one.value.as_str(), one.label.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("purlis", "purlis (built in)"),
                ("purlis-classic", "purlis-classic (built in)"),
                ("system", "Follow the system"),
                ("solarized/Solarized Dark", "Solarized Dark (Solarized)"),
            ]
        );
        assert_eq!(theme.picked.as_deref(), Some("solarized/Solarized Dark"));
        assert_eq!(theme.file.as_deref(), Some("charter.toml"));
        assert_eq!(theme.draws.as_deref(), Some("purlis"));
        assert_eq!(
            theme.why.as_deref(),
            Some(
                "charter.toml picks “Solarized Dark” from solarized, but solarized is off in this \
                 project — so the built-in theme, Purlis, is drawn"
            )
        );
        assert_eq!(
            project_theme_drawn_of(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices,
                &levels_of(&said)
            )
            .as_deref(),
            Some("purlis"),
            "the window drew a theme the project turned off"
        );

        let choices = extension::project::Choices::from_text(Some(shared), None);
        let said = extension::project::theme::Said::from_text(Some(shared), None);
        assert_eq!(
            project_theme_drawn_of(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices,
                &levels_of(&said)
            )
            .as_deref(),
            Some("solarized/Solarized Dark")
        );
        assert_eq!(
            project_theme_drawn_of(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices,
                &levels_of(&extension::project::theme::Said::default())
            ),
            None,
            "a project that picks nothing leaves the window its own theme"
        );
    }

    /// #1145: the answer carries the core's why for the icon theme too, as it does for the
    /// drawn theme, so the window says it in the core's words rather than a copy of them.
    #[test]
    fn a_projects_theme_says_why_its_icon_theme_is_not_drawn() {
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let surveyed = extension::survey(&config, &extension::BuiltIn::none());
        let shared = "[theme]\nicons = \"solarized/Gone\"\n";

        let theme = project_theme_of(
            &surveyed,
            &extension::project::Choices::from_text(Some(shared), None),
            &extension::project::theme::Said::from_text(Some(shared), None),
        );
        assert_eq!(
            theme.icons_why.as_deref(),
            Some(
                "charter.toml picks “Gone” from solarized, but solarized contributes no icon \
                 theme called “Gone” — so the built-in purlis-icons is drawn"
            )
        );
        assert_eq!(theme.why, None, "the drawn theme is not the icons'");

        let theme = project_theme_of(
            &surveyed,
            &extension::project::Choices::from_text(
                Some("[theme]\nicons = \"purlis-icons\"\n"),
                None,
            ),
            &extension::project::theme::Said::from_text(
                Some("[theme]\nicons = \"purlis-icons\"\n"),
                None,
            ),
        );
        assert_eq!(theme.icons_why, None, "the built-in is drawn as picked");
    }

    #[test]
    fn a_workspaces_theme_names_its_file_carries_its_colour_and_is_what_the_window_draws() {
        // charter-app#281: the workspace is a layer of the one resolver, for the tab and the
        // window alike.
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let shared = "[theme]\nuse = \"purlis-light\"\n";
        let manifest = r#"{"settings": {"theme": {"use": "solarized/Solarized Dark",
            "colour": "purple"}, "extensions": {"solarized": {"enabled": false}}}}"#;
        let choices = extension::project::Choices::from_text(Some(shared), None)
            .in_workspace("alpha", Some(manifest));
        let said = extension::project::theme::Said::from_text(Some(shared), None)
            .in_workspace("alpha", Some(manifest));

        let theme = project_theme_of(
            &extension::survey(&config, &extension::BuiltIn::none()),
            &choices,
            &said,
        );
        assert_eq!(
            theme.file.as_deref(),
            Some("workspaces/alpha/workspace.json")
        );
        assert_eq!(theme.colour.as_deref(), Some("purple"));
        assert_eq!(theme.draws.as_deref(), Some("purlis"));
        assert_eq!(
            project_theme_drawn_of(
                &extension::read(&config, &extension::BuiltIn::none()),
                &choices,
                &levels_of(&said)
            )
            .as_deref(),
            Some("purlis"),
            "the window drew a theme the workspace turned off"
        );

        let manifest = r#"{"settings": {"theme": {"use": "dark"}}}"#;
        // The project has no theme folders: `dark` names none (#2323).
        let said = extension::project::theme::Said::from_text(Some(shared), None)
            .in_workspace("alpha", Some(manifest))
            .with_folders(Vec::new(), Vec::new());
        let theme = project_theme_of(
            &extension::survey(&config, &extension::BuiltIn::none()),
            &choices,
            &said,
        );
        assert_eq!(theme.draws.as_deref(), Some("purlis-light"));
        assert_eq!(theme.ignored[0].file, "workspaces/alpha/workspace.json");
    }

    const SHARED: &str = "[theme]\nuse = \"purlis-light\"\n";
    const LOCAL: &str =
        "[extensions.solarized]\nenabled = false\n\n[theme]\nuse = \"purlis-dark\"\n";

    #[test]
    fn extensions_and_theme_carry_why_the_local_file_was_left_out_in_project_and_workspace() {
        // charter-app#319: a value set in Local and not applied is never shown without its
        // reason, so each answer carries the ignore check's sentence — the one
        // charter.local.toml's refusals say.
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let plane = test_plane(SHARED, LOCAL, false);
        let root = plane.path();
        let why = purlis_core::profiles::ignore_check(root).reason;
        assert!(why.contains("purlis reads nothing in it"), "{why}");

        for workspace in [None, Some("alpha")] {
            let choices = extension::project::Choices::read_in(root, workspace);
            let said = extension::project::theme::Said::read_in(root, workspace);

            let rows = project_rows(
                &extension::survey(&config, &extension::BuiltIn::none()),
                &choices,
            );
            assert_eq!(rows.local_left_out.as_deref(), Some(why.as_str()));
            assert_eq!(
                (
                    rows.extensions[0].state.as_str(),
                    rows.extensions[0].source.as_str()
                ),
                ("on", "default"),
                "{workspace:?}: Local's off was applied"
            );
            let theme = project_theme_of(
                &extension::survey(&config, &extension::BuiltIn::none()),
                &choices,
                &said,
            );
            assert_eq!(theme.local_left_out.as_deref(), Some(why.as_str()));
            assert_eq!(theme.picked.as_deref(), Some("purlis-light"));
        }
    }

    #[test]
    fn a_local_file_that_is_read_leaves_nothing_out_and_decides() {
        let (_dir, at, config) = made();
        let found =
            extension::install(&config, &extension::BuiltIn::none(), &at).expect("installed");
        extension::approve(&config, found.id(), &found.path, &found.fingerprint).expect("approved");
        let plane = test_plane(SHARED, LOCAL, true);
        let root = plane.path();

        for workspace in [None, Some("alpha")] {
            let choices = extension::project::Choices::read_in(root, workspace);
            let said = extension::project::theme::Said::read_in(root, workspace);

            let rows = project_rows(
                &extension::survey(&config, &extension::BuiltIn::none()),
                &choices,
            );
            assert_eq!(rows.local_left_out, None, "{workspace:?}");
            assert_eq!(
                (
                    rows.extensions[0].state.as_str(),
                    rows.extensions[0].source.as_str()
                ),
                ("off", "local"),
                "{workspace:?}"
            );
            let theme = project_theme_of(
                &extension::survey(&config, &extension::BuiltIn::none()),
                &choices,
                &said,
            );
            assert_eq!(theme.local_left_out, None, "{workspace:?}");
            assert_eq!(theme.picked.as_deref(), Some("purlis-dark"));
        }
    }

    /// #1296: opening the extension record is the window's alone, and the window names only the
    /// editor: the generated client carries no path for it.
    #[test]
    fn the_record_is_opened_from_the_window_alone_and_names_no_file() {
        assert!(purlis_session_protocol::ui::window_only(
            "open_extension_record"
        ));
        let bound = include_str!("../../src/bindings.ts");
        let at = bound
            .find("openExtensionRecord: (")
            .expect("the window's client has the command");
        let signature = &bound[at..at + bound[at..].find(')').expect("its parameters end")];
        assert_eq!(signature, "openExtensionRecord: (editor: YourEditor");
    }

    /// A config home, and two projects whose committed default is a theme folder of their own.
    fn two_projects() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        [std::path::PathBuf; 2],
    ) {
        let home = tempfile::tempdir().expect("a directory");
        let config = home.path().join("config");
        std::fs::create_dir_all(purlis_core::machine::dir(&config)).expect("the config home");
        let planes = ["one", "two"].map(|name| {
            let plane = home.path().join(name);
            let folder = plane.join("themes").join(name);
            std::fs::create_dir_all(&folder).expect("a theme folder");
            std::fs::write(
                folder.join(purlis_core::theme::THEME_JSON),
                format!(r#"{{"name":"{name}"}}"#),
            )
            .expect("a theme");
            std::fs::write(
                purlis_core::names::manifest(&plane),
                format!("[theme]\nuse = \"{name}\"\n"),
            )
            .expect("a manifest");
            plane
        });
        (home, config, planes)
    }

    #[test]
    fn a_link_s_ask_of_the_theme_in_force_moves_no_window_s_watch() {
        // #2697: the window's form points its watch at what it answers for; the link's form,
        // asked for another project, answers it and leaves every window's watch where it was.
        let (_home, config, [one, two]) = two_projects();
        let watching = crate::themewatch::Target {
            config: config.clone(),
            plane: Some(one),
            workspace: None,
            drafts: Vec::new(),
        };
        crate::themewatch::follow_as("main", watching.clone());
        let before = crate::themewatch::following();
        assert_eq!(before.get("main"), Some(&watching));
        let asked = ThemeAsked {
            config,
            plane: Some(two),
            workspace: None,
            chats: Vec::new(),
            pick: None,
            on: None,
            light: false,
            more_contrast: false,
            reread: false,
            person: purlis_core::theme::Layer::default(),
        };
        let answered = asked.for_a_link();
        assert_eq!(
            answered.theme.map(|theme| theme.name).as_deref(),
            Some("two")
        );
        assert_eq!(crate::themewatch::following(), before);
    }

    #[test]
    fn a_link_is_served_the_theme_in_force_only_in_the_form_that_watches_nothing() {
        use purlis_session_protocol::ui;
        assert!(ui::window_only("theme_in_force"));
        assert!(!ui::served_on_a_link("theme_in_force"));
        assert!(ui::served_on_a_link("theme_in_force_at"));
    }

    #[test]
    fn a_link_is_answered_the_theme_as_kept_and_never_a_chat_s_draft() {
        // D-2697-3: the window draws a draft of a chat working where it is; a link's ask, even
        // one that names that chat, is answered the theme the levels choose.
        let (_home, config, [one, _]) = two_projects();
        std::fs::create_dir_all(one.join(".purlis")).expect("the project's own folder");
        let chat = crate::chats::Drafting {
            session: 3,
            id: purlis_core::reopen::mint(),
            name: "chat 3".to_owned(),
            workspace: None,
        };
        let place = purlis_core::theme::drafts::Place {
            plane: &one,
            workspace: None,
        };
        let path = purlis_core::theme::drafts::start(Some(&config), place, &chat.id, "dusk")
            .expect("a draft")
            .path;
        std::fs::write(
            path.join(purlis_core::theme::THEME_JSON),
            r#"{"name":"Dusk","appearances":[{"id":"dark","appearance":"dark"}]}"#,
        )
        .expect("a draft's theme");
        let asked = || ThemeAsked {
            config: config.clone(),
            plane: Some(one.clone()),
            workspace: None,
            chats: vec![chat.clone()],
            pick: None,
            on: None,
            light: false,
            more_contrast: false,
            reread: false,
            person: purlis_core::theme::Layer::default(),
        };
        let window = asked().read(None);
        assert_eq!(
            window.theme.map(|theme| theme.name).as_deref(),
            Some("Dusk")
        );
        assert!(window.draft.is_some());
        let link = asked().for_a_link();
        assert_eq!(link.theme.map(|theme| theme.name).as_deref(), Some("one"));
        assert_eq!(link.draft, None);
    }

    #[test]
    fn a_drawn_draft_says_when_the_theme_as_kept_is_refused_and_its_notice_waits() {
        // #2708: while a draft is drawn, nothing of the kept theme is refused in the answer, so
        // its Notice waits; the draft says so, for its strip.
        let (_home, config, [one, _]) = two_projects();
        std::fs::create_dir_all(one.join(".purlis")).expect("the project's own folder");
        let chat = crate::chats::Drafting {
            session: 3,
            id: purlis_core::reopen::mint(),
            name: "chat 3".to_owned(),
            workspace: None,
        };
        let place = purlis_core::theme::drafts::Place {
            plane: &one,
            workspace: None,
        };
        purlis_core::theme::drafts::start(Some(&config), place, &chat.id, "dusk").expect("a draft");
        let asked = || ThemeAsked {
            config: config.clone(),
            plane: Some(one.clone()),
            workspace: None,
            chats: vec![chat.clone()],
            pick: None,
            on: None,
            light: false,
            more_contrast: false,
            reread: false,
            person: purlis_core::theme::Layer::default(),
        };
        let drawn = asked().read(None);
        assert_eq!(drawn.draft.map(|draft| draft.kept_refused), Some(false));
        // The project's own theme saved as no theme at all: refused as kept.
        std::fs::write(
            one.join("themes/one").join(purlis_core::theme::THEME_JSON),
            "not json",
        )
        .expect("a broken theme");
        let drawn = asked().read(None);
        assert!(drawn.refused.is_none(), "the draft is drawn");
        assert_eq!(drawn.draft.map(|draft| draft.kept_refused), Some(true));
        assert!(asked().read(Some(true)).refused.is_some(), "as kept, it is");
    }
}
