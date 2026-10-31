// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter save`, against local repositories and a local bare remote.
//!
//! **Nothing here reaches a network.** The one remote these tests push to is a bare repository
//! in the test's own temporary directory, reached through an `url.<file>.insteadOf` rewrite in
//! the pushing repo's LOCAL config — which is read for a `push` (unlike a `clone`, whose config
//! search skips the local file). So the URL charter builds, the containment of it, the
//! credential rule and the protected-branch path are all exercised against the real `git push`,
//! and a forge is never asked for anything.

use std::path::{Path, PathBuf};

use super::*;
use crate::repocmd::Say;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    /// A plane that is a git repository with one commit, an identity of its own, and no remote.
    fn plane() -> Fixture {
        let dir = tempfile::tempdir().expect("a temp dir");
        // Under the temp directory rather than AT it, so `root.parent()` is this test's own
        // scratch and a worktree or a bare remote beside the plane cannot land in /tmp.
        let root = dir
            .path()
            .canonicalize()
            .expect("a real path")
            .join("plane");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\nname = \"fixture\"\n").unwrap();
        // What `charter init` writes: the plane's own machine-local state is not committed.
        std::fs::write(root.join(".gitignore"), ".charter/\n").unwrap();
        run(&root, &["init", "-q", "-b", "main", "."]);
        // In the repo's own config, never the machine's: CI runners have no git identity. The
        // repo is made through `testgit`, so a developer's global `commit.gpgsign = true` is
        // not asked for here (charter-app#191); the test that proves `save` never runs a
        // signer turns signing on in the repo itself.
        run(&root, &["config", "user.name", "Fixture"]);
        run(&root, &["config", "user.email", "fixture@example.invalid"]);
        std::fs::create_dir_all(root.join("personas/steward/memory")).unwrap();
        std::fs::write(root.join("README.md"), "one\n").unwrap();
        run(&root, &["add", "-A"]);
        run(&root, &["commit", "-q", "-m", "one"]);
        Fixture { _dir: dir, root }
    }

    /// A bare repository standing in for the forge.
    ///
    /// **`origin` is the SSH form, and the rewrite is keyed on the HTTPS one.** That ordering
    /// is what makes a local stand-in possible at all: `git remote get-url` APPLIES
    /// `url.<base>.insteadOf`, so a rule keyed on the URL charter reads would hand it a
    /// `file://` origin, charter would answer "that is on no forge I know", and nothing would
    /// be pushed — the test would pass by never testing. Keyed on the HTTPS prefix instead,
    /// `get-url` returns the SSH URL untouched, charter rewrites it to HTTPS itself (golden
    /// rule 0's own rule), and git maps THAT onto the bare repository. Measured both ways.
    fn with_a_remote(&self) -> PathBuf {
        self.with_a_remote_on("github.com")
    }

    /// [`Fixture::with_a_remote`], with `origin` on `host`.
    fn with_a_remote_on(&self, host: &str) -> PathBuf {
        let bare = self.root.parent().unwrap().join("forge/acme/plane.git");
        std::fs::create_dir_all(&bare).unwrap();
        run(&bare, &["init", "-q", "--bare", "-b", "main", "."]);
        run(
            &self.root,
            &[
                "remote",
                "add",
                "origin",
                &format!("git@{host}:acme/plane.git"),
            ],
        );
        let base = format!("file://{}/", bare.parent().unwrap().display());
        run(
            &self.root,
            &[
                "config",
                &format!("url.{base}.insteadOf"),
                &format!("https://{host}/acme/"),
            ],
        );
        bare
    }

    /// [`Fixture::with_a_remote`], with this plane's commit on it and then somebody else's
    /// commit on top — so the next save's push is refused and it has to rebase.
    fn with_a_remote_that_moved(&self) -> PathBuf {
        let bare = self.with_a_remote();
        run(
            &self.root,
            &[
                "push",
                "-q",
                &bare.display().to_string(),
                "HEAD:refs/heads/main",
            ],
        );
        let theirs = self.root.parent().unwrap().join("theirs");
        run(
            self.root.parent().unwrap(),
            &[
                "clone",
                "-q",
                &bare.display().to_string(),
                &theirs.display().to_string(),
            ],
        );
        // The clone's own identity, as the plane's: CI runners have none of their own.
        run(&theirs, &["config", "user.name", "Other"]);
        run(&theirs, &["config", "user.email", "other@example.invalid"]);
        std::fs::write(theirs.join("theirs.md"), "theirs").unwrap();
        run(&theirs, &["add", "-A"]);
        run(&theirs, &["commit", "-q", "-m", "theirs"]);
        run(&theirs, &["push", "-q", "origin", "main"]);
        bare
    }

    fn save(&self, request: Request) -> (u8, String) {
        let mut said = String::new();
        let mut say = |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        };
        let code = super::save(&request, &mut say);
        (code, said)
    }

    fn just_save(&self) -> (u8, String) {
        self.save(Request {
            root: &self.root,
            message: Some("a save"),
            sign: false,
            no_push: false,
            cwd: &self.root,
            provenance: None,
        })
    }

    fn head_subject(&self) -> String {
        run(&self.root, &["log", "-1", "--format=%s"]).trim().into()
    }

    fn commits(&self) -> usize {
        run(&self.root, &["rev-list", "--count", "HEAD"])
            .trim()
            .parse()
            .unwrap_or(0)
    }
}

/// git, for a test's own setup — not through the module under test.
fn run(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out
}

fn ask(dir: &Path, args: &[&str]) -> String {
    crate::testgit::run(dir, args).out
}

// --------------------------------------------------------------------------------------- //
// what `save` stages                                                                        //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_bare_separator_bounds_nothing_and_a_named_path_bounds_everything() {
    // Measured against git: `add` and `add --` stage nothing, `add -A`, `add -A --` and
    // `add -u` stage the whole tree, and `add -- . u.txt` stages it too.
    for wide in [
        &["add", "-A"][..],
        &["add", "--all"],
        &["add", "-u"],
        &["add", "--update"],
        &["add", "--no-ignore-removal"],
        &["add", "-A", "--"],
        &["add", "."],
        &["add", "--", "."],
        &["add", "--", ":/"],
        &["add", "--", "workspaces/x", "."],
    ] {
        assert!(stages_the_whole_tree(wide), "{wide:?}");
    }
    for scoped in [
        &["add"][..],
        &["add", "--"],
        &["add", "--", "personas/steward/memory/x.md"],
        &["add", "-A", "--", "workspaces/x", ".gitignore"],
        // A file really can be called `-A`, and past `--` git stages that file.
        &["add", "--", "-A"],
    ] {
        assert!(!stages_the_whole_tree(scoped), "{scoped:?}");
    }
}

#[test]
fn what_is_about_to_be_committed_is_broken_down_by_directory_before_it_is() {
    let fixture = Fixture::plane();
    for (path, text) in [
        ("personas/steward/memory/a.md", "a"),
        ("personas/steward/memory/b.md", "b"),
        ("personas/_shared/memory/c.md", "c"),
        // The `uv.lock` of the 2026-09-12 incident: a project file at the plane root that a
        // count of "3 file(s)" would never have surfaced.
        ("uv.lock", "lock"),
    ] {
        let file = fixture.root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: Some("a save"),
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("4 files across 3 directories"), "{said}");
    assert!(said.contains("2  personas/steward/memory"), "{said}");
    assert!(said.contains("1  personas/_shared/memory"), "{said}");
    assert!(said.contains("1  (the plane root)"), "{said}");
    // And it is said BEFORE the commit line, which is the whole point: a breakdown printed
    // after the fact is a receipt, not a check.
    let breakdown = said.find("(the plane root)").expect("the breakdown");
    let committed = said.find("Committed ").expect("the commit line");
    assert!(breakdown < committed, "{said}");
}

#[test]
fn a_save_never_commits_a_levels_local_skills_even_where_nothing_ignores_them() {
    // ADR 0099 §1, #2767: `skills.local/` is this machine's own. This plane's `.gitignore` has
    // no line for it, as an older plane's does not, and the save still leaves it out.
    let fixture = Fixture::plane();
    let stages = save_stages();
    assert!(
        stages_the_whole_tree(&stages.iter().map(String::as_str).collect::<Vec<_>>()),
        "the save is still a whole-tree stage"
    );
    for path in [
        "skills/shared/SKILL.md",
        "skills.local/mine/SKILL.md",
        "workspaces/alpha/skills.local/notes/SKILL.md",
        // A persona's local skills too (SK-32, #3124).
        "personas/reviewer/skills.local/scratch/SKILL.md",
        "personas/reviewer/skills/shared/SKILL.md",
    ] {
        let file = fixture.root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "---\nname: x\ndescription: y\n---\n").unwrap();
    }
    assert!(
        !changed_paths(&fixture.root)
            .iter()
            .any(|path| path.contains("skills.local")),
        "the standing does not count them as unsaved either"
    );

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: Some("a save"),
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    let tree = run(&fixture.root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(tree.contains("skills/shared/SKILL.md"), "{tree}");
    assert!(
        tree.contains("personas/reviewer/skills/shared/SKILL.md"),
        "{tree}"
    );
    assert!(!tree.contains("skills.local"), "{tree}");
}

#[test]
fn a_save_of_named_paths_never_names_a_personas_local_skill() {
    // SK-32 (#3124): a skill change that names a persona's local folder leaves it out.
    assert!(kept_out("personas/reviewer/skills.local/scratch"));
    assert!(kept_out("personas/reviewer/skills.local"));
    assert!(!kept_out("personas/reviewer/skills/scratch"));
    assert!(kept_out("workspaces/ops/skills.local/x"));
    assert!(kept_out("skills.local.lock"));
    assert!(
        never_staged()
            .iter()
            .any(|spec| spec == ":(exclude,glob)personas/*/skills.local/**")
    );
}

/// SK-9 (#2772): `git add -A` with an exclude pathspec leaves an untracked file named beside it
/// unstaged, so a save of named paths passes the excludes only where a named folder could hold
/// what they keep out. A new file named alone, a new file named beside a changed one, and a new
/// file named beside a new folder, are each committed whole; a path inside what is never staged
/// is never named.
#[test]
fn a_save_of_named_paths_commits_each_new_file_it_names() {
    let fixture = Fixture::plane();
    let root = &fixture.root;
    std::fs::write(
        root.join("charter.toml"),
        "[plane]\nname = \"fixture\"\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("skills/deploy")).unwrap();
    std::fs::write(root.join("skills/deploy/SKILL.md"), "one\n").unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "base"]);

    std::fs::write(root.join("skills/deploy/SKILL.md"), "two\n").unwrap();
    std::fs::write(root.join("skills/deploy/check.sh"), "check\n").unwrap();
    std::fs::write(root.join("skills/deploy/notes.md"), "mine\n").unwrap();
    std::fs::create_dir_all(root.join("skills/triage/scripts")).unwrap();
    std::fs::write(root.join("skills/triage/SKILL.md"), "new\n").unwrap();
    std::fs::write(root.join("skills/triage/scripts/sort.sh"), "sort\n").unwrap();
    std::fs::create_dir_all(root.join("skills.local/mine")).unwrap();
    std::fs::write(root.join("skills.local/mine/SKILL.md"), "mine\n").unwrap();
    let paths: Vec<String> = [
        "skills/deploy/SKILL.md",
        "skills/deploy/check.sh",
        "skills/triage",
        "skills.local/mine/SKILL.md",
    ]
    .iter()
    .map(|one| (*one).to_owned())
    .collect();
    let mut said = String::new();
    let code = save_paths(
        &Request {
            root,
            message: Some("named"),
            sign: false,
            no_push: true,
            cwd: root,
            provenance: None,
        },
        Trigger::Skill(crate::skills::verbs::By::Window),
        &paths,
        &mut |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        },
    );
    assert_eq!(code, 0, "{said}");
    let mut committed: Vec<String> = run(
        root,
        &["show", "--no-renames", "--name-only", "--format=", "HEAD"],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect();
    committed.sort();
    assert_eq!(
        committed,
        [
            "skills/deploy/SKILL.md",
            "skills/deploy/check.sh",
            "skills/triage/SKILL.md",
            "skills/triage/scripts/sort.sh",
        ],
        "{said}"
    );
    assert!(ask(root, &["status", "--porcelain"]).contains("skills/deploy/notes.md"));
}

// --------------------------------------------------------------------------------------- //
// the two trees `save` refuses to commit                                                    //
// --------------------------------------------------------------------------------------- //

#[test]
fn an_unbounded_stage_from_a_linked_worktree_of_the_plane_is_refused() {
    let fixture = Fixture::plane();
    let worktree = fixture.root.parent().unwrap().join("wt");
    run(
        &fixture.root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            &worktree.display().to_string(),
        ],
    );
    std::fs::write(fixture.root.join("mid-edit.md"), "the operator's").unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: Some("the agent's"),
        sign: false,
        no_push: true,
        cwd: &worktree,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("a linked worktree of it"), "{said}");
    assert!(said.contains("PURLIS_ROOT="), "it names the override");
    // Nothing was staged: a refusal that has already run `git add -A` has done the damage and
    // merely declined to name it.
    assert_eq!(fixture.head_subject(), "one");
    assert!(
        ask(&fixture.root, &["diff", "--cached", "--name-only"])
            .trim()
            .is_empty(),
        "the refusal came before the add"
    );
}

#[test]
fn a_scoped_stage_from_a_linked_worktree_is_still_allowed() {
    // Reactive memory, the dispatch tally and the version pin all name plane-state files from
    // a worktree. Refusing those would stop every agent working in a worktree from recording
    // memory — a larger harm than the one the refusal above prevents.
    let fixture = Fixture::plane();
    let worktree = fixture.root.parent().unwrap().join("wt");
    run(
        &fixture.root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            &worktree.display().to_string(),
        ],
    );
    std::fs::write(fixture.root.join("personas/steward/memory/m.md"), "m").unwrap();

    let mut said = String::new();
    let mut say = |line: Say| said.push_str(&format!("{line}\n"));
    let code = super::commit_push(
        &Request {
            root: &fixture.root,
            message: Some("a memory"),
            sign: false,
            no_push: true,
            cwd: &worktree,
            provenance: None,
        },
        &crate::planesave::Settings::read(&fixture.root).plane,
        super::Staging::Add(&["add", "--", "personas/steward/memory/m.md"]),
        &crate::forge::Caller::command(),
        &mut Attempt::default(),
        &mut say,
    );

    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.head_subject(), "a memory");
}

#[test]
fn an_unbounded_stage_from_a_plane_nested_in_the_workspaces_is_refused() {
    let outer = Fixture::plane();
    let inner = outer.root.join("workspaces/ide/charter");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join("charter.toml"), "[plane]\nname = \"inner\"\n").unwrap();
    run(&inner, &["init", "-q", "-b", "main", "."]);
    std::fs::write(outer.root.join("mid-edit.md"), "the operator's").unwrap();

    let (code, said) = outer.save(Request {
        root: &outer.root,
        message: Some("the agent's"),
        sign: false,
        no_push: true,
        cwd: &inner,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("a control plane of its own"), "{said}");
    assert_eq!(outer.head_subject(), "one");
}

#[test]
fn standing_in_the_plane_being_committed_is_never_one_of_those_two() {
    // The `$CHARTER_ROOT=<the clone>` hatch the refusals themselves print as the remedy: with
    // the plane being committed set to the one the caller stands in, both detectors must
    // answer `None`, or the advice would be a lie.
    let outer = Fixture::plane();
    let inner = outer.root.join("workspaces/ide/charter");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join("charter.toml"), "[plane]\nname = \"inner\"\n").unwrap();

    assert_eq!(nested_plane_in(&inner, &inner), None);
    assert_eq!(tree_of(&outer.root, &outer.root), None);
    assert_eq!(nested_plane_in(&outer.root, &outer.root), None);
}

// --------------------------------------------------------------------------------------- //
// the third answer                                                                          //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_held_index_lock_is_a_refusal_and_the_word_clean_never_appears() {
    let fixture = Fixture::plane();
    std::fs::write(fixture.root.join("work.md"), "thirteen modified files").unwrap();
    std::fs::write(fixture.root.join(".git/index.lock"), "").unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("Refusing to save"), "{said}");
    assert!(said.contains("index.lock"), "it names the lock: {said}");
    assert!(said.contains("who holds it:"), "ps before rm: {said}");
    // The whole defect was a sentence containing this word.
    assert!(!said.to_lowercase().contains("clean"), "{said}");
    assert_eq!(fixture.head_subject(), "one", "nothing was committed");
}

#[test]
fn a_clean_tree_says_there_is_nothing_to_save_and_commits_nothing() {
    let fixture = Fixture::plane();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Nothing to save"), "{said}");
    assert_eq!(fixture.commits(), 1);
}

/// FR-24, #826: a commit changes the committed tree, so a project this purlis may not write is
/// not saved by anyone, the auto-save a chat's end pokes included.
#[test]
fn a_project_this_purlis_may_not_write_is_not_saved() {
    let fixture = Fixture::plane();
    std::fs::write(
        fixture.root.join("charter.toml"),
        "schema = 2\nrequires = [{ feature = \"from-the-future\", since = \"9.0.0\" }]\n",
    )
    .unwrap();
    std::fs::write(fixture.root.join("notes.md"), "a chat's edit\n").unwrap();
    let status = ask(&fixture.root, &["status", "--porcelain"]);

    for trigger in [Trigger::SessionEnd, Trigger::Quiet, Trigger::Manual] {
        let mut said = String::new();
        let code = save_as(
            &Request {
                root: &fixture.root,
                message: None,
                sign: false,
                no_push: true,
                cwd: &fixture.root,
                provenance: None,
            },
            trigger,
            &mut |line: Say| said.push_str(&format!("{line}\n")),
        );
        assert_eq!(code, 1, "{said}");
        assert!(said.contains("nothing was written"), "{said}");
        assert!(said.contains("from-the-future"), "{said}");
        let line = journal(&fixture.root).pop().expect("a journal line");
        assert_eq!(line["outcome"], "blocked", "{line}");
    }
    assert_eq!(fixture.commits(), 1, "nothing was committed");
    assert_eq!(ask(&fixture.root, &["status", "--porcelain"]), status);
}

#[test]
fn a_plane_that_is_not_a_git_repository_is_told_so_rather_than_reported_saved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    let mut said = String::new();
    let mut say = |line: Say| said.push_str(&format!("{line}\n"));

    let code = save(
        &Request {
            root: &root,
            message: None,
            sign: false,
            no_push: true,
            cwd: &root,
            provenance: None,
        },
        &mut say,
    );

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("is not a git repository"), "{said}");
    assert!(said.contains("purlis init does not create one"), "{said}");
}

// --------------------------------------------------------------------------------------- //
// the secret guard                                                                          //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_secret_shaped_value_in_a_memory_file_stops_the_save_and_is_never_echoed() {
    let fixture = Fixture::plane();
    let secret = "ghp_0123456789abcdefghijklmnopqrstuvwxyz";
    std::fs::write(
        fixture.root.join("personas/steward/memory/leak.md"),
        format!("# the deploy\n\ntoken: {secret}\n"),
    )
    .unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("a secret-shaped value"), "{said}");
    assert!(
        said.contains("leak.md:3  (credential assignment)"),
        "{said}"
    );
    // The KIND, never the value.
    assert!(!said.contains(secret), "the refusal repeated the secret");
    assert_eq!(fixture.head_subject(), "one", "nothing was committed");
}

impl Fixture {
    /// Write `text` at `path` in the plane and save without a push.
    fn save_with(&self, path: &str, text: &[u8]) -> (u8, String) {
        let file = self.root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
        self.save(Request {
            root: &self.root,
            message: Some("a save"),
            sign: false,
            no_push: true,
            cwd: &self.root,
            provenance: None,
        })
    }
}

#[test]
fn a_secret_in_any_staged_file_stops_the_save_and_names_its_path_and_line() {
    // A save commits and pushes every file it stages, so every one of them is asked: a todo,
    // a persona definition, a session record, the plane's own settings.
    let pem = ["-----BEGIN OPENSSH ", "PRIVATE KEY-----\nb3BlbnNzaC1rZXk\n"].concat();
    let token = ["ghp", "_0123456789abcdefABCDEFghij"].concat();
    for (path, text, line, kind) in [
        (
            "workspaces/alpha/todos/0001-deploy.md",
            format!("# deploy\n\nkey for the box:\n{pem}"),
            4,
            "private key (PEM)",
        ),
        (
            "personas/devops/persona.md",
            format!("---\nname: devops\n---\n\nUse {token} for the forge.\n"),
            5,
            "a token by its forge's prefix",
        ),
        (
            "workspaces/alpha/sessions/20260929-deploy.md",
            "# the deploy\n\npassword: hunter2is\n".to_owned(),
            3,
            "credential assignment",
        ),
        (
            "charter.toml",
            "[plane]\nname = \"fixture\"\napi_key = \"abcdef0123456789\"\n".to_owned(),
            3,
            "credential assignment",
        ),
    ] {
        let fixture = Fixture::plane();
        let (code, said) = fixture.save_with(path, text.as_bytes());

        assert_eq!(code, 1, "{path}: {said}");
        assert!(
            said.contains("Refusing to save — a secret-shaped value in a staged file:"),
            "{path}: {said}"
        );
        assert!(
            said.contains(&format!("{path}:{line}  ({kind})")),
            "{path}: {said}"
        );
        for value in [
            token.as_str(),
            "hunter2is",
            "abcdef0123456789",
            "b3BlbnNzaC1rZXk",
        ] {
            assert!(
                !said.contains(value),
                "{path}: the refusal repeated the secret"
            );
        }
        assert_eq!(
            fixture.head_subject(),
            "one",
            "{path}: nothing was committed"
        );
        assert_eq!(
            standing(&fixture.root).blocked.as_deref(),
            Some(SECRET_REFUSED),
            "{path}: the standing says what the next save would refuse"
        );
    }
}

#[test]
fn a_bare_token_in_a_memory_files_prose_stops_the_save() {
    // No assignment names it, so only the prefix rule sees it.
    let fixture = Fixture::plane();
    let token = ["ghp", "_0123456789abcdefABCDEFghij"].concat();
    let (code, said) = fixture.save_with(
        "personas/steward/memory/deploy.md",
        format!("# the deploy\n\nit uses {token} today\n").as_bytes(),
    );

    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("personas/steward/memory/deploy.md:3  (a token by its forge's prefix)"),
        "{said}"
    );
    assert!(!said.contains(&token), "the refusal repeated the secret");
}

#[test]
fn a_secret_spelled_with_escapes_stops_the_save() {
    // Every spelling through escapes (#1295, NO-7, #1304): what the file holds — what charter
    // and everyone who reads it get — is the character, whether or not the file parses.
    for shape in crate::secretshape::escaped::shapes() {
        // Another committed TOML file than the project's own: a `charter.toml` that does not
        // parse makes the project read-only, and its save is refused before the scan (#826).
        let path = match shape.path {
            "charter.toml" => "harnesses/team.toml",
            path => path,
        };
        let text = &shape.text;
        let fixture = Fixture::plane();
        let (code, said) = fixture.save_with(path, text.as_bytes());

        assert_eq!(code, 1, "{text}: {said}");
        assert!(
            said.contains("Refusing to save — a secret-shaped value in a staged file:"),
            "{text}: {said}"
        );
        assert!(
            said.contains(&format!("{path}  ({})", shape.kind)),
            "{text}: {said}"
        );
        for value in [
            "KIAIOSFODNN7EXAMPLE",
            "OSFODNN7EXAMPLE",
            "0123456789abcdef",
            "hunter2",
        ] {
            assert!(
                !said.contains(value),
                "{text}: the refusal repeated the secret"
            );
        }
        assert_eq!(
            fixture.head_subject(),
            "one",
            "{text}: nothing was committed"
        );
        assert_eq!(
            standing(&fixture.root).blocked.as_deref(),
            Some(SECRET_REFUSED),
            "{text}: the standing says what the next save would refuse"
        );
    }
}

#[test]
fn a_json_or_toml_file_that_does_not_parse_is_still_scanned_as_text() {
    let token = ["ghp", "_0123456789abcdefABCDEFghij"].concat();
    for (path, text) in [
        (
            "workspaces/alpha/workspace.json",
            format!("{{\"name\": \"alpha\",\n\"note\": \"{token}\"\n"),
        ),
        // Not the project's own `charter.toml`, whose not parsing makes the project read-only
        // and its save refused before the scan (#826).
        (
            "harnesses/team.toml",
            format!("[plane\nnote = \"{token}\"\n"),
        ),
    ] {
        let fixture = Fixture::plane();
        let (code, said) = fixture.save_with(path, text.as_bytes());

        assert_eq!(code, 1, "{path}: {said}");
        assert!(
            said.contains(&format!("{path}:2  (a token by its forge's prefix)")),
            "{path}: {said}"
        );
        assert!(
            !said.contains(&token),
            "{path}: the refusal repeated the secret"
        );
    }
}

#[test]
fn a_file_with_escapes_and_no_secret_is_saved() {
    for (path, text) in crate::secretshape::escaped::clean() {
        let fixture = Fixture::plane();
        let (code, said) = fixture.save_with(path, text.as_bytes());
        assert_eq!(code, 0, "{text}: {said}");
        assert_eq!(fixture.head_subject(), "a save", "{text}");
        assert_eq!(standing(&fixture.root).blocked, None, "{text}");
    }
}

#[test]
fn an_ordinary_file_is_saved_and_one_deleted_is_never_asked_about() {
    let fixture = Fixture::plane();
    let (code, said) = fixture.save_with(
        "workspaces/alpha/todos/0002-docs.md",
        b"# docs\n\nexplain where the deploy token lives (the vault)\n",
    );
    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.head_subject(), "a save");

    std::fs::remove_file(fixture.root.join("workspaces/alpha/todos/0002-docs.md")).unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");
}

#[test]
fn a_binary_or_oversized_file_outside_memory_is_not_scanned_and_is_said_to_be_so() {
    // The stated rule: a file git itself calls binary (a NUL in its first 8000 bytes) or one
    // over the size cap is committed unscanned, and the save names it.
    let fixture = Fixture::plane();
    let token = ["ghp", "_0123456789abcdefABCDEFghij"].concat();
    let mut binary = b"\x89PNG\0\0".to_vec();
    binary.extend_from_slice(token.as_bytes());
    let (code, said) = fixture.save_with("docs/diagram.png", &binary);
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("Not scanned for secrets") && said.contains("docs/diagram.png (binary)"),
        "{said}"
    );

    let mut big = "x\n".repeat(SCAN_LIMIT / 2 + 1).into_bytes();
    big.extend_from_slice(token.as_bytes());
    let (code, said) = fixture.save_with("docs/export.txt", &big);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("docs/export.txt (over 10 MiB)"), "{said}");
}

#[test]
fn a_link_or_a_nested_repository_outside_memory_is_committed_as_what_it_is() {
    // A link's blob is its target's path and a gitlink is a commit id: neither is text to
    // scan, and neither is a reason to refuse the save.
    let fixture = Fixture::plane();
    // One that leads out of the plane, which a memory link is refused for.
    std::os::unix::fs::symlink("../elsewhere/notes.md", fixture.root.join("notes.md")).unwrap();
    let nested = fixture.root.join("vendor/tool");
    std::fs::create_dir_all(&nested).unwrap();
    run(&nested, &["init", "-q", "-b", "main", "."]);
    run(&nested, &["config", "user.name", "Fixture"]);
    run(
        &nested,
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::fs::write(nested.join("a.txt"), "a\n").unwrap();
    run(&nested, &["add", "-A"]);
    run(&nested, &["commit", "-q", "-m", "a"]);

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert!(
        ask(&fixture.root, &["ls-files", "-s", "vendor/tool"]).starts_with("160000"),
        "the premise: the nested repository is staged as a gitlink"
    );
}

#[test]
fn among_many_staged_files_the_one_holding_a_secret_is_the_one_named() {
    // The staged blobs are read side by side; what each is found to hold must stay with its
    // own path.
    let fixture = Fixture::plane();
    for i in 0..41 {
        let text = if i == 23 {
            "# n\n\npassword: hunter2is\n".to_owned()
        } else {
            format!("# note {i}\n")
        };
        std::fs::create_dir_all(fixture.root.join("docs")).unwrap();
        std::fs::write(fixture.root.join(format!("docs/n{i:02}.md")), text).unwrap();
    }

    let (code, said) = fixture.just_save();

    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("docs/n23.md:3  (credential assignment)"),
        "{said}"
    );
    assert_eq!(said.matches("(credential assignment)").count(), 1, "{said}");
}

#[test]
fn a_memory_file_is_scanned_whatever_its_size_or_bytes() {
    // Memory and ref files were scanned whole before every file was, and still are: the
    // skip rule widens nothing away from them.
    let fixture = Fixture::plane();
    let (code, said) = fixture.save_with(
        "personas/steward/memory/n.md",
        b"# n\0\n\npassword: hunter2is\n",
    );
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("(credential assignment)"), "{said}");
}

#[test]
fn a_memory_file_that_leads_out_of_the_plane_is_refused_rather_than_read_through() {
    let fixture = Fixture::plane();
    let outside = fixture.root.parent().unwrap().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("id_rsa"),
        "-----BEGIN OPENSSH PRIVATE KEY-----\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("id_rsa"),
        fixture.root.join("personas/steward/memory/leak.md"),
    )
    .unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("it leads out of the plane"), "{said}");
    assert!(!said.contains("BEGIN OPENSSH"), "it read through the link");
}

#[test]
fn a_memory_file_that_is_a_link_is_refused_even_when_it_lands_back_inside_the_plane() {
    // What is COMMITTED for a link is the link — a blob holding the target's path — so a
    // scan of that blob would answer "no secret" about a file charter never read. The
    // containment check alone let this one through: the target is inside the plane.
    let fixture = Fixture::plane();
    // Where no save stages it (`.charter/` is ignored), so the guard judges the LINK and not
    // its target: a target the guard also flagged would make this test pass for the wrong
    // reason.
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    let target = fixture.root.join(".charter/real.md");
    std::fs::write(&target, "# k\n\npassword: hunter2is\n").unwrap();
    std::os::unix::fs::symlink(
        &target,
        fixture.root.join("personas/steward/memory/leak.md"),
    )
    .unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("leak.md"), "{said}");
    assert_eq!(fixture.head_subject(), "one", "nothing was committed");
}

// --------------------------------------------------------------------------------------- //
// the secret guard reads what is STAGED (M3)                                                //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_secret_staged_and_then_cleaned_out_of_the_working_tree_is_still_refused() {
    // **The bypass this guard had from the first line of it.** The guard took its list of
    // paths from the index and its BYTES from the working tree, and `skip-worktree` is what
    // stops `add -A` putting the two back in step. Measured with git 2.50.1: the row is
    // listed, the working tree says `clean`, the index and the commit say `password: …`.
    let fixture = Fixture::plane();
    let file = fixture.root.join("personas/steward/memory/n.md");
    std::fs::write(&file, "# the deploy\n\npassword: hunter2is\n").unwrap();
    run(&fixture.root, &["add", "personas/steward/memory/n.md"]);
    run(
        &fixture.root,
        &[
            "update-index",
            "--skip-worktree",
            "personas/steward/memory/n.md",
        ],
    );
    std::fs::write(&file, "clean\n").unwrap();
    assert_eq!(
        ask(&fixture.root, &["show", ":personas/steward/memory/n.md"]),
        "# the deploy\n\npassword: hunter2is\n",
        "the test's own premise: the index and the working tree disagree"
    );

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("(credential assignment)"), "{said}");
    assert!(
        !said.contains("hunter2is"),
        "the refusal repeated the secret"
    );
    assert_eq!(fixture.head_subject(), "one", "nothing was committed");
}

#[test]
fn a_staged_path_git_would_quote_is_examined_rather_than_skipped() {
    // `core.quotePath` is on by default, so `--name-only` printed this row as
    // `"personas/caf\303\251/memory/n.md"` — quotes and octal escapes included. The
    // substring test still selected it and the file lookup then found nothing, so the row
    // went through unexamined. `-z` is what turns the quoting off.
    let fixture = Fixture::plane();
    let dir = fixture.root.join("personas/café/memory");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("n.md"), "# the deploy\n\npassword: hunter2is\n").unwrap();
    assert!(
        ask(&fixture.root, &["diff", "--cached", "--name-only"]).is_empty(),
        "the premise: `save` is what stages it"
    );

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("(credential assignment)"), "{said}");
    assert_eq!(fixture.head_subject(), "one", "nothing was committed");
}

#[test]
fn a_staged_memory_file_that_is_not_utf8_is_examined_rather_than_skipped() {
    // `read_to_string` answered `Err` for it and the `if let Ok` fell through to no flag at
    // all — a guard failing in the one direction a guard may not. The blob is read as bytes
    // and the undecodable ones become replacement characters, which is enough to see an
    // ASCII credential sitting beside them.
    let fixture = Fixture::plane();
    let mut bytes = b"# the deploy\n\npassword: hunter2is\n".to_vec();
    bytes.extend_from_slice(&[0xff, 0xfe]);
    std::fs::write(fixture.root.join("personas/steward/memory/n.md"), bytes).unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("(credential assignment)"), "{said}");
    assert_eq!(fixture.head_subject(), "one", "nothing was committed");
}

// --------------------------------------------------------------------------------------- //
// signing                                                                                   //
// --------------------------------------------------------------------------------------- //

#[test]
fn the_signer_is_never_asked_even_when_the_operator_signs_every_commit() {
    // An operator with `commit.gpgsign = true` is the ordinary case this policy exists for. The
    // retry with `--no-gpg-sign` would rescue the commit anyway, so asserting only that a
    // commit lands proves nothing: what is asserted is that the SIGNER WAS NEVER RUN, which is
    // what "no signer prompt can hang an agent" actually means. Delete the `-c
    // commit.gpgsign=false` and this goes red.
    let fixture = Fixture::plane();
    let ran = fixture.root.parent().unwrap().join("gpg-ran");
    // Through `stand_in::program`: it is run the moment it is written, and a program this
    // process wrote through its own descriptor can lose to `ETXTBSY` (charter-app#81).
    let signer = stand_in::program(
        fixture.root.parent().unwrap(),
        "gpg",
        &format!("#!/bin/sh\ntouch '{}'\nexit 1\n", ran.display()),
    );
    run(&fixture.root, &["config", "commit.gpgsign", "true"]);
    run(
        &fixture.root,
        &["config", "gpg.program", &signer.display().to_string()],
    );
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: Some("unsigned"),
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.head_subject(), "unsigned");
    assert!(!ran.exists(), "the signer was run");
}

/// A plane with a commit of its own and a remote that moved, where checking out what the
/// remote added runs the `slow` filter ([`with_a_smudge`]) — so a rebase onto it under a smudge
/// that sleeps cannot finish inside a short deadline, standing in for a signer prompt, a lock,
/// anything that waits. Answers the bare remote.
fn a_remote_no_rebase_can_finish_onto(fixture: &Fixture) -> PathBuf {
    let bare = fixture.with_a_remote();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );
    let theirs = fixture.root.parent().unwrap().join("theirs");
    run(
        fixture.root.parent().unwrap(),
        &[
            "clone",
            "-q",
            &bare.display().to_string(),
            &theirs.display().to_string(),
        ],
    );
    // The clone's own identity, as the plane's: CI runners have none of their own.
    run(&theirs, &["config", "user.name", "Other"]);
    run(&theirs, &["config", "user.email", "other@example.invalid"]);
    std::fs::write(theirs.join(".gitattributes"), "slow.md filter=slow\n").unwrap();
    std::fs::write(theirs.join("slow.md"), "slow").unwrap();
    run(&theirs, &["add", "-A"]);
    run(&theirs, &["commit", "-q", "-m", "theirs"]);
    run(&theirs, &["push", "-q", "origin", "main"]);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    bare
}

/// `then`, with every git call it makes on this thread running `smudge` for each file the
/// `slow` filter covers. Given on the command line, as the operator's own git settings would be:
/// the runner holds a filter the repository's config names (#810).
fn with_a_smudge<T>(smudge: &str, then: impl FnOnce() -> T) -> T {
    let filter = git::Isolated::default()
        .also("filter.slow.smudge", smudge)
        .also("filter.slow.clean", "cat");
    git::isolated(&filter, then)
}

/// A smudge that does not finish in any test's time, and where it says what it is: the id of
/// the `sleep` it waits on, and beside it (`.group`) the process group it runs in, which is
/// the one git leads. The group is written first, so it is there once the id is.
fn a_smudge_that_never_finishes(fixture: &Fixture) -> (String, PathBuf) {
    let said = fixture
        .root
        .parent()
        .unwrap()
        .join("what-the-filter-started");
    let smudge = format!(
        "sleep 20 & ps -o pgid= -p $$ > '{0}.group'; echo $! > '{0}.part'; \
         mv '{0}.part' '{0}'; wait; cat",
        said.display()
    );
    (smudge, said)
}

/// Holds that a git stopped under [`a_smudge_that_never_finishes`] went with what it started:
/// the filter's `sleep` is gone, and so is every process of the group git led. Each is waited
/// for, since what was killed is there until it is reaped.
///
/// A git too slow to have reached the filter within its deadline started none, and there is
/// then nothing to hold.
#[cfg(unix)]
fn nothing_git_started_is_left(said: &Path) {
    use rustix::process::{Pid, getpgrp, test_kill_process_group};
    let soon = std::time::Duration::from_secs(10);
    let Some(sleep) = stand_in::pid_named_in(said, std::time::Duration::from_secs(5)) else {
        return;
    };
    assert!(
        stand_in::is_gone(sleep, soon),
        "the filter's own child outlived the git that was stopped"
    );
    let group = std::fs::read_to_string(said.with_extension("group"))
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok())
        .filter(|raw| *raw > 1)
        .and_then(Pid::from_raw);
    // Where `ps` would not say, the filter's child above is what is held.
    let Some(group) = group else { return };
    assert_ne!(group, getpgrp(), "git ran in this test's own group");
    let until = std::time::Instant::now() + soon;
    while test_kill_process_group(group).is_ok() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        test_kill_process_group(group).is_err(),
        "a process of the stopped git's group is still running"
    );
}

#[cfg(not(unix))]
fn nothing_git_started_is_left(_said: &Path) {}

#[test]
fn a_rebase_still_running_at_its_deadline_is_stopped_and_said_to_be_out_of_time() {
    // charter-app#242: the rebase used to have no deadline at all. The deadline is the
    // helper's argument so the test does not wait out the real one.
    let fixture = Fixture::plane();
    let bare = a_remote_no_rebase_can_finish_onto(&fixture);
    run(
        &fixture.root,
        &["fetch", "-q", &bare.display().to_string(), "main"],
    );

    let (smudge, said) = a_smudge_that_never_finishes(&fixture);
    // Should the stop leave the filter running after all, the test does not.
    let _ends = stand_in::Ends::named_in(&said);

    let started = std::time::Instant::now();
    let rebased = with_a_smudge(&smudge, || {
        rebase_onto_fetched(&fixture.root, false, std::time::Duration::from_secs(1))
    });

    assert_eq!(rebased, Rebased::OutOfTime);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "it waited for the filter: {:?}",
        started.elapsed()
    );
    // Stopped is stopped: nothing of that git goes on writing the tree, and the next git in
    // this repository is not refused for a lock the stopped one left.
    nothing_git_started_is_left(&said);
    assert!(
        !fixture.root.join(".git/index.lock").exists(),
        "the stopped rebase left its lock on the index"
    );
}

#[test]
fn a_push_whose_rebase_runs_out_of_time_is_undone_reported_and_recorded_as_not_landed() {
    // charter-app#267: the deadline, end to end through the pusher — the push refused, the
    // fetch, the rebase stopped at its deadline, the abort, what the operator is told, and the
    // record `doctor` reads. Only the deadline is shortened.
    let fixture = Fixture::plane();
    let bare = a_remote_no_rebase_can_finish_onto(&fixture);
    let mine = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let (smudge, filter_said) = a_smudge_that_never_finishes(&fixture);
    let _ends = stand_in::Ends::named_in(&filter_said);

    let started = std::time::Instant::now();
    let (pushed, said) = with_a_smudge(&smudge, || {
        push_within(&fixture, false, std::time::Duration::from_secs(1))
    });
    nothing_git_started_is_left(&filter_said);

    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "it waited for the filter: {:?}",
        started.elapsed()
    );
    assert_eq!(pushed.outcome, Outcome::Failed, "{said}");
    assert!(said.contains("remote moved"), "{said}");
    assert!(
        said.contains("the rebase onto the remote did not finish within 1 seconds"),
        "{said}"
    );
    assert!(!said.contains("conflict"), "{said}");
    let git_dir = fixture.root.join(".git");
    assert!(
        !git_dir.join("rebase-merge").exists() && !git_dir.join("rebase-apply").exists(),
        "the rebase was left in progress"
    );
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), mine);
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "theirs"
    );
    let record = push_record(&fixture.root).expect("a record");
    assert_eq!(record["outcome"], "failed");
    assert_eq!(record["head"], mine.trim());
}

/// Set on the re-executed test binary: the path the signer touches when it is asked.
const SIGNING_CHILD: &str = "CHARTER_TEST_SIGNING_HOME_MARK";

#[test]
fn a_save_that_has_to_rebase_never_asks_the_signer_the_operators_home_names() {
    // charter-app#242. The commit `save` makes is unsigned by `-c`, but a remote that moved
    // sends it through `git rebase FETCH_HEAD`, which replays that commit — and a replay is a
    // commit, so it read the operator's global `commit.gpgsign = true` and asked the signer:
    // a 1Password prompt that blocks, or a signer that fails and turns an ordinary moved
    // remote into a reported conflict. The product's git clears its environment and keeps
    // `HOME`, which a test cannot set in its own process, so the signing HOME is given to a
    // CHILD: this test re-runs itself ([`crate::testrun`]).
    if let Some(ran) = std::env::var_os(SIGNING_CHILD) {
        let ran = PathBuf::from(ran);
        let fixture = Fixture::plane();
        // A plane charter never applied its policy to, which is any plane an operator cloned
        // or made by hand: the template's `commit.gpgsign = false` is taken out, so the
        // signing HOME is what governs it.
        run(&fixture.root, &["config", "--unset", "commit.gpgsign"]);
        assert_eq!(
            ask(&fixture.root, &["config", "--get", "commit.gpgsign"]).trim(),
            "true",
            "the control: the plane's git reads the signing home"
        );
        fixture.with_a_remote_that_moved();
        std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();

        let (code, said) = fixture.just_save();

        assert_eq!(code, 0, "{said}");
        assert!(said.contains("remote moved"), "it rebased: {said}");
        assert!(said.contains("Pushed main via gh"), "{said}");
        assert!(!ran.exists(), "the signer was asked");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let (home, ran) = crate::testgit::signing_home(&top);

    crate::testrun::rerun(
        &[
            "planegit::tests::a_save_that_has_to_rebase_never_asks_the_signer_the_operators_home_names",
        ],
        &[(SIGNING_CHILD, ran.as_os_str()), ("HOME", home.as_os_str())],
    );

    assert!(!ran.exists(), "the signer was asked");
}

/// Set on the re-executed test binary: the path the signer appends to for every commit it signs.
const SIGNED_CHILD: &str = "CHARTER_TEST_SIGNED_HOME_MARK";

#[test]
fn a_save_asked_to_sign_that_has_to_rebase_signs_the_commit_it_replays() {
    // The other half of charter-app#242. `--sign` is the operator asking for a signed commit,
    // and a rebase that replayed it under `-c commit.gpgsign=false` handed the remote an
    // unsigned copy without a word. The replay follows the commit's own choice.
    if let Some(ran) = std::env::var_os(SIGNED_CHILD) {
        let ran = PathBuf::from(ran);
        let fixture = Fixture::plane();
        run(&fixture.root, &["config", "--unset", "commit.gpgsign"]);
        fixture.with_a_remote_that_moved();
        std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();

        let (code, said) = fixture.save(Request {
            root: &fixture.root,
            message: Some("a signed save"),
            sign: true,
            no_push: false,
            cwd: &fixture.root,
            provenance: None,
        });

        assert_eq!(code, 0, "{said}");
        assert!(said.contains("remote moved"), "it rebased: {said}");
        assert!(said.contains("Pushed main via gh"), "{said}");
        assert_eq!(fixture.head_subject(), "a signed save");
        assert!(
            ask(&fixture.root, &["cat-file", "commit", "HEAD"]).contains("\ngpgsig "),
            "the commit the rebase replayed is unsigned"
        );
        let signed = std::fs::read_to_string(&ran).unwrap_or_default();
        assert_eq!(
            signed.lines().count(),
            2,
            "signed once for the commit and once for its replay"
        );
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let (home, ran) = crate::testgit::home_that_signs(&top);

    crate::testrun::rerun(
        &["planegit::tests::a_save_asked_to_sign_that_has_to_rebase_signs_the_commit_it_replays"],
        &[(SIGNED_CHILD, ran.as_os_str()), ("HOME", home.as_os_str())],
    );
}

/// Push HEAD with `sign`, and answer what it did and what it said.
fn push(fixture: &Fixture, sign: bool) -> (PushResult, String) {
    push_within(fixture, sign, WRITE)
}

/// [`push`], with the rebase given `deadline`.
fn push_within(
    fixture: &Fixture,
    sign: bool,
    deadline: std::time::Duration,
) -> (PushResult, String) {
    let mut said = String::new();
    let mut say = |line: Say| {
        said.push_str(&line.to_string());
        said.push('\n');
    };
    let pushed = push_head_within(&fixture.root, None, sign, deadline, &mut say);
    (pushed, said)
}

#[test]
fn a_signer_that_fails_while_a_signed_save_rebases_is_named_and_nothing_unsigned_is_pushed() {
    // charter-app#267. `--sign`, a remote that moved, and a signer that refuses the replay: git
    // stops the rebase with no conflict in it, and that was reported as one. The signer is
    // named, in its own words, and the push stops — the commit the operator asked to sign stays
    // local rather than reaching the remote unsigned.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    let signer = stand_in::program(
        fixture.root.parent().unwrap(),
        "gpg",
        "#!/bin/sh\necho 'card not present' >&2\nexit 1\n",
    );
    run(&fixture.root, &["config", "commit.gpgsign", "true"]);
    // The format too: a developer whose global `gpg.format` is `ssh` would otherwise have their
    // real signer — a 1Password prompt — asked instead of this one. Given on the command line,
    // as the operator's own settings are: a signer the repository's config names is held (#1805).
    let signing = git::Isolated::default()
        .also("gpg.format", "openpgp")
        .also("gpg.program", &signer.display().to_string());
    let mine = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let (pushed, said) = git::isolated(&signing, || push(&fixture, true));

    assert_eq!(pushed.outcome, Outcome::Failed, "{said}");
    assert!(
        said.contains("the rebase could not sign the replayed commit"),
        "{said}"
    );
    assert!(
        said.contains("card not present"),
        "the signer's words: {said}"
    );
    assert!(!said.contains("conflict"), "{said}");
    assert!(pushed.detail.contains("card not present"), "{pushed:?}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "theirs",
        "an unsigned copy reached the remote"
    );
    assert_eq!(
        ask(&fixture.root, &["rev-parse", "HEAD"]),
        mine,
        "the rebase was not undone"
    );
    assert_eq!(push_record(&fixture.root).unwrap()["outcome"], "failed");
}

#[test]
fn a_rebase_that_really_conflicts_is_still_called_a_conflict() {
    // The control for the test above: the signer's case is carved out of `conflict`, and a
    // conflict is still one.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("theirs.md"), "mine, not theirs").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);

    let (pushed, said) = push(&fixture, false);

    assert_eq!(pushed.outcome, Outcome::Conflict, "{said}");
    assert!(said.contains("rebase hit a conflict"), "{said}");
    assert_eq!(
        pushed.detail, "the remote changed the same lines in: theirs.md",
        "the record says where"
    );
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "theirs"
    );
}

// --------------------------------------------------------------------------------------- //
// the origin, and the credential that is not in it                                          //
// --------------------------------------------------------------------------------------- //

#[test]
fn an_origin_carrying_a_token_is_refused_and_the_token_is_not_repeated() {
    let fixture = Fixture::plane();
    let token = "ghp_0123456789abcdefghijklmnopqrstuvwxyz";
    run(
        &fixture.root,
        &[
            "remote",
            "add",
            "origin",
            &format!("https://x-access-token:{token}@github.com/acme/plane.git"),
        ],
    );
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    assert_eq!(origin_https(&fixture.root), None);

    let (code, said) = fixture.just_save();

    // Committed locally, and NOT pushed — which is the honest outcome, not a failure.
    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.head_subject(), "a save");
    assert!(said.contains("isn't on a forge purlis knows"), "{said}");
    assert!(!said.contains(token), "the refusal repeated the token");
}

#[test]
fn an_origin_is_rewritten_to_its_forges_https_form_or_to_nothing_at_all() {
    let fixture = Fixture::plane();
    let set = |url: &str| {
        let _ = git::run_untimed(&fixture.root, &["remote", "remove", "origin"]);
        run(&fixture.root, &["remote", "add", "origin", url]);
        origin_https(&fixture.root)
    };

    assert_eq!(
        set("https://github.com/acme/plane.git").as_deref(),
        Some("https://github.com/acme/plane.git")
    );
    assert_eq!(
        set("git@github.com:acme/plane.git").as_deref(),
        Some("https://github.com/acme/plane.git")
    );
    assert_eq!(
        set("ssh://git@gitlab.com/acme/plane.git").as_deref(),
        Some("https://gitlab.com/acme/plane.git")
    );
    // A host this plane does not manage gets no answer rather than another forge's.
    assert_eq!(set("https://evil.example/acme/plane.git"), None);
    assert_eq!(set("/srv/plane.git"), None);
}

#[test]
fn the_pull_request_link_follows_the_forge_and_never_a_substring_of_the_url() {
    let fixture = Fixture::plane();
    let root = &fixture.root;
    assert_eq!(
        compare_url("https://github.com/acme/plane.git", "charter/abc", root).as_deref(),
        Some("https://github.com/acme/plane/compare/charter/abc?expand=1")
    );
    assert_eq!(
        compare_url("https://gitlab.com/acme/plane.git", "charter/abc", root).as_deref(),
        Some(
            "https://gitlab.com/acme/plane/-/merge_requests/new\
             ?merge_request%5Bsource_branch%5D=charter/abc"
        )
    );
    // The bug the substring check could never get right: a self-hosted GitLab with a
    // `mirrors/github.com/…` namespace was handed a GitHub compare URL.
    assert_eq!(
        compare_url(
            "https://gitlab.com/mirrors/github.com/acme/plane.git",
            "b",
            root
        )
        .as_deref(),
        Some(
            "https://gitlab.com/mirrors/github.com/acme/plane/-/merge_requests/new\
             ?merge_request%5Bsource_branch%5D=b"
        )
    );
    assert_eq!(compare_url("https://evil.example/a/b", "x", root), None);
    assert_eq!(
        compare_url("git@github.com:acme/plane.git", "x", root),
        None
    );
}

// --------------------------------------------------------------------------------------- //
// pushing, to a bare repository in this test's own directory                                //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_save_lands_on_the_branch_head_is_on_and_the_record_is_cleared() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    std::fs::write(fixture.root.join("personas/steward/memory/m.md"), "m").unwrap();
    // A record left by an earlier, worse day. A clean push deletes it rather than writing
    // "pushed": there is no condition left to carry.
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        r#"{"outcome": "failed", "branch": "main", "head": "deadbeef"}"#,
    )
    .unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Pushed main via gh"), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "a save"
    );
    assert!(!push_record_path(&fixture.root).exists(), "the record");
}

#[test]
fn a_branch_that_requires_a_pull_request_gets_one_rather_than_a_stranded_commit() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    // A real pre-receive hook refusing `refs/heads/main` in GitHub's own wording. Charter may
    // name a cause it RECOGNISED, never one it inferred, so the rejection is the evidence.
    std::fs::create_dir_all(bare.join("hooks")).unwrap();
    stand_in::program(
        &bare,
        "hooks/pre-receive",
        "#!/bin/sh\nwhile read _ _ ref; do\n  case \"$ref\" in refs/heads/main)\n    echo \
         'remote: error: GH006: Protected branch update failed for refs/heads/main.' >&2\n    \
         exit 1;; esac\ndone\nexit 0\n",
    );
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();

    // rc 0: the commit reached the remote, under another name.
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("requires a pull request"), "{said}");
    assert!(said.contains("open it: https://github.com/"), "{said}");
    // A protected branch is not a remote that moved, and is never said to be one.
    assert!(!said.contains("remote moved"), "{said}");
    let short = ask(&fixture.root, &["rev-parse", "--short", "HEAD"]);
    let branch = format!("purlis/{}", short.trim());
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", &branch]).trim(),
        "a save",
        "it landed on {branch}"
    );
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "",
        "and never on the protected branch"
    );
    // The record is what carries the open pull request to the next save.
    let record = push_record(&fixture.root).expect("a record");
    assert_eq!(record["outcome"], "branched");
    assert_eq!(record["landed"], branch);
    assert_eq!(
        record["url"],
        format!("https://github.com/acme/plane/compare/{branch}?expand=1")
    );
}

/// A save of new work to a `main` the remote on `host` refuses as protected: what it said.
fn saved_past_a_protected_main(host: &str) -> String {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_on(host);
    std::fs::create_dir_all(bare.join("hooks")).unwrap();
    stand_in::program(
        &bare,
        "hooks/pre-receive",
        "#!/bin/sh\nwhile read _ _ ref; do\n  case \"$ref\" in refs/heads/main)\n    echo \
         'remote: error: protected branch hook declined' >&2\n    exit 1;; esac\ndone\nexit 0\n",
    );
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");
    said
}

#[test]
fn a_protected_main_on_github_asks_for_a_pull_request() {
    let said = saved_past_a_protected_main("github.com");
    assert!(
        said.contains("'main' requires a pull request — pushed purlis/"),
        "{said}"
    );
    assert!(said.contains("After the PR merges"), "{said}");
}

#[test]
fn a_protected_main_on_gitlab_asks_for_a_merge_request() {
    let said = saved_past_a_protected_main("gitlab.com");
    assert!(
        said.contains("'main' requires a merge request — pushed purlis/"),
        "{said}"
    );
    assert!(said.contains("After the MR merges"), "{said}");
    assert!(!said.contains("pull request"), "{said}");
}

#[test]
fn an_open_pull_request_branch_is_advanced_rather_than_replaced() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    // A live record naming a branch the remote already has, at this plane's own HEAD.
    let head = ask(&fixture.root, &["rev-parse", "HEAD"])
        .trim()
        .to_string();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/charter/abc1234",
        ],
    );
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "branched", "branch": "main", "landed": "charter/abc1234", "head": "{head}"}}"#
        ),
    )
    .unwrap();

    let reuse = open_pull_request_branch(&fixture.root);

    // There is no upstream, so the record's commit cannot be shown to have landed — and "I
    // could not check" must never read as "it landed".
    assert_eq!(reuse.as_deref(), Some("charter/abc1234"));
    // Once the branch has reached the tracked upstream the record is spent and the branch is
    // never resurrected.
    run(
        &fixture.root,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    run(
        &fixture.root,
        &["branch", "--set-upstream-to=origin/main", "main"],
    );
    assert!(is_spent(&fixture.root, &head));
    assert_eq!(open_pull_request_branch(&fixture.root), None);
}

#[test]
fn a_push_that_simply_fails_is_reported_and_the_save_is_still_a_success() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    // The remote goes away between the commit and the push — a network, in the only shape a
    // test can have one.
    std::fs::remove_dir_all(&bare).unwrap();
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "an ordinary push failure is not a failed save");
    assert_eq!(fixture.head_subject(), "a save");
    assert!(said.contains("the gh push failed"), "{said}");
    assert!(said.contains("Check `gh auth status`."), "{said}");
    assert!(
        !said.contains("remote moved"),
        "a failure is not a remote that moved: {said}"
    );
    let record = push_record(&fixture.root).expect("a record");
    assert_eq!(record["outcome"], "failed");
    let detail = record["detail"].as_str().unwrap_or_default();
    assert!(
        !detail.is_empty() && said.contains(detail.lines().next().unwrap_or_default()),
        "the record carries what git said: {record}"
    );
}

#[test]
fn a_remote_that_moved_is_fetched_rebased_onto_and_pushed_again() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );
    // Somebody else's commit, already on the remote.
    let theirs = fixture.root.parent().unwrap().join("theirs");
    run(
        fixture.root.parent().unwrap(),
        &[
            "clone",
            "-q",
            &bare.display().to_string(),
            &theirs.display().to_string(),
        ],
    );
    run(&theirs, &["config", "user.name", "Other"]);
    run(&theirs, &["config", "user.email", "other@example.invalid"]);
    std::fs::write(theirs.join("theirs.md"), "theirs").unwrap();
    run(&theirs, &["add", "-A"]);
    run(&theirs, &["commit", "-q", "-m", "theirs"]);
    run(&theirs, &["push", "-q", "origin", "main"]);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("remote moved"), "{said}");
    assert!(said.contains("Pushed main via gh"), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "a save"
    );
    // Rebased, not merged: their commit is the parent of ours.
    assert_eq!(
        ask(&bare, &["log", "-2", "--format=%s", "main"])
            .lines()
            .nth(1)
            .unwrap_or_default(),
        "theirs"
    );
}

#[test]
fn a_record_is_written_for_what_did_not_land_and_carries_the_commit_it_is_about() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("charter.toml"), "").unwrap();

    let back = record_push(
        &root,
        PushResult {
            detail: "! [remote rejected]".into(),
            ..PushResult::of(Outcome::Failed, "main")
        },
        "cafebabe",
    );

    assert_eq!(back.outcome, Outcome::Failed);
    let written = push_record(&root).expect("a record");
    assert_eq!(written["outcome"], "failed");
    assert_eq!(written["head"], "cafebabe");
    // A record with no `outcome` is no record: a defect in this file must not take a session
    // down, and must not be believed either.
    std::fs::write(push_record_path(&root), "{}").unwrap();
    assert_eq!(push_record(&root), None);
    std::fs::write(push_record_path(&root), "not json").unwrap();
    assert_eq!(push_record(&root), None);
}

#[test]
fn a_record_that_was_edited_cannot_put_a_word_of_its_own_into_gits_argv() {
    // `plane-push.json` is machine-local state anything running as the operator can rewrite,
    // and both values taken out of it reach argv. `merge-base --is-ancestor --help` exits 0,
    // which this module would read as "it landed" — and a memory that "landed" is never pushed
    // again.
    let fixture = Fixture::plane();
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    let head = ask(&fixture.root, &["rev-parse", "HEAD"])
        .trim()
        .to_string();
    run(
        &fixture.root,
        &["remote", "add", "origin", "git@github.com:acme/plane.git"],
    );
    run(
        &fixture.root,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    run(
        &fixture.root,
        &["branch", "--set-upstream-to=origin/main", "main"],
    );

    assert!(is_spent(&fixture.root, &head), "a real sha still answers");
    assert!(!is_spent(&fixture.root, "--help"), "and a flag never does");
    assert!(
        !is_spent(&fixture.root, "HEAD"),
        "nor a revision purlis did not write"
    );

    let record = |landed: &str| {
        std::fs::write(
            push_record_path(&fixture.root),
            format!(
                r#"{{"outcome": "branched", "branch": "main", "landed": "{landed}", "head": "0000000"}}"#
            ),
        )
        .unwrap();
        open_pull_request_branch(&fixture.root)
    };
    assert_eq!(record("purlis/abc1234").as_deref(), Some("purlis/abc1234"));
    // A branch a push made before the rename is still waiting on carries on (V93j).
    assert_eq!(
        record("charter/abc1234").as_deref(),
        Some("charter/abc1234")
    );
    // The branch the whole pull-request path exists to leave alone.
    assert_eq!(record("main"), None);
    assert_eq!(record("charter/--force"), None);
    assert_eq!(record("purlis/--force"), None);
    assert_eq!(record("other/abc1234"), None);
    assert_eq!(record(""), None);
}

#[test]
fn a_planted_push_record_cannot_hang_the_save_or_be_read_whole() {
    // `save` reads this file on every run that pushes, so the three things a record must not
    // be are three ways to stop an operator's everyday command. Each assertion below goes red
    // on its own if the guard it is about is removed, and the FIFO one reports rather than
    // hangs — an unguarded read of a FIFO never returns, and a test that hangs is not a test.
    let fixture = Fixture::plane();
    let record = push_record_path(&fixture.root);
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    let good = r#"{"outcome": "failed", "branch": "main", "head": "cafebabe"}"#;

    std::fs::write(&record, good).unwrap();
    assert!(push_record(&fixture.root).is_some(), "an ordinary record");

    // Too big to read whole. git cannot carry a FIFO, but a sparse file packs small and
    // arrives full size, and this one is still valid JSON with an outcome — so a reader
    // without the bound answers `Some` and the assertion below is the whole check.
    let padding = " ".repeat(crate::reopen::MAX_BYTES as usize);
    std::fs::write(
        &record,
        format!(r#"{{"outcome": "failed", "branch": "main", "head": "cafebabe"{padding}}}"#),
    )
    .unwrap();
    assert_eq!(push_record(&fixture.root), None, "a record too big to read");

    // Reached through a link. What it holds decides which remote branch the next save pushes
    // to, so a record from somewhere else is not this plane's record.
    let elsewhere = fixture.root.parent().unwrap().join("theirs.json");
    std::fs::write(&elsewhere, good).unwrap();
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &record).unwrap();
    assert_eq!(push_record(&fixture.root), None, "a linked record");

    // A FIFO is not a link, so the link check waves it through and the read never returns.
    std::fs::remove_file(&record).unwrap();
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&record))
        .expect("mkfifo runs");
    assert!(made.success(), "the test needs a fifo to plant");
    let (say, heard) = std::sync::mpsc::channel();
    let asked = fixture.root.clone();
    // In a thread, because the whole point is that the unguarded version never returns: the
    // deadline turns a hang into a failure instead of a run that never ends.
    std::thread::spawn(move || say.send(push_record(&asked)));
    let answered = heard
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("reading a fifo record must not block the save");
    assert_eq!(answered, None, "a fifo record was accepted");
}

#[test]
fn the_record_is_written_where_the_operator_put_their_state_and_read_back_from_there() {
    // `$CHARTER_HOME` moves the state directory verbatim, and `doctor` already resolves it —
    // so a `save` that hardcoded `<root>/.charter` wrote the record where nothing would ever
    // look for it. Asked of `plane::state_dir` itself, because a test that set the variable in
    // this process would leak it into every other test in this binary.
    let fixture = Fixture::plane();
    assert_eq!(
        push_record_path(&fixture.root),
        crate::plane::state_dir(&fixture.root).join("plane-push.json")
    );
    assert_eq!(
        crate::plane::state_dir(&fixture.root),
        fixture.root.join(".charter"),
        "and with no variable set it is the plane's own"
    );
}

#[test]
fn a_protected_branch_is_recognised_only_from_words_a_forge_actually_says() {
    for rejection in [
        "remote: error: GH006: Protected branch update failed",
        "! [remote rejected] main -> main (protected branch hook declined)",
        "remote: error: pre-receive hook declined",
        "remote: You are not allowed to push code to protected branches",
        "remote: create a merge_request for this branch",
        "remote: Required status check \"ci\" is expected.",
        // A GitHub ruleset that protects the branch (GH013, GitHub docs "About rulesets"),
        // GitHub's counterpart of GitLab's protected branch: only its branch-protection rule
        // lines say so.
        "remote: error: GH013: Repository rule violations found for refs/heads/main.\n\
         remote: - Changes must be made through a pull request.",
        "remote: error: GH013: Repository rule violations found for refs/heads/main.\n\
         remote: - Cannot update this protected ref.",
    ] {
        assert!(is_protected_rejection(rejection), "{rejection}");
    }
    for ordinary in [
        "! [rejected] main -> main (non-fast-forward)",
        "fatal: could not read from remote repository",
        "error: failed to push some refs",
        // GH013 is also how GitHub refuses a push for what it carries rather than where it
        // goes: secret push protection, and file-size, path and commit-message rulesets. None
        // of them is a protected branch, and a save holding a token must not be told one is.
        "remote: error: GH013: Repository rule violations found for refs/heads/main.\n\
         remote: - GITHUB PUSH PROTECTION\n\
         remote:   Push cannot contain secrets\n\
         ! [remote rejected] main -> main (push declined due to repository rule violations)",
        "remote: error: GH013: Repository rule violations found for refs/heads/main.\n\
         remote: - File size is above the limit",
        "! [remote rejected] main -> main (push declined due to repository rule violations)",
        "",
    ] {
        assert!(!is_protected_rejection(ordinary), "{ordinary}");
    }
}

// --------------------------------------------------------------------------------------- //
// how far a save goes: `[plane] mode` (charter-app#293, ADR 0051)                            //
// --------------------------------------------------------------------------------------- //

impl Fixture {
    /// Replace the plane's `charter.toml` — committed, so it is not itself the save's work.
    fn with_settings(&self, toml: &str) {
        std::fs::write(self.root.join("charter.toml"), toml).unwrap();
        run(&self.root, &["commit", "-q", "-am", "settings"]);
    }
}

#[test]
fn a_plane_whose_mode_is_off_is_never_committed_and_is_told_why() {
    let fixture = Fixture::plane();
    fixture.with_settings("[plane]\nmode = \"off\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let before = fixture.commits();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.commits(), before, "{said}");
    assert!(
        said.contains("[plane] mode is off (charter.toml), so purlis commits nothing here"),
        "{said}"
    );
    assert_eq!(
        ask(&fixture.root, &["diff", "--cached", "--name-only"]).trim(),
        "",
        "and nothing was staged either"
    );
}

#[test]
fn a_plane_whose_mode_is_commit_is_committed_and_the_remote_is_left_where_it_was() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    fixture.with_settings("[plane]\nmode = \"commit\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.head_subject(), "a save");
    assert!(
        said.contains("Not pushed: [plane] mode is commit (charter.toml)."),
        "{said}"
    );
    assert_eq!(ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(), "");
}

#[test]
fn a_pr_mode_whose_origin_is_on_no_forge_charter_knows_commits_and_says_so() {
    // A request mode opens its PR through the GitHub or GitLab adapter; on any other host a save
    // commits and goes no further, and the plane carries a notice rather than a block
    // (charter-app#295). The request modes against a forge are
    // `tests/a_pr_mode_save_keeps_one_pr_open_…`, where a stand-in `gh` answers: here nothing
    // may reach a real one.
    for mode in ["pr", "pr-merge"] {
        let fixture = Fixture::plane();
        run(
            &fixture.root,
            &[
                "remote",
                "add",
                "origin",
                "git@forge.example.invalid:acme/plane.git",
            ],
        );
        fixture.with_settings(&format!("[plane]\nmode = \"{mode}\"\n"));
        std::fs::write(fixture.root.join("work.md"), "work").unwrap();

        let (code, said) = fixture.just_save();

        assert_eq!(code, 0, "{said}");
        assert_eq!(fixture.head_subject(), "a save");
        assert!(said.contains("committed only"), "{said}");
        assert!(!said.contains("cannot open the pull request"), "{said}");
        let line = journal(&fixture.root).pop().expect("a journal line");
        assert_eq!(line["outcome"], "committed", "{mode}");
        let standing = standing(&fixture.root);
        assert_ne!(standing.stage, Stage::Blocked, "{mode}: {standing:?}");
        assert!(!standing.pushes, "{mode}");
        assert!(
            standing
                .notice
                .as_deref()
                .is_some_and(|why| why.contains("GitHub or GitLab")),
            "{mode}: {standing:?}"
        );
    }
}

#[test]
fn a_commit_left_unpushed_is_pushed_by_the_next_save_even_with_nothing_new_to_commit() {
    // What a launch's save is for (ADR 0051): the push quitting gave up on, or a commit an agent
    // made with plain git, is carried on — by a plane that names a mode. One that names none
    // keeps `charter save`'s recorded "Nothing to save".
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    fixture.with_settings("[plane]\nmode = \"push\"\n");
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );
    run(
        &fixture.root,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "by an agent"]);
    assert_eq!(standing(&fixture.root).stage, Stage::Committed);

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: false,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("carrying on 1 commit"), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "by an agent",
        "{said}"
    );
    assert_eq!(journal(&fixture.root).pop().unwrap()["outcome"], "saved");
    assert_eq!(standing(&fixture.root).stage, Stage::Saved);
}

#[test]
fn a_push_lands_on_the_target_branch_the_settings_name() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    run(&fixture.root, &["checkout", "-q", "-b", "trunk"]);
    fixture.with_settings("[plane]\nmode = \"push\"\nbranch = \"trunk\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Pushed trunk via gh"), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "trunk"]).trim(),
        "a save"
    );
    assert_eq!(ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(), "");
}

const SETTINGS_SIGN_CHILD: &str = "CHARTER_TEST_SETTINGS_SIGN_MARK";

#[test]
fn a_plane_that_says_sign_asks_the_signer_without_being_told_on_the_command_line() {
    // Re-run in a child whose HOME signs every commit through a stand-in that SIGNS and counts
    // (`testgit::home_that_signs`): the product's git keeps HOME, so run in this process the
    // test would hand the developer's own signer — a 1Password prompt — a fixture's commit.
    // The HOME's `commit.gpgsign = true` is the machine's default, and the repo turns it off,
    // so only `[plane] sign` can be what asked the signer.
    if let Some(ran) = std::env::var_os(SETTINGS_SIGN_CHILD) {
        let ran = PathBuf::from(ran);
        let fixture = Fixture::plane();
        run(&fixture.root, &["config", "commit.gpgsign", "false"]);
        fixture.with_settings("[plane]\nsign = true\n");
        std::fs::write(fixture.root.join("work.md"), "work").unwrap();

        let (code, said) = fixture.save(Request {
            root: &fixture.root,
            message: Some("signed by the settings"),
            sign: false,
            no_push: true,
            cwd: &fixture.root,
            provenance: None,
        });

        assert_eq!(code, 0, "{said}");
        assert!(
            ask(&fixture.root, &["cat-file", "commit", "HEAD"]).contains("\ngpgsig "),
            "the commit is unsigned: {said}"
        );
        assert_eq!(
            std::fs::read_to_string(&ran)
                .unwrap_or_default()
                .lines()
                .count(),
            1
        );
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let (home, ran) = crate::testgit::home_that_signs(&top);

    crate::testrun::rerun(
        &[
            "planegit::tests::a_plane_that_says_sign_asks_the_signer_without_being_told_on_the_command_line",
        ],
        &[
            (SETTINGS_SIGN_CHILD, ran.as_os_str()),
            ("HOME", home.as_os_str()),
        ],
    );
}

#[test]
fn a_save_with_no_message_says_what_changed_in_the_planes_own_words() {
    let fixture = Fixture::plane();
    for path in [
        "personas/steward/memory/a.md",
        "personas/steward/memory/b.md",
        "personas/_dispatch/2026-09.host.jsonl",
        "workspaces/ide/todos/t.json",
        "workspaces/ide/workspace.md",
        "inventory/repos.json",
    ] {
        let file = fixture.root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "x").unwrap();
    }

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    assert_eq!(
        fixture.head_subject(),
        "charter save: 6 files (steward memory 2, dispatch 1, ide todos 1, ide workspace 1, \
         +1 more)"
    );
}

#[test]
fn one_file_is_one_file() {
    let fixture = Fixture::plane();
    std::fs::write(fixture.root.join("personas/steward/memory/a.md"), "x").unwrap();
    let (_, said) = fixture.save(Request {
        root: &fixture.root,
        message: None,
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });
    assert_eq!(
        fixture.head_subject(),
        "charter save: 1 file (steward memory 1)",
        "{said}"
    );
}

// --------------------------------------------------------------------------------------- //
// the save journal (charter-app#293)                                                        //
// --------------------------------------------------------------------------------------- //

#[test]
fn every_save_attempt_is_one_line_of_the_journal_saying_how_it_ended() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");
    let _ = fixture.just_save(); // nothing left to save

    let lines = journal(&fixture.root);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let saved = &lines[0];
    assert_eq!(saved["target"], "plane");
    assert_eq!(saved["trigger"], "cli");
    assert_eq!(saved["mode"], "push");
    assert_eq!(saved["files"], 1);
    assert_eq!(
        saved["commit"],
        ask(&fixture.root, &["rev-parse", "HEAD"]).trim(),
    );
    assert_eq!(saved["outcome"], "saved");
    assert!(saved["at"].as_f64().is_some_and(|at| at > 0.0));
    assert!(saved["ms"].as_u64().is_some());
    assert_eq!(lines[1]["outcome"], "skipped");
    assert_eq!(lines[1]["commit"], serde_json::Value::Null);
}

#[test]
fn a_secret_the_scan_caught_is_journalled_as_blocked_and_a_commit_mode_as_committed() {
    let fixture = Fixture::plane();
    std::fs::write(
        fixture.root.join("personas/steward/memory/m.md"),
        "token: ghp_0123456789abcdefghijklmnopqrstuvwxyz\n",
    )
    .unwrap();
    let _ = fixture.just_save();
    assert_eq!(journal(&fixture.root)[0]["outcome"], "blocked");

    let fixture = Fixture::plane();
    fixture.with_settings("[plane]\nmode = \"commit\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let _ = fixture.just_save();
    let line = &journal(&fixture.root)[0];
    assert_eq!(
        (line["mode"].as_str(), line["outcome"].as_str()),
        (Some("commit"), Some("committed"))
    );
}

#[test]
fn the_journal_keeps_its_newest_five_hundred_lines() {
    let fixture = Fixture::plane();
    let path = journal_path(&fixture.root);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old: String = (0..500).map(|n| format!("{{\"n\": {n}}}\n")).collect();
    std::fs::write(&path, old).unwrap();

    let _ = fixture.just_save();

    let lines = journal(&fixture.root);
    assert_eq!(lines.len(), 500);
    assert_eq!(lines[0]["n"], 1, "the oldest went");
    assert_eq!(lines[499]["outcome"], "skipped", "the newest came");
}

// --------------------------------------------------------------------------------------- //
// review of #331                                                                            //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_target_branch_that_is_not_the_one_checked_out_is_never_pushed_to_or_rebased_onto() {
    // Rebasing `main` onto a `trunk` that moved would replay main's history onto trunk and
    // rewrite the local main: nothing rewrites anyone's history (ADR 0051).
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    run(&bare, &["branch", "trunk", "main"]);
    let remote_trunk = ask(&bare, &["rev-parse", "trunk"]);
    fixture.with_settings("[plane]\nmode = \"push\"\nbranch = \"trunk\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert_eq!(
        fixture.head_subject(),
        "a save",
        "committed on main: {said}"
    );
    assert!(
        said.contains(
            "Not pushed: this plane is on main, and [plane] branch is trunk (charter.toml)."
        ),
        "{said}"
    );
    assert!(!said.contains("remote moved"), "it rebased: {said}");
    assert_eq!(ask(&bare, &["rev-parse", "trunk"]), remote_trunk);
    let line = &journal(&fixture.root)[0];
    assert_eq!(line["outcome"], "blocked");
    assert_eq!(
        line["detail"],
        "this plane is on main, and [plane] branch is trunk"
    );
}

#[test]
fn the_alias_is_named_as_itself_when_it_decided_the_mode() {
    let fixture = Fixture::plane();
    fixture.with_settings("[memory]\nshare = \"commit\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let (_, said) = fixture.just_save();
    assert!(
        said.contains("Not pushed: [memory] share is commit (charter.toml)."),
        "{said}"
    );
}

#[test]
fn a_refusal_is_journalled_in_its_own_words() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    let mut said = String::new();
    let mut say = |line: Say| said.push_str(&format!("{line}\n"));
    let code = save(
        &Request {
            root: &root,
            message: None,
            sign: false,
            no_push: false,
            cwd: &root,
            provenance: None,
        },
        &mut say,
    );
    assert_eq!(code, 1);
    let line = &journal(&root)[0];
    assert_eq!(line["outcome"], "failed");
    assert!(
        line["detail"]
            .as_str()
            .unwrap_or("")
            .contains("is not a git repository"),
        "{line}"
    );
}

const REFUSED_SIGN_CHILD: &str = "CHARTER_TEST_REFUSED_SIGN_MARK";

#[test]
fn a_save_asked_to_sign_whose_signer_refuses_commits_nothing_unsigned() {
    // Run in a child whose HOME's signer FAILS (`testgit::signing_home`), never the
    // developer's own. A save asked to sign — here by `[plane] sign` — that cannot sign stops:
    // an unsigned commit is exactly what the operator asked not to make.
    if std::env::var_os(REFUSED_SIGN_CHILD).is_some() {
        let fixture = Fixture::plane();
        fixture.with_a_remote();
        run(&fixture.root, &["config", "commit.gpgsign", "false"]);
        fixture.with_settings("[plane]\nsign = true\n");
        std::fs::write(fixture.root.join("work.md"), "work").unwrap();
        let before = fixture.commits();

        let (code, said) = fixture.just_save();

        assert_eq!(code, 1, "{said}");
        assert_eq!(
            fixture.commits(),
            before,
            "an unsigned commit was made: {said}"
        );
        assert!(said.contains("could not sign the commit"), "{said}");
        let line = &journal(&fixture.root)[0];
        assert_eq!(line["outcome"], "failed");
        assert!(
            line["detail"]
                .as_str()
                .unwrap_or("")
                .contains("could not sign"),
            "{line}"
        );
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let (home, ran) = crate::testgit::signing_home(&top);
    crate::testrun::rerun(
        &["planegit::tests::a_save_asked_to_sign_whose_signer_refuses_commits_nothing_unsigned"],
        &[
            (REFUSED_SIGN_CHILD, ran.as_os_str()),
            ("HOME", home.as_os_str()),
        ],
    );
}

// --------------------------------------------------------------------------------------- //
// where unsaved work sits: the Saving view's stage (charter-app#294)                        //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_plane_whose_work_is_on_the_remote_is_saved() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Saved, "{got:?}");
    assert_eq!(got.changed, Vec::<String>::new());
    assert_eq!(got.ahead, Some(0));
}

#[test]
fn a_changed_file_and_a_new_one_are_both_what_the_next_save_takes() {
    let fixture = Fixture::plane();
    std::fs::write(fixture.root.join("README.md"), "two\n").unwrap();
    std::fs::write(fixture.root.join("personas/steward/memory/new.md"), "n").unwrap();

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Changed);
    assert_eq!(got.changed, ["README.md", "personas/steward/memory/new.md"]);
}

#[test]
fn a_commit_the_remote_does_not_have_is_committed_and_counted() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    std::fs::write(fixture.root.join("a.md"), "a").unwrap();
    let _ = fixture.just_save();
    // An agent's own commits, with plain git, on a plane whose saves push.
    for name in ["b.md", "c.md"] {
        std::fs::write(fixture.root.join(name), name).unwrap();
        run(&fixture.root, &["add", name]);
        run(&fixture.root, &["commit", "-q", "-m", name]);
    }

    let got = standing(&fixture.root);

    assert_eq!(
        (got.stage, got.ahead, got.pushes),
        (Stage::Committed, Some(2), true),
        "{got:?}"
    );
}

#[test]
fn a_push_that_conflicted_and_has_not_landed_since_is_blocked() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    let head = ask(&fixture.root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "conflict", "branch": "main", "head": "{}", "detail": "CONFLICT (content): Merge conflict in a.md"}}"#,
            head.trim()
        ),
    )
    .unwrap();

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Blocked);
    assert_eq!(
        got.blocked.as_deref(),
        Some("CONFLICT (content): Merge conflict in a.md")
    );
}

/// The remote-tracking branch at the commit the plane pushed, as a fetch of it would leave it.
fn tracks_what_it_pushed(fixture: &Fixture) {
    run(
        &fixture.root,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
}

/// A push record as a build before qw348 wrote every failed rebase: `conflict`, naming nothing.
fn a_conflict_that_names_nothing(fixture: &Fixture) {
    let head = ask(&fixture.root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(push_record_path(&fixture.root).parent().unwrap()).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "conflict", "branch": "main", "head": "{}", "detail": ""}}"#,
            head.trim()
        ),
    )
    .unwrap();
}

#[test]
fn a_conflict_record_that_names_nothing_is_no_block_and_asks_for_a_save() {
    // qw382: the title bar read "Blocked: conflict" for good, and nothing was in conflict.
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    a_conflict_that_names_nothing(&fixture);

    let got = standing(&fixture.root);

    assert_ne!(got.stage, Stage::Blocked, "{got:?}");
    assert_eq!(got.blocked, None);
    assert_eq!(got.push_failed.as_deref(), Some(STOPPED_EARLIER));
    assert!(!got.rebase_by_hand);
}

#[test]
fn a_real_conflict_record_names_its_files_and_is_settled_by_hand() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    let head = ask(&fixture.root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(push_record_path(&fixture.root).parent().unwrap()).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "conflict", "branch": "main", "head": "{}", "detail": "the remote changed the same lines in: a.md", "conflicts": ["a.md"]}}"#,
            head.trim()
        ),
    )
    .unwrap();

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Blocked);
    assert_eq!(
        got.blocked.as_deref(),
        Some("the remote changed the same lines in: a.md")
    );
    assert_eq!(got.conflicts, vec!["a.md".to_string()]);
    assert!(got.rebase_by_hand);
}

/// A rebase of `main` onto a branch that changed README.md the other way, stopped on it.
fn a_rebase_stopped_on_a_conflict(fixture: &Fixture) {
    run(&fixture.root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(fixture.root.join("README.md"), "side\n").unwrap();
    run(&fixture.root, &["commit", "-q", "-am", "side"]);
    run(&fixture.root, &["checkout", "-q", "main"]);
    std::fs::write(fixture.root.join("README.md"), "main\n").unwrap();
    run(&fixture.root, &["commit", "-q", "-am", "main"]);
    let stopped = crate::testgit::run(&fixture.root, &["rebase", "side"]);
    assert!(!stopped.ok(), "the rebase stops on the conflict");
}

#[test]
fn a_rebase_stopped_with_every_file_added_is_still_said_to_be_a_rebase() {
    // Review of qw382: with nothing left unmerged, the window offered a chat, which may not
    // continue a rebase in the plane root.
    let fixture = Fixture::plane();
    a_rebase_stopped_on_a_conflict(&fixture);
    std::fs::write(fixture.root.join("README.md"), "both\n").unwrap();
    run(&fixture.root, &["add", "README.md"]);

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Blocked, "{got:?}");
    assert!(got.conflicts.is_empty(), "{got:?}");
    assert_eq!(got.stopped_by, Some("rebase"));
    assert!(!got.rebase_by_hand);
}

#[test]
fn a_merge_stopped_on_a_conflict_is_said_to_be_a_merge() {
    let fixture = Fixture::plane();
    run(&fixture.root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(fixture.root.join("README.md"), "side\n").unwrap();
    run(&fixture.root, &["commit", "-q", "-am", "side"]);
    run(&fixture.root, &["checkout", "-q", "main"]);
    std::fs::write(fixture.root.join("README.md"), "main\n").unwrap();
    run(&fixture.root, &["commit", "-q", "-am", "main"]);
    assert!(!crate::testgit::run(&fixture.root, &["merge", "side"]).ok());

    let got = standing(&fixture.root);

    assert_eq!(got.stopped_by, Some("merge"));
    assert_eq!(got.conflicts, vec!["README.md".to_string()]);
}

#[test]
fn save_on_a_clean_plane_whose_push_stopped_retries_it_and_clears_the_record() {
    // qw382: a clean tree and no mode answered "Nothing to save", and the record held.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    a_conflict_that_names_nothing(&fixture);

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert!(!said.contains("Nothing to save"), "{said}");
    assert!(push_record(&fixture.root).is_none(), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "mine",
        "{said}"
    );
    assert_eq!(standing(&fixture.root).stage, Stage::Saved);
}

#[test]
fn save_on_a_clean_plane_whose_push_really_conflicts_records_the_files_today() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("theirs.md"), "mine, not theirs").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    a_conflict_that_names_nothing(&fixture);

    let (_, said) = fixture.just_save();

    let got = standing(&fixture.root);
    assert_eq!(got.stage, Stage::Blocked, "{said}");
    assert_eq!(
        got.blocked.as_deref(),
        Some("the remote changed the same lines in: theirs.md"),
        "{said}"
    );
    assert_eq!(got.conflicts, vec!["theirs.md".to_string()]);
    assert!(got.rebase_by_hand);
}

#[test]
fn the_windows_save_pushes_a_mode_less_planes_commits_the_remote_lacks() {
    // The dispatcher's ruling on qw382: Save in the window never says "nothing to save" while
    // the plane has commits its upstream lacks, whatever the mode.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);

    let (code, said) = pressed_save(&fixture);

    assert_eq!(code, 0, "{said}");
    assert!(!said.contains("Nothing to save"), "{said}");
    assert!(said.contains("carrying on 1 commit"), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "mine",
        "{said}"
    );
}

/// The window's Save, as the button presses it.
fn pressed_save(fixture: &Fixture) -> (u8, String) {
    let mut said = String::new();
    let code = save_as(
        &Request {
            root: &fixture.root,
            message: None,
            sign: false,
            no_push: false,
            cwd: &fixture.root,
            provenance: None,
        },
        Trigger::Manual,
        &mut |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        },
    );
    (code, said)
}

#[test]
fn after_an_update_clears_the_record_the_windows_save_pushes_what_it_replayed() {
    // Review of qw382: an Update on a mode-less plane deleted the record, and Save then said
    // "Nothing to save" with the replayed commit unpushed.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    let head = ask(&fixture.root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(push_record_path(&fixture.root).parent().unwrap()).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "conflict", "branch": "main", "head": "{}", "detail": "the remote changed the same lines in: mine.md", "conflicts": ["mine.md"]}}"#,
            head.trim()
        ),
    )
    .unwrap();
    run(
        &fixture.root,
        &[
            "fetch",
            "-q",
            &bare.display().to_string(),
            "+refs/heads/main:refs/remotes/origin/main",
        ],
    );
    let plane = crate::planesave::Settings::read(&fixture.root).plane;
    let moved = update::bring_in(
        &fixture.root,
        &plane,
        "main",
        &forge::Caller::window(),
        WRITE,
        true,
        &mut |_: Say| {},
    );
    assert!(matches!(moved, Updated::Moved(_)), "{moved:?}");
    assert!(push_record(&fixture.root).is_none(), "the Update forgot it");

    let (code, said) = pressed_save(&fixture);

    assert_eq!(code, 0, "{said}");
    assert!(!said.contains("Nothing to save"), "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "mine",
        "{said}"
    );
}

#[test]
fn a_clean_plane_with_no_record_still_has_nothing_to_save() {
    // `purlis save` keeps its recorded answer for a mode-less plane: the retry is for a push
    // that stopped, and the window's own press is the other reason to push.
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);

    let (_, said) = fixture.just_save();

    assert!(said.contains("Nothing to save"), "{said}");
}

#[test]
fn a_commit_waiting_on_its_pull_request_is_pushed_with_the_pr_open() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    let head = ask(&fixture.root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "branched", "branch": "main", "landed": "charter/abc", "url": "https://github.com/acme/plane/compare/charter/abc", "head": "{}"}}"#,
            head.trim()
        ),
    )
    .unwrap();

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::PrOpen);
    assert_eq!(
        got.pr.as_deref(),
        Some("https://github.com/acme/plane/compare/charter/abc")
    );
}

#[test]
fn a_plane_whose_save_stops_at_the_commit_is_saved_once_nothing_is_left_to_take() {
    // No remote at all: a save commits and can go no further, so nothing more is unsaved.
    let fixture = Fixture::plane();
    let got = standing(&fixture.root);
    assert_eq!((got.stage, got.pushes), (Stage::Saved, false), "{got:?}");

    // A remote, and a mode that stops at the commit.
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    fixture.with_settings("[plane]\nmode = \"commit\"\n");
    std::fs::write(fixture.root.join("a.md"), "a").unwrap();
    let _ = fixture.just_save();
    let got = standing(&fixture.root);
    assert_eq!((got.stage, got.pushes), (Stage::Saved, false), "{got:?}");
}

#[test]
fn commits_are_counted_against_the_target_branch_the_settings_name() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    run(&fixture.root, &["checkout", "-q", "-b", "trunk"]);
    fixture.with_settings("[plane]\nmode = \"push\"\nbranch = \"trunk\"\n");
    std::fs::write(fixture.root.join("a.md"), "a").unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");

    let got = standing(&fixture.root);

    assert_eq!(
        (got.stage, got.ahead, got.branch.as_str()),
        (Stage::Saved, Some(0), "trunk")
    );
}

#[test]
fn a_plane_that_is_not_a_repository_is_blocked_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let got = standing(dir.path());
    assert_eq!(got.stage, Stage::Blocked);
    assert_eq!(
        got.blocked.as_deref(),
        Some("this plane is not a git repository, so there is nothing to commit to")
    );
}

// --------------------------------------------------------------------------------------- //
// auto-save's view of the plane, and what comes in (charter-app#296)                        //
// --------------------------------------------------------------------------------------- //

#[test]
fn the_fingerprint_moves_when_what_is_unsaved_moves_and_only_then() {
    let fixture = Fixture::plane();
    let clean = fingerprint(&fixture.root);
    assert_eq!(
        fingerprint(&fixture.root),
        clean,
        "asked twice, nothing changed"
    );

    std::fs::write(fixture.root.join("a.md"), "one").unwrap();
    let first = fingerprint(&fixture.root);
    assert_ne!(first, clean);
    std::fs::write(fixture.root.join("a.md"), "one and more").unwrap();
    assert_ne!(
        fingerprint(&fixture.root),
        first,
        "the same file, written again"
    );
}

#[test]
fn commits_on_the_remote_are_counted_as_incoming_and_a_clean_tree_is_fast_forwarded() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let got = fetch(&fixture.root, true).expect("fetched");

    assert_eq!((got.behind, got.moved), (1, true), "{got:?}");
    assert_ne!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert!(
        fixture.root.join("theirs.md").is_file(),
        "their file is here now"
    );
    assert_eq!(standing(&fixture.root).behind, Some(0));
}

#[test]
fn a_fetch_keeps_the_upstream_commit_the_remote_named_where_no_chat_writes() {
    // #2156: what the team's off is checked against is the remote's answer to purlis's own
    // fetch, kept in this machine's config home, never a ref in the clone.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    let theirs = ask(&bare, &["rev-parse", "refs/heads/main"])
        .trim()
        .to_owned();

    fetch(&fixture.root, false).expect("fetched");
    assert_eq!(
        crate::sandbox::local::fetched_upstream(&fixture.root).as_deref(),
        Some(theirs.as_str())
    );

    // A remote-tracking ref moved in the clone is not what was fetched.
    let mine = ask(&fixture.root, &["rev-parse", "HEAD"]).trim().to_owned();
    run(
        &fixture.root,
        &["update-ref", "refs/remotes/origin/main", &mine],
    );
    assert_eq!(
        crate::sandbox::local::fetched_upstream(&fixture.root).as_deref(),
        Some(theirs.as_str())
    );
}

#[test]
fn a_tree_with_work_in_it_is_told_what_is_incoming_and_never_moved() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let got = fetch(&fixture.root, true).expect("fetched");

    assert_eq!((got.behind, got.moved), (1, false), "{got:?}");
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert_eq!(standing(&fixture.root).behind, Some(1));
}

#[test]
fn quitting_commits_at_once_and_gives_a_push_that_never_answers_only_its_bound() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    std::fs::create_dir_all(bare.join("hooks")).unwrap();
    // A remote that takes the push and never answers. Quitting leaves that push running, as
    // it says, so the test ends the hook itself, by the id the hook wrote: the push then
    // fails and goes. Once only: the push that is tried again after it is refused at once.
    let hook_said = bare.join("the-hook-that-never-answers");
    let ends = stand_in::Ends::named_in(&hook_said);
    stand_in::program(
        &bare,
        "hooks/pre-receive",
        &format!(
            "#!/bin/sh\n[ -e '{0}' ] && exit 1\necho $$ > '{0}.part'\nmv '{0}.part' '{0}'\n\
             exec sleep 30\n",
            hook_said.display()
        ),
    );
    fixture.with_settings("[plane]\nmode = \"push\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let before = fixture.commits();

    let started = std::time::Instant::now();
    let got = crate::autosave::at_quit(&fixture.root, Duration::from_secs(2));
    let took = started.elapsed();

    assert!(took < Duration::from_secs(6), "quit waited {took:?}");
    assert_eq!(got, crate::autosave::AtQuit::CommittedPushStillRunning);
    assert_eq!(fixture.commits(), before + 1, "the work was committed");
    assert_eq!(journal(&fixture.root)[0]["trigger"], "quit");
    let hook = stand_in::pid_named_in(&hook_said, Duration::from_secs(5));
    drop(ends);
    #[cfg(unix)]
    if let Some(hook) = hook {
        assert!(
            stand_in::is_gone(hook, Duration::from_secs(10)),
            "the test left its own hook running"
        );
    }
    #[cfg(not(unix))]
    let _ = hook;
}

#[test]
fn quitting_a_plane_with_nothing_to_save_or_auto_save_off_does_nothing() {
    let fixture = Fixture::plane();
    fixture.with_settings("[plane]\nmode = \"push\"\n");
    assert_eq!(
        crate::autosave::at_quit(&fixture.root, Duration::from_secs(2)),
        crate::autosave::AtQuit::Nothing
    );

    let fixture = Fixture::plane();
    fixture.with_settings("[plane]\nmode = \"push\"\nautosave = false\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let before = fixture.commits();
    assert_eq!(
        crate::autosave::at_quit(&fixture.root, Duration::from_secs(2)),
        crate::autosave::AtQuit::Nothing
    );
    assert_eq!(fixture.commits(), before);
}

#[test]
fn with_auto_save_off_incoming_commits_are_counted_and_never_pulled() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let got = fetch(&fixture.root, false).expect("fetched");

    assert_eq!((got.behind, got.moved), (1, false), "{got:?}");
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
}

// --------------------------------------------------------------------------------------- //
// `charter save --pull`: the app's incoming path, asked for from a terminal (#375)          //
// --------------------------------------------------------------------------------------- //

impl Fixture {
    fn pull(&self) -> (u8, String) {
        let mut said = String::new();
        let mut say = |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        };
        let code = super::pull(&self.root, &mut say);
        (code, said)
    }
}

#[test]
fn a_pull_brings_incoming_commits_into_a_clean_tree_and_says_how_many() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();

    let (code, said) = fixture.pull();

    assert_eq!(code, 0, "{said}");
    assert!(fixture.root.join("theirs.md").is_file(), "{said}");
    assert!(said.contains("Brought in 1 incoming commit"), "{said}");
}

#[test]
fn a_pull_with_nothing_incoming_says_so() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );

    let (code, said) = fixture.pull();

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Nothing incoming"), "{said}");
}

#[test]
fn a_pull_into_a_tree_with_work_in_it_leaves_it_and_says_why_without_failing() {
    // The save that follows commits the work; a pull is not what may cost it.
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let (code, said) = fixture.pull();

    assert_eq!(code, 0, "{said}");
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert!(said.contains("1 incoming commit not brought in"), "{said}");
    assert!(said.contains("unsaved changes"), "{said}");
}

#[test]
fn a_pull_into_a_tree_with_conflicts_is_refused_and_names_them() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    // A merge of two sides that both changed README.md, stopped on its conflict.
    run(&fixture.root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(fixture.root.join("README.md"), "side\n").unwrap();
    run(&fixture.root, &["commit", "-q", "-am", "side"]);
    run(&fixture.root, &["checkout", "-q", "main"]);
    std::fs::write(fixture.root.join("README.md"), "main\n").unwrap();
    run(&fixture.root, &["commit", "-q", "-am", "main"]);
    let _ = crate::testgit::run(&fixture.root, &["merge", "side"]);
    assert!(fixture.root.join(".git/MERGE_HEAD").exists());
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let (code, said) = fixture.pull();

    assert_eq!(code, 1, "{said}");
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert!(said.contains("conflict"), "{said}");
    assert!(said.contains("README.md"), "{said}");
}

#[test]
fn a_pull_with_nowhere_to_fetch_from_fails_in_words() {
    let fixture = Fixture::plane();
    let (code, said) = fixture.pull();
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("origin is not on a forge purlis knows"),
        "{said}"
    );
}

// --------------------------------------------------------------------------------------- //
// review of #335                                                                            //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_push_that_failed_leaves_the_plane_committed_and_says_why_rather_than_blocked() {
    // Offline is not blocked: auto-save tries again later, and a conflict is what stops it.
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    let head = ask(&fixture.root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(fixture.root.join(".charter")).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        format!(
            r#"{{"outcome": "failed", "branch": "main", "head": "{}", "detail": "Could not resolve host: github.com"}}"#,
            head.trim()
        ),
    )
    .unwrap();

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Committed, "{got:?}");
    assert_eq!(got.blocked, None);
    assert_eq!(
        got.push_failed.as_deref(),
        Some("Could not resolve host: github.com")
    );
}

#[test]
fn a_second_save_of_a_plane_while_one_runs_is_refused_and_leaves_no_journal_line() {
    let fixture = Fixture::plane();
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let running = Claim::of(&fixture.root).expect("the first save");
    let before = fixture.commits();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("A save of this plane is already running"),
        "{said}"
    );
    assert_eq!(fixture.commits(), before);
    assert!(journal(&fixture.root).is_empty());
    drop(running);
    assert_eq!(
        fixture.just_save().0,
        0,
        "the next save, once the first is done"
    );
}

/// Another process's hold on the plane's save lock: a lock taken through a file of its own,
/// as `flock` sees another process's (#2002).
fn held_elsewhere(root: &Path) -> std::fs::File {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(".git").join(SAVE_LOCK))
        .expect("the lock file opens");
    lock.try_lock().expect("nobody holds it yet");
    lock
}

#[test]
fn a_save_another_process_is_running_refuses_a_second_and_the_next_one_goes_ahead() {
    // The window's Save, `purlis save` in a terminal and the session host's auto-save are
    // three processes: each claims the plane through a lock in its git directory.
    let fixture = Fixture::plane();
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let elsewhere = held_elsewhere(&fixture.root);
    let before = fixture.commits();

    assert!(
        Claim::of(&fixture.root).is_none(),
        "the plane was claimed twice"
    );
    let (code, said) = fixture.just_save();
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("A save of this plane is already running"),
        "{said}"
    );
    assert_eq!(fixture.commits(), before);

    drop(elsewhere);
    assert_eq!(
        fixture.just_save().0,
        0,
        "the next save, once the other is done"
    );
}

#[test]
fn a_claim_holds_the_plane_against_every_other_process_until_it_is_let_go_of() {
    let fixture = Fixture::plane();
    let claim = Claim::of(&fixture.root).expect("nobody holds it");

    let other = std::fs::OpenOptions::new()
        .write(true)
        .open(fixture.root.join(".git").join(SAVE_LOCK))
        .expect("the claim's lock file is in the git directory");
    assert!(
        matches!(other.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
        "another process could save while this one holds the claim"
    );

    drop(claim);
    other.try_lock().expect("let go of with the claim");
}

/// Whether another process could take the plane's save lock now: a lock taken through a file
/// of its own, as `flock` sees another process's, and let go of at once.
fn free_elsewhere(root: &Path) -> bool {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(".git").join(SAVE_LOCK))
        .expect("the lock file opens");
    let free = lock.try_lock().is_ok();
    if free {
        lock.unlock().expect("let go of");
    }
    free
}

#[test]
fn two_claims_of_one_repository_in_one_process_share_its_lock_until_the_last_lets_go() {
    // A claim nested in another, of the same repository reached by another spelling, is this
    // process's own rule to settle, as it was before the lock: the two share it.
    let fixture = Fixture::plane();
    let link = fixture
        .root
        .parent()
        .unwrap()
        .join("the-plane-through-a-link");
    std::os::unix::fs::symlink(&fixture.root, &link).unwrap();
    let outer = Claim::of(&fixture.root).expect("nobody holds it");

    let nested = Claim::of(&link).expect("the nested claim shares this process's lock");
    assert!(!free_elsewhere(&fixture.root));

    drop(nested);
    assert!(
        !free_elsewhere(&fixture.root),
        "the nested claim let go of the outer one's lock"
    );
    drop(outer);
    assert!(
        free_elsewhere(&fixture.root),
        "the lock outlived its claims"
    );
}

#[test]
fn a_fetch_that_fails_lets_go_of_the_plane_for_every_other_process() {
    // No remote to fetch from: the fetch answers an error, and its claim goes with it.
    let fixture = Fixture::plane();

    assert!(fetch(&fixture.root, true).is_err());

    assert!(
        free_elsewhere(&fixture.root),
        "the failed fetch kept the lock"
    );
    assert!(Claim::of(&fixture.root).is_some());
}

#[test]
fn a_save_then_a_fetch_in_one_process_each_take_the_plane_in_turn() {
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");

    assert!(
        fetch(&fixture.root, true).is_ok(),
        "the save kept the plane"
    );
}

#[test]
fn a_fetch_waits_its_turn_behind_a_running_save() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    let running = Claim::of(&fixture.root).expect("a save");
    assert!(fetch(&fixture.root, true).is_err());
    drop(running);
    assert!(fetch(&fixture.root, true).is_ok());
}

#[test]
fn reading_where_unsaved_work_sits_never_writes_the_index_a_save_needs() {
    // A plain `git status` refreshes stale stat data and writes `.git/index` under
    // `index.lock` — which a save's `git add -A` landing at that moment failed on, from the
    // title bar asking every ten seconds. The read must be read-only.
    let fixture = Fixture::plane();
    let index = fixture.root.join(".git/index");
    let old = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&index)
        .unwrap()
        .set_modified(old)
        .unwrap();
    // A tracked file whose stat data no longer matches the index: a refresh would rewrite it.
    // Its mtime is set, not left to the write: git compares whole seconds, so a rewrite in the
    // second the fixture committed in matched, and this test only saw a refresh when the
    // commit and the rewrite straddled a second — about one run in twenty under load.
    let readme = fixture.root.join("README.md");
    std::fs::write(&readme, "one\n").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&readme)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let before = std::fs::metadata(&index).unwrap().modified().unwrap();

    let _ = standing(&fixture.root);

    assert_eq!(
        std::fs::metadata(&index).unwrap().modified().unwrap(),
        before
    );
}

// --------------------------------------------------------------------------------------- //
// the blocked state and its ways out (charter-app#295)                                      //
// --------------------------------------------------------------------------------------- //

#[test]
fn two_machines_appending_to_the_same_log_and_index_both_save_by_the_union_merge_rules() {
    let fixture = Fixture::plane();
    // The rules `init` writes, committed as a plane carries them.
    crate::scaffold::ensure_gitattributes(&fixture.root.join(".gitattributes")).unwrap();
    std::fs::create_dir_all(fixture.root.join("personas/_dispatch")).unwrap();
    std::fs::write(
        fixture.root.join("personas/_dispatch/2026-09.a.jsonl"),
        "{\"n\":0}\n",
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("personas/steward/memory/MEMORY.md"),
        "- [one](one.md)\n",
    )
    .unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(
        &fixture.root,
        &["commit", "-q", "-m", "the rules and the files"],
    );
    let bare = fixture.with_a_remote();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );

    // Somebody else appends to both and pushes first.
    let theirs = fixture.root.parent().unwrap().join("theirs");
    run(
        fixture.root.parent().unwrap(),
        &[
            "clone",
            "-q",
            &bare.display().to_string(),
            &theirs.display().to_string(),
        ],
    );
    run(&theirs, &["config", "user.name", "Other"]);
    run(&theirs, &["config", "user.email", "other@example.invalid"]);
    append(
        &theirs.join("personas/_dispatch/2026-09.a.jsonl"),
        "{\"n\":1}\n",
    );
    append(
        &theirs.join("personas/steward/memory/MEMORY.md"),
        "- [theirs](theirs.md)\n",
    );
    run(&theirs, &["commit", "-q", "-am", "theirs"]);
    run(&theirs, &["push", "-q", "origin", "main"]);

    // This machine appends to the same two files, and saves.
    append(
        &fixture.root.join("personas/_dispatch/2026-09.a.jsonl"),
        "{\"n\":2}\n",
    );
    append(
        &fixture.root.join("personas/steward/memory/MEMORY.md"),
        "- [mine](mine.md)\n",
    );
    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Pushed main"), "{said}");
    let index =
        std::fs::read_to_string(fixture.root.join("personas/steward/memory/MEMORY.md")).unwrap();
    assert!(
        index.contains("theirs.md") && index.contains("mine.md"),
        "{index}"
    );
}

fn append(path: &Path, text: &str) {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

#[test]
fn a_conflict_names_the_files_it_is_in() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    std::fs::write(fixture.root.join("notes.md"), "base\n").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "base"]);
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );
    let theirs = fixture.root.parent().unwrap().join("theirs");
    run(
        fixture.root.parent().unwrap(),
        &[
            "clone",
            "-q",
            &bare.display().to_string(),
            &theirs.display().to_string(),
        ],
    );
    run(&theirs, &["config", "user.name", "Other"]);
    run(&theirs, &["config", "user.email", "other@example.invalid"]);
    std::fs::write(theirs.join("notes.md"), "theirs\n").unwrap();
    run(&theirs, &["commit", "-q", "-am", "theirs"]);
    run(&theirs, &["push", "-q", "origin", "main"]);
    std::fs::write(fixture.root.join("notes.md"), "mine\n").unwrap();

    let (_, said) = fixture.just_save();

    let got = standing(&fixture.root);
    assert_eq!(got.stage, Stage::Blocked, "{said}");
    assert_eq!(got.conflicts, ["notes.md"], "{got:?}");
}

#[test]
fn a_save_the_secret_scan_refused_is_blocked_until_a_save_goes_through() {
    let fixture = Fixture::plane();
    std::fs::write(
        fixture.root.join("personas/steward/memory/m.md"),
        "token: ghp_0123456789abcdefghijklmnopqrstuvwxyz\n",
    )
    .unwrap();
    let _ = fixture.just_save();

    let got = standing(&fixture.root);
    assert_eq!(got.stage, Stage::Blocked, "{got:?}");
    assert!(
        got.blocked.as_deref().unwrap_or("").contains("secret"),
        "{got:?}"
    );

    std::fs::write(fixture.root.join("personas/steward/memory/m.md"), "clean\n").unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");
    assert_ne!(standing(&fixture.root).stage, Stage::Blocked);
}

#[test]
fn a_pr_mode_on_a_remote_no_forge_adapter_serves_keeps_committing_and_says_why_it_goes_no_further()
{
    // Not blocked: auto-save pauses on blocked, and a commit is still what a save can do here.
    let fixture = Fixture::plane();
    run(
        &fixture.root,
        &["remote", "add", "origin", "git@git.corp:team/plane.git"],
    );
    fixture.with_settings("[plane]\nmode = \"pr\"\n");

    let got = standing(&fixture.root);

    assert_ne!(got.stage, Stage::Blocked);
    assert_eq!(
        got.notice.as_deref(),
        Some(
            "[plane] mode is pr, and this plane's origin is not a GitHub or GitLab forge purlis knows, so a save goes no further than a commit"
        )
    );
}

#[test]
fn a_memory_file_deleted_is_saved_because_a_deletion_carries_no_secret() {
    // charter-app#301 found it: going LOCAL untracks a workspace's memory, and the secret
    // guard asked the index for a file the save was deleting — "could not read what is
    // staged" — and refused every save that removed a memory.
    let fixture = Fixture::plane();
    std::fs::write(
        fixture.root.join("personas/steward/memory/old.md"),
        "an old note\n",
    )
    .unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "a memory"]);
    std::fs::remove_file(fixture.root.join("personas/steward/memory/old.md")).unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert_eq!(ask(&fixture.root, &["status", "--porcelain"]), "");
}

#[test]
fn a_block_resolved_by_hand_clears_without_a_save() {
    // A secret the scan caught: blocked while the file is unsaved, not once it is gone.
    let fixture = Fixture::plane();
    let leak = fixture.root.join("personas/steward/memory/m.md");
    std::fs::write(&leak, "token: ghp_0123456789abcdefghijklmnopqrstuvwxyz\n").unwrap();
    let _ = fixture.just_save();
    assert_eq!(standing(&fixture.root).stage, Stage::Blocked);
    std::fs::remove_file(&leak).unwrap();
    assert_ne!(standing(&fixture.root).stage, Stage::Blocked);

    // The plane on another branch than [plane] branch: blocked while it is, not after.
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    fixture.with_settings("[plane]\nmode = \"push\"\nbranch = \"trunk\"\n");
    let got = standing(&fixture.root);
    assert_eq!(got.stage, Stage::Blocked, "{got:?}");
    assert_eq!(
        got.blocked.as_deref(),
        Some("this plane is on main, and [plane] branch is trunk")
    );
    run(&fixture.root, &["checkout", "-q", "-b", "trunk"]);
    assert_ne!(standing(&fixture.root).stage, Stage::Blocked);
}

// --------------------------------------------------------------------------------------- //
// a tree git stopped part-way through something is never saved (#433)                       //
// --------------------------------------------------------------------------------------- //

impl Fixture {
    /// README.md changed on `side` and on `main` both, with HEAD on `main`.
    fn diverged(&self) {
        run(&self.root, &["checkout", "-q", "-b", "side"]);
        std::fs::write(self.root.join("README.md"), "side\n").unwrap();
        run(&self.root, &["commit", "-q", "-am", "side"]);
        run(&self.root, &["checkout", "-q", "main"]);
        std::fs::write(self.root.join("README.md"), "main\n").unwrap();
        run(&self.root, &["commit", "-q", "-am", "main"]);
    }

    /// Save, and assert it was refused as blocked with nothing staged or committed.
    fn refused(&self, words: &str) -> String {
        let head = ask(&self.root, &["rev-parse", "HEAD"]);
        let staged = ask(&self.root, &["diff", "--cached", "--name-only"]);
        let unmerged = ask(&self.root, &["diff", "--name-only", "--diff-filter=U"]);

        let (code, said) = self.just_save();

        assert_eq!(code, 1, "{said}");
        assert!(said.contains("Not saved"), "{said}");
        assert!(said.contains(words), "{said}");
        assert_eq!(ask(&self.root, &["rev-parse", "HEAD"]), head, "{said}");
        assert_eq!(
            ask(&self.root, &["diff", "--cached", "--name-only"]),
            staged,
            "nothing staged: {said}"
        );
        assert_eq!(
            ask(&self.root, &["diff", "--name-only", "--diff-filter=U"]),
            unmerged,
            "a conflict is still one: {said}"
        );
        let last = journal(&self.root).pop().expect("journalled");
        assert_eq!(last["outcome"], "blocked", "{last}");
        assert!(
            last["detail"].as_str().unwrap_or("").contains(words),
            "{last}"
        );
        let got = standing(&self.root);
        assert_eq!(got.stage, Stage::Blocked, "{got:?}");
        assert!(
            got.blocked.as_deref().unwrap_or("").contains(words),
            "{got:?}"
        );
        said
    }
}

#[test]
fn a_save_in_a_conflicted_merge_is_refused_and_commits_no_markers() {
    let fixture = Fixture::plane();
    fixture.diverged();
    std::fs::write(fixture.root.join("mine.md"), "mine\n").unwrap();
    let _ = crate::testgit::run(&fixture.root, &["merge", "side"]);
    assert!(fixture.root.join(".git/MERGE_HEAD").exists());

    let said = fixture.refused("git merge --abort");

    assert!(said.contains("README.md"), "{said}");
    assert_eq!(
        standing(&fixture.root).conflicts,
        vec!["README.md".to_string()]
    );
    let committed = ask(&fixture.root, &["show", "HEAD:README.md"]);
    assert!(!committed.contains("<<<<<<<"), "{committed}");
}

#[test]
fn a_save_while_a_rebase_is_stopped_part_way_is_refused() {
    let fixture = Fixture::plane();
    fixture.diverged();
    let _ = crate::testgit::run(&fixture.root, &["rebase", "side"]);

    fixture.refused("git rebase --continue");
}

#[test]
fn a_save_while_a_revert_is_stopped_is_refused() {
    let fixture = Fixture::plane();
    fixture.diverged();
    // Reverting `main~1` ("one") conflicts with `main`, which changed the line it added.
    let _ = crate::testgit::run(&fixture.root, &["revert", "--no-edit", "main~1"]);
    assert!(fixture.root.join(".git/REVERT_HEAD").exists());

    fixture.refused("git revert --abort");
}

#[test]
fn a_save_during_a_bisect_is_refused() {
    let fixture = Fixture::plane();
    run(&fixture.root, &["bisect", "start"]);
    std::fs::write(fixture.root.join("work.md"), "work\n").unwrap();

    fixture.refused("git bisect reset");
}

#[test]
fn once_the_merge_is_finished_the_save_goes_ahead() {
    let fixture = Fixture::plane();
    fixture.diverged();
    let _ = crate::testgit::run(&fixture.root, &["merge", "side"]);
    fixture.refused("git merge --abort");
    std::fs::write(fixture.root.join("README.md"), "both\n").unwrap();
    run(&fixture.root, &["add", "README.md"]);
    run(&fixture.root, &["commit", "-q", "--no-edit"]);
    std::fs::write(fixture.root.join("work.md"), "work\n").unwrap();

    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: Some("after"),
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });

    assert_eq!(code, 0, "{said}");
    assert_eq!(fixture.head_subject(), "after");
    assert_ne!(standing(&fixture.root).stage, Stage::Blocked);
}

#[test]
fn aborting_the_merge_by_hand_clears_the_planes_block_at_once() {
    let fixture = Fixture::plane();
    fixture.diverged();
    let _ = crate::testgit::run(&fixture.root, &["merge", "side"]);
    fixture.refused("git merge --abort");

    run(&fixture.root, &["merge", "--abort"]);

    let got = standing(&fixture.root);
    assert_ne!(got.stage, Stage::Blocked, "{got:?}");
    assert!(got.conflicts.is_empty(), "{got:?}");
}

// --------------------------------------------------------------------------------------- //
// the small answers the rest is built from (#464)                                           //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_stage_is_named_by_the_word_the_window_reads() {
    // `app/src/saving.ts` switches on these strings; a stage the window cannot name is shown
    // as nothing at all.
    let words: Vec<&str> = [
        Stage::Blocked,
        Stage::Changed,
        Stage::Committed,
        Stage::PrOpen,
        Stage::Saved,
    ]
    .into_iter()
    .map(Stage::word)
    .collect();
    assert_eq!(
        words,
        ["blocked", "changed", "committed", "pr-open", "saved"]
    );
}

#[test]
fn a_message_naming_every_group_it_has_counts_none_as_more() {
    let staged: Vec<String> = [
        "personas/steward/memory/a.md",
        "personas/_dispatch/b.jsonl",
        "workspaces/ide/todos/c.md",
        "README.md",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(
        summary(&staged, "charter"),
        "charter save: 4 files (README.md 1, dispatch 1, ide todos 1, steward memory 1)"
    );
}

#[test]
fn a_save_counts_skill_changes_by_the_skill_they_are_of() {
    let staged: Vec<String> = [
        "skills/deploy/SKILL.md",
        "skills/deploy.toml",
        "workspaces/ops/skills/review/SKILL.md",
        "personas/steward/skills/triage/scripts/run.sh",
        "workspaces/ops/todos/c.md",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(
        summary(&staged, "purlis"),
        "purlis save: 5 files (skill deploy 2, ops skill review 1, ops todos 1, steward skill \
         triage 1)"
    );
}

#[test]
fn the_signers_words_are_kept_and_gits_progress_hints_and_bare_prefixes_are_not() {
    let err = "Rebasing (1/1)\rerror: gpg failed to sign the data\n\
               \n\
               hint: Could not execute the todo command\n\
               error:\n\
               error: failed to write commit object\n";
    assert_eq!(signer_said(err), "gpg failed to sign the data");
    assert_eq!(
        signer_said("hint: nothing but a hint\nfailed to write commit object"),
        "the signer gave no reason"
    );
}

#[test]
fn a_claim_asked_for_within_a_bound_is_had_once_its_holder_lets_go_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let held = Claim::of(&root).expect("nobody holds it yet");
    assert!(
        Claim::within(&root, Duration::from_millis(120)).is_none(),
        "a claim still held when the bound runs out is not had"
    );
    let letting_go = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        drop(held);
    });
    let started = std::time::Instant::now();
    let had = Claim::within(&root, Duration::from_secs(10));
    letting_go.join().unwrap();
    assert!(had.is_some(), "the holder let go well inside the bound");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "it waited {:?} for a claim let go after 150 ms",
        started.elapsed()
    );
}

#[test]
fn the_push_record_is_stamped_with_the_time_it_is_now() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let stamped = now();
    assert!(
        (stamped - before).abs() < 60.0,
        "stamped {stamped}, and it is {before}"
    );
}

// ------------------------------------------------------------------------------------------ //
// One shared standing per plane (FD-11, #651).
// ------------------------------------------------------------------------------------------ //

/// How many files the shared-standing fixture holds: a few thousand by default, and the 300,000
/// of the ticket's monorepo when `CHARTER_STANDING_FILES` says so (`stress.yml`).
fn fixture_files() -> usize {
    std::env::var("CHARTER_STANDING_FILES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(2_000)
}

/// A plane holding `files` committed files in directories of a thousand, and one changed.
fn a_large_plane(files: usize) -> Fixture {
    let fixture = Fixture::plane();
    for n in 0..files {
        let dir = fixture.root.join(format!("tree/{:04}", n / 1000));
        if n % 1000 == 0 {
            std::fs::create_dir_all(&dir).unwrap();
        }
        std::fs::write(dir.join(format!("{n}.txt")), "x\n").unwrap();
    }
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "many"]);
    std::fs::write(fixture.root.join("README.md"), "changed\n").unwrap();
    fixture
}

/// `calls` callers on `threads` threads at once, each asking `ask` of the plane.
fn at_once(threads: usize, calls: usize, ask: &(dyn Fn() + Sync)) {
    let start = std::sync::Barrier::new(threads);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                start.wait();
                for _ in 0..calls {
                    ask();
                }
            });
        }
    });
}

#[test]
fn pollers_at_once_without_the_shared_standing_run_git_in_the_plane_side_by_side() {
    // The measurement's own control: the four pollers each reading the standing for itself
    // overlap in one repo, which is what the shared standing below is measured against.
    let plane = a_large_plane(fixture_files());
    let before = git::tally(&plane.root);

    at_once(4, 2, &|| {
        standing(&plane.root);
    });

    let after = git::tally(&plane.root);
    assert!(after.most_at_once > 1, "{after:?}");
    assert!(
        after.spawned - before.spawned >= 8 * 5,
        "{before:?} {after:?}"
    );
}

#[test]
fn pollers_at_once_share_one_standing_and_never_run_git_in_the_plane_two_at_a_time() {
    let plane = a_large_plane(fixture_files());
    let one = {
        let before = git::tally(&plane.root).spawned;
        standing(&plane.root);
        git::tally(&plane.root).spawned - before
    };
    let before = git::tally(&plane.root);

    // The four pollers: auto-save's look, the title bar, the Saving tab and the alerts, which
    // read the root's dirt from the same standing (FD-11).
    at_once(4, 2, &|| {
        assert_eq!(shared_standing(&plane.root).stage, Stage::Changed);
        let alerts = crate::alerts::read(&crate::alerts::Asking {
            root: &plane.root,
            active: None,
            standing: &plane.root,
            shared: true,
        });
        assert!(alerts.stopped.is_none(), "{alerts:?}");
    });

    let after = git::tally(&plane.root);
    let statuses = git::tally::asked(&plane.root)
        .iter()
        .filter(|argv| argv.iter().any(|arg| arg == "status"))
        .count();
    assert_eq!(
        statuses, 2,
        "one for the lone standing, one shared by every poller"
    );
    assert_eq!(
        after.most_at_once, 1,
        "two git processes ran in the plane at once"
    );
    assert!(
        after.spawned - before.spawned <= one,
        "eight reads cost {} git processes, one costs {one}",
        after.spawned - before.spawned
    );
}

#[test]
fn a_commit_is_in_the_next_shared_standing_without_anyone_saying_so() {
    let plane = Fixture::plane();
    std::fs::write(plane.root.join("README.md"), "two\n").unwrap();
    assert_eq!(shared_standing(&plane.root).stage, Stage::Changed);

    run(&plane.root, &["commit", "-q", "-am", "two"]);

    assert_ne!(shared_standing(&plane.root).stage, Stage::Changed);
}

#[test]
fn an_edit_is_in_the_next_shared_standing_once_the_watcher_says_the_plane_moved() {
    let plane = Fixture::plane();
    assert!(shared_standing(&plane.root).changed.is_empty());

    std::fs::write(plane.root.join("README.md"), "two\n").unwrap();
    touch(&plane.root);

    assert_eq!(
        shared_standing(&plane.root).changed,
        vec!["README.md".to_owned()]
    );
}

#[test]
fn auto_saves_fingerprint_of_a_standing_already_read_runs_no_git() {
    let plane = Fixture::plane();
    std::fs::write(plane.root.join("README.md"), "two\n").unwrap();
    let read = standing(&plane.root);
    let before = git::tally(&plane.root).spawned;

    let seen = fingerprint_of(&plane.root, &read);

    assert_eq!(git::tally(&plane.root).spawned, before);
    assert!(
        seen.starts_with(run(&plane.root, &["rev-parse", "HEAD"]).trim()),
        "{seen}"
    );
}

#[test]
fn a_save_leaves_the_index_carrying_gits_untracked_cache() {
    // `core.untrackedCache` (FD-11): the save's `git add` already writes the index, so it is the
    // write that adds the cache. charter's own reads never write it (`git::UNTRACKED_CACHE`).
    let fixture = Fixture::plane();
    fixture.with_settings("[plane]\nmode = \"commit\"\n");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    let index = std::fs::read(fixture.root.join(".git/index")).unwrap();
    assert!(
        index.windows(4).any(|w| w == b"UNTR"),
        "no untracked cache in the index"
    );
}

#[test]
fn the_untracked_cache_is_asked_for_only_where_the_operators_config_says_nothing() {
    // A `-c` on the command line beats every config file, so charter passes one only when no
    // config sets `core.untrackedCache`: an operator's `false` (or `keep`) is theirs.
    let fixture = Fixture::plane();
    assert_eq!(
        git::untracked_cache(&fixture.root),
        Some(git::UNTRACKED_CACHE)
    );

    run(&fixture.root, &["config", "core.untrackedCache", "false"]);
    assert_eq!(git::untracked_cache(&fixture.root), None);

    run(&fixture.root, &["config", "core.untrackedCache", "keep"]);
    assert_eq!(git::untracked_cache(&fixture.root), None);
}

// --------------------------------------------------------------------------------------- //
// provenance trailers (GL-8, V67)                                                           //
// --------------------------------------------------------------------------------------- //

fn an_agent_run() -> crate::provenance::Provenance {
    crate::provenance::Provenance {
        harness: Some("claude-code".into()),
        model: Some("claude-opus-5-5".into()),
        chat: Some("01J9ZQ3W5Y7X8V6T4R2P0N1M3K".into()),
        persona: Some("steward".into()),
        change: None,
    }
}

impl Fixture {
    fn save_for(&self, message: &str, provenance: Option<&crate::provenance::Provenance>) -> u8 {
        std::fs::write(self.root.join("notes.md"), message).unwrap();
        let (code, said) = self.save(Request {
            root: &self.root,
            message: Some(message),
            sign: false,
            no_push: true,
            cwd: &self.root,
            provenance,
        });
        assert_eq!(code, 0, "{said}");
        code
    }

    /// HEAD's trailers, as git itself parses them.
    fn head_trailers(&self) -> String {
        run(
            &self.root,
            &["log", "-1", "--format=%(trailers:only,unfold)"],
        )
        .trim()
        .to_owned()
    }
}

#[test]
fn a_save_an_agent_run_asked_for_carries_its_trailers_after_the_operators_own() {
    let fixture = Fixture::plane();
    let run_ = an_agent_run();
    fixture.save_for(
        "memory: one lesson\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
        Some(&run_),
    );
    assert_eq!(
        fixture.head_trailers(),
        "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\
         Assisted-by: claude-code:claude-opus-5-5\n\
         Purlis-Chat: 01J9ZQ3W5Y7X8V6T4R2P0N1M3K\n\
         Purlis-Persona: steward"
    );
    assert_eq!(fixture.head_subject(), "memory: one lesson");
}

#[test]
fn a_save_nobody_ran_for_an_agent_carries_no_trailer() {
    let fixture = Fixture::plane();
    fixture.save_for("by hand", None);
    assert_eq!(fixture.head_trailers(), "");
}

#[test]
fn a_plane_that_follows_the_kernel_says_assisted_by_llm() {
    let fixture = Fixture::plane();
    std::fs::write(
        fixture.root.join("charter.toml"),
        "[plane]\nname = \"fixture\"\nassisted_by = \"llm\"\n",
    )
    .unwrap();
    let run_ = an_agent_run();
    fixture.save_for("a save", Some(&run_));
    assert!(
        fixture
            .head_trailers()
            .starts_with("Assisted-by: LLM\nPurlis-Chat: "),
        "{}",
        fixture.head_trailers()
    );
}

#[test]
fn a_projects_trailer_config_runs_nothing_and_the_agents_own_lines_stay_as_written() {
    let fixture = Fixture::plane();
    let ran = fixture.root.parent().unwrap().join("trailer command ran");
    run(
        &fixture.root,
        &[
            "config",
            "trailer.foo.command",
            &format!("touch '{}'; echo v", ran.display()),
        ],
    );
    run(
        &fixture.root,
        &["config", "trailer.Assisted-by.ifexists", "replace"],
    );
    let run_ = an_agent_run();
    fixture.save_for(
        "a save\n\nCo-authored-by:x\nRefs:\nhttps://example.com/a",
        Some(&run_),
    );

    assert!(!ran.exists(), "a trailer command ran");
    let body = run(&fixture.root, &["log", "-1", "--format=%B"]);
    assert!(
        body.starts_with("a save\n\nCo-authored-by:x\nRefs:\nhttps://example.com/a\nAssisted-by: "),
        "{body}"
    );
    assert!(!body.contains("foo:"), "{body}");
}

// --------------------------------------------------------------------------------------- //
// what holds incoming commits off, one reason at a time (#464)                              //
// --------------------------------------------------------------------------------------- //

#[test]
fn incoming_commits_are_held_off_by_a_commit_of_the_planes_own() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);

    let got = fetch(&fixture.root, true).expect("fetched");

    assert!(matches!(got.held, Some(Held::Ahead(1))), "{got:?}");
    assert!(!got.moved, "{got:?}");
}

/// Under a chat's sandbox, incoming commits that change a file the sandbox denies are left
/// on the remote-tracking branch: git would write the files it may, skip that one, and leave
/// the project half-updated with HEAD unmoved (qw346). Outside one they come in as before.
#[test]
fn incoming_commits_a_chats_sandbox_would_stop_partway_are_left_for_the_window() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let got = {
        let _sandbox = crate::sandbox::held::stand_in(&["theirs.md"]);
        fetch(&fixture.root, true).expect("fetched")
    };

    assert_eq!((got.behind, got.moved), (1, false), "{got:?}");
    let Some(Held::Sandbox(files)) = &got.held else {
        panic!("{got:?}");
    };
    assert_eq!(files, &["theirs.md"]);
    let why = got.held.as_ref().unwrap().why();
    assert!(
        why.starts_with("this chat's sandbox does not let it write `theirs.md`"),
        "{why}"
    );
    assert!(why.contains("half-updated"), "{why}");
    assert!(
        why.contains("`purlis save --pull` in their own terminal"),
        "{why}"
    );
    // Nothing was started: the commit, the tree and git's own state are as they were.
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert!(
        !fixture.root.join("theirs.md").exists(),
        "a file was written"
    );
    assert_eq!(ask(&fixture.root, &["status", "--porcelain"]), "");

    // A sandbox that denies none of the files the commits change holds nothing off.
    let got = {
        let _sandbox = crate::sandbox::held::stand_in(&["other.md"]);
        fetch(&fixture.root, true).expect("fetched")
    };
    assert_eq!((got.moved, &got.held), (true, &None), "{got:?}");
    assert!(fixture.root.join("theirs.md").is_file());
}

/// `purlis save --pull` under a chat's sandbox says why nothing came in, and does not stop
/// the save that follows: what it commits is still the person's work (qw346).
#[test]
fn a_pull_a_chats_sandbox_would_stop_partway_says_so_and_brings_nothing_in() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    let mut said = String::new();
    let code = {
        let _sandbox = crate::sandbox::held::stand_in(&["theirs.md"]);
        pull(&fixture.root, &mut |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        })
    };
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains(
            "1 incoming commit not brought in: this chat's sandbox does not let it write \
             `theirs.md`"
        ),
        "{said}"
    );
    assert!(
        !fixture.root.join("theirs.md").exists(),
        "a file was written"
    );
}

/// A save whose push the remote refuses because it moved rebases onto it. Under a chat's
/// sandbox that rebase is not started where it would stop partway: the commit stays local,
/// nothing is called a conflict, and the tree is left whole (qw346).
#[test]
fn a_saves_rebase_a_chats_sandbox_would_stop_partway_is_not_started() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    let mine = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let (pushed, said) = {
        let _sandbox = crate::sandbox::held::stand_in(&["theirs.md"]);
        push_within(&fixture, false, WRITE)
    };

    assert_eq!(pushed.outcome, Outcome::Failed, "{said}");
    assert!(
        said.contains(
            "Committed locally, but this chat's sandbox does not let it write `theirs.md`"
        ),
        "{said}"
    );
    assert!(said.contains("Nothing was rebased or pushed."), "{said}");
    assert!(!said.contains("conflict"), "{said}");
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), mine);
    assert!(
        !fixture.root.join("theirs.md").exists(),
        "a file was written"
    );
    assert!(fixture.root.join("mine.md").is_file());
    assert_eq!(ask(&fixture.root, &["status", "--porcelain"]), "");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "theirs"
    );
    let record = push_record(&fixture.root).expect("a record");
    assert_eq!(record["outcome"], "failed");

    // The same save from the window, under no sandbox, lands it.
    let (pushed, said) = push_within(&fixture, false, WRITE);
    assert_eq!(pushed.outcome, Outcome::Pushed, "{said}");
    assert!(fixture.root.join("theirs.md").is_file());
}

#[test]
fn incoming_commits_wait_while_another_git_holds_the_index() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join(".git/index.lock"), "").unwrap();

    let got = fetch(&fixture.root, true).expect("fetched");

    assert!(matches!(got.held, Some(Held::IndexLocked)), "{got:?}");
    assert!(!got.moved, "{got:?}");
}

#[test]
fn a_fast_forward_git_itself_refuses_is_said_in_its_words_and_nothing_is_called_moved() {
    // Their commit adds a file this clone ignores (its own `info/exclude`) and has a copy of:
    // `status` does not list an ignored file, so nothing holds the fast-forward off, and
    // `merge --no-overwrite-ignore` then refuses to put their file over it.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );
    let theirs = fixture.root.parent().unwrap().join("theirs");
    run(
        fixture.root.parent().unwrap(),
        &[
            "clone",
            "-q",
            &bare.display().to_string(),
            &theirs.display().to_string(),
        ],
    );
    run(&theirs, &["config", "user.name", "Other"]);
    run(&theirs, &["config", "user.email", "other@example.invalid"]);
    std::fs::write(theirs.join("local.txt"), "theirs").unwrap();
    run(&theirs, &["add", "-A"]);
    run(&theirs, &["commit", "-q", "-m", "theirs"]);
    run(&theirs, &["push", "-q", "origin", "main"]);
    std::fs::create_dir_all(fixture.root.join(".git/info")).unwrap();
    std::fs::write(fixture.root.join(".git/info/exclude"), "local.txt\n").unwrap();
    std::fs::write(fixture.root.join("local.txt"), "mine, ignored").unwrap();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);

    let got = fetch(&fixture.root, true).expect("fetched");

    assert_eq!(got.behind, 1, "{got:?}");
    assert!(!got.moved, "{got:?}");
    assert!(matches!(got.held, Some(Held::Refused(_))), "{got:?}");
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("local.txt")).unwrap(),
        "mine, ignored"
    );
}

#[test]
fn a_request_mode_on_a_forge_charter_knows_and_a_push_mode_anywhere_carry_no_notice() {
    // The notice is for a request mode charter cannot open a request in, and for nothing else.
    let fixture = Fixture::plane();
    fixture.with_a_remote();
    fixture.with_settings("[plane]\nmode = \"pr\"\n");
    let got = standing(&fixture.root);
    assert_eq!(got.notice, None, "{got:?}");

    let fixture = Fixture::plane();
    run(
        &fixture.root,
        &["remote", "add", "origin", "git@git.corp:team/plane.git"],
    );
    fixture.with_settings("[plane]\nmode = \"push\"\n");
    let got = standing(&fixture.root);
    assert_eq!(got.notice, None, "{got:?}");
}

#[test]
fn a_merge_stopped_with_nothing_unmerged_leaves_the_last_pushs_conflicts_to_settle() {
    // The files a stopped merge left unmerged are the ones to settle now; a merge stopped with
    // none (`--no-commit`) has none to offer, so the record's list is still the answer.
    let fixture = Fixture::plane();
    run(&fixture.root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(fixture.root.join("side.md"), "side").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "side"]);
    run(&fixture.root, &["checkout", "-q", "main"]);
    let head = ask(&fixture.root, &["rev-parse", "HEAD"])
        .trim()
        .to_string();
    record_push(
        &fixture.root,
        PushResult {
            conflicts: vec!["theirs.md".into()],
            ..PushResult::of(Outcome::Conflict, "main")
        },
        &head,
    );
    run(
        &fixture.root,
        &["merge", "-q", "--no-commit", "--no-ff", "side"],
    );

    let got = standing(&fixture.root);

    assert_eq!(got.stage, Stage::Blocked, "{got:?}");
    assert_eq!(got.conflicts, ["theirs.md"], "{got:?}");
}

// --------------------------------------------------------------------------------------- //
// the pull-request path and the rebase, at their edges (#464)                               //
// --------------------------------------------------------------------------------------- //

/// A bare remote whose pre-receive hook refuses `refs/heads/main` in GitHub's words, and runs
/// `otherwise` for every other branch.
fn protect_main(bare: &Path, otherwise: &str) {
    std::fs::create_dir_all(bare.join("hooks")).unwrap();
    stand_in::program(
        bare,
        "hooks/pre-receive",
        &format!(
            "#!/bin/sh\nwhile read _ _ ref; do\n  case \"$ref\" in refs/heads/main)\n    echo \
             'remote: error: GH006: Protected branch update failed for refs/heads/main.' >&2\n    \
             exit 1;; esac\ndone\n{otherwise}\n"
        ),
    );
}

#[test]
fn a_charter_branch_a_push_before_the_rename_left_waiting_is_advanced_and_no_purlis_one_made() {
    // V93j: the open request is from `charter/<sha>`, as a push before the rename named it.
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    protect_main(&bare, "exit 0");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");
    let fresh = push_record(&fixture.root).expect("a record")["landed"]
        .as_str()
        .expect("a branch")
        .to_owned();
    let old = fresh.replacen("purlis/", "charter/", 1);
    assert_ne!(old, fresh);
    run(&bare, &["branch", "-m", &fresh, &old]);
    let record = std::fs::read_to_string(push_record_path(&fixture.root)).unwrap();
    std::fs::write(
        push_record_path(&fixture.root),
        record.replace(&fresh, &old),
    )
    .unwrap();

    std::fs::write(fixture.root.join("more.md"), "more").unwrap();
    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert_eq!(
        push_record(&fixture.root).expect("a record")["landed"],
        old.as_str()
    );
    let branches = ask(
        &bare,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/purlis",
            "refs/heads/charter",
        ],
    );
    assert_eq!(branches.trim(), old, "one branch, one request");
    assert_eq!(
        ask(&bare, &["rev-parse", &old]),
        ask(&fixture.root, &["rev-parse", "HEAD"]),
        "carrying both saves"
    );
}

#[test]
fn a_second_save_past_a_protected_main_advances_the_branch_its_open_request_is_from() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    protect_main(&bare, "exit 0");
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();
    let (code, said) = fixture.just_save();
    assert_eq!(code, 0, "{said}");
    let first = push_record(&fixture.root).expect("a record")["landed"]
        .as_str()
        .expect("a branch")
        .to_owned();

    std::fs::write(fixture.root.join("more.md"), "more").unwrap();
    let (code, said) = fixture.just_save();

    assert_eq!(code, 0, "{said}");
    assert_eq!(
        push_record(&fixture.root).expect("a record")["landed"],
        first.as_str()
    );
    let branches = ask(
        &bare,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/purlis",
            "refs/heads/charter",
        ],
    );
    assert_eq!(branches.trim(), first, "one branch, one request");
    assert_eq!(
        ask(&bare, &["rev-parse", &first]),
        ask(&fixture.root, &["rev-parse", "HEAD"]),
        "carrying both saves"
    );
}

#[test]
fn a_commit_no_branch_would_take_is_stranded_and_the_record_says_what_the_remote_said() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    protect_main(
        &bare,
        "echo 'remote: branch pushes are closed today' >&2\nexit 1",
    );
    std::fs::write(fixture.root.join("work.md"), "work").unwrap();

    let (_, said) = fixture.just_save();

    assert!(said.contains("also failed"), "{said}");
    let record = push_record(&fixture.root).expect("a record");
    assert_eq!(record["outcome"], "stranded", "{record}");
    assert!(
        record["detail"]
            .as_str()
            .is_some_and(|d| d.contains("branch pushes are closed today")),
        "{record}"
    );
}

#[test]
fn a_signed_rebase_that_really_conflicts_is_a_conflict_and_not_the_signers_refusal() {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    std::fs::write(fixture.root.join("theirs.md"), "mine, not theirs").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    run(
        &fixture.root,
        &["fetch", "-q", &bare.display().to_string(), "main"],
    );

    let rebased = rebase_onto_fetched(&fixture.root, true, Duration::from_secs(30));

    assert_eq!(rebased, Rebased::Conflict(vec!["theirs.md".into()]));
}

#[test]
fn a_rebase_git_did_not_finish_for_a_reason_of_its_own_is_not_called_out_of_time() {
    // Killed by a signal that is not charter's — here a filter that kills the git running it —
    // git has no exit code, and it is still not charter's deadline that stopped it.
    let fixture = Fixture::plane();
    let bare = a_remote_no_rebase_can_finish_onto(&fixture);
    run(
        &fixture.root,
        &["fetch", "-q", &bare.display().to_string(), "main"],
    );

    let rebased = with_a_smudge("kill -9 $PPID; cat", || {
        rebase_onto_fetched(&fixture.root, false, Duration::from_secs(30))
    });

    // Nor a conflict (qw348): nothing is unmerged, so it is git's own failure, in its words.
    assert!(matches!(rebased, Rebased::Failed(_)), "{rebased:?}");
}

#[test]
fn a_breakdown_names_every_directory_it_can_show_and_counts_only_the_ones_it_cannot() {
    let said_for = |dirs: usize| {
        let staged: Vec<String> = (0..dirs).map(|n| format!("d{n:02}/f.md")).collect();
        let mut said = String::new();
        let mut say = |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        };
        show_what_is_staged(Path::new("/plane"), &staged, &mut say);
        said
    };

    let all = said_for(BREAKDOWN_ROWS);
    assert!(
        all.contains(&format!("d{:02}", BREAKDOWN_ROWS - 1)),
        "{all}"
    );
    assert!(!all.contains("more directories"), "{all}");

    let more = said_for(BREAKDOWN_ROWS + 1);
    assert!(more.contains("… and 1 more directories"), "{more}");
}

#[test]
fn an_empty_message_is_no_message_and_the_save_says_what_changed_itself() {
    let fixture = Fixture::plane();
    std::fs::write(fixture.root.join("personas/steward/memory/a.md"), "x").unwrap();
    let (code, said) = fixture.save(Request {
        root: &fixture.root,
        message: Some(""),
        sign: false,
        no_push: true,
        cwd: &fixture.root,
        provenance: None,
    });
    assert_eq!(code, 0, "{said}");
    assert_eq!(
        fixture.head_subject(),
        "charter save: 1 file (steward memory 1)"
    );
}

#[test]
fn the_standing_reads_an_unchanged_file_once_and_a_changed_one_again() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("workspace.json");
    let asked = std::cell::Cell::new(0);
    let ask = |answer: bool| {
        asked.set(asked.get() + 1);
        answer
    };
    let write = |text: &str, age: u64| {
        std::fs::write(&file, text).unwrap();
        let at = SystemTime::now() - Duration::from_secs(age);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(at)
            .unwrap();
    };
    write("{}", 60);
    assert!(!remembered(&file, || ask(false)));
    assert!(
        !remembered(&file, || ask(true)),
        "an unchanged file is not read again"
    );
    assert_eq!(asked.get(), 1);

    write("{\"a\": 1}", 61);
    assert!(
        remembered(&file, || ask(true)),
        "a changed file is read again"
    );
    assert_eq!(asked.get(), 2);

    // The same length, written again.
    write("{\"b\": 1}", 62);
    assert!(!remembered(&file, || ask(false)));
    assert_eq!(asked.get(), 3);

    // Modified a moment ago: its times may not have moved yet, so it is asked every time.
    std::fs::write(&file, "{\"c\": 1}").unwrap();
    assert!(remembered(&file, || ask(true)));
    assert!(remembered(&file, || ask(true)));
    assert_eq!(asked.get(), 5);

    // A file whose metadata cannot be read is asked every time.
    let gone = dir.path().join("gone.json");
    assert!(remembered(&gone, || ask(true)));
    assert!(remembered(&gone, || ask(true)));
    assert_eq!(asked.get(), 7);
}

// --------------------------------------------------------------------------------------- //
// a push record that cannot be written is said (#1144)                                      //
// --------------------------------------------------------------------------------------- //

#[test]
fn a_push_record_that_cannot_be_written_is_carried_on_the_result_and_said() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // A directory where the record goes: no write lands there, as on a full disk.
    std::fs::create_dir_all(push_record_path(root)).unwrap();

    let blocked = record_push(root, PushResult::of(Outcome::Blocked, "main"), "abc");
    let pushed = record_push(root, PushResult::of(Outcome::Pushed, "main"), "abc");
    let mut said = Vec::new();
    let told = said_if_unrecorded(blocked.clone(), &mut |line| said.push(line));

    assert!(blocked.unrecorded.is_some(), "{blocked:?}");
    assert!(
        pushed.unrecorded.is_some(),
        "a stale record left in place is as wrong as one never written: {pushed:?}"
    );
    assert_eq!(told, blocked, "saying it changes nothing of the result");
    assert!(
        matches!(said.as_slice(), [Say::Warn(line)] if line.contains("not recorded")),
        "{said:?}"
    );
}

#[test]
fn a_push_record_written_or_cleared_says_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let blocked = record_push(root, PushResult::of(Outcome::Blocked, "main"), "abc");
    let pushed = record_push(root, PushResult::of(Outcome::Pushed, "main"), "abc");
    let mut said = Vec::new();
    said_if_unrecorded(blocked.clone(), &mut |line| said.push(line));

    assert_eq!(blocked.unrecorded, None);
    assert_eq!(pushed.unrecorded, None);
    assert!(!push_record_path(root).exists(), "a clean push clears it");
    assert!(said.is_empty(), "{said:?}");
}

/// A token-shaped value, built where it is used so no source file holds one.
fn token() -> String {
    ["ghp", "_0123456789abcdefABCDEF"].concat()
}

/// A plane on a stand-in forge that already has the plane's commit, and knows it has it.
fn published() -> (Fixture, PathBuf) {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote();
    run(
        &fixture.root,
        &[
            "push",
            "-q",
            &bare.display().to_string(),
            "HEAD:refs/heads/main",
        ],
    );
    run(
        &fixture.root,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    (fixture, bare)
}

/// SQ-7: a commit made by hand in the plane, then pushed by a save, is scanned before it is
/// pushed. A credential in its files or its message stops the push; nothing reaches the remote,
/// the value is never said, and the outcome is recorded as blocked.
#[test]
fn a_saves_push_of_a_hand_made_commit_holding_a_credential_is_refused() {
    for (file, message) in [
        (format!("T={}\n", token()), "by hand".to_owned()),
        ("plain\n".to_owned(), format!("by hand\n\nwith {}", token())),
    ] {
        let (fixture, bare) = published();
        let before = ask(&bare, &["rev-parse", "main"]);
        std::fs::write(fixture.root.join("notes.md"), &file).unwrap();
        run(&fixture.root, &["add", "-A"]);
        run(&fixture.root, &["commit", "-q", "-m", &message]);

        let (pushed, said) = push(&fixture, false);

        assert_eq!(pushed.outcome, Outcome::Blocked, "{said}");
        assert_eq!(pushed.detail, PUSH_SECRET_REFUSED);
        assert!(said.contains("Not pushed"), "{said}");
        assert!(!said.contains(&token()), "{said}");
        assert_eq!(ask(&bare, &["rev-parse", "main"]), before, "{said}");
        assert_eq!(
            push_record(&fixture.root).unwrap()["outcome"],
            "blocked",
            "{said}"
        );
    }
}

/// Train 77 review: a scan git cannot answer refuses the save's push (fail closed). The blob a
/// hand-made commit adds is taken out of the object store, so the push scan cannot read it:
/// nothing is pushed, the outcome is blocked, and the save says it could not scan.
#[test]
fn a_saves_push_whose_scan_git_cannot_answer_is_refused() {
    let (fixture, bare) = published();
    let before = ask(&bare, &["rev-parse", "main"]);
    std::fs::write(
        fixture.root.join("notes.md"),
        "plain, and unreadable soon\n",
    )
    .unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "by hand"]);
    let blob = ask(&fixture.root, &["rev-parse", "HEAD:notes.md"]);
    let blob = blob.trim();
    let objects = fixture.root.join(".git/objects");
    std::fs::remove_file(objects.join(&blob[..2]).join(&blob[2..])).expect("a loose blob");

    let (pushed, said) = push(&fixture, false);

    assert_eq!(pushed.outcome, Outcome::Blocked, "{said}");
    assert!(
        pushed.detail.starts_with("purlis could not scan"),
        "{}: {said}",
        pushed.detail
    );
    assert_eq!(ask(&bare, &["rev-parse", "main"]), before, "{said}");
    assert_eq!(push_record(&fixture.root).unwrap()["outcome"], "blocked");
}

/// The save's own scan lets personal data through — a plane's memory names people — so its push
/// does not refuse it either, and a clean commit made by hand is pushed as before.
#[test]
fn a_saves_push_of_personal_data_or_of_nothing_secret_goes_on() {
    let (fixture, bare) = published();
    std::fs::write(
        fixture.root.join("personas/steward/memory/people.md"),
        format!("Ada is {}\n", ["ada", "@lovelace.dev"].concat()),
    )
    .unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(
        &fixture.root,
        &[
            "commit",
            "-q",
            "-m",
            &format!(
                "people\n\nCo-Authored-By: Ada <{}>",
                ["ada", "@lovelace.dev"].concat()
            ),
        ],
    );

    let (pushed, said) = push(&fixture, false);

    assert_eq!(pushed.outcome, Outcome::Pushed, "{said}");
    assert_eq!(
        ask(&bare, &["rev-parse", "main"]),
        ask(&fixture.root, &["rev-parse", "HEAD"]),
        "{said}"
    );
}

// --------------------------------------------------------------------------------------- //
// resolving a save's conflict from the window (qw382)                                      //
// --------------------------------------------------------------------------------------- //

/// A plane whose own commit changes `theirs.md` the other way from the remote's, so a replay
/// of it stops on a conflict there.
fn a_plane_that_conflicts_with_its_remote() -> (Fixture, PathBuf) {
    let fixture = Fixture::plane();
    let bare = fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("theirs.md"), "mine, not theirs").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    (fixture, bare)
}

fn resolve_start(fixture: &Fixture) -> resolve::Settled {
    resolve::start(&fixture.root, &mut |_: Say| {})
}

fn resolve_continue(fixture: &Fixture, shown: &[&str]) -> resolve::Settled {
    let shown: Vec<String> = shown.iter().map(|s| (*s).to_string()).collect();
    resolve::carry_on(&fixture.root, &shown, &mut |_: Say| {})
}

#[test]
fn resolve_stops_on_the_conflict_and_continue_lands_it_once_the_markers_are_gone() {
    let (fixture, bare) = a_plane_that_conflicts_with_its_remote();

    assert_eq!(
        resolve_start(&fixture),
        resolve::Settled::Stopped(vec!["theirs.md".into()])
    );
    assert_eq!(standing(&fixture.root).stopped_by, Some("rebase"));
    assert_eq!(
        resolve::sides(&fixture.root),
        Some(("theirs".to_string(), "mine".to_string()))
    );

    // The markers are still there: nothing is staged, and the rebase stays stopped.
    let refused = resolve_continue(&fixture, &["theirs.md"]);
    assert!(
        matches!(&refused, resolve::Settled::Refused(why) if why.contains("conflict markers")),
        "{refused:?}"
    );
    assert_eq!(
        gitstate::unmerged(&fixture.root),
        vec!["theirs.md".to_string()]
    );

    std::fs::write(fixture.root.join("theirs.md"), "both").unwrap();
    assert_eq!(
        resolve_continue(&fixture, &["theirs.md"]),
        resolve::Settled::Replayed
    );
    assert_eq!(
        run(&fixture.root, &["log", "--format=%s", "-2"]).trim(),
        "mine\ntheirs"
    );
    let left = gitstate::stopped(&fixture.root);
    assert!(left.is_none(), "{left:?}");

    let (code, said) = pressed_save(&fixture);
    assert_eq!(code, 0, "{said}");
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "mine",
        "{said}"
    );
}

#[test]
fn continue_stages_only_files_the_replayed_commit_changes() {
    let (fixture, _) = a_plane_that_conflicts_with_its_remote();
    resolve_start(&fixture);
    std::fs::write(fixture.root.join("theirs.md"), "both").unwrap();

    // A file the commit being replayed does not change is never staged by the window.
    let outside = resolve_continue(&fixture, &["theirs.md", "README.md"]);
    assert!(
        matches!(&outside, resolve::Settled::Refused(why) if why.contains("`README.md`")),
        "{outside:?}"
    );
    // Nor is a commit continued with something else staged beside it.
    std::fs::write(fixture.root.join("other.md"), "other").unwrap();
    run(&fixture.root, &["add", "other.md"]);
    let staged = resolve_continue(&fixture, &["theirs.md"]);
    assert!(
        matches!(&staged, resolve::Settled::Refused(why) if why.contains("`other.md`")),
        "{staged:?}"
    );
    assert!(gitstate::stopped(&fixture.root).is_some(), "still stopped");
}

#[test]
fn abort_puts_the_plane_back_where_it_was() {
    let (fixture, _) = a_plane_that_conflicts_with_its_remote();
    let before = ask(&fixture.root, &["rev-parse", "HEAD"]);
    resolve_start(&fixture);

    assert_eq!(resolve::abort(&fixture.root), resolve::Settled::Aborted);

    assert!(gitstate::stopped(&fixture.root).is_none());
    assert_eq!(ask(&fixture.root, &["rev-parse", "HEAD"]), before);
    assert!(matches!(
        resolve::abort(&fixture.root),
        resolve::Settled::Refused(_)
    ));
}

#[test]
fn resolve_with_nothing_in_conflict_replays_and_forgets_the_record() {
    let fixture = Fixture::plane();
    fixture.with_a_remote_that_moved();
    tracks_what_it_pushed(&fixture);
    std::fs::write(fixture.root.join("mine.md"), "mine").unwrap();
    run(&fixture.root, &["add", "-A"]);
    run(&fixture.root, &["commit", "-q", "-m", "mine"]);
    a_conflict_that_names_nothing(&fixture);

    assert_eq!(resolve_start(&fixture), resolve::Settled::Replayed);
    assert!(push_record(&fixture.root).is_none());
}

/// A plane and its remote with a shared base, and a clone of the remote standing in for
/// somebody else: what the conflicts below are made of.
struct Scene {
    fixture: Fixture,
    theirs: PathBuf,
}

impl Scene {
    /// `base` committed on the plane and pushed, and a second clone at it.
    fn new(base: &[(&str, &[u8])]) -> Scene {
        let fixture = Fixture::plane();
        let bare = fixture.with_a_remote();
        for (path, bytes) in base {
            write(&fixture.root, path, bytes);
        }
        run(&fixture.root, &["add", "-A"]);
        run(&fixture.root, &["commit", "-q", "-m", "base"]);
        run(
            &fixture.root,
            &[
                "push",
                "-q",
                &bare.display().to_string(),
                "HEAD:refs/heads/main",
            ],
        );
        tracks_what_it_pushed(&fixture);
        let theirs = fixture.root.parent().unwrap().join("theirs");
        run(
            fixture.root.parent().unwrap(),
            &[
                "clone",
                "-q",
                &bare.display().to_string(),
                &theirs.display().to_string(),
            ],
        );
        run(&theirs, &["config", "user.name", "Other"]);
        run(&theirs, &["config", "user.email", "other@example.invalid"]);
        Scene { fixture, theirs }
    }

    /// A commit on the remote, made in the other clone by `change` and pushed.
    fn theirs(&self, subject: &str, change: impl FnOnce(&Path)) {
        change(&self.theirs);
        run(&self.theirs, &["add", "-A"]);
        run(&self.theirs, &["commit", "-q", "-m", subject]);
        run(&self.theirs, &["push", "-q", "origin", "main"]);
    }

    /// A commit of the plane's own, made by `change`.
    fn mine(&self, subject: &str, change: impl FnOnce(&Path)) {
        change(&self.fixture.root);
        run(&self.fixture.root, &["add", "-A"]);
        run(&self.fixture.root, &["commit", "-q", "-m", subject]);
    }

    fn at(&self, path: &str) -> Vec<u8> {
        std::fs::read(self.fixture.root.join(path)).unwrap_or_default()
    }
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, bytes).unwrap();
}

fn refused_with(settled: &resolve::Settled, words: &str) -> bool {
    matches!(settled, resolve::Settled::Refused(why) if why.contains(words))
}

#[test]
fn a_binary_conflict_is_the_persons_to_settle_and_continue_waits_for_it() {
    let scene = Scene::new(&[("logo.bin", b"base\0bytes")]);
    scene.theirs("theirs", |dir| write(dir, "logo.bin", b"theirs\0bytes"));
    scene.mine("mine", |dir| write(dir, "logo.bin", b"mine\0bytes"));

    assert_eq!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(vec!["logo.bin".into()])
    );
    assert_eq!(
        resolve::conflicts(&scene.fixture.root).by_hand,
        vec!["logo.bin".to_string()]
    );
    let refused = resolve_continue(&scene.fixture, &["logo.bin"]);
    assert!(refused_with(&refused, "`logo.bin`"), "{refused:?}");
    assert!(refused_with(&refused, "terminal"), "{refused:?}");

    // The person keeps the project's own side and stages it: that is carried on as staged.
    run(
        &scene.fixture.root,
        &["checkout", "--theirs", "--", "logo.bin"],
    );
    run(&scene.fixture.root, &["add", "--", "logo.bin"]);
    assert_eq!(
        resolve_continue(&scene.fixture, &["logo.bin"]),
        resolve::Settled::Replayed
    );
    assert_eq!(scene.at("logo.bin"), b"mine\0bytes");
}

#[test]
fn a_file_one_side_deleted_is_never_carried_on_with_the_other_sides_copy() {
    let scene = Scene::new(&[("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| {
        std::fs::remove_file(dir.join("a.md")).unwrap()
    });

    assert!(matches!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(_)
    ));
    assert_eq!(
        resolve::conflicts(&scene.fixture.root).by_hand,
        vec!["a.md".to_string()]
    );
    let refused = resolve_continue(&scene.fixture, &["a.md"]);
    assert!(refused_with(&refused, "`a.md`"), "{refused:?}");
    assert!(
        gitstate::stopped(&scene.fixture.root).is_some(),
        "still stopped"
    );
}

#[test]
fn a_change_git_carried_through_a_rename_is_kept_and_continue_accepts_it() {
    let scene = Scene::new(&[("a.md", b"one\n"), ("b.md", b"first\nsecond\n")]);
    scene.theirs("theirs", |dir| {
        write(dir, "a.md", b"theirs\n");
        run(dir, &["mv", "b.md", "c.md"]);
    });
    scene.mine("mine", |dir| {
        write(dir, "a.md", b"mine\n");
        write(dir, "b.md", b"first\nsecond, mine\n");
    });

    assert_eq!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(vec!["a.md".into()])
    );
    write(&scene.fixture.root, "a.md", b"both\n");

    assert_eq!(
        resolve_continue(&scene.fixture, &["a.md"]),
        resolve::Settled::Replayed
    );
    assert_eq!(scene.at("c.md"), b"first\nsecond, mine\n");
}

#[test]
fn a_cleanly_merged_file_staged_otherwise_stops_continue() {
    let scene = Scene::new(&[("a.md", b"one\n"), ("b.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| {
        write(dir, "a.md", b"mine\n");
        write(dir, "b.md", b"mine\n");
    });
    resolve_start(&scene.fixture);
    write(&scene.fixture.root, "a.md", b"both\n");
    write(&scene.fixture.root, "b.md", b"something else\n");
    run(&scene.fixture.root, &["add", "--", "b.md"]);

    let refused = resolve_continue(&scene.fixture, &["a.md"]);

    assert!(refused_with(&refused, "`b.md`"), "{refused:?}");
    assert_eq!(
        gitstate::unmerged(&scene.fixture.root),
        vec!["a.md".to_string()],
        "nothing was staged"
    );
}

#[test]
fn a_replay_of_several_commits_stops_on_each_conflict_in_turn() {
    let scene = Scene::new(&[("x.md", b"one\n"), ("y.md", b"one\n")]);
    scene.theirs("theirs", |dir| {
        write(dir, "x.md", b"theirs\n");
        write(dir, "y.md", b"theirs\n");
    });
    scene.mine("first", |dir| write(dir, "x.md", b"mine\n"));
    scene.mine("second", |dir| write(dir, "y.md", b"mine\n"));

    assert_eq!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(vec!["x.md".into()])
    );
    write(&scene.fixture.root, "x.md", b"both\n");
    assert_eq!(
        resolve_continue(&scene.fixture, &["x.md"]),
        resolve::Settled::Stopped(vec!["y.md".into()])
    );
    // The first stop's files are not the second's.
    assert!(refused_with(
        &resolve_continue(&scene.fixture, &["x.md"]),
        "`x.md`"
    ));
    write(&scene.fixture.root, "y.md", b"both\n");
    assert_eq!(
        resolve_continue(&scene.fixture, &["y.md"]),
        resolve::Settled::Replayed
    );
    assert_eq!(
        run(&scene.fixture.root, &["log", "--format=%s", "-3"]).trim(),
        "second\nfirst\ntheirs"
    );
}

#[cfg(unix)]
#[test]
fn a_conflicted_file_turned_into_a_link_is_not_staged() {
    let scene = Scene::new(&[("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\n"));
    resolve_start(&scene.fixture);
    std::fs::remove_file(scene.fixture.root.join("a.md")).unwrap();
    std::os::unix::fs::symlink("README.md", scene.fixture.root.join("a.md")).unwrap();

    let refused = resolve_continue(&scene.fixture, &["a.md"]);

    assert!(refused_with(&refused, "not a plain file"), "{refused:?}");
}

#[test]
fn file_names_that_start_with_a_dash_or_hold_a_newline_are_staged_as_names() {
    let odd = "line\nbreak.md";
    let scene = Scene::new(&[("-x.md", b"one\n"), (odd, b"one\n")]);
    scene.theirs("theirs", |dir| {
        write(dir, "-x.md", b"theirs\n");
        write(dir, odd, b"theirs\n");
    });
    scene.mine("mine", |dir| {
        write(dir, "-x.md", b"mine\n");
        write(dir, odd, b"mine\n");
    });

    let resolve::Settled::Stopped(mut files) = resolve_start(&scene.fixture) else {
        panic!("it stops on the conflict");
    };
    files.sort();
    assert_eq!(files, vec!["-x.md".to_string(), odd.to_string()]);
    write(&scene.fixture.root, "-x.md", b"both\n");
    write(&scene.fixture.root, odd, b"both\n");
    assert_eq!(
        resolve_continue(&scene.fixture, &["-x.md", odd]),
        resolve::Settled::Replayed
    );
    assert_eq!(scene.at(odd), b"both\n");
}

#[test]
fn a_common_ancestor_marker_left_in_a_file_stops_continue() {
    let scene = Scene::new(&[("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\n"));
    resolve_start(&scene.fixture);
    write(&scene.fixture.root, "a.md", b"mine\n||||||| base\none\n");

    let refused = resolve_continue(&scene.fixture, &["a.md"]);

    assert!(refused_with(&refused, "conflict markers"), "{refused:?}");
}

#[test]
fn a_settling_that_leaves_nothing_to_commit_is_skipped() {
    let scene = Scene::new(&[("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\n"));
    resolve_start(&scene.fixture);
    // Settled as the remote has it: the project's own commit has nothing left.
    write(&scene.fixture.root, "a.md", b"theirs\n");

    let mut got = resolve_continue(&scene.fixture, &["a.md"]);
    if got == resolve::Settled::Empty {
        got = resolve::skip(&scene.fixture.root);
    }

    assert_eq!(got, resolve::Settled::Replayed);
    assert_eq!(scene.at("a.md"), b"theirs\n");
    assert!(gitstate::stopped(&scene.fixture.root).is_none());
}

#[test]
fn after_a_resolve_the_push_carries_what_is_committed_and_leaves_new_files_alone() {
    let (fixture, bare) = a_plane_that_conflicts_with_its_remote();
    resolve_start(&fixture);
    std::fs::write(fixture.root.join("theirs.md"), "both").unwrap();
    std::fs::write(fixture.root.join("scratch.md"), "a chat's notes").unwrap();
    assert_eq!(
        resolve_continue(&fixture, &["theirs.md"]),
        resolve::Settled::Replayed
    );

    let (pushed, left) = resolve::push_after(&fixture.root, &mut |_: Say| {});

    assert_eq!(pushed.map(|res| res.outcome), Some(Outcome::Pushed));
    assert_eq!(left, vec!["scratch.md".to_string()]);
    assert_eq!(
        ask(&bare, &["log", "-1", "--format=%s", "main"]).trim(),
        "mine"
    );
    assert!(
        ask(&bare, &["ls-tree", "--name-only", "main"])
            .lines()
            .all(|name| name != "scratch.md"),
        "not committed"
    );
}

#[test]
fn a_file_a_chats_sandbox_may_not_write_is_held_from_chats() {
    let root = Path::new("/plane");
    let files = vec![
        crate::names::PLANE_MANIFEST.write.to_string(),
        "notes.md".to_string(),
    ];
    assert_eq!(
        resolve::held_from_chats(root, &files),
        vec![crate::names::PLANE_MANIFEST.write.to_string()]
    );
}

#[test]
fn skip_never_drops_a_commit_that_still_changes_files() {
    let scene = Scene::new(&[("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\n"));
    resolve_start(&scene.fixture);

    // Unsettled, then settled with a change of its own: neither is skipped.
    assert!(refused_with(
        &resolve::skip(&scene.fixture.root),
        "nothing left to commit"
    ));
    write(&scene.fixture.root, "a.md", b"both\n");
    run(&scene.fixture.root, &["add", "--", "a.md"]);
    assert!(refused_with(
        &resolve::skip(&scene.fixture.root),
        "nothing left to commit"
    ));
    assert!(
        gitstate::stopped(&scene.fixture.root).is_some(),
        "still stopped"
    );
}

#[test]
fn a_project_that_stores_crlf_line_endings_can_continue() {
    let scene = Scene::new(&[
        (".gitattributes", b"*.md text eol=crlf\n"),
        ("a.md", b"one\r\n"),
    ]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\r\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\r\n"));
    assert!(matches!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(_)
    ));
    write(&scene.fixture.root, "a.md", b"both\r\n");

    assert_eq!(
        resolve_continue(&scene.fixture, &["a.md"]),
        resolve::Settled::Replayed
    );
    assert_eq!(
        ask(&scene.fixture.root, &["show", "HEAD:a.md"]),
        "both\n",
        "stored as git stores it"
    );
}

#[test]
fn a_file_merged_without_markers_by_its_attributes_is_the_persons() {
    let scene = Scene::new(&[(".gitattributes", b"a.md -merge\n"), ("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\n"));

    assert!(matches!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(_)
    ));

    assert_eq!(
        resolve::conflicts(&scene.fixture.root).by_hand,
        vec!["a.md".to_string()]
    );
    let refused = resolve_continue(&scene.fixture, &["a.md"]);
    assert!(refused_with(&refused, "`a.md`"), "{refused:?}");
    assert!(refused_with(&refused, "terminal"), "{refused:?}");
}

// --------------------------------------------------------------------------------------- //
// a save commits a file only as the one name it has on disk                                //
// --------------------------------------------------------------------------------------- //

/// What the file outside the plane holds: nothing the secret guard calls a secret, so only
/// the one-name rule stands between it and a commit.
const OUTSIDE: &str = "a line from outside the plane: canary-5d1e9b";

impl Fixture {
    /// This plane in commit mode, with a workspace `w` whose local skill `s` is a folder at
    /// `skill` holding `notes.md`, a second name of a file outside the plane holding `text`.
    fn with_a_second_name(&self, skill: &str, text: &[u8]) {
        std::fs::write(
            self.root.join("charter.toml"),
            "[plane]\nname = \"fixture\"\nmode = \"commit\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(self.root.join("workspaces/w")).unwrap();
        std::fs::write(self.root.join("workspaces/w/workspace.md"), "# w\n").unwrap();
        run(&self.root, &["add", "-A"]);
        run(&self.root, &["commit", "-q", "-m", "commit mode"]);
        let local = self.root.join(skill);
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(
            local.join("SKILL.md"),
            "---\nname: s\ndescription: Keeps notes.\n---\n\nbody\n",
        )
        .unwrap();
        let outside = self.root.parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("notes.md"), text).unwrap();
        std::fs::hard_link(outside.join("notes.md"), local.join("notes.md")).unwrap();
    }

    fn head(&self) -> String {
        run(&self.root, &["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Whether any commit, on any ref, carries [`OUTSIDE`].
    fn carries_outside(&self) -> bool {
        ask(&self.root, &["log", "-p", "--all", "--text"]).contains("canary-5d1e9b")
    }

    fn staged(&self) -> String {
        ask(&self.root, &["diff", "--cached", "--name-only"])
    }

    /// Whether the plane's object store holds the bytes of the file outside it, in any form.
    fn stores_outside(&self) -> bool {
        let outside = self.root.parent().unwrap().join("outside/notes.md");
        let sha = run(&self.root, &["hash-object", outside.to_str().unwrap()]);
        crate::testgit::run(&self.root, &["cat-file", "-e", sha.trim()]).ok()
    }
}

fn skill_change(root: &Path, change: impl FnOnce(&mut crate::skills::verbs::Ctx<'_>)) -> String {
    let mut said = String::new();
    let mut say = |line: Say| {
        said.push_str(&line.to_string());
        said.push('\n');
    };
    change(&mut crate::skills::verbs::Ctx {
        plane: root,
        built_in: None,
        by: crate::skills::verbs::By::Command,
        say: &mut say,
    });
    said
}

fn saved_paths(root: &Path, paths: &[&str]) -> (u8, String) {
    let paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
    let mut said = String::new();
    let code = save_paths(
        &Request {
            root,
            message: Some("named"),
            sign: false,
            no_push: true,
            cwd: root,
            provenance: None,
        },
        Trigger::Skill(crate::skills::verbs::By::Command),
        &paths,
        &mut |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        },
    );
    (code, said)
}

fn saved_as(root: &Path, trigger: Trigger) -> (u8, String) {
    let mut said = String::new();
    let code = save_as(
        &Request {
            root,
            message: None,
            sign: false,
            no_push: true,
            cwd: root,
            provenance: None,
        },
        trigger,
        &mut |line: Say| {
            said.push_str(&line.to_string());
            said.push('\n');
        },
    );
    (code, said)
}

#[test]
fn a_save_refuses_a_file_with_a_second_name_as_the_readers_already_do() {
    for text in [OUTSIDE.as_bytes().to_vec(), {
        let mut binary = b"\0\x01\x02".to_vec();
        binary.extend_from_slice(OUTSIDE.as_bytes());
        binary
    }] {
        let fixture = Fixture::plane();
        fixture.with_a_second_name("workspaces/w/skills.local/s", &text);
        let before = fixture.head();
        let named = "workspaces/w/skills/s/notes.md has a second name (a hard link)";

        // The skill change that moves it into what the save commits.
        let said = skill_change(&fixture.root, |cx| {
            crate::skills::verbs::share(cx, &crate::skills::Whose::Workspace("w".into()), "s")
                .unwrap();
        });
        assert!(said.contains(named), "{said}");
        // Refused before anything is staged: the skill's other file is not staged either.
        assert!(said.contains("Nothing was staged or committed."), "{said}");
        assert_eq!(fixture.head(), before, "{said}");
        assert_eq!(fixture.staged(), "", "{said}");

        // A save of exactly its paths, and the whole plane's, by every trigger.
        let (code, said) = saved_paths(&fixture.root, &["workspaces/w/skills/s"]);
        assert_eq!(code, 1, "{said}");
        assert!(said.contains(named), "{said}");
        for trigger in [Trigger::Cli, Trigger::Manual, Trigger::Quiet] {
            let (code, said) = saved_as(&fixture.root, trigger);
            assert_eq!(code, 1, "{said}");
            assert!(said.contains(named), "{said}");
            let line = journal(&fixture.root).pop().expect("a journal line");
            assert_eq!(line["outcome"], "blocked", "{line}");
        }

        assert_eq!(fixture.head(), before);
        assert!(!fixture.staged().contains("notes.md"));
        assert!(!fixture.carries_outside(), "a commit carries the file");
        assert!(!fixture.stores_outside(), "the object store holds the file");
    }
}

#[test]
fn a_file_staged_and_then_swapped_out_from_under_the_save_is_not_committed() {
    let fixture = Fixture::plane();
    fixture.with_a_second_name("workspaces/w/skills/s", OUTSIDE.as_bytes());
    let before = fixture.head();
    let local = "workspaces/w/skills/s/notes.md";
    // Staged while it had its second name, git told to look no further at it, and then a
    // file of one name in its place: the save's own `add` does not stage it again.
    run(&fixture.root, &["add", local]);
    run(
        &fixture.root,
        &["update-index", "--assume-unchanged", local],
    );
    std::fs::remove_file(fixture.root.join(local)).unwrap();
    std::fs::write(fixture.root.join(local), "one name\n").unwrap();
    assert!(fixture.stores_outside(), "the fixture's own add stored it");
    // A loose object nothing reaches that the save has nothing to do with.
    let unrelated = fixture.root.parent().unwrap().join("unrelated");
    std::fs::write(&unrelated, "not the save's\n").unwrap();
    let unrelated = run(
        &fixture.root,
        &["hash-object", "-w", unrelated.to_str().unwrap()],
    );

    let (code, said) = saved_as(&fixture.root, Trigger::Cli);

    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains(&format!(
            "{local} is marked for git not to look at it on disk"
        )),
        "{said}"
    );
    assert_eq!(fixture.head(), before);
    assert!(!fixture.staged().contains("notes.md"), "{said}");
    assert!(!fixture.carries_outside(), "a commit carries the file");
    assert!(!fixture.stores_outside(), "the object store still holds it");
    assert!(
        crate::testgit::run(&fixture.root, &["cat-file", "-e", unrelated.trim()]).ok(),
        "an unrelated loose object was removed"
    );
}

// --------------------------------------------------------------------------------------- //
// what a test does at a point inside a save or a Continue                                  //
// --------------------------------------------------------------------------------------- //

/// What a test does to a repository at one point inside a save or a Continue.
pub(super) type Then = Box<dyn Fn(&Path)>;

thread_local! {
    /// What a test does when the plane's save reaches its secret guard.
    static AT_THE_SECRET_GUARD: std::cell::RefCell<Option<Then>> =
        const { std::cell::RefCell::new(None) };
    /// What a test does once the plane's save has staged, before it asks what it staged.
    pub(super) static AFTER_THE_ADD: std::cell::RefCell<Option<Then>> =
        const { std::cell::RefCell::new(None) };
    /// What a test does once the plane's save, Continue or a checked add has looked at what it
    /// stages, before it stages it.
    pub(super) static BEFORE_THE_ADD: std::cell::RefCell<Option<Then>> =
        const { std::cell::RefCell::new(None) };
}

pub(super) type Point = std::thread::LocalKey<std::cell::RefCell<Option<Then>>>;

/// Runs what the test set at `point`, if it set anything.
fn reached(point: &'static Point, root: &Path) {
    point.with(|then| {
        if let Some(then) = then.borrow().as_ref() {
            then(root);
        }
    });
}

pub(super) fn at_the_secret_guard(root: &Path) {
    reached(&AT_THE_SECRET_GUARD, root);
}

pub(super) fn after_the_add(root: &Path) {
    reached(&AFTER_THE_ADD, root);
}

pub(super) fn before_the_add(root: &Path) {
    reached(&BEFORE_THE_ADD, root);
}

/// `run`, with `then` done at `point`.
pub(super) fn doing_at<T>(
    point: &'static Point,
    then: impl Fn(&Path) + 'static,
    run: impl FnOnce() -> T,
) -> T {
    struct Clear(&'static Point);
    impl Drop for Clear {
        fn drop(&mut self) {
            self.0.with(|then| then.borrow_mut().take());
        }
    }
    point.with(|held| *held.borrow_mut() = Some(Box::new(then)));
    let _clear = Clear(point);
    run()
}

#[test]
fn a_held_back_row_is_out_of_the_object_store_before_the_secret_guard_reads_a_blob() {
    let fixture = Fixture::plane();
    fixture.with_a_second_name("workspaces/w/skills/s", OUTSIDE.as_bytes());
    let before = fixture.head();
    let local = "workspaces/w/skills/s/notes.md";
    // Staged before the save and unchanged since, so nothing the save's own add would stage
    // names it: the save finds its second name once staged, and holds the row back.
    run(&fixture.root, &["add", local]);
    assert!(fixture.stores_outside(), "the fixture's own add stored it");
    let outside = fixture.root.parent().unwrap().join("outside/notes.md");
    let sha = run(&fixture.root, &["hash-object", outside.to_str().unwrap()])
        .trim()
        .to_owned();

    let seen = std::rc::Rc::new(std::cell::Cell::new(false));
    let (code, said) = doing_at(
        &AT_THE_SECRET_GUARD,
        {
            let seen = seen.clone();
            move |root: &Path| {
                seen.set(true);
                assert!(
                    !crate::testgit::run(root, &["cat-file", "-e", &sha]).ok(),
                    "the held-back blob is still in the object store at the secret guard"
                );
            }
        },
        || saved_as(&fixture.root, Trigger::Cli),
    );

    assert!(
        seen.get(),
        "the save never reached its secret guard: {said}"
    );
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains(&format!("{local} has a second name (a hard link)")),
        "{said}"
    );
    assert_eq!(fixture.head(), before);
    assert!(!fixture.staged().contains("notes.md"), "{said}");
}

/// A file given a second name after the save looked and before it staged, as a whole-tree save
/// stages it: its descriptor shows the second name, so its bytes are never hashed.
#[cfg(unix)]
#[test]
fn a_second_name_made_as_a_whole_tree_save_stages_never_reaches_the_object_store() {
    let fixture = Fixture::plane();
    fixture.with_settings("[plane]\nname = \"fixture\"\nmode = \"commit\"\n");
    let before = fixture.head();
    let second = fixture.root.parent().unwrap().join("second-name");
    write(
        &fixture.root,
        "work.md",
        b"two names by now: canary-8c2f40\n",
    );

    let stored_once_staged = std::rc::Rc::new(std::cell::Cell::new(None));
    let (code, said) = doing_at(
        &BEFORE_THE_ADD,
        {
            let second = second.clone();
            move |root: &Path| std::fs::hard_link(root.join("work.md"), &second).unwrap()
        },
        || {
            doing_at(
                &AFTER_THE_ADD,
                {
                    let stored = stored_once_staged.clone();
                    let second = second.clone();
                    move |root: &Path| {
                        stored.set(Some(crate::testgit::stores_file(root, &second)));
                    }
                },
                || fixture.just_save(),
            )
        },
    );

    assert_eq!(
        stored_once_staged.get(),
        Some(false),
        "the file's bytes were hashed into the object store: {said}"
    );
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains("work.md has a second name (a hard link)"),
        "{said}"
    );
    assert!(!crate::testgit::stores_file(&fixture.root, &second));
    assert_eq!(fixture.head(), before);
    assert!(!fixture.staged().contains("work.md"), "{said}");
}

#[test]
fn a_row_gone_after_the_add_is_out_of_the_object_store_before_the_secret_guard_reads_a_blob() {
    let fixture = Fixture::plane();
    let gone = "personas/steward/memory/gone.md";
    write(
        &fixture.root,
        gone,
        b"staged, then removed: canary-3e7a51\n",
    );
    let before = fixture.head();
    let sha = run(&fixture.root, &["hash-object", gone]).trim().to_owned();

    let seen = std::rc::Rc::new(std::cell::Cell::new(false));
    let (code, said) = doing_at(
        &AFTER_THE_ADD,
        move |root: &Path| std::fs::remove_file(root.join(gone)).unwrap(),
        || {
            doing_at(
                &AT_THE_SECRET_GUARD,
                {
                    let seen = seen.clone();
                    move |root: &Path| {
                        seen.set(true);
                        assert!(
                            !crate::testgit::run(root, &["cat-file", "-e", &sha]).ok(),
                            "the gone row's blob is still in the object store at the secret guard"
                        );
                    }
                },
                || saved_as(&fixture.root, Trigger::Cli),
            )
        },
    );

    assert!(
        seen.get(),
        "the save never reached its secret guard: {said}"
    );
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains(&format!("{gone} is no longer on disk as it was staged")),
        "{said}"
    );
    assert_eq!(fixture.head(), before);
    assert!(!fixture.staged().contains("gone.md"), "{said}");
}

/// A plane stopped on a conflict in `a.md`, settled on disk with no markers left.
fn a_settled_conflict() -> Scene {
    let scene = Scene::new(&[("a.md", b"one\n")]);
    scene.theirs("theirs", |dir| write(dir, "a.md", b"theirs\n"));
    scene.mine("mine", |dir| write(dir, "a.md", b"mine\n"));
    assert_eq!(
        resolve_start(&scene.fixture),
        resolve::Settled::Stopped(vec!["a.md".into()])
    );
    write(&scene.fixture.root, "a.md", b"settled under two names\n");
    scene
}

impl Scene {
    /// Whether the project's object store holds the blob of what `a.md` holds now.
    fn stores_a(&self) -> bool {
        let sha = run(&self.fixture.root, &["hash-object", "a.md"]);
        crate::testgit::run(&self.fixture.root, &["cat-file", "-e", sha.trim()]).ok()
    }

    fn remote_main(&self) -> String {
        run(&self.theirs, &["ls-remote", "origin", "refs/heads/main"])
    }

    /// The stop is as git left it: `a.md` in conflict, nothing committed, nothing pushed.
    fn still_stopped(&self, head: &str, remote: &str) {
        let root = &self.fixture.root;
        assert!(gitstate::stopped(root).is_some(), "the rebase went on");
        assert_eq!(gitstate::unmerged(root), vec!["a.md".to_string()]);
        assert_eq!(run(root, &["rev-parse", "HEAD"]), head, "a commit was made");
        assert_eq!(self.remote_main(), remote, "something was pushed");
    }
}

#[cfg(unix)]
#[test]
fn continue_refuses_a_settled_file_with_a_second_name_and_goes_on_once_it_has_one() {
    let scene = a_settled_conflict();
    let root = scene.fixture.root.clone();
    let second = root.parent().unwrap().join("second-name");
    std::fs::hard_link(root.join("a.md"), &second).unwrap();
    let head = run(&root, &["rev-parse", "HEAD"]);
    let remote = scene.remote_main();

    let refused = resolve_continue(&scene.fixture, &["a.md"]);

    assert!(refused_with(&refused, "has a second name"), "{refused:?}");
    assert!(refused_with(&refused, "Nothing was staged"), "{refused:?}");
    scene.still_stopped(&head, &remote);
    assert!(
        !scene.stores_a(),
        "the file's bytes reached the object store"
    );

    // With one name again, Continue carries on, and the push follows.
    std::fs::remove_file(&second).unwrap();
    assert_eq!(
        resolve_continue(&scene.fixture, &["a.md"]),
        resolve::Settled::Replayed
    );
    let (pushed, _) = resolve::push_after(&root, &mut |_: Say| {});
    assert_eq!(pushed.map(|res| res.outcome), Some(Outcome::Pushed));
    assert_ne!(scene.remote_main(), remote, "nothing was pushed");
    assert_eq!(scene.at("a.md"), b"settled under two names\n");
}

#[cfg(unix)]
#[test]
fn a_second_name_made_as_continue_stages_puts_the_conflict_back_and_its_blob_out() {
    let scene = a_settled_conflict();
    let root = scene.fixture.root.clone();
    let second = root.parent().unwrap().join("second-name");
    let head = run(&root, &["rev-parse", "HEAD"]);
    let remote = scene.remote_main();

    let refused = doing_at(
        &BEFORE_THE_ADD,
        {
            let second = second.clone();
            move |root: &Path| std::fs::hard_link(root.join("a.md"), &second).unwrap()
        },
        || resolve_continue(&scene.fixture, &["a.md"]),
    );

    assert!(refused_with(&refused, "has a second name"), "{refused:?}");
    assert!(
        refused_with(&refused, "marked as conflicted again"),
        "{refused:?}"
    );
    scene.still_stopped(&head, &remote);
    assert!(
        !scene.stores_a(),
        "the file's bytes are in the object store"
    );

    std::fs::remove_file(&second).unwrap();
    assert_eq!(
        resolve_continue(&scene.fixture, &["a.md"]),
        resolve::Settled::Replayed
    );
}

#[test]
fn a_file_changed_as_continue_stages_it_puts_the_conflict_back_and_its_blob_out() {
    let scene = a_settled_conflict();
    let root = scene.fixture.root.clone();
    let head = run(&root, &["rev-parse", "HEAD"]);
    let remote = scene.remote_main();

    let refused = doing_at(
        &BEFORE_THE_ADD,
        |root: &Path| write(root, "a.md", b"written after the check\n"),
        || resolve_continue(&scene.fixture, &["a.md"]),
    );

    assert!(
        refused_with(&refused, "changed while purlis was checking"),
        "{refused:?}"
    );
    scene.still_stopped(&head, &remote);
    assert!(
        !scene.stores_a(),
        "the unchecked bytes are in the object store"
    );

    assert_eq!(
        resolve_continue(&scene.fixture, &["a.md"]),
        resolve::Settled::Replayed
    );
    assert_eq!(scene.at("a.md"), b"written after the check\n");
}
