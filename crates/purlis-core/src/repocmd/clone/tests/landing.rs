// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A clone the window watches (#4079), through [`clone`] itself, against bare repositories
//! standing in for a forge: the record's `https://` URL is sent to them by a
//! `url.<base>.insteadOf`. A script run in git's place says what a forge's failures say.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::super::*;
use super::plane;
use crate::forge::gitcredential::as_the_window;
use crate::forge::route::HostScope;
use crate::testgit;
use serde_json::json;

const HOST: &str = "forge.example";

/// A plane with `repos` in its inventory, each a bare repository of one commit.
struct World {
    _plane: tempfile::TempDir,
    root: PathBuf,
    forge: tempfile::TempDir,
    home: tempfile::TempDir,
}

impl World {
    fn holding(repos: &[&str]) -> Self {
        let (dir, root) = plane();
        std::fs::write(
            root.join("charter.toml"),
            format!("[[forge]]\nkind = \"gitlab\"\nhost = \"{HOST}\"\nowner = \"acme\"\n"),
        )
        .unwrap();
        let rows: Vec<Value> = repos
            .iter()
            .map(|name| {
                json!({
                    "name": name,
                    "path_with_namespace": format!("acme/{name}"),
                    "web_url": format!("https://{HOST}/acme/{name}"),
                    "forge": "gitlab",
                })
            })
            .collect();
        std::fs::create_dir_all(root.join("inventory")).unwrap();
        std::fs::write(
            root.join("inventory/repos.json"),
            json!({"group": "acme", "count": rows.len(), "repos": rows}).to_string(),
        )
        .unwrap();
        let forge = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(forge.path().join("acme")).unwrap();
        for name in repos {
            seed(forge.path(), name);
        }
        Self {
            _plane: dir,
            root,
            forge,
            home: tempfile::tempdir().unwrap(),
        }
    }

    fn ws(&self) -> PathBuf {
        self.root.join("workspaces/alpha")
    }

    fn dest(&self, name: &str) -> PathBuf {
        self.ws().join(name)
    }

    fn hidden(&self, name: &str) -> PathBuf {
        self.ws().join(format!(".cloning-{name}"))
    }

    fn bare(&self, name: &str) -> PathBuf {
        self.forge.path().join(format!("acme/{name}.git"))
    }

    fn sent_to_the_stand_in(&self) -> String {
        format!(
            "url.file://{}/acme/.insteadOf=https://{HOST}/acme/",
            self.forge.path().display()
        )
    }

    /// Clones `repos` as the window does: on its credential scope, with `watch`.
    fn clone_as_the_window(&self, repos: &[&str], watch: &Watch) -> (u8, Vec<Say>) {
        let repos: Vec<String> = repos.iter().map(|name| (*name).to_owned()).collect();
        let mut said = Vec::new();
        let code = as_the_window(
            &HostScope::for_a_test(),
            Path::new("/Applications/purlis.app/Contents/MacOS/charter-app"),
            self.home.path(),
            || {
                clone(
                    &Request {
                        root: &self.root,
                        ws: "alpha",
                        repos: &repos,
                        now: chrono::Utc::now(),
                        author: "tester",
                        hosts: None,
                        watch: Some(watch),
                    },
                    &mut |line| said.push(line),
                )
            },
        );
        (code, said)
    }

    /// Clones `repos` as the app does for a sandboxed chat (#4080): under git's isolation, held
    /// to the chat's hosts, with `watch` standing in for the forge.
    fn clone_for_a_chat(&self, repos: &[&str], watch: &Watch) -> (u8, Vec<Say>) {
        let repos: Vec<String> = repos.iter().map(|name| (*name).to_owned()).collect();
        let hosts = [HOST.to_owned()];
        let mut said = Vec::new();
        let code = git::isolated(&git::Isolated::default(), || {
            clone(
                &Request {
                    root: &self.root,
                    ws: "alpha",
                    repos: &repos,
                    now: chrono::Utc::now(),
                    author: "tester",
                    hosts: Some(&hosts),
                    watch: Some(watch),
                },
                &mut |line| said.push(line),
            )
        });
        (code, said)
    }

    /// [`Self::clone_as_the_window`] with nobody listening and nobody pressing Cancel.
    fn clone_plainly(&self, repos: &[&str]) -> (u8, Vec<Say>) {
        let off = AtomicBool::new(false);
        let heard = |_: Stage, _: u8| {};
        let watch = self.watching(&heard, &off, Duration::from_secs(60), None);
        self.clone_as_the_window(repos, &watch)
    }

    fn watching<'a>(
        &self,
        heard: &'a (dyn Fn(Stage, u8) + Sync),
        off: &'a AtomicBool,
        stall: Duration,
        script: Option<&str>,
    ) -> Watch<'a> {
        let mut watch = Watch::new(heard, off);
        watch.stand_in.config.push(self.sent_to_the_stand_in());
        watch.stand_in.stall = Some(stall);
        watch.stand_in.script = script.map(str::to_owned);
        watch
    }

    /// `upload-pack` that hangs: a FIFO where `name`'s refs are, which it waits on for ever.
    /// Dropping what this answers puts the refs back.
    fn silent(&self, name: &str) -> Silent {
        let refs = self.bare(name).join("packed-refs");
        let kept = refs.with_extension("kept");
        std::fs::rename(&refs, &kept).unwrap();
        let made =
            crate::forklock::status(std::process::Command::new("mkfifo").arg(&refs)).unwrap();
        assert!(made.success());
        Silent { refs, kept }
    }

    fn members(&self) -> Vec<String> {
        Plane::open(&self.root)
            .workspace("alpha")
            .unwrap()
            .manifest()
            .0
            .and_then(|doc| doc.get("repos").cloned())
            .and_then(|rows| rows.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|row| py_str(&row["name"]))
            .collect()
    }
}

/// A FIFO standing in for a repository's refs, until dropped.
struct Silent {
    refs: PathBuf,
    kept: PathBuf,
}

impl Drop for Silent {
    fn drop(&mut self) {
        let _ = std::fs::rename(&self.kept, &self.refs);
    }
}

fn seed(top: &Path, name: &str) {
    let work = top.join(format!("{name}-seed"));
    std::fs::create_dir(&work).unwrap();
    std::fs::write(work.join("README.md"), "# seed\n").unwrap();
    let ok = |run: git::Run| assert!(run.ok(), "{run:?}");
    ok(testgit::run(
        &work,
        &["-c", "init.defaultBranch=trunk", "init"],
    ));
    ok(testgit::run(&work, &["add", "."]));
    ok(testgit::run(
        &work,
        &[
            "-c",
            "user.name=Seed",
            "-c",
            "user.email=seed@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "first",
        ],
    ));
    let bare = format!("acme/{name}.git");
    ok(testgit::run(
        top,
        &["clone", "--bare", "--", &format!("{name}-seed"), &bare],
    ));
}

/// Every line said as one text, for what a person would read.
fn words(said: &[Say]) -> String {
    said.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn names_a_forge_cli(said: &str) -> bool {
    said.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "gh" || word == "glab")
}

#[test]
fn a_clone_lands_at_its_destination_with_one_percent_that_never_goes_back() {
    let world = World::holding(&["widget"]);
    let seen = Mutex::new(Vec::new());
    let heard = |stage: Stage, percent: u8| seen.lock().unwrap().push((stage, percent));
    let off = AtomicBool::new(false);
    let watch = world.watching(&heard, &off, Duration::from_secs(60), None);
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file(), "{said:?}");
    assert!(!world.hidden("widget").exists());
    assert_eq!(world.members(), ["widget"]);
    let seen = seen.into_inner().unwrap();
    let percents: Vec<u8> = seen.iter().map(|(_, percent)| *percent).collect();
    assert!(percents.is_sorted(), "{seen:?}");
    assert_eq!(percents.last(), Some(&100), "{seen:?}");
}

#[test]
fn a_window_clone_records_its_remote_though_a_worker_thread_runs_it() {
    let world = World::holding(&["widget"]);
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    let recorded = forge::gitcredential::read_push_remotes(world.home.path());
    assert_eq!(
        recorded.values().collect::<Vec<_>>(),
        [&format!("{HOST}/acme/widget")],
        "the worker thread was not the window's: {said:?}"
    );
}

#[test]
fn until_it_lands_the_repo_is_only_in_the_hidden_folder() {
    let world = World::holding(&["widget"]);
    let at_dest = Mutex::new(Vec::new());
    let heard = |_: Stage, _: u8| {
        at_dest.lock().unwrap().push((
            world.dest("widget").exists(),
            world.hidden("widget").is_dir(),
        ));
    };
    let off = AtomicBool::new(false);
    let watch = world.watching(&heard, &off, Duration::from_secs(60), None);
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    let mut at_dest = at_dest.into_inner().unwrap();
    // The last it hears is that it landed.
    assert_eq!(at_dest.pop(), Some((true, false)));
    assert!(!at_dest.is_empty());
    assert!(
        at_dest.iter().all(|seen| *seen == (false, true)),
        "{at_dest:?}"
    );
}

#[test]
fn a_repo_that_is_not_there_leaves_nothing_and_a_retry_really_clones() {
    let world = World::holding(&["widget"]);
    let bare = world.bare("widget");
    let moved = bare.with_extension("moved");
    std::fs::rename(&bare, &moved).unwrap();
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 1, "{said:?}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
    assert!(world.members().is_empty());
    assert!(words(&said).contains(HOST), "{said:?}");

    std::fs::rename(&moved, &bare).unwrap();
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file());
    assert!(!words(&said).contains("already cloned"), "{said:?}");
}

#[test]
fn a_clone_cancelled_part_way_stops_git_and_leaves_nothing() {
    let world = World::holding(&["widget"]);
    let silent = world.silent("widget");
    let off = Arc::new(AtomicBool::new(false));
    let press = {
        let off = off.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            off.store(true, Ordering::SeqCst);
        })
    };
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_secs(60), None);
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    press.join().unwrap();
    assert_eq!(code, 0, "a person's Cancel is no failure: {said:?}");
    assert!(watch.was_stopped(), "{said:?}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
    assert!(world.members().is_empty());
    drop(silent);
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file());
}

#[test]
fn a_silent_clone_is_stopped_at_the_stall_limit_says_so_and_leaves_nothing() {
    let world = World::holding(&["widget"]);
    let _silent = world.silent("widget");
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_millis(400), None);
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 1, "{said:?}");
    assert!(words(&said).contains("stalled"), "{said:?}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
}

#[test]
fn a_leftover_hidden_folder_is_ours_by_name_and_replaced() {
    let world = World::holding(&["widget"]);
    let left = world.hidden("widget");
    std::fs::create_dir_all(left.join(".git/objects")).unwrap();
    std::fs::write(left.join("half-a-file"), "x").unwrap();
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file());
    assert!(!world.dest("widget").join("half-a-file").exists());
    assert!(!left.exists());
}

#[test]
fn a_link_at_the_hidden_name_is_taken_away_and_never_followed() {
    let world = World::holding(&["widget"]);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("keep"), "x").unwrap();
    std::os::unix::fs::symlink(outside.path(), world.hidden("widget")).unwrap();
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file());
    assert!(outside.path().join("keep").is_file(), "{said:?}");
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1);
}

#[test]
fn a_clone_another_run_holds_is_left_to_it_and_said() {
    let world = World::holding(&["widget"]);
    let running = world.hidden("widget");
    std::fs::create_dir(&running).unwrap();
    std::fs::write(running.join("its-own"), "x").unwrap();
    let _held = crate::rewrite::Lock::try_on(&running).unwrap();
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 1, "{said:?}");
    assert!(words(&said).contains("already being cloned"), "{said:?}");
    assert!(running.join("its-own").is_file(), "the run it left to it");
}

/// A folder as a killed clone of an older version left it: a repository with no commit yet.
fn unfinished_at(dest: &Path) {
    std::fs::create_dir_all(dest).unwrap();
    let run = testgit::run(dest, &["-c", "init.defaultBranch=trunk", "init"]);
    assert!(run.ok(), "{run:?}");
}

#[test]
fn a_folder_an_older_clone_left_unfinished_is_replaced() {
    let world = World::holding(&["widget"]);
    unfinished_at(&world.dest("widget"));
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file(), "{said:?}");
    assert!(!words(&said).contains("already cloned"), "{said:?}");
}

#[test]
fn an_unfinished_folder_purlis_wired_its_layer_into_is_replaced_too() {
    // The older clone's own run wired what it left: it listed as a clone (#4079, finding 4).
    let world = World::holding(&["widget"]);
    let dest = world.dest("widget");
    unfinished_at(&dest);
    std::fs::create_dir_all(world.root.join(".claude/skills/one")).unwrap();
    std::fs::write(
        world.root.join(".claude/skills/one/SKILL.md"),
        "---\nname: one\ndescription: a skill\n---\n",
    )
    .unwrap();
    std::fs::write(
        world.root.join(".claude/settings.json"),
        "{\"permissions\": {\"allow\": []}}\n",
    )
    .unwrap();
    let wired = crate::guest::wire(&world.root, &dest);
    assert!(
        std::fs::read_dir(&dest).unwrap().count() > 1,
        "nothing was wired: {:?}",
        wired.rows.len()
    );
    let (code, said) = world.clone_plainly(&["widget"]);
    assert_eq!(code, 0, "{said:?}");
    assert!(dest.join("README.md").is_file(), "{said:?}");
}

#[test]
fn anything_else_at_the_destination_stays_and_is_never_deleted() {
    let world = World::holding(&["widget", "gadget", "doohickey"]);
    // No commit yet, and something of the person's in it.
    let theirs = world.dest("widget");
    unfinished_at(&theirs);
    std::fs::write(theirs.join("notes.txt"), "mine").unwrap();
    // A repository with a commit.
    let committed = world.dest("gadget");
    let run = testgit::run(
        &world.ws(),
        &[
            "clone",
            "--",
            &world.bare("gadget").display().to_string(),
            "gadget",
        ],
    );
    assert!(run.ok(), "{run:?}");
    // A folder that is no repository.
    let plain = world.dest("doohickey");
    std::fs::create_dir(&plain).unwrap();

    let (code, said) = world.clone_plainly(&["widget", "gadget", "doohickey"]);
    assert_eq!(code, 0, "{said:?}");
    let text = words(&said);
    assert_eq!(text.matches("already cloned").count(), 3, "{text}");
    assert!(theirs.join("notes.txt").is_file());
    assert!(!theirs.join("README.md").exists());
    assert!(committed.join("README.md").is_file());
    assert_eq!(std::fs::read_dir(&plain).unwrap().count(), 0);
}

#[test]
fn a_batch_with_one_failing_repo_lands_the_others_and_reports_only_that_one() {
    let world = World::holding(&["alpha-api", "broken", "gamma-web"]);
    std::fs::remove_dir_all(world.bare("broken")).unwrap();
    let (code, said) = world.clone_plainly(&["alpha-api", "broken", "gamma-web"]);
    assert_eq!(code, 1, "{said:?}");
    assert!(world.dest("alpha-api").join("README.md").is_file());
    assert!(world.dest("gamma-web").join("README.md").is_file());
    assert!(!world.dest("broken").exists());
    let failed: Vec<&Say> = said.iter().filter(|l| matches!(l, Say::Fail(_))).collect();
    assert_eq!(failed.len(), 1, "{said:?}");
    assert!(failed[0].to_string().contains("broken"), "{said:?}");
    assert_eq!(world.members(), ["alpha-api", "gamma-web"]);
}

#[test]
fn a_transfer_that_breaks_is_tried_once_more_over_http_1_1_and_lands() {
    let world = World::holding(&["widget"]);
    let tries = world.forge.path().join("tries");
    let script = format!(
        "echo \"$2\" >> '{tries}'\n\
         if [ \"$(wc -l < '{tries}')\" -lt 2 ]; then\n\
         mkdir \"$1/half-done\"\n\
         echo 'fatal: the remote end hung up unexpectedly' >&2; exit 128; fi\n\
         ls -A \"$1\" > '{tries}.found'; git init -q \"$1\"",
        tries = tries.display()
    );
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_secs(60), Some(&script));
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join(".git").is_dir(), "{said:?}");
    assert_eq!(
        std::fs::read_to_string(tries.with_extension("found")).unwrap(),
        "",
        "the second try started clean"
    );
    let asked = std::fs::read_to_string(&tries).unwrap();
    let asked: Vec<&str> = asked.lines().collect();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(!asked[0].contains("http.version"), "{asked:?}");
    assert!(asked[1].contains("http.version=HTTP/1.1"), "{asked:?}");
}

#[test]
fn a_failure_no_second_try_can_cure_is_tried_once() {
    let world = World::holding(&["widget"]);
    let tries = world.forge.path().join("tries");
    let script = format!(
        "echo x >> '{}'\necho \"fatal: Authentication failed for 'https://{HOST}/'\" >&2\n\
         exit 128",
        tries.display()
    );
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_secs(60), Some(&script));
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 1, "{said:?}");
    assert_eq!(
        std::fs::read_to_string(&tries).unwrap().lines().count(),
        1,
        "{said:?}"
    );
}

#[test]
fn an_auth_failure_says_sign_in_to_that_host_and_tells_the_window_which() {
    let world = World::holding(&["widget"]);
    let script =
        format!("echo \"fatal: Authentication failed for 'https://{HOST}/'\" >&2; exit 128");
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_secs(60), Some(&script));
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 1, "{said:?}");
    assert!(
        words(&said).contains(&format!("Sign in to {HOST}")),
        "{said:?}"
    );
    assert_eq!(watch.sign_in_to().as_deref(), Some(HOST));
}

#[test]
fn no_sentence_from_a_window_clone_names_a_forge_cli() {
    let world = World::holding(&["landed", "refused", "hidden", "gone", "broke", "said"]);
    let failing = |line: &str| format!("echo \"{line}\" >&2; exit 128");
    let refused = failing("fatal: Authentication failed for 'https://forge.example/'");
    // What a forge CLI that is not there leaves in git's words.
    let hidden = "echo '/bin/sh: gh: command not found' >&2; \
         echo \"fatal: could not read Username for 'https://forge.example': \
         terminal prompts disabled\" >&2; exit 128"
        .to_owned();
    let broke = failing("error: gh auth git-credential get: something else entirely");
    let said_line = failing("fatal: glab: not what anybody expected");
    std::fs::remove_dir_all(world.bare("gone")).unwrap();
    for (repo, script) in [
        ("landed", None),
        ("refused", Some(refused)),
        ("hidden", Some(hidden)),
        ("gone", None),
        ("broke", Some(broke)),
        ("said", Some(said_line)),
    ] {
        let off = AtomicBool::new(false);
        let heard = |_: Stage, _: u8| {};
        let watch = world.watching(&heard, &off, Duration::from_secs(60), script.as_deref());
        let (_, said) = world.clone_as_the_window(&[repo], &watch);
        let text = words(&said);
        assert!(!names_a_forge_cli(&text), "{repo}: {text}");
    }
}

// A chat's brokered clone (#4080): the same landing, its own words.

#[test]
fn a_chats_clone_lands_through_the_hidden_folder_too() {
    let world = World::holding(&["widget"]);
    let at_dest = Mutex::new(Vec::new());
    let heard = |_: Stage, _: u8| {
        at_dest.lock().unwrap().push((
            world.dest("widget").exists(),
            world.hidden("widget").is_dir(),
        ));
    };
    let off = AtomicBool::new(false);
    let watch = world.watching(&heard, &off, Duration::from_secs(60), None);
    let (code, said) = world.clone_for_a_chat(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join("README.md").is_file(), "{said:?}");
    assert!(!world.hidden("widget").exists());
    assert_eq!(world.members(), ["widget"]);
    let mut at_dest = at_dest.into_inner().unwrap();
    assert_eq!(at_dest.pop(), Some((true, false)), "{said:?}");
    assert!(!at_dest.is_empty(), "{said:?}");
    assert!(
        at_dest.iter().all(|seen| *seen == (false, true)),
        "{at_dest:?}"
    );
}

#[test]
fn a_chats_clone_that_keeps_making_progress_outlasts_the_stall_limit() {
    let world = World::holding(&["widget"]);
    let script = "i=0; while [ $i -lt 10 ]; do i=$((i+1)); \
         printf 'Receiving objects: %d%% (%d/10)\r' $((i*10)) $i >&2; sleep 0.1; done; \
         git init -q \"$1\"";
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_millis(400), Some(script));
    let (code, said) = world.clone_for_a_chat(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    assert!(world.dest("widget").join(".git").is_dir(), "{said:?}");
}

#[test]
fn a_chats_silent_clone_is_stopped_at_the_stall_limit_says_so_and_leaves_nothing() {
    let world = World::holding(&["widget"]);
    let _silent = world.silent("widget");
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let watch = world.watching(&heard, &off, Duration::from_millis(400), None);
    let (code, said) = world.clone_for_a_chat(&["widget"], &watch);
    assert_eq!(code, 1, "{said:?}");
    assert!(words(&said).contains("stalled"), "{said:?}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
}

/// What a chat's clone says when `script` stands in for git, with the forge's CLI on this
/// machine or not.
fn a_chats_failure(world: &World, script: &str, cli: bool) -> String {
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let mut watch = world.watching(&heard, &off, Duration::from_secs(60), Some(script));
    watch.stand_in.cli = Some(cli);
    let (code, said) = world.clone_for_a_chat(&["widget"], &watch);
    assert_eq!(code, 1, "{said:?}");
    words(&said)
}

#[test]
fn a_chats_clone_with_no_forge_cli_here_names_none() {
    let world = World::holding(&["widget"]);
    let refused =
        format!("echo \"fatal: Authentication failed for 'https://{HOST}/'\" >&2; exit 128");
    let hidden = format!(
        "echo '/bin/sh: glab: command not found' >&2; \
         echo \"fatal: could not read Username for 'https://{HOST}': terminal prompts \
         disabled\" >&2; exit 128"
    );
    let other = "echo 'error: glab auth git-credential get: broke' >&2; \
         echo 'fatal: something else' >&2; exit 128";
    for script in [refused.as_str(), hidden.as_str(), other] {
        let text = a_chats_failure(&world, script, false);
        assert!(!names_a_forge_cli(&text), "{text}");
        assert!(text.contains("widget"), "{text}");
    }
    // Refused: where it can be cloned instead, since a chat's clone never has the sign-in.
    let text = a_chats_failure(&world, &refused, false);
    assert!(text.contains(HOST), "{text}");
    assert!(text.contains("window"), "{text}");
}

#[test]
fn a_chats_refused_clone_with_the_forge_cli_here_says_to_sign_it_in() {
    let world = World::holding(&["widget"]);
    let refused =
        format!("echo \"fatal: Authentication failed for 'https://{HOST}/'\" >&2; exit 128");
    let text = a_chats_failure(&world, &refused, true);
    assert!(text.contains("glab auth login"), "{text}");
    assert!(!text.contains("Settings › You › Accounts"), "{text}");
}

#[test]
fn a_chats_clone_never_gets_the_persons_sign_in() {
    // #752: the account the person signed in with in the window signs the window's clones
    // alone, even where a chat's clone runs on the window's scope.
    use crate::forge::gitcredential::HELPER_ARG;
    let world = World::holding(&["widget"]);
    let home = super::account_on(HOST);
    let url = format!("https://{HOST}/acme/widget.git");
    let forge = Forge::for_clone(&json!({"forge": "gitlab"}), &url);
    let dest = world.dest("widget");
    let ws = world.ws();
    let route = landing::Route {
        root: &world.root,
        ws: "alpha",
        ws_dir: &ws,
        name: "widget",
        dest: &dest,
        url: &url,
        forge: &forge,
        watch: None,
    };
    let exe = Path::new("/Applications/purlis.app/Contents/MacOS/charter-app");
    let (window, chat) = as_the_window(&HostScope::for_a_test(), exe, home.path(), || {
        let window = route.helper().is_some_and(|h| h.contains(HELPER_ARG));
        let chat = git::isolated(&git::Isolated::default(), || route.helper())
            .is_some_and(|h| h.contains(HELPER_ARG));
        (window, chat)
    });
    assert!(window, "the window's clone is signed by the account");
    assert!(!chat, "a chat's clone never is");
}
