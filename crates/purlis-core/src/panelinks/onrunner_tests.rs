// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use purlis_session_protocol::session::{
    FileHome, FilePiece, FileReach, FileRead, MOST_FILE_PIECE, MOST_FILE_READ,
};

use super::{Asked, read_for_window};

/// A runner's kept project on disk: workspace `app` with a clone `web` the chat works in, a
/// second workspace `docs`, and a file outside the project.
struct Runner {
    _held: tempfile::TempDir,
    project: PathBuf,
    cwd: PathBuf,
    outside: PathBuf,
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    std::fs::write(path, bytes).expect("written");
}

fn runner() -> Runner {
    let held = tempfile::tempdir().expect("a temporary folder");
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let project = top.join("project");
    let cwd = project.join("workspaces/app/repos/web");
    write(&cwd.join("src/main.rs"), b"fn main() {}\n");
    write(&project.join("workspaces/app/workspace.md"), b"# app\n");
    write(&project.join("workspaces/docs/guide.md"), b"# docs\n");
    let outside = top.join("etc/hosts");
    write(&outside, b"127.0.0.1 localhost\n");
    Runner {
        _held: held,
        project,
        cwd,
        outside,
    }
}

impl Runner {
    fn asked<'a>(&'a self, path: &'a str, reach: FileReach, at: Option<u64>) -> Asked<'a> {
        Asked {
            project: &self.project,
            cwd: Some(&self.cwd),
            path,
            reach,
            at,
        }
    }

    fn look(&self, path: &str) -> FileRead {
        read_for_window(&self.asked(path, FileReach::Workspace, None))
    }

    fn look_anywhere(&self, path: &str) -> FileRead {
        read_for_window(&self.asked(path, FileReach::Anywhere, None))
    }

    fn read(&self, path: &str, reach: FileReach, at: u64) -> FileRead {
        read_for_window(&self.asked(path, reach, Some(at)))
    }

    fn shown(&self, path: &Path) -> String {
        path.to_str().expect("a path of text").to_owned()
    }
}

/// The plain file's resolved path and its path from the workspace.
fn plain(read: &FileRead) -> (&str, Option<&str>) {
    match read {
        FileRead::Plain {
            path,
            from_workspace,
            ..
        } => (path.as_str(), from_workspace.as_deref()),
        other => panic!("expected a plain file, got {other:?}"),
    }
}

fn piece(read: &FileRead) -> &FilePiece {
    match read {
        FileRead::Plain {
            piece: Some(piece), ..
        } => piece,
        other => panic!("expected a piece of a plain file, got {other:?}"),
    }
}

#[test]
fn a_relative_path_is_found_from_the_chats_folder_and_said_from_its_workspace() {
    let r = runner();

    let found = r.look("src/main.rs");

    let (path, from_workspace) = plain(&found);
    assert_eq!(path, r.shown(&r.cwd.join("src/main.rs")));
    assert_eq!(from_workspace, Some("repos/web/src/main.rs"));
    assert!(
        matches!(
            found,
            FileRead::Plain {
                size: 13,
                piece: None,
                ..
            }
        ),
        "a look carries no bytes: {found:?}"
    );
}

#[test]
fn an_absolute_path_in_the_workspace_is_found_as_written() {
    let r = runner();
    let path = r.shown(&r.project.join("workspaces/app/workspace.md"));

    assert_eq!(plain(&r.look(&path)).1, Some("workspace.md"));
}

#[test]
fn a_read_carries_the_files_bytes_from_where_it_was_asked() {
    let r = runner();

    let read = r.read("src/main.rs", FileReach::Workspace, 3);

    assert_eq!(
        piece(&read),
        &FilePiece {
            at: 3,
            bytes: b"main() {}\n".to_vec()
        }
    );
}

#[test]
fn a_file_larger_than_a_piece_is_read_in_pieces_of_one_version() {
    let r = runner();
    let size = usize::try_from(MOST_FILE_PIECE).expect("a size") + 10;
    write(&r.cwd.join("big.txt"), &vec![b'a'; size]);

    let first = r.read("big.txt", FileReach::Workspace, 0);
    let last = r.read("big.txt", FileReach::Workspace, MOST_FILE_PIECE);

    assert_eq!(piece(&first).bytes.len() as u64, MOST_FILE_PIECE);
    assert_eq!(piece(&last).bytes, vec![b'a'; 10]);
    let version = |read: &FileRead| match read {
        FileRead::Plain { version, .. } => version.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(version(&first), version(&last));
    assert!(!version(&first).is_empty());
}

#[test]
fn nothing_there_is_not_there() {
    let r = runner();

    assert_eq!(r.look("src/gone.rs"), FileRead::NotThere);
    assert_eq!(r.look(""), FileRead::NotThere);
}

#[test]
fn a_home_path_is_never_looked_up_on_the_runner() {
    let r = runner();
    // Not even as a folder named `~` in the chat's own folder.
    write(&r.cwd.join("~/.config/gh/hosts.yml"), b"token\n");

    for path in ["~/.config/gh/hosts.yml", "~", "~purlis/x"] {
        assert_eq!(r.look_anywhere(path), FileRead::NotThere, "{path}");
    }
}

#[test]
fn a_path_with_a_control_character_is_not_there() {
    let r = runner();

    assert_eq!(r.look("src/main.rs\n"), FileRead::NotThere);
    assert_eq!(r.look("src/\u{0}main.rs"), FileRead::NotThere);
}

#[test]
fn dot_dot_out_of_the_workspace_is_outside_and_not_read() {
    let r = runner();
    let up = "../../../../../etc/hosts";

    assert_eq!(
        r.read(up, FileReach::Workspace, 0),
        FileRead::Outside {
            path: r.shown(&r.outside)
        }
    );
    // Into another workspace of the same project is outside too.
    assert!(matches!(
        r.look("../../../docs/guide.md"),
        FileRead::Outside { .. }
    ));
}

#[test]
fn dot_dot_that_stays_in_the_workspace_is_found() {
    let r = runner();

    assert_eq!(plain(&r.look("../../workspace.md")).1, Some("workspace.md"));
}

#[test]
fn a_file_outside_is_read_only_when_it_may_reach_anywhere() {
    let r = runner();
    let path = r.shown(&r.outside);

    assert!(matches!(r.look(&path), FileRead::Outside { .. }));
    let read = r.read(&path, FileReach::Anywhere, 0);
    assert_eq!(plain(&read), (path.as_str(), None));
    assert_eq!(piece(&read).bytes, b"127.0.0.1 localhost\n");
}

#[cfg(unix)]
#[test]
fn a_link_in_the_workspace_that_leads_out_of_it_is_outside() {
    let r = runner();
    std::os::unix::fs::symlink(&r.outside, r.cwd.join("hosts")).expect("an absolute link");
    std::os::unix::fs::symlink("../../../../../etc", r.cwd.join("etc")).expect("a folder link");

    for path in ["hosts", "etc/hosts"] {
        assert_eq!(
            r.read(path, FileReach::Workspace, 0),
            FileRead::Outside {
                path: r.shown(&r.outside)
            },
            "{path}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_that_stays_in_the_workspace_is_followed_to_its_file() {
    let r = runner();
    std::os::unix::fs::symlink("src/main.rs", r.cwd.join("main")).expect("a link");

    let found = r.look("main");
    let (path, from_workspace) = plain(&found);
    assert_eq!(path, r.shown(&r.cwd.join("src/main.rs")));
    assert_eq!(from_workspace, Some("repos/web/src/main.rs"));
}

#[cfg(unix)]
#[test]
fn a_workspace_reached_through_a_link_holds_only_what_resolves_inside_it() {
    let r = runner();
    // The chat's folder spelled through a link: what is inside it is still inside.
    let top = r.project.parent().expect("a parent").to_path_buf();
    std::os::unix::fs::symlink(&r.project, top.join("alias")).expect("a link");
    let cwd = top.join("alias/workspaces/app/repos/web");
    let asked = Asked {
        project: &r.project,
        cwd: Some(&cwd),
        path: "src/main.rs",
        reach: FileReach::Workspace,
        at: None,
    };

    assert_eq!(
        plain(&read_for_window(&asked)).1,
        Some("repos/web/src/main.rs")
    );
}

#[test]
fn a_folder_is_not_a_plain_file() {
    let r = runner();

    assert_eq!(
        r.look("src"),
        FileRead::NotPlain {
            path: r.shown(&r.cwd.join("src"))
        }
    );
}

#[cfg(unix)]
#[test]
fn a_pipe_is_not_a_plain_file_and_is_never_waited_on() {
    let r = runner();
    let fifo = r.cwd.join("pipe");
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&fifo))
        .expect("mkfifo runs");
    assert!(made.success(), "a pipe");

    assert_eq!(
        r.read("pipe", FileReach::Workspace, 0),
        FileRead::NotPlain {
            path: r.shown(&fifo)
        }
    );
}

#[cfg(unix)]
#[test]
fn a_device_is_never_read_even_when_it_may_reach_anywhere() {
    let r = runner();
    std::os::unix::fs::symlink("/dev/zero", r.cwd.join("zero")).expect("a link");

    for path in ["/dev/null", "/dev/zero", "zero"] {
        assert!(
            matches!(
                r.read(path, FileReach::Anywhere, 0),
                FileRead::NotPlain { .. }
            ),
            "{path}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn the_systems_own_files_are_never_read() {
    let r = runner();

    for path in [
        "/proc/self/environ",
        "/proc/self/cmdline",
        "/sys/kernel/hostname",
    ] {
        assert!(
            matches!(
                r.read(path, FileReach::Anywhere, 0),
                FileRead::NotPlain { .. } | FileRead::NotThere
            ),
            "{path}"
        );
    }
}

#[test]
fn a_file_with_a_nul_byte_is_binary_and_carries_none_of_its_bytes() {
    let r = runner();
    // Valid UTF-8 but for its NUL: a NUL alone makes it binary.
    write(&r.cwd.join("data.bin"), b"PNG\0\0\0IHDR");

    assert_eq!(
        r.read("data.bin", FileReach::Workspace, 0),
        FileRead::Binary {
            path: r.shown(&r.cwd.join("data.bin")),
            size: 10
        }
    );
}

#[test]
fn a_file_that_is_not_utf8_is_binary() {
    let r = runner();
    write(&r.cwd.join("latin1.txt"), b"caf\xe9\n");

    assert!(matches!(
        r.look("latin1.txt"),
        FileRead::Binary { size: 5, .. }
    ));
}

#[test]
fn a_file_past_the_largest_read_is_too_large_and_carries_none_of_its_bytes() {
    let r = runner();
    let size = MOST_FILE_READ + 1;
    write(
        &r.cwd.join("huge.log"),
        &vec![b'x'; usize::try_from(size).expect("a size")],
    );

    assert_eq!(
        r.read("huge.log", FileReach::Workspace, 0),
        FileRead::TooLarge {
            path: r.shown(&r.cwd.join("huge.log")),
            size
        }
    );
}

#[test]
fn a_file_of_exactly_the_largest_read_is_read() {
    let r = runner();
    write(
        &r.cwd.join("edge.log"),
        &vec![b'x'; usize::try_from(MOST_FILE_READ).expect("a size")],
    );

    assert!(matches!(
        r.look("edge.log"),
        FileRead::Plain { size, .. } if size == MOST_FILE_READ
    ));
}

#[test]
fn a_relative_path_with_no_folder_to_start_from_is_not_there() {
    let r = runner();
    let base = r.asked("src/main.rs", FileReach::Anywhere, None);
    let asked = Asked { cwd: None, ..base };

    assert_eq!(read_for_window(&asked), FileRead::NotThere);
}

#[test]
fn a_chat_at_the_projects_root_counts_the_project_as_its_workspace() {
    let r = runner();
    let base = r.asked("workspaces/docs/guide.md", FileReach::Workspace, None);
    let asked = Asked {
        cwd: Some(&r.project),
        ..base
    };

    assert_eq!(
        plain(&read_for_window(&asked)).1,
        Some("workspaces/docs/guide.md")
    );
}

#[test]
fn a_files_identity_is_its_device_and_number_whatever_its_size_and_time() {
    assert_eq!(
        super::identity_of("64769:1234:13:1700000000.5"),
        Some("64769:1234")
    );
    assert_eq!(
        super::identity_of("64769:1234:99:1800000000.0"),
        Some("64769:1234")
    );
    assert_eq!(super::identity_of("13:1700000000"), None);
    assert_eq!(super::identity_of(""), None);
}

/// The chat's home as the runner said it with a plain file.
fn home(read: &FileRead) -> Option<&FileHome> {
    match read {
        FileRead::Plain { home, .. } => home.as_ref(),
        other => panic!("expected a plain file, got {other:?}"),
    }
}

#[test]
fn a_plain_file_comes_with_the_chats_workspace_there_for_the_ask() {
    let r = runner();

    let read = r.look_anywhere(&r.shown(&r.outside));

    assert_eq!(
        home(&read),
        Some(&FileHome {
            workspace: Some("app".into()),
            folder: r.shown(&r.project.join("workspaces/app")),
        })
    );
}

#[test]
fn a_chat_at_the_projects_root_says_its_project_for_the_ask() {
    let r = runner();
    let path = r.shown(&r.outside);
    let base = r.asked(&path, FileReach::Anywhere, None);
    let asked = Asked {
        cwd: Some(&r.project),
        ..base
    };

    assert_eq!(
        home(&read_for_window(&asked)),
        Some(&FileHome {
            workspace: None,
            folder: r.shown(&r.project),
        })
    );
}
