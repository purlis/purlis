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
    /// The runner's home, beside the project, and the runner as purlis knows it.
    home: PathBuf,
    machine: crate::sandbox::Machine,
}

/// A runner whose home is `home`, as purlis knows a runner: Linux, with no variable moving a
/// folder.
fn machine_at(home: &Path) -> crate::sandbox::Machine {
    crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(home.to_path_buf()),
        os: crate::sandbox::Os::Linux,
    }
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
    let home = top.join("home");
    std::fs::create_dir_all(&home).expect("a home");
    Runner {
        _held: held,
        project,
        cwd,
        outside,
        machine: machine_at(&home),
        home,
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
            machine: &self.machine,
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
        machine: &r.machine,
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

/// Every harness's sign-in files on the runner, as each harness's adapter names them, written.
fn sign_ins(r: &Runner) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for harness in crate::harness::Harness::ALL {
        let folders = crate::sandbox::signin::ProfileFolders::of(harness, &[], &r.machine);
        files.extend(crate::sandbox::signin::credential_files(harness, &folders));
    }
    assert!(
        files.len() >= 3,
        "every harness names its sign-in: {files:?}"
    );
    for file in &files {
        write(file, b"{\"token\":\"secret\"}\n");
    }
    files
}

/// Whether `read` is the runner keeping the file from the window, with none of its bytes.
fn kept(read: &FileRead) -> bool {
    matches!(read, FileRead::Kept { .. })
}

#[test]
fn a_harness_sign_in_is_kept_even_when_the_person_answered_the_ask() {
    let r = runner();

    for file in sign_ins(&r) {
        let path = r.shown(&file);
        assert!(kept(&r.look_anywhere(&path)), "{path}");
        assert!(kept(&r.read(&path, FileReach::Anywhere, 0)), "{path}");
    }
}

#[test]
fn purlis_own_folders_on_the_runner_are_kept_even_when_the_person_answered_the_ask() {
    let r = runner();
    let data = r.home.join(".local/share/purlis");
    let config = r.home.join(".config/purlis");
    let files = [
        data.join("profiles/work/.claude/settings.json"),
        data.join("app/events.jsonl"),
        r.home.join(".local/share/charter/app/events.jsonl"),
        config.join("runners.json"),
        r.home.join(".config/charter/machine.json"),
    ];
    for file in &files {
        write(file, b"{}\n");
    }

    for file in &files {
        let path = r.shown(file);
        assert!(kept(&r.look_anywhere(&path)), "{path}");
        assert!(kept(&r.read(&path, FileReach::Anywhere, 0)), "{path}");
    }
}

#[test]
fn a_kept_file_is_kept_by_every_path_that_names_it() {
    let r = runner();
    let files = sign_ins(&r);
    let codex = files
        .iter()
        .find(|file| file.ends_with(".codex/auth.json"))
        .expect("Codex's sign-in");
    let relative = format!(
        "../../../../../home/{}",
        codex.strip_prefix(&r.home).expect("in the home").display()
    );
    let dotted = format!("{}/../.codex/./auth.json", r.shown(&r.home.join(".codex")));

    for path in [relative.as_str(), dotted.as_str()] {
        assert!(kept(&r.read(path, FileReach::Anywhere, 0)), "{path}");
    }
    // `~` is never looked up on a runner, so its sign-in is not read that way either.
    assert_eq!(
        r.read("~/.codex/auth.json", FileReach::Anywhere, 0),
        FileRead::NotThere
    );
}

#[cfg(unix)]
#[test]
fn a_link_to_a_kept_file_or_folder_is_kept() {
    let r = runner();
    let files = sign_ins(&r);
    let codex = files
        .iter()
        .find(|file| file.ends_with(".codex/auth.json"))
        .expect("Codex's sign-in");
    std::os::unix::fs::symlink(codex, r.cwd.join("auth.json")).expect("a link to the file");
    std::os::unix::fs::symlink(r.home.join(".codex"), r.cwd.join("codex"))
        .expect("a link to its folder");
    std::fs::hard_link(codex, r.cwd.join("second.json")).expect("a second name of it");
    write(&r.home.join(".local/share/purlis/app/state.json"), b"{}\n");
    std::os::unix::fs::symlink(r.home.join(".local/share/purlis"), r.cwd.join("data"))
        .expect("a link to purlis's data home");

    for path in [
        "auth.json",
        "codex/auth.json",
        "second.json",
        "data/app/state.json",
    ] {
        assert!(kept(&r.read(path, FileReach::Workspace, 0)), "{path}");
        assert!(kept(&r.read(path, FileReach::Anywhere, 0)), "{path}");
    }
}

#[test]
fn a_kept_file_is_kept_under_any_case_on_a_disk_that_folds_it() {
    let r = runner();
    let files = sign_ins(&r);
    let codex = files
        .iter()
        .find(|file| file.ends_with(".codex/auth.json"))
        .expect("Codex's sign-in");
    let shouted = r.shown(&r.home.join(".CODEX/Auth.JSON"));
    if std::fs::metadata(&shouted).is_err() {
        // This disk tells the two names apart: there is no other file to keep.
        return;
    }

    assert!(kept(&r.read(&shouted, FileReach::Anywhere, 0)), "{shouted}");
    assert!(codex.exists());
}

#[test]
fn a_project_the_runner_keeps_in_its_data_home_is_read_and_the_rest_of_it_is_not() {
    let held = tempfile::tempdir().expect("a temporary folder");
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let home = top.join("home");
    let projects = home.join(".local/share/purlis/projects");
    let project = projects.join("01J9PRJCT00000000000000000");
    let cwd = project.join("workspaces/app");
    write(&cwd.join("notes.md"), b"# notes\n");
    let other = projects.join("01J9PRJCT00000000000000001/workspaces/app/notes.md");
    write(&other, b"# not this chat's\n");
    let machine = machine_at(&home);
    let read = |path: &str| {
        read_for_window(&Asked {
            project: &project,
            cwd: Some(&cwd),
            path,
            reach: FileReach::Anywhere,
            at: Some(0),
            machine: &machine,
        })
    };

    assert_eq!(piece(&read("notes.md")).bytes, b"# notes\n");
    assert!(kept(&read(&other.display().to_string())));
}

#[cfg(unix)]
#[test]
fn a_hard_link_in_the_workspace_is_placed_by_its_own_name_as_the_host_places_one() {
    let r = runner();
    std::fs::hard_link(&r.outside, r.cwd.join("hosts")).expect("a second name of it");

    let found = r.look("hosts");

    let (path, from_workspace) = plain(&found);
    assert_eq!(path, r.shown(&r.cwd.join("hosts")));
    assert_eq!(from_workspace, Some("repos/web/hosts"));
    assert_eq!(
        piece(&r.read("hosts", FileReach::Workspace, 0)).bytes,
        b"127.0.0.1 localhost\n"
    );
}

#[cfg(unix)]
#[test]
fn a_hard_link_whose_other_name_is_in_a_kept_folder_is_kept() {
    let r = runner();
    let in_kept = [
        r.home.join(".config/purlis/runners.json"),
        r.home.join(".local/share/purlis/profiles/work/notes.md"),
        r.home.join(".ssh/notes"),
    ];
    for (n, file) in in_kept.iter().enumerate() {
        write(file, b"{}\n");
        std::fs::hard_link(file, r.cwd.join(format!("second-{n}"))).expect("a second name");
    }
    sign_ins(&r);

    for n in 0..in_kept.len() {
        let path = format!("second-{n}");
        assert!(kept(&r.look(&path)), "{path}");
        assert!(kept(&r.read(&path, FileReach::Anywhere, 0)), "{path}");
    }
}

#[cfg(unix)]
#[test]
fn a_hard_link_between_two_files_of_a_project_in_the_data_home_is_read() {
    let held = tempfile::tempdir().expect("a temporary folder");
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let home = top.join("home");
    let project = home.join(".local/share/purlis/projects/01J9PRJCT00000000000000000");
    let cwd = project.join("workspaces/app");
    write(&cwd.join("notes.md"), b"# notes\n");
    std::fs::hard_link(cwd.join("notes.md"), cwd.join("again.md")).expect("a second name");
    let machine = machine_at(&home);

    let read = read_for_window(&Asked {
        project: &project,
        cwd: Some(&cwd),
        path: "again.md",
        reach: FileReach::Workspace,
        at: Some(0),
        machine: &machine,
    });

    assert_eq!(piece(&read).bytes, b"# notes\n");
}
