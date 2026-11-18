// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{Asks, Candidate, Hover, Kind, Opening, Pane, Resolved, Resolver, Runs, Target};
use super::IssueRef;

/// A project with a clone `app` in workspace `smart-ide`, and a folder outside it.
struct Fixture {
    _held: tempfile::TempDir,
    project: PathBuf,
    outside: PathBuf,
}

fn fixture() -> Fixture {
    let held = tempfile::tempdir().expect("a temporary folder");
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let project = top.join("project");
    std::fs::create_dir_all(project.join("workspaces/smart-ide/repos/app")).expect("a clone");
    let outside = top.join("elsewhere");
    std::fs::create_dir_all(&outside).expect("a folder");
    Fixture {
        _held: held,
        project,
        outside,
    }
}

impl Fixture {
    fn app(&self) -> PathBuf {
        self.project.join("workspaces/smart-ide/repos/app")
    }

    fn pane_in(&self, folder: &Path) -> Pane {
        Pane {
            project: Some(self.project.clone()),
            session: Some(7),
            folder: Some(folder.to_path_buf()),
            runs: Runs::Here,
            now_in: None,
        }
    }
}

/// A resolver whose every folder has `remotes`.
fn links(remotes: &[(&str, &str)]) -> Resolver {
    let remotes: Vec<(String, String)> = remotes
        .iter()
        .map(|(name, url)| ((*name).to_owned(), (*url).to_owned()))
        .collect();
    Resolver::with(vec![Box::new(IssueRef::with(move |_, _| remotes.clone()))])
}

fn issue(text: &str) -> Candidate {
    Candidate {
        kind: Kind::Issue,
        text: text.to_owned(),
        target: None,
    }
}

fn page(text: &str) -> Resolved {
    Resolved::Link(Target::Url(url::Url::parse(text).expect("an address")))
}

const ORIGIN: (&str, &str) = ("origin", "git@github.com:purlis/purlis.git");

#[test]
fn a_bare_number_is_the_folder_s_origin_s_issue_or_pull_request() {
    let here = fixture();
    let pane = here.pane_in(&here.app());
    for origin in [
        "git@github.com:purlis/purlis.git",
        "https://github.com/purlis/purlis.git",
        "https://github.com/purlis/purlis",
        "ssh://git@github.com/purlis/purlis.git",
    ] {
        assert_eq!(
            links(&[("origin", origin)]).resolve(&issue("#6223"), &pane),
            page("https://github.com/purlis/purlis/issues/6223"),
            "{origin}"
        );
    }
}

#[test]
fn an_owner_s_repo_and_number_is_that_repo_s_on_the_same_forge() {
    let here = fixture();
    let pane = here.pane_in(&here.app());
    assert_eq!(
        links(&[ORIGIN]).resolve(&issue("purlis/purlis-plane#12"), &pane),
        page("https://github.com/purlis/purlis-plane/issues/12")
    );
}

#[test]
fn a_gitlab_number_is_its_issue() {
    let here = fixture();
    let pane = here.pane_in(&here.app());
    let links = links(&[("origin", "git@gitlab.com:team/sub/app.git")]);
    assert_eq!(
        links.resolve(&issue("#5"), &pane),
        page("https://gitlab.com/team/sub/app/-/issues/5")
    );
    assert_eq!(
        links.resolve(&issue("team/other#7"), &pane),
        page("https://gitlab.com/team/other/-/issues/7")
    );
}

#[test]
fn it_opens_as_a_web_address_does_without_asking() {
    let here = fixture();
    let pane = here.pane_in(&here.app());
    let links = links(&[ORIGIN]);
    let target =
        Target::Url(url::Url::parse("https://github.com/purlis/purlis/issues/6223").unwrap());
    assert_eq!(
        links.open(&issue("#6223"), &pane, &Asks::default(), None),
        Opening::Open(target.clone())
    );
    assert_eq!(
        links.hover(&issue("#6223"), &pane),
        Hover::Link {
            opening: Box::new(Opening::Open(target.clone())),
            target,
        }
    );
}

#[test]
fn with_no_single_forge_remote_a_number_is_no_link_and_says_nothing() {
    let here = fixture();
    let pane = here.pane_in(&here.app());
    let fork = ("fork", "https://gitlab.com/me/app.git");
    for remotes in [
        vec![],
        vec![("origin", "https://git.example.org/purlis/purlis.git")],
        vec![("origin", "/srv/git/purlis.git")],
        vec![("upstream", "git@github.com:purlis/purlis.git"), fork],
    ] {
        assert_eq!(
            links(&remotes).resolve(&issue("#6223"), &pane),
            Resolved::Quiet,
            "{remotes:?}"
        );
    }
    // Origin elsewhere, and one remote on a forge: that one's.
    assert_eq!(
        links(&[("origin", "https://git.example.org/x/y.git"), fork]).resolve(&issue("#3"), &pane),
        page("https://gitlab.com/me/app/-/issues/3")
    );
    // Origin on a forge, among others: origin's.
    assert_eq!(
        links(&[fork, ORIGIN]).resolve(&issue("#3"), &pane),
        page("https://github.com/purlis/purlis/issues/3")
    );
}

#[test]
fn a_folder_outside_the_project_has_no_remote_purlis_reads() {
    // #6367: a chat that said (OSC 7) it moved outside its project.
    let here = fixture();
    let pane = Pane {
        now_in: Some(here.outside.clone()),
        ..here.pane_in(&here.app())
    };
    assert_eq!(
        links(&[ORIGIN]).resolve(&issue("#6223"), &pane),
        Resolved::Quiet
    );
    let no_project = Pane {
        project: None,
        ..here.pane_in(&here.app())
    };
    assert_eq!(
        links(&[ORIGIN]).resolve(&issue("#6223"), &no_project),
        Resolved::Quiet
    );
}

#[test]
fn the_remotes_are_those_of_the_folder_the_chat_is_in_now() {
    let here = fixture();
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
    let seen = asked.clone();
    let links = Resolver::with(vec![Box::new(IssueRef::with(move |folder, _| {
        seen.lock().unwrap().push(folder.to_path_buf());
        vec![(
            "origin".to_owned(),
            "git@github.com:purlis/purlis.git".to_owned(),
        )]
    }))]);
    let ws = here.project.join("workspaces/smart-ide");
    let pane = Pane {
        now_in: Some(here.app()),
        ..here.pane_in(&ws)
    };
    links.resolve(&issue("#1"), &pane);
    assert_eq!(*asked.lock().unwrap(), vec![here.app()]);
}

#[test]
fn text_that_is_no_number_or_no_repo_is_no_link() {
    let here = fixture();
    let pane = here.pane_in(&here.app());
    let links = links(&[ORIGIN]);
    for text in [
        "#",
        "#0",
        "#012",
        "#1234567890",
        "#12a",
        "purlis#12",
        "/purlis#12",
        "purlis/#12",
        "-x/y#12",
        "x/y.git#12",
        "x/y z#12",
        "x/y#12#13",
        "https://github.com/purlis/purlis#12",
    ] {
        assert_eq!(
            links.resolve(&issue(text), &pane),
            Resolved::Quiet,
            "{text}"
        );
    }
}

#[test]
fn a_repo_s_remotes_are_read_from_its_own_config() {
    // gitoxide, isolated: a real repository's origin, and a folder in it.
    let here = fixture();
    let app = here.app();
    // A repository as git lays one out, made by hand: no git runs in a test.
    for folder in [".git/objects", ".git/refs/heads"] {
        std::fs::create_dir_all(app.join(folder)).expect("a folder");
    }
    std::fs::write(app.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::write(
        app.join(".git/config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\
         [remote \"origin\"]\n\turl = git@github.com:purlis/purlis.git\n\
         \tfetch = +refs/heads/*:refs/remotes/origin/*\n",
    )
    .expect("config");
    std::fs::create_dir_all(app.join("src")).expect("a folder");

    let links = Resolver::with(vec![Box::new(IssueRef::here())]);
    assert_eq!(
        links.resolve(&issue("#9"), &here.pane_in(&app.join("src"))),
        page("https://github.com/purlis/purlis/issues/9")
    );
}

#[test]
fn a_repository_above_the_project_is_never_read() {
    // The project is in no repository of its own, and one holds it: its origin is not the chat's.
    let here = fixture();
    let above = here.project.parent().expect("a parent").to_path_buf();
    for folder in [".git/objects", ".git/refs/heads"] {
        std::fs::create_dir_all(above.join(folder)).expect("a folder");
    }
    std::fs::write(above.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::write(
        above.join(".git/config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\
         [remote \"origin\"]\n\turl = git@github.com:someone/else.git\n",
    )
    .expect("config");

    let links = Resolver::with(vec![Box::new(IssueRef::here())]);
    assert_eq!(
        links.resolve(&issue("#9"), &here.pane_in(&here.app())),
        Resolved::Quiet
    );
}

/// A repository's git folder at `git`, as git lays one out, whose origin is `origin`.
fn git_folder(git: &Path, origin: &str) {
    for folder in ["objects", "refs/heads"] {
        std::fs::create_dir_all(git.join(folder)).expect("a folder");
    }
    std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::write(
        git.join("config"),
        format!(
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\
             [remote \"origin\"]\n\turl = {origin}\n"
        ),
    )
    .expect("config");
}

#[test]
fn a_git_file_pointing_outside_the_project_is_never_followed() {
    // A `.git` file an agent wrote can name any folder: purlis reads no git folder outside the
    // project, so a guarded one is never looked in.
    let here = fixture();
    let elsewhere = here.outside.join("repo.git");
    git_folder(&elsewhere, "git@github.com:someone/else.git");
    std::fs::write(
        here.app().join(".git"),
        format!("gitdir: {}\n", elsewhere.display()),
    )
    .expect("a git file");

    let links = Resolver::with(vec![Box::new(IssueRef::here())]);
    assert_eq!(
        links.resolve(&issue("#9"), &here.pane_in(&here.app())),
        Resolved::Quiet
    );
}

#[test]
fn a_branch_folder_reads_its_clone_s_remotes() {
    // A branch folder's `.git` file names its clone's `.git/worktrees/<name>`, whose
    // `commondir` names the clone's git folder: both in the project.
    let here = fixture();
    let clone_git = here.app().join(".git");
    git_folder(&clone_git, "git@github.com:purlis/purlis.git");
    let named = clone_git.join("worktrees/feature");
    std::fs::create_dir_all(&named).expect("a folder");
    std::fs::write(named.join("HEAD"), "ref: refs/heads/feature\n").expect("HEAD");
    std::fs::write(named.join("commondir"), "../..\n").expect("commondir");
    let branch = here
        .project
        .join("workspaces/smart-ide/.worktrees/app/feature");
    std::fs::create_dir_all(&branch).expect("a branch folder");
    std::fs::write(
        named.join("gitdir"),
        format!("{}\n", branch.join(".git").display()),
    )
    .expect("gitdir");
    std::fs::write(
        branch.join(".git"),
        format!("gitdir: {}\n", named.display()),
    )
    .expect("a git file");

    let links = Resolver::with(vec![Box::new(IssueRef::here())]);
    assert_eq!(
        links.resolve(&issue("#9"), &here.pane_in(&branch)),
        page("https://github.com/purlis/purlis/issues/9")
    );
}
