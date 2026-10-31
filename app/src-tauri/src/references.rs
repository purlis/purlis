// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Files into chats (FM-9, #1103 F7): a file, a folder or a range of lines of a branch, typed
//! into a chat as a reference in its harness's own syntax, and never sent.
//!
//! **The window names a branch and a path; the core builds the text.** Every command here
//! takes the branch by its names and the path inside it, places it with
//! `purlis_core::reference::of` (confined by `files::place`), and has the chat's harness
//! adapter render it. Nothing the window sends is typed as it was sent.
//!
//! **Typed the way a curation prompt is** (ADR 0061): one bracketed paste with nothing after
//! it ([`purlis_core::reference::pasted`]). Into a running chat only at its prompt
//! ([`purlis_core::reference::may_type_into`]: never mid-turn, never while it asks the
//! operator something); as the first prompt of a chat "Start a chat here" opens, through
//! `curation::start_typed_where_it_can_be`, held until its harness is ready exactly as a
//! curation prompt is.
//!
//! **Where it cannot be typed, it is copied.** opencode (ADR 0061 measured why), a program
//! charter has no adapter for, and a chat that is not at its prompt now: the reference goes on
//! the clipboard, and the answer says why, so the operator can paste it themselves.

use std::path::Path;
use std::sync::Arc;

use purlis_core::engine::Size;
use purlis_core::harness::Harness;
use purlis_core::reference::{self, Lines, Now, Reference};

use crate::curation::{self, ChatTyped, StartRefused};
use crate::planes::{Held, PlaneId, Planes};

/// A range of lines, as the window names one: both ends counted from 1 and included.
#[derive(Debug, Clone, Copy, serde::Deserialize, specta::Type)]
pub struct LineRange {
    pub first: u32,
    pub last: u32,
}

impl From<LineRange> for Lines {
    fn from(range: LineRange) -> Self {
        Lines {
            first: range.first,
            last: range.last,
        }
    }
}

/// What became of a reference handed to a chat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Handed {
    /// Typed into the chat, unsent: `text` is what was typed.
    Typed { text: String },
    /// Put on the clipboard instead, with the sentence saying why.
    Copied { text: String, why: String },
}

/// One file, folder or range of lines of a branch, typed into chat `session` as a reference in
/// its harness's syntax, and never sent — or copied, with the reason, where it cannot be typed
/// now.
// Refused, in the core's sentence, for a path the branch does not confine. Not a doc comment
// past here, because the generated bindings carry those.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub fn reference_into_chat(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, crate::vaults::SystemClipboard>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
    lines: Option<LineRange>,
    session: u32,
) -> Result<Handed, String> {
    let held = planes.held(&plane)?;
    let at = crate::piecefiles::branch(&workspace, &repo, &piece);
    into_chat(&held, at, &path, lines.map(Lines::from), session, &|text| {
        clipboard.put_text(text)
    })
}

/// Several files or folders of one branch, typed into chat `session` as one paste, each a
/// reference in its harness's syntax and a space between them, and never sent (#1151): what
/// several rows picked in the explorer hand a chat at once. Copied, with the reason, where it
/// cannot be typed now, as one is. Refused whole, in the core's sentence, where any path is one
/// the branch does not confine, and where there is none or more than [`MOST_AT_ONCE`].
///
/// **The window's alone** (`purlis_session_protocol::ui::WINDOW_ONLY`): it types into a chat as
/// the person, and until that kind of command has its ruling a new one fails closed.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub fn references_into_chat(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, crate::vaults::SystemClipboard>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    paths: Vec<String>,
    session: u32,
) -> Result<Handed, String> {
    let held = planes.held(&plane)?;
    let at = crate::piecefiles::branch(&workspace, &repo, &piece);
    into_chat_all(&held, at, &paths, session, &|text| clipboard.put_text(text))
}

/// The most references one paste carries: past it the person picks fewer, since a prompt of
/// hundreds of mentions is not one anybody reads.
pub const MOST_AT_ONCE: usize = 64;

/// [`references_into_chat`], against a project the registry has already vouched for, with the
/// clipboard as `copy`.
pub fn into_chat_all(
    held: &Arc<Held>,
    at: purlis_core::files::Branch<'_>,
    paths: &[String],
    session: u32,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Handed, String> {
    if paths.is_empty() {
        return Err("No files were picked, so there is nothing to hand the chat.".to_owned());
    }
    if paths.len() > MOST_AT_ONCE {
        return Err(format!(
            "{} files were picked, and purlis hands a chat at most {MOST_AT_ONCE} at once. Pick \
             fewer.",
            paths.len()
        ));
    }
    let chats = held.chats();
    let cwd = chats.chat_at(session).and_then(|chat| chat.cwd);
    let harness = chats.harness(session);
    let mut texts = Vec::with_capacity(paths.len());
    for path in paths {
        let made = reference::of(held.root(), at, path, None, cwd.as_deref())
            .map_err(|why| why.to_string())?;
        texts.push(rendered(harness, &made));
    }
    handed(held, session, texts.join(" "), Some(paths.len()), copy)
}

/// [`reference_into_chat`], against a project the registry has already vouched for, with the
/// clipboard as `copy`.
pub fn into_chat(
    held: &Arc<Held>,
    at: purlis_core::files::Branch<'_>,
    path: &str,
    lines: Option<Lines>,
    session: u32,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Handed, String> {
    let chats = held.chats();
    let cwd = chats.chat_at(session).and_then(|chat| chat.cwd);
    let made = reference::of(held.root(), at, path, lines, cwd.as_deref())
        .map_err(|why| why.to_string())?;
    handed(
        held,
        session,
        rendered(chats.harness(session), &made),
        None,
        copy,
    )
}

/// `text`, a reference or `several`, typed into chat `session` unsent, or copied with the reason
/// where it cannot be typed now.
fn handed(
    held: &Arc<Held>,
    session: u32,
    text: String,
    several: Option<usize>,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Handed, String> {
    // What the reason says went on the clipboard: one reference, or the several of one paste.
    let said = match several {
        Some(n) if n > 1 => Copied {
            on: "the references are",
            it: "they are",
            may: reference::may_type_into,
        },
        _ => Copied {
            on: "the reference is",
            it: "it is",
            may: reference::may_type_into,
        },
    };
    typed_or_copied(held, session, text, said, copy)
}

/// What a reason says went on the clipboard, in [`typed_or_copied`]'s two sentences: `on` ("the
/// reference is") once the chat was found, `it` ("it is") where its harness is not known; and
/// the rule that says whether the chat may be typed into now.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Copied {
    pub on: &'static str,
    pub it: &'static str,
    pub may: fn(Harness, Now) -> Result<(), reference::NotNow>,
}

/// **`text` typed into chat `session` unsent, as the person**, or copied with the reason where it
/// cannot be typed now: the one way the window types a line into a running chat, a reference's
/// (FM-9) or a skill's (SK-12, Use skill…), with `said` naming what went on the clipboard.
pub(crate) fn typed_or_copied(
    held: &Arc<Held>,
    session: u32,
    text: String,
    said: Copied,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<Handed, String> {
    let Copied { on, it, may } = said;
    let chats = held.chats();
    let harness = chats.harness(session);
    let copied = |why: String| -> Result<Handed, String> {
        copy(&text)?;
        Ok(Handed::Copied {
            text: text.clone(),
            why,
        })
    };
    let Some(harness) = harness else {
        return copied(format!(
            "purlis types a reference only into a chat on a harness it knows, so {it} on \
                 the clipboard"
        ));
    };
    if held.typed().waiting(session) {
        return copied(format!(
            "the chat's first prompt has not been typed yet, so {on} on the clipboard"
        ));
    }
    let glance = held.board().glance(session);
    let readiness = chats.sessions().readiness(session)?;
    let now = Now {
        state: glance.state,
        asking: glance.asking,
        edits_lines: readiness.edits_lines,
    };
    if let Err(not_now) = may(harness, now) {
        return copied(format!("{not_now}, so {on} on the clipboard"));
    }
    // Judged on what is pasted: the reference and the space after it.
    if let Some(why) = harness.why_drawn_as_a_placeholder(&format!("{text} ")) {
        return copied(format!("{why}, so {on} on the clipboard"));
    }
    // The operator's own act, so it goes where their keys go: it cancels a smart close as
    // typing does (ADR 0064).
    held.operator_input(session, reference::pasted(&text).as_bytes())?;
    Ok(Handed::Typed { text })
}

/// `made` in `harness`'s syntax, or in plain words for a program charter has no adapter for.
fn rendered(harness: Option<Harness>, made: &Reference) -> String {
    harness.map_or_else(|| made.plain(), |harness| harness.reference(made))
}

/// A chat "Start a chat here" opened: what the window needs to put its tab on the right strip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StartedHere {
    pub session: u32,
    /// The chat's name, which is its number.
    pub name: String,
    /// What its tab says: `About <file>`.
    pub label: String,
    /// The harness, by the word the plane calls it.
    pub harness: Option<String>,
    /// The workspace it is filed under.
    pub workspace: Option<String>,
    /// The reference, as typed or copied.
    pub text: String,
    /// Why it was copied rather than typed, or none when it is typed once the harness starts.
    pub copied: Option<String>,
}

/// Opens a chat on the project's default profile in the branch's own folder, with a reference
/// to one of its files or folders — or a range of lines of a file — typed as its first prompt
/// once its harness has started, and never sent. Where the harness cannot be typed into, the
/// chat still opens and the reference is put on the clipboard.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub async fn start_chat_here(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, crate::vaults::SystemClipboard>,
    plane: PlaneId,
    workspace: String,
    repo: String,
    piece: Option<String>,
    path: String,
    lines: Option<LineRange>,
    columns: u16,
    rows: u16,
) -> Result<StartedHere, StartRefused> {
    // Off the window's thread (SC-2): finding the branch's folder waits on the bounded reader's
    // child, and a busy gate holds each ask for its deadline, twice over (`BUSY_TRIES`). Only
    // the clipboard is left for here: a reference that could not be typed is handed back to
    // be copied.
    let held = planes.held(&plane)?;
    let (started, to_copy) = tauri::async_runtime::spawn_blocking(move || {
        let to_copy = std::sync::Mutex::new(None);
        let started = here(
            &held,
            crate::piecefiles::branch(&workspace, &repo, &piece),
            &path,
            lines.map(Lines::from),
            Size { columns, rows },
            &|text| {
                *to_copy
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text.to_string());
                Ok(())
            },
        );
        let to_copy = to_copy
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (started, to_copy)
    })
    .await
    .map_err(|err| format!("starting a chat here did not finish: {err}"))?;
    let started = started?;
    if let Some(text) = to_copy {
        clipboard.put_text(&text)?;
    }
    Ok(started)
}

/// How many times "Start a chat here" asks for its branch's folder while every reader place is
/// taken: once more. Each busy answer has already waited the reader's deadline, and the person
/// who pressed it is waiting too.
const BUSY_TRIES: usize = 2;

/// The folder of the branch `find` finds, asked again once if it met every reader place taken
/// (#1189); what refused it otherwise, as a sentence.
fn folder_found(
    find: impl FnMut() -> Result<purlis_core::files::Root, purlis_core::files::Refused>,
) -> Result<std::path::PathBuf, String> {
    crate::branchwatch::asked_past_busy(BUSY_TRIES, find)
        .and_then(|found| found.folder(""))
        .map_err(|why| why.to_string())
}

/// [`start_chat_here`], against a project the registry has already vouched for. A refused start
/// says the approval its profile needs, where it needs one (#3968).
pub fn here(
    held: &Arc<Held>,
    at: purlis_core::files::Branch<'_>,
    path: &str,
    lines: Option<Lines>,
    size: Size,
    copy: &dyn Fn(&str) -> Result<(), String>,
) -> Result<StartedHere, StartRefused> {
    let root = held.root().to_path_buf();
    // The branch's folder is found by the bounded reader's child, so this process starts no git
    // for it (#1189).
    let folder = folder_found(|| purlis_core::files::root(&crate::reader(), &root, at))?;
    let made =
        reference::of(&root, at, path, lines, Some(&folder)).map_err(|why| why.to_string())?;
    let (profile, kind) = curation::default_profile(&root, "no chat was started")?;
    let text = rendered(Harness::of_kind(&kind), &made);
    let label = label_for(path);
    let started = curation::start_typed_where_it_can_be(
        held,
        ChatTyped {
            profile: profile.clone(),
            persona: None,
            cwd: folder.clone(),
            label: label.clone(),
            prompt: text.clone(),
            then_a_space: true,
            starts_runs: None,
        },
        size,
        |_, declared, _| {
            Ok(declared.and_then(purlis_core::harness_declaration::Declaration::ready_to_type))
        },
        |why| format!("{why}, so the reference would not be readable and no chat was started"),
    )
    .map_err(StartRefused::on(&profile, &root))?;
    let copied = if started.typed {
        None
    } else {
        copy(&text)?;
        let harness = started
            .ready
            .declaration()
            .map_or("this profile's program", |declared| declared.title.as_str());
        Some(format!(
            "purlis cannot type into {harness} yet, so the reference is on the clipboard"
        ))
    };
    Ok(StartedHere {
        session: started.session,
        name: started.name,
        label,
        harness: started.ready.harness.map(|h| h.name().to_owned()),
        workspace: purlis_core::workspaces::Plane::open(&root)
            .workspace_of(started.ready.cwd.as_deref().unwrap_or(&folder)),
        text,
        copied,
    })
}

/// What a "Start a chat here" chat's tab says: `About <name>`, held to a chat name's length.
fn label_for(path: &str) -> String {
    let leaf = Path::new(path).file_name().map_or_else(
        || path.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let whole = format!("About {leaf}");
    let most = purlis_core::reopen::MOST_LABEL;
    if whole.chars().count() <= most {
        return whole;
    }
    let mut cut: String = whole.chars().take(most.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// #1189: "Start a chat here" asks again once when its folder found every reader place
    /// taken, and then says the reader was busy; a read that failed otherwise is said at once.
    #[test]
    fn start_a_chat_here_asks_a_busy_reader_once_more_then_says_it_was_busy() {
        let busy = || {
            purlis_core::files::Refused::Read(format!(
                "{}it was busy reading other branches for 30 seconds",
                purlis_core::files::READ_FAILED
            ))
        };
        let mut asks = 0;
        let said = folder_found(|| {
            asks += 1;
            Err(busy())
        });
        assert_eq!(asks, 2);
        assert_eq!(said, Err(busy().to_string()));

        let mut asks = 0;
        let said = folder_found(|| {
            asks += 1;
            if asks < 2 {
                Err(busy())
            } else {
                Err(purlis_core::files::Refused::Read("gone".into()))
            }
        });
        assert_eq!(asks, 2);
        assert_eq!(
            said,
            Err(purlis_core::files::Refused::Read("gone".into()).to_string())
        );
    }

    const SIZE: Size = Size {
        columns: 80,
        rows: 24,
    };

    /// A project with a workspace `shop` holding one clone, `shop`, and a profile `work` of
    /// `kind`, the default, running a stand-in harness that waits on its input.
    struct Project {
        _dir: tempfile::TempDir,
        _bin: tempfile::TempDir,
        root: std::path::PathBuf,
    }

    impl Project {
        fn new(kind: &str) -> Self {
            let dir = tempfile::tempdir().expect("a directory");
            let root = std::fs::canonicalize(dir.path()).expect("it resolves");
            std::fs::write(root.join(purlis_core::plane::MANIFEST), "schema = 1\n").unwrap();
            let clone = root.join("workspaces/shop/shop");
            std::fs::create_dir_all(&clone).unwrap();
            std::fs::write(root.join("workspaces/shop/workspace.md"), "# shop\n").unwrap();
            git(&clone, &["init", "-q", "-b", "main", "."]);
            std::fs::write(clone.join("README.md"), "# shop\n").unwrap();
            git(&clone, &["add", "-A"]);
            // Outside the project, and answering as Claude Code where it stands in for it: the floor
            // starts no program the chat could have written, and asks the one it starts what it is
            // (#2071).
            let bin = tempfile::tempdir().expect("a folder for the program");
            let program = stand_in::claude_code(
                bin.path(),
                "harness-stand-in",
                "#!/bin/sh\nstty raw -echo\nexec cat > /dev/null\n",
            );
            std::fs::write(
                root.join(purlis_core::profiles::LOCAL_FILE),
                format!(
                    "[harness]\ndefault = \"work\"\n\n[harness.work]\nkind = {kind:?}\n\
                     command = [{:?}]\n",
                    program.display().to_string()
                ),
            )
            .unwrap();
            let set = purlis_core::profiles::current(&root);
            let work = set.get("work").expect("the profile reads");
            purlis_core::profiletrust::record_launched(
                &root,
                "work",
                &purlis_core::profiletrust::fingerprint(work),
            )
            .expect("approved");
            // Codex and opencode run wrapped on macOS and Linux (#1040); these tests are about
            // the reference handed to the chat, so it starts without the sandbox here.
            if matches!(kind, "codex" | "opencode") {
                crate::sandboxing::no_sandbox_here_for_tests(&root);
            }
            Self {
                _dir: dir,
                _bin: bin,
                root,
            }
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let ran = purlis_core::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.email=t@e.invalid", "-c", "user.name=t"])
                .args(args),
        )
        .unwrap();
        assert!(ran.status.success(), "git {args:?}: {ran:?}");
    }

    fn planes() -> Planes {
        Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
    }

    fn shop() -> purlis_core::files::Branch<'static> {
        purlis_core::files::Branch::repo("shop", "shop")
    }

    #[cfg(unix)]
    #[test]
    fn start_a_chat_here_holds_the_reference_as_its_first_prompt_on_claude_code() {
        let project = Project::new("claude");
        let planes = planes();
        let held = planes.held(&planes.open(&project.root)).expect("held");
        let copied = Mutex::new(Vec::<String>::new());

        let started = here(&held, shop(), "README.md", None, SIZE, &|text| {
            copied.lock().unwrap().push(text.to_owned());
            Ok(())
        })
        .expect("it starts");

        assert_eq!(started.text, "@README.md");
        assert_eq!(started.copied, None);
        assert_eq!(started.label, "About README.md");
        assert_eq!(started.workspace.as_deref(), Some("shop"));
        assert!(
            held.typed().waiting(started.session),
            "held until it starts"
        );
        assert!(copied.lock().unwrap().is_empty());

        // A chat whose first prompt is still waiting is never typed into: the reference is
        // copied, and the waiting prompt is left as it was.
        let handed = into_chat(&held, shop(), "README.md", None, started.session, &|text| {
            copied.lock().unwrap().push(text.to_owned());
            Ok(())
        })
        .expect("it answers");
        assert!(matches!(handed, Handed::Copied { ref text, .. } if text == "@README.md"));
        assert!(held.typed().waiting(started.session));

        // Typed with a space after it, inside the one paste, so the operator's first word does
        // not stick to the mention.
        let typed = Mutex::new(String::new());
        held.typed().type_now(started.session, |paste| {
            *typed.lock().unwrap() = paste.to_owned();
            Ok(())
        });
        assert_eq!(*typed.lock().unwrap(), "\x1b[200~@README.md \x1b[201~");
        held.close_chat(started.session).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn start_a_chat_here_on_opencode_starts_it_and_copies_the_reference() {
        let project = Project::new("opencode");
        let planes =
            crate::sandboxing::keeping_events_for_tests(planes(), &project.root.join("events"));
        let held = planes.held(&planes.open(&project.root)).expect("held");
        let copied = Mutex::new(Vec::<String>::new());

        let started = here(&held, shop(), "README.md", None, SIZE, &|text| {
            copied.lock().unwrap().push(text.to_owned());
            Ok(())
        })
        .expect("it starts");

        assert_eq!(started.text, "@README.md");
        assert!(started.copied.as_deref().unwrap().contains("opencode"));
        assert_eq!(*copied.lock().unwrap(), vec!["@README.md".to_owned()]);
        assert!(
            !held.typed().waiting(started.session),
            "nothing is held to type"
        );
        held.close_chat(started.session).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn several_files_go_into_a_chat_as_one_paste_and_one_path_out_refuses_them_all() {
        // #1151 line 3: several rows picked in the explorer hand a chat one paste.
        let project = Project::new("claude");
        std::fs::write(
            project.root.join("workspaces/shop/shop/NOTES.md"),
            "notes\n",
        )
        .unwrap();
        let planes = planes();
        let held = planes.held(&planes.open(&project.root)).expect("held");
        let started = here(&held, shop(), "README.md", None, SIZE, &|_| Ok(())).expect("it starts");
        let copied = Mutex::new(Vec::<String>::new());
        let copy = |text: &str| {
            copied.lock().unwrap().push(text.to_owned());
            Ok(())
        };

        let paths = ["README.md".to_owned(), "NOTES.md".to_owned()];
        let handed = into_chat_all(&held, shop(), &paths, started.session, &copy).expect("answers");

        // Its first prompt still waits, so the one paste is copied, whole.
        assert!(
            matches!(handed, Handed::Copied { ref text, ref why }
                if text == "@README.md @NOTES.md" && why.ends_with("so the references are on the clipboard")),
            "{handed:?}"
        );
        let out = ["README.md".to_owned(), "../workspace.md".to_owned()];
        assert!(into_chat_all(&held, shop(), &out, started.session, &copy).is_err());
        assert_eq!(*copied.lock().unwrap(), ["@README.md @NOTES.md"]);
        held.close_chat(started.session).unwrap();
    }

    #[test]
    fn no_files_or_too_many_at_once_are_refused_before_anything_is_read() {
        let dir = tempfile::tempdir().expect("a directory");
        let root = std::fs::canonicalize(dir.path()).expect("it resolves");
        std::fs::write(root.join(purlis_core::plane::MANIFEST), "schema = 1\n").unwrap();
        let planes = planes();
        let held = planes.held(&planes.open(&root)).expect("held");
        let copy = |_: &str| -> Result<(), String> { panic!("nothing is copied") };

        let none = into_chat_all(&held, shop(), &[], 1, &copy).expect_err("refused");
        assert!(none.contains("No files were picked"), "{none}");
        let many: Vec<String> = (0..=MOST_AT_ONCE).map(|n| format!("f{n}.md")).collect();
        let refused = into_chat_all(&held, shop(), &many, 1, &copy).expect_err("refused");
        assert!(refused.contains("at most 64 at once"), "{refused}");
    }

    #[test]
    fn a_path_out_of_the_branch_starts_nothing() {
        let project = Project::new("claude");
        let planes = planes();
        let held = planes.held(&planes.open(&project.root)).expect("held");

        for path in ["../workspace.md", ".git/config", "/etc/passwd"] {
            let refused = here(&held, shop(), path, None, SIZE, &|_| Ok(()));
            assert!(refused.is_err(), "{path} started a chat");
        }
        assert!(held.chats().open_now().is_empty());
    }

    #[test]
    fn a_tab_is_labelled_by_the_file_it_is_about() {
        assert_eq!(label_for("src/main.rs"), "About main.rs");
        assert_eq!(label_for("src"), "About src");
        assert!(label_for(&"x".repeat(500)).chars().count() <= purlis_core::reopen::MOST_LABEL);
    }
}
