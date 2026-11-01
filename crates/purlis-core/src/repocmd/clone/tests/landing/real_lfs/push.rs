// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A branch folder's push against a real git-lfs** (#4399): `pieceupdate::push` itself, with a
//! bare repository as the forge and an upload server on a loopback port. Only the address and the
//! helper a forge would give stand in ([`crate::lfsupload::StandIn`]): a remote on this machine
//! has neither.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::lfsupload::testserver::{HELPER, Taking, Uploads, Watching, marker_in};
use crate::lfsupload::{STAND_IN, StandIn};
use crate::pieces::Who;
use crate::pieceupdate::{self, Piece, Reach};

use super::{Object, Say, real_git_lfs};

fn plain_git(dest: &Path, args: &[&str]) -> String {
    let mut command = std::process::Command::new("git");
    command
        .args([
            "-c",
            "user.name=Plain",
            "-c",
            "user.email=plain@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dest);
    let out = crate::forklock::output(&mut command).unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stored(dest: &Path, rev: &str, at: &str) -> String {
    plain_git(dest, &["cat-file", "blob", &format!("{rev}:{at}")])
}

const WS: &str = "alpha";
const REPO: &str = "widget";
const PIECE: &str = "lane";
const BRANCH: &str = "refs/heads/lane";

struct Place {
    _dir: tempfile::TempDir,
    base: std::path::PathBuf,
    plane: std::path::PathBuf,
    forge: std::path::PathBuf,
    clone: std::path::PathBuf,
    folder: std::path::PathBuf,
}

/// A forge with one commit, a clone in a workspace, and a branch folder cut from it.
fn place() -> Place {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let plane = base.join("plane");
    std::fs::create_dir_all(plane.join("workspaces").join(WS)).unwrap();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let forge = base.join("forge.git");
    plain_git(
        &base,
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            forge.to_str().unwrap(),
        ],
    );
    let theirs = base.join("theirs");
    let url = format!("file://{}", forge.display());
    plain_git(&base, &["clone", "-q", &url, theirs.to_str().unwrap()]);
    std::fs::write(theirs.join("README.md"), "one\n").unwrap();
    plain_git(&theirs, &["add", "README.md"]);
    plain_git(&theirs, &["commit", "-q", "-m", "one"]);
    plain_git(&theirs, &["push", "-q", "origin", "HEAD:main"]);
    let clone = plane.join("workspaces").join(WS).join(REPO);
    plain_git(&base, &["clone", "-q", &url, clone.to_str().unwrap()]);
    let mut said = Vec::new();
    let code = crate::piececmd::add(
        &plane,
        WS,
        REPO,
        PIECE,
        None,
        None,
        None,
        None,
        &Who::here(None, Some("3".to_owned()), None),
        chrono::Utc::now(),
        &mut |line| said.push(line),
    );
    assert_eq!(code, 0, "{said:?}");
    let folder = plane
        .join("workspaces")
        .join(WS)
        .join(".worktrees")
        .join(REPO)
        .join(PIECE);
    Place {
        _dir: dir,
        base,
        plane,
        forge,
        clone,
        folder,
    }
}

impl Place {
    /// The real clean filter stores `object` and commits its pointer at `at`.
    fn commit_large(&self, at: &str, object: &Object) {
        std::fs::write(
            self.folder.join(".gitattributes"),
            "*.bin filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        std::fs::write(self.folder.join(at), &object.bytes).unwrap();
        plain_git(&self.folder, &["add", ".gitattributes", at]);
        plain_git(&self.folder, &["commit", "-q", "-m", at]);
        assert_eq!(stored(&self.folder, "HEAD", at), object.pointer());
    }

    fn the_copy_of(&self, object: &Object) -> std::path::PathBuf {
        self.clone
            .join(".git/lfs/objects")
            .join(&object.oid[..2])
            .join(&object.oid[2..4])
            .join(&object.oid)
    }

    fn watching(&self) -> Option<Watching> {
        Some(Watching {
            forge: self.forge.clone(),
            reference: BRANCH.to_owned(),
        })
    }

    fn ref_moved(&self) -> bool {
        let mut command = std::process::Command::new("git");
        command
            .args(["rev-parse", "--verify", "--quiet", BRANCH])
            .current_dir(&self.forge);
        crate::forklock::output(&mut command)
            .unwrap()
            .status
            .success()
    }

    /// `purlis worktree push`, with `server` where the forge's LFS address would be.
    fn push(&self, server: &Uploads, stall: Duration) -> (u8, String) {
        STAND_IN.with(|held| {
            *held.borrow_mut() = Some(StandIn {
                endpoint: server.endpoint(),
                helper: Some(HELPER.to_owned()),
                stall,
            });
        });
        let mut lines: Vec<Say> = Vec::new();
        let at = Piece {
            root: &self.plane,
            ws: WS,
            repo: REPO,
            piece: PIECE,
        };
        let code = pieceupdate::push(&at, &Reach::default(), &mut |line| lines.push(line));
        STAND_IN.with(|held| *held.borrow_mut() = None);
        (code, format!("{lines:?}"))
    }
}

const NO_STALL: Duration = Duration::from_secs(60);

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_push_uploads_each_object_before_the_ref_moves() {
    if !real_git_lfs() {
        return;
    }
    let place = place();
    let object = Object::of(4, 21);
    place.commit_large("large.bin", &object);
    let server = Uploads::serving(Taking::Everything, place.watching());

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 0, "{said}");
    let (bytes, moved) = server.holds(&object.oid).expect("the object arrived");
    assert!(
        bytes == object.bytes,
        "the object's bytes changed on the way"
    );
    assert!(!moved, "the ref had moved before the object arrived");
    assert!(place.ref_moved(), "{said}");
    assert!(server.signed() >= 1, "never signed by the push's helper");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_failed_upload_leaves_the_remote_ref_unmoved_and_says_so() {
    if !real_git_lfs() {
        return;
    }
    let place = place();
    let object = Object::of(1, 22);
    place.commit_large("large.bin", &object);
    let server = Uploads::serving(Taking::Nothing, None);

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert!(said.contains("nothing was pushed"), "{said}");
    assert!(server.holds(&object.oid).is_none());
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_an_upload_that_stops_is_stopped_at_the_stall_limit_and_nothing_is_pushed() {
    if !real_git_lfs() {
        return;
    }
    let place = place();
    place.commit_large("large.bin", &Object::of(64, 23));
    let server = Uploads::serving(Taking::GoesQuiet, None);
    let started = Instant::now();

    let (code, said) = place.push(&server, Duration::from_secs(4));

    assert_eq!(code, 1, "{said}");
    assert!(started.elapsed() < Duration::from_secs(25), "{said}");
    assert!(!place.ref_moved(), "{said}");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_an_object_not_here_refuses_the_push_naming_what_to_run() {
    if !real_git_lfs() {
        return;
    }
    let place = place();
    let object = Object::of(1, 24);
    place.commit_large("large.bin", &object);
    // Gone from the worktree too: a status would clean the file there into the store again.
    plain_git(&place.folder, &["rm", "-q", "large.bin"]);
    plain_git(&place.folder, &["commit", "-q", "-m", "gone"]);
    std::fs::remove_file(place.the_copy_of(&object)).unwrap();
    let server = Uploads::serving(Taking::Everything, None);

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("git lfs fetch --all"), "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert_eq!(server.requests(), 0, "{said}");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_the_upload_reads_nothing_through_a_link_or_a_changed_copy_in_the_store() {
    if !real_git_lfs() {
        return;
    }
    // What a chat can do, which can write `.git/lfs` and not what this process reads for it.
    let place = place();
    let object = Object::of(1, 25);
    place.commit_large("large.bin", &object);
    let copy = place.the_copy_of(&object);
    let secret = place.base.join("what-the-chat-cannot-read");
    std::fs::write(&secret, b"the person's own secret").unwrap();
    std::fs::remove_file(&copy).unwrap();
    std::os::unix::fs::symlink(&secret, &copy).unwrap();
    let server = Uploads::serving(Taking::Everything, None);

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert_eq!(server.requests(), 0, "{said}");

    // A folder of the store that is a link, to the bytes it names, kept elsewhere.
    std::fs::remove_file(&copy).unwrap();
    let folder = copy.parent().unwrap();
    let kept = place.base.join("kept-elsewhere");
    std::fs::create_dir(&kept).unwrap();
    std::fs::write(kept.join(&object.oid), &object.bytes).unwrap();
    std::fs::remove_dir(folder).unwrap();
    std::os::unix::fs::symlink(&kept, folder).unwrap();

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert_eq!(server.requests(), 0, "{said}");
    std::fs::remove_file(folder).unwrap();
    std::fs::create_dir(folder).unwrap();

    // A second name for the right bytes.
    std::fs::write(kept.join("twin"), &object.bytes).unwrap();
    std::fs::hard_link(kept.join("twin"), &copy).unwrap();

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert_eq!(server.requests(), 0, "{said}");

    // The right size, the wrong bytes.
    std::fs::remove_file(&copy).unwrap();
    let mut other = object.bytes.clone();
    other[0] ^= 1;
    std::fs::write(&copy, &other).unwrap();

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert_eq!(server.requests(), 0, "{said}");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_chat_cannot_send_the_upload_to_any_other_server() {
    if !real_git_lfs() {
        return;
    }
    let place = place();
    let object = Object::of(1, 26);
    let elsewhere = Uploads::serving(Taking::Everything, None);
    // Each place an address can be named from: what the chat commits, and the repository's own.
    std::fs::write(
        place.folder.join(".lfsconfig"),
        format!(
            "[lfs]\n\turl = {0}\n\tpushurl = {0}\n[remote \"origin\"]\n\tlfsurl = {0}\n",
            elsewhere.endpoint()
        ),
    )
    .unwrap();
    plain_git(&place.folder, &["add", ".lfsconfig"]);
    place.commit_large("large.bin", &object);
    for (key, value) in [
        ("lfs.url", elsewhere.endpoint()),
        ("lfs.pushurl", elsewhere.endpoint()),
        ("remote.origin.lfsurl", elsewhere.endpoint()),
        ("remote.origin.lfspushurl", elsewhere.endpoint()),
    ] {
        plain_git(&place.clone, &["config", key, &value]);
    }
    let server = Uploads::serving(Taking::Everything, place.watching());

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 0, "{said}");
    assert!(server.holds(&object.oid).is_some(), "{said}");
    assert_eq!(elsewhere.requests(), 0, "{said}");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_program_the_repository_names_refuses_the_push_and_never_runs() {
    if !real_git_lfs() {
        return;
    }
    for (key, name) in [
        ("credential.helper", "helper"),
        ("lfs.customtransfer.planted.path", "transfer"),
        ("lfs.standalonetransferagent", "agent"),
        ("alias.lfs", "alias"),
        ("xlfs.customtransfer.planted.path", "loose-transfer"),
        (
            "lfs.https://f.example/.standalonetransferagent",
            "scoped-agent",
        ),
    ] {
        let place = place();
        let object = Object::of(1, 27);
        place.commit_large("large.bin", &object);
        let marker = marker_in(&place.base, name);
        plain_git(
            &place.clone,
            &["config", key, &format!("!touch '{}'", marker.display())],
        );
        let server = Uploads::serving(Taking::Everything, None);

        let (code, said) = place.push(&server, NO_STALL);

        assert_eq!(code, 1, "{key}: {said}");
        assert!(!place.ref_moved(), "{key}: {said}");
        assert_eq!(server.requests(), 0, "{key}: {said}");
        assert!(!marker.exists(), "{key}: {said}");
    }
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_secret_in_a_large_text_file_refuses_the_push_before_anything_is_sent() {
    if !real_git_lfs() {
        return;
    }
    let place = place();
    let token = format!("ghp_{}", "aB3dE6gH9jK2mN5pQ8sT1vW4yZ7cF0iL3oR6");
    let bytes = format!("TOKEN={token}\n").into_bytes();
    let object = Object {
        oid: crate::extension::hex(&<sha2::Sha256 as sha2::Digest>::digest(&bytes)),
        bytes,
    };
    place.commit_large("settings.bin", &object);
    let server = Uploads::serving(Taking::Everything, None);

    let (code, said) = place.push(&server, NO_STALL);

    assert_eq!(code, 1, "{said}");
    assert!(said.contains("settings.bin"), "{said}");
    assert!(!said.contains(&token), "{said}");
    assert!(!place.ref_moved(), "{said}");
    assert_eq!(server.requests(), 0, "{said}");
}

/// A plane in push mode and a clone of a bare repository, as a save reaches a forge in its own
/// tests: `origin` on a known host, and an `insteadOf` that lands on the bare repository.
fn saving_place() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let plane = top.join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(
        plane.join("charter.toml"),
        "[repos.widget]\nmode = \"push\"\n",
    )
    .unwrap();
    let clone = plane.join("workspaces/alpha/widget");
    std::fs::create_dir_all(&clone).unwrap();
    plain_git(&clone, &["init", "-q", "-b", "main", "."]);
    plain_git(&clone, &["config", "user.name", "Fixture"]);
    plain_git(&clone, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    plain_git(&clone, &["add", "-A"]);
    plain_git(&clone, &["commit", "-q", "-m", "one"]);
    let bare = top.join("forge/acme/widget.git");
    std::fs::create_dir_all(&bare).unwrap();
    plain_git(&bare, &["init", "-q", "--bare", "-b", "main", "."]);
    plain_git(
        &clone,
        &["remote", "add", "origin", "git@github.com:acme/widget.git"],
    );
    let base = format!("file://{}/", bare.parent().unwrap().display());
    plain_git(
        &clone,
        &[
            "config",
            &format!("url.{base}.insteadOf"),
            "https://github.com/acme/",
        ],
    );
    let at = bare.display().to_string();
    plain_git(&clone, &["push", "-q", &at, "main"]);
    plain_git(
        &clone,
        &["fetch", "-q", &at, "+refs/heads/*:refs/remotes/origin/*"],
    );
    plain_git(&clone, &["checkout", "-q", "-b", "feature/x"]);
    (dir, plane, clone, bare)
}

fn save(plane: &Path, clone: &Path, server: &Uploads) -> (u8, String) {
    STAND_IN.with(|held| {
        *held.borrow_mut() = Some(StandIn {
            endpoint: server.endpoint(),
            helper: Some(HELPER.to_owned()),
            stall: NO_STALL,
        });
    });
    let mut lines: Vec<Say> = Vec::new();
    let code = crate::reposave::save_as(
        &crate::reposave::Request {
            plane,
            workspace: "alpha",
            name: "widget",
            clone,
            message: None,
            no_push: false,
            mid_turn: &crate::reposave::nobody_working,
        },
        crate::planegit::Trigger::Manual,
        &mut |line| lines.push(line),
    );
    STAND_IN.with(|held| *held.borrow_mut() = None);
    (code, format!("{lines:?}"))
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_save_uploads_each_object_before_the_ref_moves_or_stays_committed() {
    if !real_git_lfs() {
        return;
    }
    let (_dir, plane, clone, bare) = saving_place();
    let reference = "refs/heads/feature/x";
    let has_ref = || {
        let mut command = std::process::Command::new("git");
        command
            .args(["rev-parse", "--verify", "--quiet", reference])
            .current_dir(&bare);
        crate::forklock::output(&mut command)
            .unwrap()
            .status
            .success()
    };
    let object = Object::of(2, 28);
    std::fs::write(
        clone.join(".gitattributes"),
        "*.bin filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    std::fs::write(clone.join("large.bin"), &object.bytes).unwrap();

    // A failed upload: the save stays a commit, and the remote does not move.
    let refusing = Uploads::serving(Taking::Nothing, None);
    let (code, said) = save(&plane, &clone, &refusing);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("not pushed"), "{said}");
    assert!(!has_ref(), "{said}");
    assert_eq!(stored(&clone, "HEAD", "large.bin"), object.pointer());

    // The next save, against one that takes it.
    let server = Uploads::serving(
        Taking::Everything,
        Some(Watching {
            forge: bare.clone(),
            reference: reference.to_owned(),
        }),
    );
    std::fs::write(clone.join("note.md"), "more\n").unwrap();
    let (code, said) = save(&plane, &clone, &server);
    assert_eq!(code, 0, "{said}");
    let (bytes, moved) = server.holds(&object.oid).expect("the object arrived");
    assert!(
        bytes == object.bytes,
        "the object's bytes changed on the way"
    );
    assert!(!moved, "the ref had moved before the object arrived");
    assert!(has_ref(), "{said}");
}
