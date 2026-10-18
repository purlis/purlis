// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's layout and the operator's theme, **handed to the page as it is created** and
//! written back when the layout changes (`purlis_core::windowprefs` holds the files and the
//! reasons; `purlis_core::theme` resolves the theme).
//!
//! **Injected, never fetched.** A command is asynchronous, so a layout the window asked for
//! would land after the first paint and the window would paint the default arrangement and then
//! re-lay itself out — the flash ADR 0038 removed. So `lib.rs` builds the main window itself,
//! from its entry in `tauri.conf.json`, with [`creation_script`] as its initialization script:
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

/// What the page receives. Every theme in it is a [`WindowTheme`], the one wire shape a
/// resolved theme has, which `theme_in_force` answers in too.
#[derive(Debug, serde::Serialize)]
struct AtCreation {
    layout: Reading,
    /// The person's own theme, resolved by the core: what the first frame is drawn in when it
    /// is in force.
    theme: TheirTheme,
    /// The themes purlis ships, resolved: what the first frame is drawn in otherwise.
    built_in: Vec<WindowTheme>,
    /// The person's density and font override, read by the engine from the layout file's
    /// `appearance` (#2326): what the first frame draws over the theme.
    person: PersonInForce,
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
}

impl From<Theirs> for TheirTheme {
    fn from(theirs: Theirs) -> Self {
        // What the first frame draws: the theme's first appearance for the contrast the system
        // asks for, which the engine draws with no pick (`purlis_core::theme::first_frame`), the
        // consent floors held. The window knows which contrast is asked; the core does not, so
        // both are handed.
        let drawn = |contrast| {
            theirs.theme.as_ref().and_then(|theme| {
                let drawn = theme::first_frame(Some(theme), contrast);
                Some(TheirDrawn {
                    theme: WindowTheme::from(drawn.theme.as_ref()?),
                    floors: drawn.held.map(|held| held.said()),
                })
            })
        };
        let standard = drawn(Contrast::Standard);
        let more = drawn(Contrast::More)
            .filter(|more| Some(&more.theme) != standard.as_ref().map(|it| &it.theme));
        Self {
            path: theirs.path,
            found: theirs.found,
            trouble: theirs.trouble,
            complaints: theirs.complaints,
            floors: standard.as_ref().and_then(|it| it.floors.clone()),
            theme: standard.map(|it| it.theme),
            more,
            format_1: theirs.format_1,
        }
    }
}

/// **The native window as the first frame draws it** (#2328): the appearance and the colour the
/// window is created in, from the same read of the person's theme as [`creation_script`], so
/// nothing is painted in another colour before the page's first frame. The system's contrast is
/// the page's to know: the window is created for the standard one, and the page tells the window
/// again as it draws (`nativewindow.rs`).
pub(crate) fn native_at_creation(config_root: Option<&Path>) -> theme::Native {
    native_at_creation_from(&THEIRS, config_root)
}

fn native_at_creation_from(kept: &TheirsKept, config_root: Option<&Path>) -> theme::Native {
    let theirs = kept.get(config_root);
    theme::first_frame(theirs.theme.as_ref(), Contrast::Standard)
        .theme
        .map_or_else(|| theme::default_theme().native(), |drawn| drawn.native())
}

/// **The person's own theme, read once for this process** (`purlis_core::theme::TheirsKept`):
/// every window's creation script and every `theme_in_force` answer resolve from the same read,
/// so a later window, or a link that never created one, draws what the first did.
pub(crate) static THEIRS: TheirsKept = TheirsKept::new();

/// The script that puts what was read on the page before any of the page's own code runs.
///
/// **What is read from disk reaches the page as JSON data, never as code.** It is serialised
/// by `serde_json`, which escapes every quote and control character in a string, so nothing in a
/// hand-edited file can end the literal it sits in; and the value is frozen, so nothing that
/// runs later can quietly rewrite what the launch read. A theme's values are the core's
/// resolved ones, each a colour or a number, never the file's text.
///
/// A machine with no config home hands the page an empty layout and no theme of the person's:
/// the default arrangement and the built-in theme, which is what such a machine has always had.
pub(crate) fn creation_script(config_root: Option<&Path>) -> String {
    creation_script_from(&THEIRS, config_root)
}

/// [`creation_script`], from the person's theme as `kept` holds it.
fn creation_script_from(kept: &TheirsKept, config_root: Option<&Path>) -> String {
    let layout = config_root.map_or_else(Reading::default, windowprefs::read_layout);
    // A layout file purlis refused has no document: the person's settings are the defaults.
    let appearance = layout
        .document
        .as_ref()
        .and_then(|document| document.get("appearance"));
    let person = PersonInForce::of(appearance);
    let at = AtCreation {
        theme: kept.get(config_root).into(),
        built_in: theme::built_ins().iter().map(WindowTheme::from).collect(),
        person,
        layout,
    };
    let json = serde_json::to_string(&at).expect("a reading is plain data serde can always write");
    format!(
        "Object.defineProperty(window, {GLOBAL:?}, {{ value: Object.freeze({json}), \
         writable: false, configurable: false }});"
    )
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
            theme::default_theme().value("surface.base")
        );
        let built: Vec<&str> = at["built_in"]
            .as_array()
            .expect("the built-ins")
            .iter()
            .filter_map(|one| one["name"].as_str())
            .collect();
        assert_eq!(built, purlis_core::extension::BUILT_IN_THEMES);
    }

    #[test]
    fn the_person_s_theme_is_handed_with_the_consent_floors_held_and_the_notice_for_them() {
        let home = tempfile::tempdir().unwrap();
        let dir = purlis_core::machine::dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let lost = theme::default_theme().value("surface.base");
        std::fs::write(
            dir.join(windowprefs::THEME),
            format!(r#"{{"name":"Mine","tokens":{{"state.failed":"{lost}"}}}}"#),
        )
        .unwrap();
        let at = defined(&creation_script_from(&TheirsKept::new(), Some(home.path())));
        assert_eq!(
            at["theme"]["theme"]["values"]["state.failed"],
            theme::default_theme().value("state.failed")
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
        assert_eq!(at["built_in"][0], one);
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
        // No theme of the person's: Purlis's, as the page draws it.
        let none = native_at_creation_from(&TheirsKept::new(), Some(home.path()));
        assert_eq!(none, theme::default_theme().native());
        std::fs::write(
            dir.join(windowprefs::THEME),
            r##"{"name":"Mine","appearance":"light","tokens":{"window.background":"#abcdef"}}"##,
        )
        .unwrap();
        let kept = TheirsKept::new();
        let native = native_at_creation_from(&kept, Some(home.path()));
        assert_eq!(native.appearance, theme::Appearance::Light);
        assert_eq!(native.background, [0xab, 0xcd, 0xef]);
        // The same read the page is handed: the theme its first frame draws.
        let at = defined(&creation_script_from(&kept, Some(home.path())));
        assert_eq!(at["theme"]["theme"]["appearance"], "light");
        assert_eq!(
            at["theme"]["theme"]["values"]["window.background"],
            "#abcdef"
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
        assert_eq!(at["built_in"].as_array().map(Vec::len), Some(2));
    }
}
