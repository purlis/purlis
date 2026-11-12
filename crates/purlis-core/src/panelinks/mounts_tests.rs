// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{
    Asks, Candidate, Kind, LocalFile, Opening, Pane, ReadFrom, Resolved, Resolver, Runs, Target,
};
use super::{Mount, Mounts};

/// A project on disk with workspace `smart-ide` and its repo `app`, a folder outside the
/// project (`srv`) with a file of its own, a secret file beside the repo's folder, and a guarded
/// folder.
struct Fixture {
    _held: tempfile::TempDir,
    top: PathBuf,
    project: PathBuf,
    app: PathBuf,
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    std::fs::write(path, text).expect("written");
}

fn fixture() -> Fixture {
    let held = tempfile::tempdir().expect("a temporary folder");
    // Links resolved: macOS's `/var` is a link, and a mount's folder must be its real path.
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let project = top.join("project");
    let app = project.join("workspaces/smart-ide/repos/app");
    std::fs::create_dir_all(app.join(".git")).expect("a clone");
    write(&app.join("src/main.rs"), "fn main() {}\n");
    write(&app.join("Cargo.toml"), "[package]\n");
    write(
        &project.join("workspaces/smart-ide/secret.env"),
        "TOKEN=1\n",
    );
    write(&top.join("srv/src/main.rs"), "// the nested mount's\n");
    write(&top.join("srv/notes.md"), "notes\n");
    write(&top.join("etc/hosts"), "127.0.0.1 localhost\n");
    write(&top.join("guarded/mounted/a.md"), "a\n");
    Fixture {
        _held: held,
        top,
        project,
        app,
    }
}

impl Fixture {
    /// A Docker chat's pane in the project, mapped through `mounts`. A runner chat names no
    /// folder of this machine.
    fn docker(&self, mounts: Vec<Mount>) -> Pane {
        Pane {
            project: Some(self.project.clone()),
            session: Some(9),
            folder: None,
            runs: Runs::Mapped(Mounts::of(mounts)),
        }
    }

    /// `/work` mounting the repo `app`.
    fn work(&self) -> Pane {
        self.docker(vec![bind("/work", &self.app)])
    }

    fn links(&self) -> Resolver {
        Resolver::with(vec![Box::new(LocalFile::with(
            Some(self.top.join("home")),
            vec![self.top.join("guarded")],
        ))])
    }

    fn resolve(&self, text: &str, pane: &Pane) -> Resolved {
        self.links().resolve(&file(text), pane)
    }
}

fn bind(inside: &str, source: &Path) -> Mount {
    Mount::bind(inside, source).expect("a plain mount")
}

fn file(text: &str) -> Candidate {
    Candidate {
        kind: Kind::File,
        text: text.to_owned(),
    }
}

/// The local file `text` maps to, or a panic saying what it was.
fn mapped(here: &Fixture, text: &str, pane: &Pane) -> super::super::FileAt {
    match here.resolve(text, pane) {
        Resolved::Link(Target::File(at)) => *at,
        other => panic!("{text}: expected a file, got {other:?}"),
    }
}

#[test]
fn a_path_in_a_mount_maps_to_the_local_file_it_mounts() {
    let here = fixture();

    let at = mapped(&here, "/work/src/main.rs:3:2", &here.work());

    assert_eq!(at.path, here.app.join("src/main.rs"));
    assert_eq!((at.line, at.column), (Some(3), Some(2)));
    assert_eq!(
        at.read_from,
        ReadFrom::Branch {
            ws: "smart-ide".into(),
            repo: "app".into(),
            piece: None,
            path: "src/main.rs".into(),
        }
    );
    assert_eq!(
        at.shown(),
        format!("{}:3:2", here.app.join("src/main.rs").display())
    );
}

#[test]
fn a_file_address_in_a_mount_maps_too() {
    let here = fixture();

    let at = mapped(&here, "file:///work/src/main.rs", &here.work());

    assert_eq!(at.path, here.app.join("src/main.rs"));
}

#[test]
fn a_path_that_maps_through_no_mount_is_quiet_even_where_this_machine_has_it() {
    let here = fixture();
    let hosts = here.top.join("etc/hosts").display().to_string();

    for text in [hosts.as_str(), "/workx/src/main.rs", "/srv/notes.md", "/"] {
        assert_eq!(here.resolve(text, &here.work()), Resolved::Quiet, "{text}");
    }
}

#[test]
fn a_mounted_path_to_no_file_is_quiet() {
    let here = fixture();

    assert_eq!(
        here.resolve("/work/src/gone.rs", &here.work()),
        Resolved::Quiet
    );
    // The mount's folder itself is no file.
    assert_eq!(here.resolve("/work", &here.work()), Resolved::Quiet);
}

#[test]
fn a_nested_mount_maps_its_paths_by_the_longest_prefix() {
    let here = fixture();
    let srv = here.top.join("srv/src");
    // In either order: the deeper mount wins.
    for mounts in [
        vec![bind("/work", &here.app), bind("/work/src", &srv)],
        vec![bind("/work/src", &srv), bind("/work", &here.app)],
    ] {
        let pane = here.docker(mounts);

        assert_eq!(
            mapped(&here, "/work/src/main.rs", &pane).path,
            srv.join("main.rs")
        );
        // Outside the nested mount, the outer one maps.
        assert_eq!(
            mapped(&here, "/work/Cargo.toml", &pane).path,
            here.app.join("Cargo.toml")
        );
    }
}

#[test]
fn a_prefix_is_compared_a_whole_part_at_a_time() {
    let here = fixture();
    let pane = here.docker(vec![bind("/wo", &here.app)]);

    assert_eq!(here.resolve("/work/src/main.rs", &pane), Resolved::Quiet);
}

#[test]
fn two_mounts_at_one_place_map_nothing_there() {
    let here = fixture();
    let pane = here.docker(vec![
        bind("/work", &here.app),
        bind("/work", &here.top.join("srv")),
    ]);

    assert_eq!(here.resolve("/work/src/main.rs", &pane), Resolved::Quiet);
}

#[test]
fn a_file_mounted_on_its_own_maps() {
    let here = fixture();
    let notes = here.top.join("srv/notes.md");
    let pane = here.docker(vec![bind("/etc/notes.md", &notes)]);

    assert_eq!(mapped(&here, "/etc/notes.md:1", &pane).path, notes);
}

#[test]
fn dot_dot_never_crosses_a_mount_s_edge() {
    let here = fixture();
    let pane = here.work();

    // `/work/..` is the repos folder beside `app` on this machine, and `secret.env` two up.
    for text in [
        "/work/../secret.env",
        "/work/../../secret.env",
        "/work/src/../../secret.env",
        "/work/src/../main.rs",
        "/work/../app/src/main.rs",
    ] {
        assert_eq!(here.resolve(text, &pane), Resolved::Quiet, "{text}");
    }
}

#[cfg(unix)]
#[test]
fn a_link_in_a_mount_that_leads_out_of_it_is_quiet() {
    let here = fixture();
    let secret = here.project.join("workspaces/smart-ide/secret.env");
    std::os::unix::fs::symlink(&secret, here.app.join("absolute")).expect("a link");
    std::os::unix::fs::symlink("../../secret.env", here.app.join("relative")).expect("a link");
    std::os::unix::fs::symlink(here.top.join("etc"), here.app.join("etc")).expect("a link");
    let pane = here.work();

    for text in ["/work/absolute", "/work/relative", "/work/etc/hosts"] {
        assert_eq!(here.resolve(text, &pane), Resolved::Quiet, "{text}");
    }
}

#[cfg(unix)]
#[test]
fn a_link_that_stays_inside_its_mount_maps_to_where_it_leads() {
    let here = fixture();
    std::os::unix::fs::symlink("src/main.rs", here.app.join("entry")).expect("a link");

    let at = mapped(&here, "/work/entry", &here.work());

    assert_eq!(at.path, here.app.join("src/main.rs"));
}

#[cfg(unix)]
#[test]
fn a_mount_whose_folder_is_a_link_maps_nothing() {
    let here = fixture();
    let linked = here.top.join("linked");
    std::os::unix::fs::symlink(&here.app, &linked).expect("a link");
    // And one whose folder a link leads through.
    let through = here.top.join("through");
    std::os::unix::fs::symlink(here.app.join("src"), &through).expect("a link");

    for pane in [
        here.docker(vec![bind("/work", &linked)]),
        here.docker(vec![bind("/work/src", &through)]),
    ] {
        assert_eq!(here.resolve("/work/src/main.rs", &pane), Resolved::Quiet);
    }
}

#[test]
fn a_mount_of_a_guarded_folder_is_never_looked_in() {
    let here = fixture();
    let pane = here.docker(vec![bind("/data", &here.top.join("guarded/mounted"))]);

    assert_eq!(here.resolve("/data/a.md", &pane), Resolved::Quiet);
}

#[test]
fn a_runner_chat_s_home_and_relative_paths_are_quiet() {
    let here = fixture();
    write(&here.top.join("home/notes.md"), "notes\n");
    let pane = Pane {
        // Even with a folder, a relative path names the runner's folder, not this one.
        folder: Some(here.app.clone()),
        ..here.work()
    };

    for text in ["~/notes.md", "src/main.rs", "./src/main.rs"] {
        assert_eq!(here.resolve(text, &pane), Resolved::Quiet, "{text}");
    }
}

#[test]
fn a_mapped_file_outside_the_project_asks_first_as_a_local_one_does() {
    let here = fixture();
    let pane = here.docker(vec![bind("/srv", &here.top.join("srv"))]);
    let links = here.links();

    let opening = links.open(&file("/srv/notes.md"), &pane, &Asks::default(), None);

    let Opening::Ask { target, .. } = opening else {
        panic!("expected an ask, got {opening:?}");
    };
    let Target::File(at) = target else {
        panic!("expected a file");
    };
    assert_eq!(at.path, here.top.join("srv/notes.md"));
    assert_eq!(at.read_from, ReadFrom::Outside);
}

#[test]
fn no_mounts_map_nothing() {
    let here = fixture();
    let pane = here.docker(Vec::new());

    assert_eq!(here.resolve("/work/src/main.rs", &pane), Resolved::Quiet);
}

#[test]
fn a_local_chat_s_absolute_paths_are_this_machine_s_as_before() {
    let here = fixture();
    let pane = Pane {
        runs: Runs::Here,
        folder: Some(here.app.clone()),
        ..here.work()
    };

    assert_eq!(
        mapped(
            &here,
            &here.app.join("src/main.rs").display().to_string(),
            &pane
        )
        .path,
        here.app.join("src/main.rs")
    );
    assert_eq!(
        mapped(&here, "src/main.rs", &pane).path,
        here.app.join("src/main.rs")
    );
    assert_eq!(here.resolve("/work/src/main.rs", &pane), Resolved::Quiet);
}

#[test]
fn a_mount_is_absolute_and_plain_on_both_sides() {
    assert!(Mount::bind("work", "/srv").is_none());
    assert!(Mount::bind("/work", "srv").is_none());
    assert!(Mount::bind("/work/../etc", "/srv").is_none());
    assert!(Mount::bind("/work", "/srv/../etc").is_none());
    assert!(Mount::bind("/work", "/srv").is_some());
}

#[test]
fn a_docker_pool_runner_mounts_only_its_home_volume_so_its_paths_map_nothing() {
    let runner = crate::dockerpool::Runner::new(
        "first",
        crate::dockerpool::Image::pinned(&format!(
            "ghcr.io/purlis/purlis-runner@sha256:{}",
            "a".repeat(64)
        ))
        .expect("an image"),
        crate::dockerpool::User::new(10001, 10001).expect("a user"),
        "/home/purlis",
    )
    .expect("a runner");

    let mounts = Mounts::docker(&runner);

    assert_eq!(mounts, Mounts::none());
    assert_eq!(mounts.mapped(Path::new("/home/purlis/work/main.rs")), None);
}

#[test]
fn docker_s_mounts_map_its_binds_and_skip_its_volumes() {
    use crate::dockerpool::{Mounted, MountedFrom};
    let here = fixture();
    let mounted = [
        Mounted {
            from: MountedFrom::Volume("purlis-runner-first-home".into()),
            target: "/home/purlis".into(),
        },
        Mounted {
            from: MountedFrom::Bind(here.app.clone()),
            target: "/work".into(),
        },
    ];

    let mounts = Mounts::mounted(mounted.into());

    assert_eq!(mounts, Mounts::of(vec![bind("/work", &here.app)]));
}
