// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Git LFS stage against a real git-lfs** (#4162): [`clone`] itself, with nothing standing
//! in for `git lfs pull`, fetching from an LFS server in this process on a loopback port. The
//! repository's own `.lfsconfig` names the server, and the server asks for the clone's helper's
//! credentials, so what is proved is what a person's clone does.
//!
//! `#[ignore]`d everywhere but the CI job `real git-lfs`, which installs it and runs these by
//! name. Run by hand on a machine with none they say so and pass; with `CI` set they fail, so a
//! job that lost git-lfs is red and never a skip. The stall limit is shortened to seconds: a
//! download a line of git-lfs's meter apart, and one that stops, are the real thing either way.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::lfsupload::testserver::{CHUNK, Object, named, on_a_machine, real_git_lfs};

use super::*;

mod push;

/// How the server sends a download.
#[derive(Clone, Copy)]
enum Sending {
    AtOnce,
    /// A chunk, then a wait, to the end.
    Slowly {
        gap: Duration,
    },
    /// `chunks` chunks, then nothing, with the connection held open.
    Stops {
        chunks: usize,
    },
    /// Nothing at all: the batch call itself is never answered.
    NoAnswer,
}

#[derive(Default)]
struct Asked {
    batches: AtomicU32,
    signed_batches: AtomicU32,
    downloads: AtomicU32,
    over: AtomicBool,
}

/// An LFS server on a loopback port, for as long as it is held.
struct LfsServer {
    port: u16,
    asked: Arc<Asked>,
}

impl Drop for LfsServer {
    fn drop(&mut self) {
        self.asked.over.store(true, Ordering::SeqCst);
    }
}

impl LfsServer {
    fn serving(objects: Vec<Object>, sending: Sending) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let asked = Arc::new(Asked::default());
        let objects = Arc::new(objects);
        let serving = Arc::clone(&asked);
        std::thread::spawn(move || {
            while !serving.over.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (objects, asked) = (Arc::clone(&objects), Arc::clone(&serving));
                        std::thread::spawn(move || {
                            let _ = answer(stream, port, &objects, &asked, sending);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Self { port, asked }
    }

    fn batches(&self) -> u32 {
        self.asked.batches.load(Ordering::SeqCst)
    }
}

fn hold_until_over(asked: &Asked) {
    while !asked.over.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// "x:y", the stand-in helper's answer, as HTTP Basic says it.
const SIGNED: &str = "Basic eDp5";

fn answer(
    stream: TcpStream,
    port: u16,
    objects: &[Object],
    asked: &Asked,
    sending: Sending,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    loop {
        let mut request = String::new();
        if reader.read_line(&mut request)? == 0 {
            return Ok(());
        }
        let mut length = 0usize;
        let mut signed = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim();
            if header.is_empty() {
                break;
            }
            let lower = header.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            signed |= header.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization") && value.trim() == SIGNED
            });
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        let path = request.split(' ').nth(1).unwrap_or_default().to_owned();
        if path.ends_with("/objects/batch") {
            asked.batches.fetch_add(1, Ordering::SeqCst);
            if !signed {
                let said = br#"{"message":"Credentials needed"}"#;
                write!(
                    stream,
                    "HTTP/1.1 401 Unauthorized\r\nLFS-Authenticate: Basic realm=\"lfs\"\r\n\
                     Content-Type: application/vnd.git-lfs+json\r\nContent-Length: {}\r\n\r\n",
                    said.len()
                )?;
                stream.write_all(said)?;
                continue;
            }
            asked.signed_batches.fetch_add(1, Ordering::SeqCst);
            if matches!(sending, Sending::NoAnswer) {
                hold_until_over(asked);
                return Ok(());
            }
            // Only what was asked for, as a server answers: git-lfs panics on any other object.
            let wanted: Value = serde_json::from_slice(&body).unwrap_or_default();
            let wanted = |oid: &str| {
                wanted["objects"]
                    .as_array()
                    .is_some_and(|asked| asked.iter().any(|one| one["oid"] == oid))
            };
            let rows: Vec<Value> = objects
                .iter()
                .filter(|object| wanted(&object.oid))
                .map(|object| {
                    json!({
                        "oid": object.oid,
                        "size": object.bytes.len(),
                        "authenticated": true,
                        "actions": {"download": {
                            "href": format!("http://127.0.0.1:{port}/objects/{}", object.oid),
                        }},
                    })
                })
                .collect();
            let said = json!({"transfer": "basic", "objects": rows}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.git-lfs+json\r\n\
                 Content-Length: {}\r\n\r\n{said}",
                said.len()
            )?;
            continue;
        }
        let Some(object) = objects
            .iter()
            .find(|object| path.ends_with(&format!("/objects/{}", object.oid)))
        else {
            stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
            continue;
        };
        asked.downloads.fetch_add(1, Ordering::SeqCst);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\
             Content-Length: {}\r\n\r\n",
            object.bytes.len()
        )?;
        for (at, chunk) in object.bytes.chunks(CHUNK).enumerate() {
            match sending {
                Sending::Stops { chunks } if at == chunks => {
                    stream.flush()?;
                    hold_until_over(asked);
                    return Ok(());
                }
                Sending::Slowly { gap } if at > 0 => std::thread::sleep(gap),
                _ => {}
            }
            stream.write_all(chunk)?;
            stream.flush()?;
        }
    }
}

/// The credential helper the server's sign-in is answered by, as a clone's route names one.
const HELPER: &str = "!f() { test \"$1\" = get && printf 'username=x\\npassword=y\\n'; }; f";

impl World {
    /// [`World::holding`], with `objects` Git LFS files `file0`.. in `name`, and the repo's own
    /// `.lfsconfig` naming `server`.
    fn holding_real_large_files(name: &str, objects: &[&Object], server: &LfsServer) -> Self {
        let lfsconfig = format!(
            "[lfs]\n\turl = http://127.0.0.1:{}/acme/{name}.git/info/lfs\n",
            server.port
        );
        let pointers: Vec<(String, String)> = objects
            .iter()
            .enumerate()
            .map(|(at, object)| (format!("file{at}.bin"), object.pointer()))
            .collect();
        let mut files: Vec<(&str, &str)> = vec![(".lfsconfig", &lfsconfig)];
        files.extend(
            pointers
                .iter()
                .map(|(at, text)| (at.as_str(), text.as_str())),
        );
        Self::holding_seeded(name, &files, "*.bin filter=lfs diff=lfs merge=lfs -text\n")
    }

    /// A clone of `name` through the real stage, as the window runs one, with the stall limit
    /// `stall`, the helper where `signed`, and what it told the window.
    fn real_clone(
        &self,
        name: &str,
        stall: Duration,
        signed: bool,
    ) -> (u8, Vec<Say>, Vec<(Stage, u8)>, Duration) {
        let seen = Mutex::new(Vec::new());
        let heard = |stage: Stage, percent: u8| seen.lock().unwrap().push((stage, percent));
        let off = AtomicBool::new(false);
        let mut watch = self.watching(&heard, &off, stall, None);
        watch.stand_in.helper = signed.then(|| HELPER.to_owned());
        let started = Instant::now();
        let (code, said) = self.clone_as_the_window(&[name], &watch);
        let took = started.elapsed();
        (code, said, seen.into_inner().unwrap(), took)
    }
}

/// What the stall limit is held to here: long enough for a runner under load, short enough that
/// git-lfs's own 30 s activity timeout never answers first.
const STALL: Duration = Duration::from_secs(4);

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_lands_each_file_as_its_content_signed_by_the_clones_helper() {
    if !real_git_lfs() {
        return;
    }
    let (first, second) = (Object::of(16, 1), Object::of(4, 2));
    let server = LfsServer::serving(vec![first.clone(), second.clone()], Sending::AtOnce);
    let world = World::holding_real_large_files("widget", &[&first, &second], &server);

    let (code, said, seen, _) = world.real_clone("widget", Duration::from_secs(60), true);

    assert_eq!(code, 0, "{said:?}");
    let dest = world.dest("widget");
    assert!(
        std::fs::read(dest.join("file0.bin")).unwrap() == first.bytes,
        "file0 is not its content"
    );
    assert!(
        std::fs::read(dest.join("file1.bin")).unwrap() == second.bytes,
        "file1 is not its content"
    );
    assert!(
        server.asked.signed_batches.load(Ordering::SeqCst) >= 1,
        "never signed"
    );
    assert_eq!(server.asked.downloads.load(Ordering::SeqCst), 2);
    assert!(!world.hidden("widget").exists());
    // Nothing reads as changed: the files are what git expects of a checkout.
    // In the person's own git, with nothing of purlis's between.
    let plain = |args: &[&str]| {
        let mut command = std::process::Command::new("git");
        command.args(args).current_dir(&dest);
        let out = crate::forklock::output(&mut command).unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    let status = plain(&["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "{status}");
    // And in purlis's own, in the clone it wired, however stale the index's record of them is.
    assert_wired(&dest);
    assert_reads_clean(&dest);
    // The percent never goes back, across git's stages and git-lfs's own, and the window says it.
    let percents: Vec<u8> = seen.iter().map(|(_, percent)| *percent).collect();
    assert!(percents.is_sorted(), "{seen:?}");
    assert!(
        seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
    assert_eq!(seen.last(), Some(&(Stage::LargeFiles, 100)), "{seen:?}");
    assert_eq!(Stage::LargeFiles.words(), "Downloading large files");
}

/// `dest`'s own config names the helper purlis's git policy writes.
fn assert_wired(dest: &Path) {
    let said = git::run(
        dest,
        &["config", "--local", "--get-all", "credential.helper"],
        git::READ,
    )
    .unwrap();
    assert!(crate::forge::is_policy_helper(said.out.trim()), "{said:?}");
}

/// git in `dest` with nothing of purlis's between, its filter as the machine names it.
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

/// Each tracked file's record in the index goes stale, so git has to read it through the filter.
fn touch_everything(dest: &Path) {
    for name in plain_git(dest, &["ls-files"]).lines() {
        std::fs::File::options()
            .write(true)
            .open(dest.join(name))
            .unwrap()
            .set_modified(std::time::SystemTime::now())
            .unwrap();
    }
}

fn assert_reads_clean(dest: &Path) {
    touch_everything(dest);
    let run = git::run(dest, &["status", "--porcelain"], git::READ).unwrap();
    assert!(run.ok() && run.out.trim().is_empty(), "{run:?}");
}

/// A commit through the runner, as a save makes one.
fn commit_through_purlis(dest: &Path, message: &str) {
    let run = git::run_untimed(
        dest,
        &[
            "-c",
            "user.name=Purlis",
            "-c",
            "user.email=purlis@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            message,
        ],
    )
    .unwrap();
    assert!(run.ok(), "{run:?}");
}

/// What `at` holds at `rev`.
fn stored(dest: &Path, rev: &str, at: &str) -> String {
    plain_git(dest, &["cat-file", "blob", &format!("{rev}:{at}")])
}

/// A clone of two large files through the real stage, and the server it came from.
fn landed(world_name: &str) -> (World, LfsServer, Object, PathBuf) {
    let (first, second) = (Object::of(4, 11), Object::of(2, 12));
    let server = LfsServer::serving(vec![first.clone(), second.clone()], Sending::AtOnce);
    let world = World::holding_real_large_files(world_name, &[&first, &second], &server);
    let (code, said, _, _) = world.real_clone(world_name, Duration::from_secs(60), true);
    assert_eq!(code, 0, "{said:?}");
    let dest = world.dest(world_name);
    assert_wired(&dest);
    (world, server, first, dest)
}

/// What the server has been asked, so far.
fn asked_so_far(server: &LfsServer) -> (u32, u32) {
    (
        server.batches(),
        server.asked.downloads.load(Ordering::SeqCst),
    )
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_commit_through_purlis_in_a_wired_clone_stores_the_pointer() {
    if !real_git_lfs() {
        return;
    }
    let (_world, server, _, dest) = landed("widget");
    let before = asked_so_far(&server);
    let changed = Object::of(3, 13);
    std::fs::write(dest.join("file0.bin"), &changed.bytes).unwrap();

    let added = git::run_untimed(&dest, &["add", "--", "file0.bin"]).unwrap();
    assert!(added.ok(), "{added:?}");
    commit_through_purlis(&dest, "a large file changed");

    assert_eq!(stored(&dest, "HEAD", "file0.bin"), changed.pointer());
    assert!(std::fs::read(dest.join("file0.bin")).unwrap() == changed.bytes);
    assert_reads_clean(&dest);
    assert_eq!(asked_so_far(&server), before, "a commit reached the server");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_an_update_through_purlis_in_a_wired_clone_leaves_pointers_and_asks_nobody() {
    if !real_git_lfs() {
        return;
    }
    let (_world, server, first, dest) = landed("widget");
    // Upstream's next commits: a file whose object is nowhere here, and a change to one that is.
    let (new, changed) = (Object::of(1, 14), Object::of(1, 15));
    let main = plain_git(&dest, &["symbolic-ref", "--short", "HEAD"]);
    let main = main.trim();
    plain_git(&dest, &["switch", "-q", "-c", "upstream"]);
    std::fs::write(dest.join("new.bin"), new.pointer()).unwrap();
    std::fs::write(dest.join("file1.bin"), changed.pointer()).unwrap();
    plain_git(&dest, &["add", "--", "new.bin", "file1.bin"]);
    plain_git(&dest, &["commit", "-q", "-m", "upstream"]);
    plain_git(&dest, &["switch", "-q", main]);
    let before = asked_so_far(&server);

    // A branch folder of the clone, cut from there.
    let folder = dest.with_extension("piece");
    let folder_text = folder.to_str().unwrap();
    let cut = git::run_untimed(
        &dest,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "piece",
            folder_text,
            "upstream",
        ],
    )
    .unwrap();
    assert!(cut.ok(), "{cut:?}");
    assert_eq!(
        std::fs::read_to_string(folder.join("new.bin")).unwrap(),
        new.pointer()
    );
    assert_reads_clean(&folder);

    // A rebase of a commit of its own, onto a base that moved.
    plain_git(&dest, &["switch", "-q", "-c", "mine", main]);
    std::fs::write(dest.join("mine.txt"), "mine\n").unwrap();
    plain_git(&dest, &["add", "--", "mine.txt"]);
    plain_git(&dest, &["commit", "-q", "-m", "mine"]);
    let rebased = git::run_untimed(
        &dest,
        &[
            "-c",
            "user.name=Purlis",
            "-c",
            "user.email=purlis@example.invalid",
            "rebase",
            "upstream",
        ],
    )
    .unwrap();
    assert!(rebased.ok(), "{rebased:?}");
    assert_reads_clean(&dest);
    assert_eq!(stored(&dest, "HEAD", "new.bin"), new.pointer());
    assert!(
        std::fs::read(dest.join("file0.bin")).unwrap() == first.bytes,
        "file0 was rewritten"
    );
    plain_git(&dest, &["switch", "-q", main]);

    // The fast-forward a merge lands, and a pull under the one-credential rule.
    let merged = git::run_untimed(&dest, &["merge", "--ff-only", "upstream"]).unwrap();
    assert!(merged.ok(), "{merged:?}");
    assert_eq!(
        std::fs::read_to_string(dest.join("new.bin")).unwrap(),
        new.pointer()
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("file1.bin")).unwrap(),
        changed.pointer()
    );
    assert_reads_clean(&dest);
    plain_git(&dest, &["reset", "-q", "--hard", "HEAD~1"]);
    let pulled = git::run_network(
        &dest,
        Some(HELPER),
        &["pull", "-q", "--ff-only", ".", "upstream"],
    )
    .unwrap();
    assert!(pulled.ok(), "{pulled:?}");
    assert_eq!(
        std::fs::read_to_string(dest.join("new.bin")).unwrap(),
        new.pointer()
    );
    assert_reads_clean(&dest);

    assert_eq!(
        asked_so_far(&server),
        before,
        "the filter reached the server"
    );
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_pull_in_a_wired_clone_downloads_under_the_one_credential() {
    // `git lfs` is not told to skip: it is how a pointer the filter left becomes its content.
    if !real_git_lfs() {
        return;
    }
    let (_world, server, first, dest) = landed("widget");
    std::fs::remove_dir_all(dest.join(".git/lfs/objects")).unwrap();
    std::fs::remove_file(dest.join("file0.bin")).unwrap();
    let back = git::run_untimed(&dest, &["checkout", "--", "file0.bin"]).unwrap();
    assert!(back.ok(), "{back:?}");
    assert_eq!(
        std::fs::read_to_string(dest.join("file0.bin")).unwrap(),
        first.pointer()
    );
    let signed = server.asked.signed_batches.load(Ordering::SeqCst);

    let pulled = git::run_network(&dest, Some(HELPER), &["lfs", "pull"]).unwrap();

    assert!(pulled.ok(), "{pulled:?}");
    assert!(std::fs::read(dest.join("file0.bin")).unwrap() == first.bytes);
    assert!(server.asked.signed_batches.load(Ordering::SeqCst) > signed);
    assert_reads_clean(&dest);
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_writes_nothing_through_a_link_in_a_wired_clones_store() {
    if !real_git_lfs() {
        return;
    }
    let (world, _server, _, dest) = landed("widget");
    let elsewhere = world.home.path().join("chosen-by-the-chat");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::remove_dir_all(dest.join(".git/lfs")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dest.join(".git/lfs")).unwrap();
    std::fs::write(dest.join("file0.bin"), Object::of(1, 16).bytes).unwrap();

    let read = git::run_untimed(&dest, &["diff", "--stat"]).unwrap();
    let added = git::run_untimed(&dest, &["add", "--", "file0.bin"]).unwrap();

    assert!(read.ok() && added.ok(), "{read:?} {added:?}");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_helper_the_repository_wrote_still_holds_the_filter() {
    if !real_git_lfs() {
        return;
    }
    let (world, server, first, dest) = landed("widget");
    let marker = world.home.path().join("the-planted-helper-ran");
    let planted = format!(
        "!f() {{ touch '{}'; printf 'username=x\\npassword=y\\n'; }}; f",
        marker.display()
    );
    plain_git(
        &dest,
        &["config", "--local", "--add", "credential.helper", &planted],
    );
    let objects = dest.join(".git/lfs/objects");
    std::fs::remove_dir_all(&objects).unwrap();
    std::fs::remove_file(dest.join("file0.bin")).unwrap();
    let before = asked_so_far(&server);

    // The filter did not run: the file comes back as what is stored, its pointer...
    let back = git::run_untimed(&dest, &["checkout", "--", "file0.bin"]).unwrap();
    assert!(back.ok(), "{back:?}");
    assert_eq!(
        std::fs::read_to_string(dest.join("file0.bin")).unwrap(),
        first.pointer()
    );
    // ...and status has no clean filter to read file1's content with.
    touch_everything(&dest);
    let status = git::run(&dest, &["status", "--porcelain"], git::READ).unwrap();
    assert!(status.out.contains(" M file1.bin"), "{status:?}");
    assert!(!marker.exists(), "purlis's git ran the planted helper");
    assert_eq!(
        asked_so_far(&server),
        before,
        "the filter reached the server"
    );

    // Control: the person's own git, with that config, does ask it.
    std::fs::remove_file(dest.join("file0.bin")).unwrap();
    let mut control = std::process::Command::new("git");
    control
        .args(["checkout", "--", "file0.bin"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .current_dir(&dest);
    let out = crate::forklock::output(&mut control).unwrap();
    assert!(
        marker.exists(),
        "the helper was never one the filter would ask: {out:?}"
    );
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_asks_no_helper_where_the_command_line_empties_them() {
    // What `credential.helper=` off the network rests on: git-lfs's `git credential` inherits it.
    if !real_git_lfs() {
        return;
    }
    let (world, _server, _, dest) = landed("widget");
    let marker = world.home.path().join("a-helper-ran");
    let helper = format!("!f() {{ touch '{}'; }}; f", marker.display());
    plain_git(&dest, &["config", "--local", "credential.helper", &helper]);
    std::fs::remove_dir_all(dest.join(".git/lfs/objects")).unwrap();
    let fetch = |before: &[&str]| {
        let mut command = std::process::Command::new("git");
        command
            .args(before)
            .args(["lfs", "fetch"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .current_dir(&dest);
        crate::forklock::output(&mut command).unwrap()
    };

    let emptied = fetch(&["-c", "credential.helper="]);

    assert!(!emptied.status.success(), "{emptied:?}");
    assert!(
        String::from_utf8_lossy(&emptied.stderr).contains("credentials"),
        "{emptied:?}"
    );
    assert!(
        !marker.exists(),
        "git-lfs asked a helper the command line emptied"
    );

    // Control: left alone, it asks.
    let _ = fetch(&[]);
    assert!(
        marker.exists(),
        "the helper was never one git-lfs would ask"
    );
}

/// A repository purlis never wired, its own `.lfsconfig` naming `server`, with `object` committed
/// as the pointer at `large.bin`, whose object is not here.
fn not_wired(top: &Path, server: &LfsServer, object: &Object) -> PathBuf {
    let repo = top.join("not-wired");
    std::fs::create_dir(&repo).unwrap();
    plain_git(&repo, &["init", "-q", "-b", "main", "."]);
    std::fs::write(
        repo.join(".gitattributes"),
        "*.bin filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    std::fs::write(
        repo.join(".lfsconfig"),
        format!(
            "[lfs]\n\turl = http://127.0.0.1:{}/acme/widget.git/info/lfs\n",
            server.port
        ),
    )
    .unwrap();
    std::fs::write(repo.join("large.bin"), object.pointer()).unwrap();
    plain_git(&repo, &["add", "-A"]);
    plain_git(&repo, &["commit", "-q", "-m", "one"]);
    repo
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_filter_outside_a_wired_clone_asks_none_of_the_persons_helpers() {
    on_a_machine(
        &named(
            module_path!(),
            "real_git_lfs_a_filter_outside_a_wired_clone_asks_none_of_the_persons_helpers",
        ),
        |machine| {
            let object = Object::of(2, 41);
            let server = LfsServer::serving(vec![object.clone()], Sending::AtOnce);
            let dir = tempfile::tempdir().unwrap();
            let top = std::fs::canonicalize(dir.path()).unwrap();
            let repo = not_wired(&top, &server, &object);
            std::fs::remove_file(repo.join("large.bin")).unwrap();

            // Each way a call through purlis checks a file out.
            let back = git::run_untimed(&repo, &["checkout", "--", "large.bin"]).unwrap();
            assert!(back.ok(), "{back:?}");
            let folder = top.join("branch-folder");
            let folder_text = folder.to_str().unwrap();
            let cut = git::run_untimed(
                &repo,
                &["worktree", "add", "-q", "-b", "piece", folder_text],
            )
            .unwrap();
            assert!(cut.ok(), "{cut:?}");

            for checked_out in [repo.join("large.bin"), folder.join("large.bin")] {
                assert!(
                    std::fs::read(&checked_out).unwrap() == object.pointer().into_bytes(),
                    "{} is not its pointer",
                    checked_out.display()
                );
            }
            assert!(!machine.persons_helper_ran(), "purlis's git asked it");
            assert_eq!(server.batches(), 0, "the filter reached the server");

            // Control: the person's own git, on the same repository, does ask it.
            std::fs::remove_file(repo.join("large.bin")).unwrap();
            let mut control = std::process::Command::new("git");
            control
                .args(["checkout", "--", "large.bin"])
                .env("GIT_TERMINAL_PROMPT", "0")
                .current_dir(&repo);
            let out = crate::forklock::output(&mut control).unwrap();
            assert!(
                machine.persons_helper_ran(),
                "the helper was never one the filter would ask: {out:?}"
            );
            assert!(std::fs::read(repo.join("large.bin")).unwrap() == object.bytes);
        },
    );
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_writes_nothing_through_a_link_in_a_store_outside_a_wired_clone() {
    if !real_git_lfs() {
        return;
    }
    let object = Object::of(1, 42);
    let server = LfsServer::serving(vec![object.clone()], Sending::AtOnce);
    let dir = tempfile::tempdir().unwrap();
    let top = std::fs::canonicalize(dir.path()).unwrap();
    let repo = not_wired(&top, &server, &object);
    let elsewhere = top.join("chosen-by-the-chat");
    std::fs::create_dir(&elsewhere).unwrap();
    let _ = std::fs::remove_dir_all(repo.join(".git/lfs"));
    std::os::unix::fs::symlink(&elsewhere, repo.join(".git/lfs")).unwrap();
    std::fs::write(repo.join("large.bin"), Object::of(1, 43).bytes).unwrap();

    let read = git::run_untimed(&repo, &["diff", "--stat"]).unwrap();
    let added = git::run_untimed(&repo, &["add", "--", "large.bin"]).unwrap();

    assert!(read.ok() && added.ok(), "{read:?} {added:?}");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_downloading_slowly_past_the_stall_limit_is_not_stopped() {
    if !real_git_lfs() {
        return;
    }
    // 12 s in all, three stall limits, a chunk every half second. The small file ends first, and
    // git-lfs counts files, so its meter's own line moves the percent part way.
    let (small, large) = (Object::of(4, 7), Object::of(24, 3));
    let server = LfsServer::serving(
        vec![small.clone(), large.clone()],
        Sending::Slowly {
            gap: Duration::from_millis(500),
        },
    );
    let world = World::holding_real_large_files("widget", &[&small, &large], &server);

    let (code, said, seen, took) = world.real_clone("widget", STALL, true);

    assert_eq!(code, 0, "{said:?}");
    assert!(took > STALL * 2, "it never outlasted the limit: {took:?}");
    assert!(std::fs::read(world.dest("widget").join("file1.bin")).unwrap() == large.bytes);
    let percents: Vec<u8> = seen.iter().map(|(_, percent)| *percent).collect();
    assert!(percents.is_sorted(), "{seen:?}");
    // Only git-lfs's meter, read in its own words, can say this: the stage's start says 95, and
    // the landing 100.
    assert!(
        seen.iter()
            .any(|(stage, percent)| *stage == Stage::LargeFiles && (96..100).contains(percent)),
        "{seen:?}"
    );
}

/// A clone of a server that sends as `sending` is stopped at the limit, in LFS's words.
fn stalls(sending: Sending) {
    let object = Object::of(24, 4);
    let server = LfsServer::serving(vec![object.clone()], sending);
    let world = World::holding_real_large_files("widget", &[&object], &server);

    let (code, said, seen, took) = world.real_clone("widget", STALL, true);

    assert_eq!(code, 1, "{said:?}");
    let text = words(&said);
    assert!(
        text.contains("stalled") && text.contains("Git LFS"),
        "{text}"
    );
    // git-lfs's own 30 s timeout did not answer for it.
    assert!(took < Duration::from_secs(25), "{took:?}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
    assert!(
        seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_whose_download_stops_is_stopped_at_the_stall_limit() {
    if real_git_lfs() {
        stalls(Sending::Stops { chunks: 4 });
    }
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_whose_server_never_answers_is_stopped_at_the_stall_limit() {
    if real_git_lfs() {
        stalls(Sending::NoAnswer);
    }
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_with_no_credentials_for_its_server_fails_and_leaves_nothing() {
    if !real_git_lfs() {
        return;
    }
    let object = Object::of(1, 5);
    let server = LfsServer::serving(vec![object.clone()], Sending::AtOnce);
    let world = World::holding_real_large_files("widget", &[&object], &server);

    let (code, said, _, took) = world.real_clone("widget", Duration::from_secs(60), false);

    assert_eq!(code, 1, "{said:?}");
    assert!(
        took < Duration::from_secs(30),
        "it waited on a prompt: {took:?}"
    );
    assert!(server.batches() >= 1, "git-lfs never asked its server");
    assert_eq!(server.asked.downloads.load(Ordering::SeqCst), 0);
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
}

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn a_repo_with_no_large_files_never_meets_real_git_lfs() {
    if !real_git_lfs() {
        return;
    }
    let world = World::holding(&["widget"]);

    let (code, said, seen, _) = world.real_clone("widget", Duration::from_secs(60), true);

    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file());
    assert!(
        !seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
    assert_eq!(
        seen.last().map(|(_, percent)| *percent),
        Some(100),
        "{seen:?}"
    );
}
