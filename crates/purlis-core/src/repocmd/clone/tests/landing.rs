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
    seed_with(top, name, &[], None);
}

/// [`seed`], with `files` beside the README, and `attributes` as its `.gitattributes`. That is
/// committed after what it names, so no filter of this machine's runs as the seed is made.
fn seed_with(top: &Path, name: &str, files: &[(&str, &str)], attributes: Option<&str>) {
    let work = top.join(format!("{name}-seed"));
    std::fs::create_dir(&work).unwrap();
    std::fs::write(work.join("README.md"), "# seed\n").unwrap();
    // An hour old, so git never takes a file for racily clean and hashes it again through the
    // attributes committed after it.
    let then = std::time::SystemTime::now() - Duration::from_secs(3600);
    for (file, text) in files {
        std::fs::write(work.join(file), text).unwrap();
        std::fs::File::options()
            .write(true)
            .open(work.join(file))
            .unwrap()
            .set_modified(then)
            .unwrap();
    }
    let ok = |run: git::Run| assert!(run.ok(), "{run:?}");
    let commit = |message: &str| {
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
                message,
            ],
        ));
    };
    ok(testgit::run(
        &work,
        &["-c", "init.defaultBranch=trunk", "init"],
    ));
    ok(testgit::run(&work, &["add", "."]));
    commit("first");
    if let Some(attributes) = attributes {
        std::fs::write(work.join(".gitattributes"), attributes).unwrap();
        ok(testgit::run(&work, &["add", "--", ".gitattributes"]));
        commit("attributes");
    }
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

// A chat's clone, heard in the session host and run in the window (#4161): the real asker over a
// real socket, its wait shortened in place of `A_CLONE_IS_SILENT_AT_MOST`.

/// What the asker of a chat's clone came to, with `script` standing in for git in the window and
/// the asker waiting `within` on each silence.
fn asked_through_the_host(
    script: &'static str,
    stall: Duration,
    within: Duration,
) -> (std::io::Result<crate::hookwire::Answer>, Arc<World>) {
    use crate::hookwire::{Answer, Ask, Asking, GitAsk, GitWork, Listener, relay};
    let world = Arc::new(World::holding(&["widget"]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hooks.sock");
    let listener = Listener::bind(dir.path(), &path).unwrap();
    let token = listener.tokens().issue_to_this_process(3).unwrap();
    let off = Arc::new(AtomicBool::new(false));
    let hearing = relay::to_a_window_answering(
        listener.tokens(),
        Box::new({
            let (world, off) = (Arc::clone(&world), Arc::clone(&off));
            move |_, _| {
                let heard = |_: Stage, _: u8| {};
                let watch = world.watching(&heard, &off, stall, Some(script));
                let (code, lines) = world.clone_for_a_chat(&["widget"], &watch);
                Answer::Said { lines, code }
            }
        }),
    );
    let _reading = listener.hear(hearing);
    let answered = Asking::on(&path, Some(token)).unwrap().ask(
        &Ask::Git(Box::new(GitAsk {
            chat: 3,
            workspace: "alpha".to_owned(),
            work: GitWork::Clone {
                repos: vec!["widget".to_owned()],
            },
            alive: true,
        })),
        within,
    );
    off.store(true, std::sync::atomic::Ordering::SeqCst);
    (answered, world)
}

#[test]
fn a_chats_clone_through_the_host_that_keeps_making_progress_outlasts_the_askers_wait() {
    // Four seconds of git speaking, four of the asker's waits.
    let script = "i=0; while [ $i -lt 40 ]; do i=$((i+1)); \
         printf 'Receiving objects: %d%% (%d/40)\r' $((i*100/40)) $i >&2; sleep 0.1; done; \
         git init -q \"$1\"";
    let (answered, world) =
        asked_through_the_host(script, Duration::from_secs(60), Duration::from_secs(1));
    let answer = answered.expect("answered, not given up on");
    assert!(
        matches!(answer, crate::hookwire::Answer::Said { code: 0, .. }),
        "{answer:?}"
    );
    assert!(world.dest("widget").join(".git").is_dir());
}

#[test]
fn a_chats_clone_through_the_host_that_shows_no_life_is_given_up_on_at_the_askers_wait() {
    let within = Duration::from_millis(800);
    let started = std::time::Instant::now();
    let (answered, _world) =
        asked_through_the_host("sleep 30", Duration::from_secs(60), within);
    let gave_up = answered.expect_err("given up on");
    assert!(
        matches!(
            gave_up.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ),
        "{gave_up:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(20), "it waited on");
}

#[test]
fn a_chats_stalled_clone_through_the_host_says_the_stall_to_its_asker() {
    // As the real bounds stand: the app's stall limit ends first, so its sentence is what is read.
    let (answered, world) = asked_through_the_host(
        "sleep 30",
        Duration::from_millis(500),
        Duration::from_secs(10),
    );
    let answer = answered.expect("answered");
    let crate::hookwire::Answer::Said { lines, code } = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(code, 1, "{lines:?}");
    assert!(words(&lines).contains("stalled"), "{lines:?}");
    assert!(!world.dest("widget").exists(), "{lines:?}");
}

// Git LFS (#4102): one large file silent past the stall limit.

/// A smudge filter standing in for git-lfs's: a second and a half per file, as a download is,
/// unless told to skip as git-lfs is (`GIT_LFS_SKIP_SMUDGE`).
const SLOW_SMUDGE: &str =
    "filter.lfs.smudge=sh -c '[ \"$GIT_LFS_SKIP_SMUDGE\" = 1 ] && exec cat; sleep 1.5; cat'";

impl World {
    /// [`World::holding`], with `big.bin` in `name` a Git LFS file.
    fn holding_a_large_file(name: &str) -> Self {
        Self::holding_seeded(name, &[("big.bin", "pointer\n")], "big.bin filter=lfs\n")
    }

    /// [`World::holding`], with `name` holding `files` under `attributes`.
    fn holding_seeded(name: &str, files: &[(&str, &str)], attributes: &str) -> Self {
        let world = Self::holding(&[]);
        let rows = json!([{
            "name": name,
            "path_with_namespace": format!("acme/{name}"),
            "web_url": format!("https://{HOST}/acme/{name}"),
            "forge": "gitlab",
        }]);
        std::fs::write(
            world.root.join("inventory/repos.json"),
            json!({"group": "acme", "count": 1, "repos": rows}).to_string(),
        )
        .unwrap();
        seed_with(world.forge.path(), name, files, Some(attributes));
        world
    }

    /// [`World::watching`], with the slow smudge filter, and git reading none of this machine's
    /// own config, whose LFS filter would run in the stand-in's place.
    fn watching_large_files<'a>(
        &self,
        heard: &'a (dyn Fn(Stage, u8) + Sync),
        off: &'a AtomicBool,
        stall: Duration,
    ) -> Watch<'a> {
        let mut watch = self.watching(heard, off, stall, None);
        watch.stand_in.config.push(SLOW_SMUDGE.to_owned());
        watch
            .stand_in
            .config
            .push("filter.lfs.required=true".to_owned());
        watch.stand_in.pass = vec![
            ("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".into()),
            ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".into()),
        ];
        watch
    }
}

/// `git lfs pull` as a stand-in: `lines` progress lines 0.1 s apart, then the file's content
/// written, as git-lfs draws its meter while one large file arrives. Then `after`.
fn lfs_pull(lines: u32, after: &str) -> String {
    format!(
        "i=0; while [ $i -lt {lines} ]; do i=$((i+1)); \\
         printf 'Downloading LFS objects:   0%% (0/1), %d KB | 1 MB/s\\r' $((i*100)) >&2; \\
         sleep 0.1; done; \\
         printf 'Downloading LFS objects: 100%% (1/1), 1 MB | 1 MB/s, done.\\n' >&2; \\
         {after}"
    )
}

#[test]
fn a_clone_whose_one_large_file_downloads_past_the_stall_limit_lands() {
    let world = World::holding_a_large_file("widget");
    let seen = Mutex::new(Vec::new());
    let heard = |stage: Stage, percent: u8| seen.lock().unwrap().push((stage, percent));
    let off = AtomicBool::new(false);
    let mut watch = world.watching_large_files(&heard, &off, Duration::from_millis(500));
    // 1.5 s in all, more than three stall limits, never silent for one.
    watch.stand_in.lfs = Some(lfs_pull(15, "echo fetched > \"$1/big.bin\""));
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    assert_eq!(
        std::fs::read_to_string(world.dest("widget").join("big.bin")).unwrap(),
        "fetched\n",
        "{said:?}"
    );
    assert!(!world.hidden("widget").exists());
    // The percent never goes back through it, and the large files are a stage of their own.
    let seen = seen.into_inner().unwrap();
    let percents: Vec<u8> = seen.iter().map(|(_, percent)| *percent).collect();
    assert!(percents.is_sorted(), "{seen:?}");
    assert!(
        seen.iter().any(|(stage, _)| *stage == Stage::LargeFiles),
        "{seen:?}"
    );
    assert_eq!(seen.last(), Some(&(Stage::LargeFiles, 100)), "{seen:?}");
}

#[test]
fn a_large_file_download_gone_dead_is_stopped_says_so_and_leaves_nothing() {
    let world = World::holding_a_large_file("widget");
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let mut watch = world.watching_large_files(&heard, &off, Duration::from_millis(500));
    watch.stand_in.lfs = Some(lfs_pull(2, "sleep 30"));
    let started = std::time::Instant::now();
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "it was not stopped"
    );
    assert_eq!(code, 1, "{said:?}");
    let text = words(&said);
    assert!(text.contains("stalled"), "{text}");
    assert!(text.contains("Git LFS"), "{text}");
    assert!(!world.dest("widget").exists(), "{said:?}");
    assert!(!world.hidden("widget").exists(), "{said:?}");
}

#[test]
fn a_clone_with_no_large_files_never_fetches_any() {
    let world = World::holding(&["widget"]);
    let ran = world.forge.path().join("lfs-ran");
    let off = AtomicBool::new(false);
    let heard = |_: Stage, _: u8| {};
    let mut watch = world.watching(&heard, &off, Duration::from_secs(60), None);
    watch.stand_in.lfs = Some(format!("touch '{}'", ran.display()));
    let (code, said) = world.clone_as_the_window(&["widget"], &watch);
    assert_eq!(code, 0, "{said:?}");
    assert!(!ran.exists(), "{said:?}");
}

mod real_lfs;
