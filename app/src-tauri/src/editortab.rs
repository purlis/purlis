// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A terminal `$VISUAL`/`$EDITOR` opens in a shell tab, at the line** (#1042, ADR 0062,
//! ADR 0081 §3).
//!
//! A terminal editor started with no terminal reads the end of its input and exits, so
//! purlis starts it in a tab of its own instead: a shell tab (ADR 0062) whose program is the
//! editor, standing in the file's branch folder. An editor that opens its own window is started
//! as it always was ([`crate::piecefiles::started`]). Which is which is
//! [`youreditor::in_a_terminal`]'s known list (D-qw309-1).
//!
//! **The editor is an argument vector, never a command line.** The program, the words
//! `$VISUAL`/`$EDITOR` was split into, `+line` and the file are the tab's program and its
//! arguments, started as the operator's shell is ([`crate::chats::Chats::start_operator_shell`]).
//! No shell reads them, so nothing in the variable or in the file's name is interpreted.
//!
//! The core starts the tab and the window is told on [`OPENED`], as a handoff's chat is: the
//! window draws it in front and files it on the branch's workspace, since the person asked for
//! it (D-qw309-3).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use purlis_core::engine::Size;
use purlis_core::reopen::Chat;
use purlis_core::youreditor;

use crate::planes::{Held, PlaneId};

/// The event the window is sent when a terminal editor opened in a tab.
pub const OPENED: &str = "editor-tab-opened";

/// Where the window files a tab that stands in no workspace: the project root's strip. The
/// window's own word for it (`OUTSIDE` in `app/src/actions.ts`), which no workspace can be
/// named, as a workspace's name is a folder's.
pub const PROJECT_ROOT: &str = "outside/every/workspace";

/// A terminal editor's tab, as the window needs it to draw one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Opened {
    pub plane: PlaneId,
    pub session: u32,
    /// The tab's name: the editor and the file's name, `vim lib.rs`.
    pub name: String,
    /// The workspace whose strip it is filed on.
    pub workspace: String,
}

/// The size the tab starts at: it has no pane yet to ask, and the pane it lands in tells it the
/// real one when it is first shown.
const STARTING: Size = Size {
    columns: 80,
    rows: 24,
};

/// The file a branch holds, opened in the editor `$VISUAL`/`$EDITOR` named: in a tab of its own
/// standing in `cwd` when it is a terminal editor, and started detached otherwise. `cwd` is
/// worked out only for a tab.
pub(crate) async fn started(
    app: &tauri::AppHandle,
    held: &Held,
    plane: &PlaneId,
    workspace: &str,
    cwd: impl FnOnce() -> Result<PathBuf, String>,
    program: String,
    args: Vec<OsString>,
) -> Result<(), String> {
    if !youreditor::in_a_terminal(&program, &args) {
        return crate::piecefiles::started(program, args).await;
    }
    let chat = tab_chat(program, args, cwd()?)?;
    let session = in_a_tab(held, &chat)?;
    crate::windows::emit_for_plane(
        app,
        plane,
        OPENED,
        Opened {
            plane: plane.clone(),
            session,
            name: chat.name,
            workspace: workspace.to_owned(),
        },
    );
    Ok(())
}

/// The tab's chat: the editor as its program, its words, `+line` and the file as its
/// arguments, standing in `cwd`, named for the editor and the file. Refused for a word that is
/// not UTF-8, since a chat's words are recorded as text (D-qw309-4).
pub(crate) fn tab_chat(program: String, args: Vec<OsString>, cwd: PathBuf) -> Result<Chat, String> {
    let name = tab_name(&program, args.last().map(Path::new));
    let args = args
        .into_iter()
        .map(|arg| {
            arg.into_string().map_err(|arg| {
                format!(
                    "{} is not UTF-8, and a terminal editor's tab records its words as text, \
                     so nothing was opened. Choose VS Code, Zed or a JetBrains IDE in Settings \
                     to open it",
                    Path::new(&arg).display()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(crate::unprofiled(Some(program), args, Some(cwd), name))
}

/// `vim lib.rs`: the editor's own name and the file's.
fn tab_name(program: &str, file: Option<&Path>) -> String {
    let editor = Path::new(program)
        .file_name()
        .map_or_else(|| program.into(), |name| name.to_string_lossy());
    match file.and_then(Path::file_name) {
        Some(file) => format!("{editor} {}", file.to_string_lossy()),
        None => editor.into_owned(),
    }
}

/// `chat` started as the operator's own shell is: in a tab, through the kill switch, as the
/// detached editor always was (D-qw309-2).
pub(crate) fn in_a_tab(held: &Held, chat: &Chat) -> Result<u32, String> {
    held.chats().start_operator_shell(chat, STARTING)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_tab_runs_the_editor_with_its_words_the_line_and_the_file() {
        let chat = tab_chat(
            "/usr/bin/vim".to_owned(),
            os(&["-u", "NONE", "+12", "/work/a b;$(x).md"]),
            PathBuf::from("/work"),
        )
        .unwrap();

        assert_eq!(chat.program, "/usr/bin/vim");
        assert_eq!(chat.args, ["-u", "NONE", "+12", "/work/a b;$(x).md"]);
        assert_eq!(chat.cwd.as_deref(), Some(Path::new("/work")));
        assert_eq!(chat.name, "vim a b;$(x).md");
        assert_eq!(chat.profile, None);
        assert_eq!(chat.harness(), None);
    }

    #[test]
    fn the_tab_is_named_for_the_editor_alone_when_there_is_no_file() {
        assert_eq!(tab_name("nvim", None), "nvim");
        assert_eq!(
            tab_name("/bin/nano", Some(Path::new("/w/x.rs"))),
            "nano x.rs"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_word_that_is_not_utf8_is_refused_with_a_sentence() {
        use std::os::unix::ffi::OsStringExt as _;
        let file = OsString::from_vec(b"/work/caf\xe9.md".to_vec());

        let refused = tab_chat(
            "vim".to_owned(),
            vec![OsString::from("+1"), file],
            PathBuf::from("/work"),
        )
        .unwrap_err();

        assert!(refused.contains("is not UTF-8"), "{refused}");
        assert!(refused.contains("nothing was opened"), "{refused}");
    }

    /// #1042: the tab's program gets the file's name as one argument, never read by a shell.
    /// It needs a pseudo-terminal, so it first runs on CI.
    #[cfg(unix)]
    #[test]
    fn the_editor_in_its_tab_gets_the_file_as_one_word_no_shell_reads() {
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        let dir = tempfile::tempdir().expect("a directory");
        let config = dir.path().join("config-home");
        std::fs::create_dir_all(&config).expect("a config home");
        let project = dir.path().join("one");
        std::fs::create_dir_all(&project).expect("the project");
        std::fs::write(project.join(purlis_core::plane::MANIFEST), "").expect("its manifest");
        let branch = dir.path().join("branch");
        std::fs::create_dir_all(&branch).expect("the branch folder");
        let planes = crate::planes::Planes::telling(
            Arc::new(|_: crate::hooks::Moved| {}),
            crate::Shipped::default(),
            Some(config),
        );
        let held = planes.held(&planes.open(&project)).expect("held");
        let file = branch.join("a b;$(touch pwned).md");
        // A stand-in editor: `/bin/sh -c` with a fixed script, which writes the words it was
        // handed after its own, one per line.
        let chat = tab_chat(
            "/bin/sh".to_owned(),
            vec![
                OsString::from("-c"),
                OsString::from(r#"printf '%s\n' "$@" > heard"#),
                OsString::from("sh"),
                OsString::from("+7"),
                file.clone().into_os_string(),
            ],
            branch.clone(),
        )
        .expect("a tab's chat");

        in_a_tab(&held, &chat).expect("the tab starts");

        let heard = branch.join("heard");
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut said = String::new();
        while Instant::now() < deadline {
            said = std::fs::read_to_string(&heard).unwrap_or_default();
            if said.lines().count() == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        planes.let_go_of_all();
        assert_eq!(said, format!("+7\n{}\n", file.display()));
        assert!(
            !branch.join("pwned").exists(),
            "no shell read the file's name"
        );
    }
}
