// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{
    Answer, Asks, Candidate, Hover, Kind, Opening, Pane, Resolved, Resolver, Runs, Target,
};
use super::{FileAt, LocalFile, Within, home_of};

/// A project on disk: workspace `smart-ide` with repo `app` (its clone and a branch folder
/// `feature`) and notes of its own, workspace `docs`, the project's own README, and a home
/// folder and a file outside the project.
struct Fixture {
    _held: tempfile::TempDir,
    project: PathBuf,
    home: PathBuf,
    outside: PathBuf,
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    std::fs::write(path, text).expect("written");
}

fn fixture() -> Fixture {
    let held = tempfile::tempdir().expect("a temporary folder");
    // Links resolved, as the core resolves every file's path (macOS's `/var` is a link).
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let project = top.join("project");
    let ws = project.join("workspaces/smart-ide");
    std::fs::create_dir_all(ws.join("repos/app/.git")).expect("a clone");
    write(&ws.join("repos/app/src/main.rs"), "fn main() {}\n");
    write(
        &ws.join(".worktrees/app/feature/src/lib.rs"),
        "pub fn one() {}\n",
    );
    write(&ws.join("workspace.md"), "# smart-ide\n");
    write(&project.join("workspaces/docs/guide.md"), "# docs\n");
    write(&project.join("README.md"), "# project\n");
    let home = top.join("home");
    write(&home.join("notes.md"), "notes\n");
    let outside = top.join("etc/hosts");
    write(&outside, "127.0.0.1 localhost\n");
    Fixture {
        _held: held,
        project,
        home,
        outside,
    }
}

impl Fixture {
    /// The pane of a chat working in `folder`, a path under the project.
    fn pane_in(&self, folder: &str) -> Pane {
        Pane {
            project: Some(self.project.clone()),
            session: Some(7),
            folder: Some(self.project.join(folder)),
            runs: Runs::Here,
        }
    }

    /// A chat in the branch folder `feature` of `app`, in `smart-ide`.
    fn chat(&self) -> Pane {
        self.pane_in("workspaces/smart-ide/.worktrees/app/feature")
    }

    fn links(&self) -> Resolver {
        Resolver::with(vec![Box::new(LocalFile::with_home(Some(
            self.home.clone(),
        )))])
    }
}

fn file(text: &str) -> Candidate {
    Candidate {
        kind: Kind::File,
        text: text.to_owned(),
    }
}

/// The file `text` resolves to, or a panic saying what it was.
fn resolved(links: &Resolver, text: &str, pane: &Pane) -> FileAt {
    match links.resolve(&file(text), pane) {
        Resolved::Link(Target::File(at)) => *at,
        other => panic!("{text}: expected a file, got {other:?}"),
    }
}

#[test]
fn a_relative_path_is_read_from_the_chat_s_working_folder_at_its_line_and_column() {
    let here = fixture();
    let links = here.links();
    let pane = here.chat();
    let lib = here
        .project
        .join("workspaces/smart-ide/.worktrees/app/feature/src/lib.rs");

    let at = resolved(&links, "src/lib.rs:12:7", &pane);
    assert_eq!(at.path, lib);
    assert_eq!((at.line, at.column), (Some(12), Some(7)));
    assert_eq!(at.shown(), format!("{}:12:7", lib.display()));
    assert_eq!(
        at.within,
        Within::Branch {
            ws: "smart-ide".into(),
            repo: "app".into(),
            piece: Some("feature".into()),
            path: "src/lib.rs".into(),
        }
    );
    assert!(at.home);

    let at = resolved(&links, "src/lib.rs:12", &pane);
    assert_eq!((at.line, at.column), (Some(12), None));
    let at = resolved(&links, "src/lib.rs", &pane);
    assert_eq!((at.line, at.column), (None, None));
    assert_eq!(at.shown(), lib.display().to_string());
    let at = resolved(&links, "./src/../src/lib.rs:3", &pane);
    assert_eq!((at.path.as_path(), at.line), (lib.as_path(), Some(3)));
}

#[test]
fn an_absolute_path_links_with_and_without_a_line() {
    let here = fixture();
    let main = here
        .project
        .join("workspaces/smart-ide/repos/app/src/main.rs");
    let text = main.display().to_string();

    let at = resolved(&here.links(), &format!("{text}:2"), &here.chat());
    assert_eq!((at.path.as_path(), at.line), (main.as_path(), Some(2)));
    assert_eq!(
        at.within,
        Within::Branch {
            ws: "smart-ide".into(),
            repo: "app".into(),
            piece: None,
            path: "src/main.rs".into(),
        }
    );
    let at = resolved(&here.links(), &text, &here.chat());
    assert_eq!(at.line, None);
}

#[test]
fn a_home_path_is_read_from_the_home_folder() {
    let here = fixture();

    let at = resolved(&here.links(), "~/notes.md:1", &here.chat());
    assert_eq!(at.path, here.home.join("notes.md"));
    assert_eq!(at.line, Some(1));
    assert_eq!(at.within, Within::Outside);
    // Another person's home is never looked up.
    assert_eq!(
        here.links().resolve(&file("~root/notes.md"), &here.chat()),
        Resolved::Quiet
    );
}

#[test]
fn a_path_to_no_file_is_no_link_and_says_nothing() {
    let here = fixture();
    let links = here.links();
    for text in [
        "src/old/gone.rs:10",
        "src",
        "/",
        "",
        "src/lib.rs\u{0}",
        "src/ lib.rs",
        "e.g.",
    ] {
        assert_eq!(
            links.resolve(&file(text), &here.chat()),
            Resolved::Quiet,
            "{text:?}"
        );
        assert_eq!(
            links.hover(&file(text), &here.chat()),
            Hover::Quiet,
            "{text:?}"
        );
        assert!(
            matches!(
                links.open(&file(text), &here.chat(), &Asks::default(), None),
                Answer::Refused(_)
            ),
            "{text:?}"
        );
    }
}

#[test]
fn a_relative_path_with_no_working_folder_is_no_link() {
    let here = fixture();
    let pane = Pane {
        folder: None,
        ..here.chat()
    };

    assert_eq!(
        here.links().resolve(&file("src/lib.rs"), &pane),
        Resolved::Quiet
    );
}

#[test]
fn a_chat_on_a_runner_names_its_own_machine_s_files_which_are_no_link_here() {
    let here = fixture();
    let pane = Pane {
        runs: Runs::Elsewhere,
        ..here.chat()
    };
    let text = here.outside.display().to_string();

    assert_eq!(here.links().resolve(&file(&text), &pane), Resolved::Quiet);
    assert_eq!(
        here.links().resolve(&file("src/lib.rs"), &pane),
        Resolved::Quiet
    );
}

#[test]
fn where_each_file_is_read_from() {
    let here = fixture();
    let links = here.links();
    let pane = here.chat();
    let at = |rel: &str| resolved(&links, &here.project.join(rel).display().to_string(), &pane);

    assert_eq!(
        at("workspaces/smart-ide/workspace.md").within,
        Within::Workspace {
            ws: "smart-ide".into(),
            path: "workspace.md".into()
        }
    );
    assert_eq!(
        at("workspaces/docs/guide.md").within,
        Within::Workspace {
            ws: "docs".into(),
            path: "guide.md".into()
        }
    );
    assert_eq!(
        at("README.md").within,
        Within::Project {
            path: "README.md".into()
        }
    );
    let outside = resolved(&links, &here.outside.display().to_string(), &pane);
    assert_eq!(outside.within, Within::Outside);
}

#[test]
fn a_file_in_the_chat_s_workspace_opens_and_one_outside_it_asks_first() {
    let here = fixture();
    let links = here.links();
    let pane = here.chat();
    let asks = Asks::default();

    assert!(matches!(
        links.open(&file("src/lib.rs:4"), &pane, &asks, None),
        Answer::Open(Target::File(at)) if at.line == Some(4)
    ));
    assert!(matches!(
        links.open(&file("../../../workspace.md"), &pane, &asks, None),
        Answer::Open(_)
    ));

    for outside in [
        here.outside.display().to_string(),
        here.project.join("README.md").display().to_string(),
        here.project
            .join("workspaces/docs/guide.md")
            .display()
            .to_string(),
        "~/notes.md".to_owned(),
    ] {
        match links.open(&file(&outside), &pane, &asks, None) {
            Answer::Ask {
                reason,
                shown,
                token,
                ..
            } => {
                assert_eq!(reason, "outside smart-ide", "{outside}");
                assert_eq!(shown, outside);
                assert!(!token.is_empty());
            }
            other => panic!("{outside}: expected an ask, got {other:?}"),
        }
        assert!(matches!(
            links.hover(&file(&outside), &pane),
            Hover::Link {
                opening: Opening::Ask { .. },
                ..
            }
        ));
    }
}

#[test]
fn a_chat_at_the_project_s_root_counts_the_project_as_its_own() {
    let here = fixture();
    let pane = here.pane_in("");

    assert_eq!(home_of(&pane).expect("a home").name, "the project");
    assert!(matches!(
        here.links()
            .open(&file("README.md:1"), &pane, &Asks::default(), None),
        Answer::Open(_)
    ));
    assert!(matches!(
        here.links().open(
            &file(&here.outside.display().to_string()),
            &pane,
            &Asks::default(),
            None
        ),
        Answer::Ask { reason, .. } if reason == "outside the project"
    ));
}

#[test]
fn accepting_the_ask_opens_the_file_once_and_keeps_one_outside_the_project_as_opened() {
    let here = fixture();
    let links = here.links();
    let pane = here.chat();
    let asks = Asks::default();
    let hosts = format!("{}:3", here.outside.display());

    let Answer::Ask { token, .. } = links.open(&file(&hosts), &pane, &asks, None) else {
        panic!("expected an ask");
    };
    assert!(!asks.opened(&here.outside));
    assert!(matches!(
        links.open(&file(&hosts), &pane, &asks, Some(&token)),
        Answer::Open(Target::File(at)) if at.line == Some(3)
    ));
    assert!(asks.opened(&here.outside));
    // Declined, nothing is sent: the next ⌘-click asks again.
    assert!(matches!(
        links.open(&file(&hosts), &pane, &asks, None),
        Answer::Ask { .. }
    ));
}

#[test]
fn a_file_address_on_this_machine_is_a_file_link_and_one_naming_another_host_is_refused() {
    let here = fixture();
    let links = here.links();
    let pane = here.chat();
    let hosts = here.outside.display().to_string();

    for text in [
        format!("file://{hosts}"),
        format!("file://localhost{hosts}"),
        format!("file://{hosts}:3"),
    ] {
        let at = resolved(&links, &text, &pane);
        assert_eq!(at.path, here.outside, "{text}");
        assert_eq!(at.within, Within::Outside);
    }
    assert_eq!(
        resolved(&links, &format!("file://{hosts}:3"), &pane).line,
        Some(3)
    );

    let spaced = here.home.join("my notes.md");
    write(&spaced, "x\n");
    let encoded = format!("file://{}", spaced.display()).replace(' ', "%20");
    assert_eq!(resolved(&links, &encoded, &pane).path, spaced);

    let why = match links.resolve(&file("file://build-box/etc/hosts"), &pane) {
        Resolved::NotALink(why) => why,
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert!(why.contains("only this machine's files"), "{why}");
    assert!(why.contains("build-box"), "{why}");
    assert!(matches!(
        links.open(&file("file://build-box/etc/hosts"), &pane, &Asks::default(), None),
        Answer::Refused(why) if why.contains("build-box")
    ));
}

#[test]
fn a_link_on_the_way_is_resolved_and_judged_where_it_leads() {
    let here = fixture();
    let link = here
        .project
        .join("workspaces/smart-ide/.worktrees/app/feature/hosts");
    std::os::unix::fs::symlink(&here.outside, &link).expect("a link");

    let at = resolved(&here.links(), "hosts", &here.chat());
    assert_eq!(at.path, here.outside);
    assert_eq!(at.within, Within::Outside);
    assert!(!at.home);
}

#[test]
fn a_folder_or_something_that_is_not_a_regular_file_is_no_link() {
    let here = fixture();
    assert_eq!(
        here.links().resolve(&file("/dev/null"), &here.chat()),
        Resolved::Quiet
    );
    assert_eq!(
        here.links().resolve(&file("src:3"), &here.chat()),
        Resolved::Quiet
    );
}

#[test]
fn the_window_s_resolver_links_files() {
    let here = fixture();
    let text = here.outside.display().to_string();

    assert!(matches!(
        Resolver::default().resolve(&file(&text), &here.chat()),
        Resolved::Link(Target::File(_))
    ));
}
