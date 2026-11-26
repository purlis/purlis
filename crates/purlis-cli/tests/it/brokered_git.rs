// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis clone` and `purlis worktree add` in a chat the app started: handed to the app over
//! the chat's hook socket, run there by the same core functions, and printed here as the
//! terminal's own run prints them (#1335, ADR 0067 §2).
//!
//! **The app here is the core's broker on a real listener** (`purlis_core::gitbroker`, which
//! the app's answerer calls with its record of the chat), and the forge is a directory of bare
//! repositories that `$HOME/.gitconfig` rewrites GitHub's `acme` to, as `repo_commands.rs`
//! does. The broker's git reads THIS process's `HOME`, so the test that runs it re-runs itself
//! with `HOME` set ([`purlis_core::testrun::rerun`]).

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use purlis_core::gitbroker::{self, Asker};
use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, ChatToken, GitWork, Listener, Reading, SANDBOXED_ENV, SOCKET_ENV,
    Stage, TOKEN_ENV,
};
use purlis_core::worktree::git;
use serde_json::json;

const IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "Tester"),
    ("GIT_AUTHOR_EMAIL", "t@e.invalid"),
    ("GIT_COMMITTER_NAME", "Tester"),
    ("GIT_COMMITTER_EMAIL", "t@e.invalid"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// A home whose git config points GitHub's `acme` at a directory of bare repos, and planes
/// beside it.
struct World {
    base: PathBuf,
    home: PathBuf,
    forge: PathBuf,
}

impl World {
    fn at(base: &Path) -> World {
        let base = std::fs::canonicalize(base).unwrap();
        let home = base.join("home");
        let forge = base.join("forge");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(forge.join("acme")).unwrap();
        std::fs::write(
            home.join(".gitconfig"),
            // And a content filter of the operator's, which `widget`'s `.gitattributes` asks
            // for: it runs on a terminal's clone, and never on one the app makes for a chat.
            // The value is double-quoted: unquoted, git reads the `;` as the start of a
            // comment, the command is cut to `sh -c 'touch …`, and a filter that is not
            // `required` fails silently. It marks one absolute path, so whichever checkout
            // ran it, the test sees it.
            format!(
                "[url \"file://{}/acme/\"]\n\tinsteadOf = https://github.com/acme/\n\
                 [filter \"mark\"]\n\tsmudge = \"sh -c 'touch {}; cat'\"\n\tclean = cat\n",
                forge.display(),
                base.join(SMUDGED).display()
            ),
        )
        .unwrap();
        World { base, home, forge }
    }

    fn git(&self, dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("HOME", &self.home)
            .envs(IDENTITY)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// `acme/widget` on the stand-in forge, carrying the editor settings a sandboxed chat may
    /// not write: `.vscode/settings.json` and `.claude/settings.json`.
    fn widget(&self) {
        let src = self.forge.join("widget-src");
        std::fs::create_dir_all(src.join(".vscode")).unwrap();
        std::fs::create_dir_all(src.join(".claude")).unwrap();
        self.git(&src, &["init", "-q", "-b", "main", "."]);
        std::fs::write(src.join("README.md"), "hello\n").unwrap();
        std::fs::write(src.join(".vscode/settings.json"), "{}\n").unwrap();
        std::fs::write(src.join(".claude/settings.json"), "{}\n").unwrap();
        std::fs::write(src.join(".gitattributes"), "README.md filter=mark\n").unwrap();
        self.git(&src, &["add", "-A"]);
        self.git(&src, &["commit", "-q", "-m", "one"]);
        let bare = self.forge.join("acme/widget.git");
        self.git(
            &self.forge,
            &[
                "clone",
                "-q",
                "--bare",
                &src.display().to_string(),
                &bare.display().to_string(),
            ],
        );
    }

    /// A plane with workspace `alpha`, GitHub's `acme` as its forge, the sandbox on with
    /// `egress`, and `widget` in its inventory.
    fn plane(&self, name: &str, egress: &str) -> PathBuf {
        let root = self.base.join(name);
        std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(root.join("inventory")).unwrap();
        std::fs::write(
            root.join("charter.toml"),
            format!(
                "schema = 1\n\n[[forge]]\nkind = \"github\"\nowner = \"acme\"\n\n\
                 [sandbox]\nmode = \"on\"\negress = {egress}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("inventory/repos.json"),
            serde_json::to_string_pretty(&json!({"group": "acme", "count": 1, "repos": [{
                "name": "widget",
                "path_with_namespace": "acme/widget",
                "ssh_url": "git@github.com:acme/widget.git",
                "default_branch": "main",
                "kind": "app",
                "stack": "unknown",
                "description": "",
                "topics": [],
                "web_url": "https://github.com/acme/widget",
                "forge": "github"
            }]}))
            .unwrap(),
        )
        .unwrap();
        root
    }

    /// `purlis <args>` in `root`, with `chat` the env of a chat the app started, or none: the
    /// person's own terminal, which no app runs above (#6995), so it is started as an orphan.
    fn purlis(&self, root: &Path, args: &[&str], chat: &[(&str, &str)]) -> Output {
        let mut command = support::purlis();
        command
            .args(args)
            .current_dir(root)
            .env_clear()
            .env("CHARTER_ROOT", root)
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .env("USER", "tester")
            .envs(chat.iter().copied());
        if chat.is_empty() {
            return orphaned(&command, &self.base);
        }
        command.output().expect("the binary runs")
    }
}

/// `command` run as an orphan: started in the background by a shell that exits at once, so the
/// process that started it (this test, which plays the app) is not above it. What it said and
/// its status, through files under `scratch`; it reads nothing on its standard input.
fn orphaned(command: &Command, scratch: &Path) -> Output {
    use std::os::unix::process::ExitStatusExt;
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let id = format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let (out, err, code) = (
        scratch.join(format!("orphan-{id}.out")),
        scratch.join(format!("orphan-{id}.err")),
        scratch.join(format!("orphan-{id}.code")),
    );
    let mut shell = Command::new("/bin/sh");
    shell
        .arg("-c")
        .arg(r#"o=$1; e=$2; c=$3; shift 3; ( "$@" >"$o" 2>"$e" </dev/null; echo $? >"$c.part"; mv "$c.part" "$c" ) &"#)
        .arg("sh")
        .arg(&out)
        .arg(&err)
        .arg(&code)
        .arg(command.get_program())
        .args(command.get_args())
        .env_clear()
        .stdin(std::process::Stdio::null());
    if let Some(dir) = command.get_current_dir() {
        shell.current_dir(dir);
    }
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            shell.env(key, value);
        }
    }
    assert!(shell.status().unwrap().success());
    let until = std::time::Instant::now() + Duration::from_secs(120);
    while !code.exists() {
        assert!(
            std::time::Instant::now() < until,
            "the orphan never finished"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let status: i32 = std::fs::read_to_string(&code)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    Output {
        status: std::process::ExitStatus::from_raw(status << 8),
        stdout: std::fs::read(&out).unwrap_or_default(),
        stderr: std::fs::read(&err).unwrap_or_default(),
    }
}

/// An app listening at `<base>/app/hooks.sock` whose answer to every git ask is `answer`'s,
/// with chat 3's token, and every ask it was sent.
fn an_app(
    base: &Path,
    answer: impl Fn(Ask) -> Answer + Send + Sync + 'static,
) -> (Reading, ChatToken, PathBuf, mpsc::Receiver<Ask>) {
    let socket = base.join("app").join("hooks.sock");
    let listener = Listener::bind(base, &socket).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            answer(ask)
        }),
    );
    (reading, token, socket, rx)
}

/// The environment of chat 3, as the app starts it.
fn chat_3<'a>(socket: &'a Path, token: &'a ChatToken) -> [(&'a str, &'a str); 3] {
    [
        (SOCKET_ENV, socket.to_str().unwrap()),
        (CHAT_ENV, "3"),
        (TOKEN_ENV, token.expose()),
    ]
}

/// The core's broker, for chat 3 standing at `root`'s top, sandboxed, as persona `steward`.
///
/// Its git reads no global config, so the stand-in forge's rewrite is handed to it on top of
/// the identity, which is all the app hands it.
fn the_broker(root: PathBuf, forge: &Path) -> impl Fn(Ask) -> Answer + Send + Sync + 'static {
    the_broker_at(root.clone(), root, forge)
}

/// [`the_broker`], for chat 3 standing at `cwd`.
fn the_broker_at(
    root: PathBuf,
    cwd: PathBuf,
    forge: &Path,
) -> impl Fn(Ask) -> Answer + Send + Sync + 'static {
    let isolation = git::Isolated::identity(Some("Tester"), Some("t@e.invalid")).also(
        &format!("url.file://{}/acme/.insteadOf", forge.display()),
        "https://github.com/acme/",
    );
    move |ask| match ask {
        Ask::Git(asked) => gitbroker::answer(
            &root,
            &Asker {
                chat: 3,
                cwd: Some(cwd.clone()),
                persona: Some("steward".to_owned()),
                harnessed: true,
                unsandboxed: false,
                roots_git_held: false,
                policy: None,
                unreadable: Vec::new(),
                config: None,
                id: None,
                live: None,
            },
            &asked,
            &isolation,
            jiff::Timestamp::now(),
        ),
        _ => Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        },
    }
}

/// What the operator's global `mark` filter leaves in a world's base directory when it runs.
const SMUDGED: &str = "smudged";

/// Whether the operator's filter has run since this was last asked, forgetting that it had.
fn the_filter_ran(world: &World) -> bool {
    std::fs::remove_file(world.base.join(SMUDGED)).is_ok()
}

fn err(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// What `out` said, with `root` written as `<root>` so two planes' runs can be compared.
fn said(out: &Output, root: &Path) -> String {
    err(out).replace(&root.display().to_string(), "<root>")
}

/// What a brokered run says beside the terminal's (#1413): the note that git run by the app
/// ran no filter of the person's config, which the fixture's `.gitattributes` names.
const FILTER_NOTE: &str = "names the `mark` filter";

/// `said` split into the filter notes and everything else.
fn noted(said: &str) -> (Vec<String>, String) {
    let (notes, rest): (Vec<&str>, Vec<&str>) =
        said.lines().partition(|line| line.contains(FILTER_NOTE));
    (
        notes.into_iter().map(str::to_owned).collect(),
        rest.join("\n"),
    )
}

const IN_THE_CHILD: &str = "BROKERED_GIT_CHILD";

#[test]
fn a_sandboxed_chats_clone_and_worktree_are_made_by_the_app_and_say_what_the_terminal_says_and_the_filter_skipped()
 {
    purlis_core::unsteered!();
    let Some(base) = std::env::var_os(IN_THE_CHILD) else {
        let tmp = tempfile::tempdir().unwrap();
        let home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
        purlis_core::testrun::rerun(
            // Its libtest path, module first, as [`CREATING`].
            &[
                "brokered_git::a_sandboxed_chats_clone_and_worktree_are_made_by_the_app_and_say_what_the_terminal_says_and_the_filter_skipped",
            ],
            &[
                (IN_THE_CHILD, tmp.path().as_os_str()),
                ("HOME", home.as_os_str()),
            ],
        );
        return;
    };
    let world = World::at(Path::new(&base));
    world.widget();
    let terminal = world.plane("terminal", "[\"forge\"]");
    let brokered = world.plane("brokered", "[\"forge\"]");
    let (_reading, token, socket, asked) =
        an_app(&world.base, the_broker(brokered.clone(), &world.forge));
    let chat = chat_3(&socket, &token);

    // The clone: in a chat, the app makes it. The operator's global filter runs on the
    // terminal's clone, and not on the app's.
    let here = world.purlis(&terminal, &["clone", "widget", "-w", "alpha"], &[]);
    assert!(here.status.success(), "{}", err(&here));
    assert!(
        the_filter_ran(&world),
        "the operator's filter did not run on the terminal's clone, so this test checks \
         nothing: {}",
        err(&here)
    );
    let there = world.purlis(&brokered, &["clone", "widget", "-w", "alpha"], &chat);
    assert!(there.status.success(), "{}", err(&there));
    assert!(
        !the_filter_ran(&world),
        "a global-config filter ran on a clone made for a chat: {}",
        err(&there)
    );
    assert_eq!(there.status.code(), here.status.code());
    // The terminal's own words, and one note of the filter it skipped, which the
    // terminal's git ran (#1413).
    let (notes, rest) = noted(&said(&there, &brokered));
    let (none, terminal_said) = noted(&said(&here, &terminal));
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        none.is_empty(),
        "the terminal said a brokered note: {none:?}"
    );
    assert_eq!(rest, terminal_said);
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to clone");
    };
    assert_eq!(git.chat, 3);
    assert_eq!(git.workspace, "alpha");
    assert_eq!(
        git.work,
        GitWork::Clone {
            repos: vec!["widget".to_owned()]
        }
    );
    let clone = brokered.join("workspaces/alpha/repos/widget");
    for written in [
        ".git/config",
        ".vscode/settings.json",
        ".claude/settings.json",
    ] {
        assert!(
            clone.join(written).is_file(),
            "{written} is not in the clone"
        );
    }
    // And the workspace's own manifest records it, written by the app: a chat writes no
    // workspace's `workspace.json` itself.
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(brokered.join("workspaces/alpha/workspace.json"))
            .expect("the workspace's manifest"),
    )
    .expect("JSON");
    assert!(
        manifest["repos"]
            .as_array()
            .is_some_and(|repos| repos.iter().any(|repo| repo["name"] == "widget")),
        "{manifest}"
    );

    // The worktree: the same.
    let args = ["worktree", "add", "widget", "p1", "-w", "alpha"];
    let here = world.purlis(&terminal, &args, &[]);
    assert!(here.status.success(), "{}", err(&here));
    the_filter_ran(&world);
    let there = world.purlis(&brokered, &args, &chat);
    assert!(there.status.success(), "{}", err(&there));
    assert!(
        !the_filter_ran(&world),
        "a global-config filter ran on a worktree cut for a chat: {}",
        err(&there)
    );
    // The terminal's own words, and one note of the filter it skipped, which the
    // terminal's git ran (#1413).
    let (notes, rest) = noted(&said(&there, &brokered));
    let (none, terminal_said) = noted(&said(&here, &terminal));
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        none.is_empty(),
        "the terminal said a brokered note: {none:?}"
    );
    // But how to enter the new folder (#5770): the terminal is told to start a chat there,
    // and the sandboxed chat, which cannot write it, is told what works instead.
    let entering = |line: &&str| line.contains("enter:") || line.starts_with("•          ");
    let (chat_enters, rest): (Vec<&str>, Vec<&str>) = rest.lines().partition(entering);
    let (terminal_enters, terminal_said): (Vec<&str>, Vec<&str>) =
        terminal_said.lines().partition(entering);
    assert_eq!(rest, terminal_said);
    assert!(
        terminal_enters.concat().contains("EnterWorktree"),
        "{terminal_enters:?}"
    );
    let chat_enters = chat_enters.concat();
    assert!(!chat_enters.contains("EnterWorktree"), "{chat_enters}");
    assert!(chat_enters.contains("--in worktree"), "{chat_enters}");
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to cut a worktree");
    };
    assert_eq!(
        git.work,
        GitWork::WorktreeCut {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            branch: None,
            base: None,
        }
    );
    let piece = brokered.join("workspaces/alpha/.worktrees/widget/p1");
    assert!(piece.join(".vscode/settings.json").is_file(), "no piece");
    assert!(piece.join(".claude/settings.json").is_file(), "no piece");

    // Credited to the chat and its persona, by the app's record.
    let log = std::fs::read_dir(brokered.join("workspaces/alpha/pieces"))
        .expect("a piece log")
        .filter_map(Result::ok)
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
        .collect::<String>();
    let claimed: serde_json::Value =
        serde_json::from_str(log.lines().last().expect("a line")).expect("json");
    assert_eq!(claimed["event"], "claimed");
    assert_eq!(claimed["session"], "3");
    assert_eq!(claimed["persona"], "steward");
}

/// The test's libtest path, which [`purlis_core::testrun::rerun`] matches exactly: its module
/// in the crate's one test binary (`tests/it/main.rs`), then its name.
const CREATING: &str =
    "brokered_git::a_sandboxed_chats_create_with_repos_is_made_by_the_app_whole_and_only_there";

#[test]
fn a_sandboxed_chats_create_with_repos_is_made_by_the_app_whole_and_only_there() {
    // qw387: `workspace create <name> <repo>` in a sandboxed chat ran in the chat's own
    // process, said the workspace was ready, and made nothing.
    purlis_core::unsteered!();
    let Some(base) = std::env::var_os(IN_THE_CHILD) else {
        let tmp = tempfile::tempdir().unwrap();
        let home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
        purlis_core::testrun::rerun(
            &[CREATING],
            &[
                (IN_THE_CHILD, tmp.path().as_os_str()),
                ("HOME", home.as_os_str()),
            ],
        );
        return;
    };
    let world = World::at(Path::new(&base));
    world.widget();
    let root = world.plane("creating", "[\"forge\"]");
    // The chat works in `alpha`.
    let (_reading, token, socket, asked) = an_app(
        &world.base,
        the_broker_at(root.clone(), root.join("workspaces/alpha"), &world.forge),
    );
    let mut chat = chat_3(&socket, &token).to_vec();
    chat.push((SANDBOXED_ENV, "1"));

    // The workspace and its clone, made by the app in one ask.
    let made = world.purlis(
        &root,
        &[
            "workspace",
            "create",
            "beta",
            "widget",
            "--vision",
            "Ship it",
        ],
        &chat,
    );
    assert!(made.status.success(), "{}", err(&made));
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!(
            "the app was not asked to make the workspace: {}",
            err(&made)
        );
    };
    assert_eq!(git.workspace, "beta");
    assert_eq!(
        git.work,
        GitWork::Create {
            vision: Some("Ship it".to_owned()),
            repos: vec!["widget".to_owned()],
        }
    );
    let beta = root.join("workspaces/beta");
    let charter = std::fs::read_to_string(beta.join("workspace.md")).expect("a charter");
    assert!(charter.contains("Ship it"), "{charter}");
    for written in [".git/config", ".vscode/settings.json", "README.md"] {
        assert!(
            beta.join("repos/widget").join(written).is_file(),
            "{written} is not in the clone: {}",
            err(&made)
        );
    }

    // Made once: the same ask again is refused, and so is a clone into the workspace now that
    // it is there, as a clone into any workspace the chat does not work in is.
    let again = world.purlis(&root, &["workspace", "create", "beta", "widget"], &chat);
    assert_eq!(again.status.code(), Some(1), "{}", err(&again));
    assert!(err(&again).contains("already there"), "{}", err(&again));
    let cloned = world.purlis(&root, &["clone", "widget", "-w", "beta"], &chat);
    assert_eq!(cloned.status.code(), Some(1), "{}", err(&cloned));
    assert!(
        err(&cloned).contains("this chat works in workspace 'alpha'"),
        "{}",
        err(&cloned)
    );

    // What the app does not do for a chat is refused before anything is asked or made.
    while asked.try_recv().is_ok() {}
    for (args, names) in [
        (
            &["workspace", "create", "gamma", "--live"][..],
            "purlis workspace live gamma",
        ),
        (&["workspace", "create", "gamma", "--use"][..], "-w gamma"),
        (
            &["workspace", "create", "gamma", "widget", "--use", "--force"][..],
            "-w gamma",
        ),
    ] {
        let ran = world.purlis(&root, args, &chat);
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", err(&ran));
        assert!(err(&ran).contains("nothing was created"), "{}", err(&ran));
        assert!(err(&ran).contains(names), "{}", err(&ran));
        assert!(!err(&ran).contains("ready"), "{}", err(&ran));
    }
    assert!(asked.try_recv().is_err(), "the app was asked");
    assert!(!root.join("workspaces/gamma").exists());
}

/// The test's libtest path, as [`CREATING`].
const UPDATING: &str =
    "brokered_git::a_sandboxed_chats_update_and_push_of_its_branch_folder_are_run_by_the_app";

#[test]
fn a_sandboxed_chats_update_and_push_of_its_branch_folder_are_run_by_the_app() {
    // qw387: a chat may not write a branch folder's git directory, so its own fetch and rebase
    // fail there; `purlis worktree update` and `push` hand the work to the app.
    purlis_core::unsteered!();
    let Some(base) = std::env::var_os(IN_THE_CHILD) else {
        let tmp = tempfile::tempdir().unwrap();
        let home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
        purlis_core::testrun::rerun(
            &[UPDATING],
            &[
                (IN_THE_CHILD, tmp.path().as_os_str()),
                ("HOME", home.as_os_str()),
            ],
        );
        return;
    };
    let world = World::at(Path::new(&base));
    world.widget();
    let root = world.plane("updating", "[\"forge\"]");
    for args in [
        &["clone", "widget", "-w", "alpha"][..],
        &["worktree", "add", "widget", "p1", "-w", "alpha"][..],
    ] {
        let made = world.purlis(&root, args, &[]);
        assert!(made.status.success(), "{args:?}: {}", err(&made));
    }
    let piece = root.join("workspaces/alpha/.worktrees/widget/p1");
    let (_reading, token, socket, asked) = an_app(
        &world.base,
        the_broker_at(root.clone(), root.join("workspaces/alpha"), &world.forge),
    );
    let run = |args: &[&str]| {
        support::purlis()
            .args(args)
            .current_dir(&piece)
            .env_clear()
            .env("CHARTER_ROOT", &root)
            .env("HOME", &world.home)
            .env("PATH", "/usr/bin:/bin")
            .env("USER", "tester")
            .env(SANDBOXED_ENV, "1")
            .envs(chat_3(&socket, &token))
            .output()
            .expect("the binary runs")
    };

    let ran = run(&["worktree", "update"]);
    assert!(ran.status.success(), "{}", err(&ran));
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to update: {}", err(&ran));
    };
    assert_eq!(git.workspace, "alpha");
    assert_eq!(
        git.work,
        GitWork::WorktreeUpdate {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            step: hookwire::UpdateStep::Start { base: None },
        }
    );
    assert!(err(&ran).contains("already on main"), "{}", err(&ran));

    let ran = run(&["worktree", "push"]);
    assert!(ran.status.success(), "{}", err(&ran));
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to push: {}", err(&ran));
    };
    assert_eq!(
        git.work,
        GitWork::WorktreePush {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            lease: false,
        }
    );
    assert_eq!(
        git_said(
            &world.forge.join("acme/widget.git"),
            &["rev-parse", "refs/heads/p1"]
        ),
        git_said(&piece, &["rev-parse", "HEAD"]),
        "{}",
        err(&ran)
    );

    // #5253: the merge is the app's to run too, told apart from a rebase on the wire.
    let ran = run(&["worktree", "update", "--merge", "--take-base-baselines"]);
    assert!(ran.status.success(), "{}", err(&ran));
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to merge: {}", err(&ran));
    };
    assert_eq!(
        git.work,
        GitWork::WorktreeUpdate {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            step: hookwire::UpdateStep::Merge {
                base: None,
                take_base_baselines: true,
            },
        }
    );
    assert!(
        err(&ran).contains("already has everything on main"),
        "{}",
        err(&ran)
    );
    // Which files take the base's copy is only ever said of a merge.
    let ran = run(&["worktree", "update", "--take-base-baselines"]);
    assert!(!ran.status.success(), "{}", err(&ran));
    assert!(
        asked.recv_timeout(Duration::from_millis(200)).is_err(),
        "the app was asked to rebase on a line that asked for a merge's option"
    );

    // #3831: standing in the clone itself, the update is the clone's, and the app runs it.
    let clone = root.join("workspaces/alpha/repos/widget");
    let ran = support::purlis()
        .args(["worktree", "update"])
        .current_dir(&clone)
        .env_clear()
        .env("CHARTER_ROOT", &root)
        .env("HOME", &world.home)
        .env("PATH", "/usr/bin:/bin")
        .env("USER", "tester")
        .env(SANDBOXED_ENV, "1")
        .envs(chat_3(&socket, &token))
        .output()
        .expect("the binary runs");
    assert!(ran.status.success(), "{}", err(&ran));
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to update the clone: {}", err(&ran));
    };
    assert_eq!(git.workspace, "alpha");
    assert_eq!(
        git.work,
        GitWork::CloneUpdate {
            repo: "widget".to_owned(),
            step: hookwire::UpdateStep::Start { base: None },
        }
    );
    assert!(err(&ran).contains("already up to date"), "{}", err(&ran));
}

#[test]
fn a_host_outside_the_chats_internet_access_is_refused_and_never_cloned() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    // No preset: the chat reaches no host, so the forge's is refused before git runs. The
    // broker's git would read the suite's HOME, which is why nothing here may reach it.
    let root = world.plane("closed", "[]");
    let (_reading, token, socket, _asked) =
        an_app(&world.base, the_broker(root.clone(), &world.forge));

    let ran = world.purlis(
        &root,
        &["clone", "widget", "-w", "alpha"],
        &chat_3(&socket, &token),
    );

    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(err(&ran).contains("'github.com'"), "{}", err(&ran));
    assert!(err(&ran).contains("sandbox"), "{}", err(&ran));
    assert!(!root.join("workspaces/alpha/repos/widget").exists());
}

#[test]
fn a_git_action_the_app_refuses_is_refused_here_and_never_run_here() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    world.widget();
    let root = world.plane("refused", "[\"forge\"]");
    let (_reading, token, socket, _asked) = an_app(&world.base, |_| Answer::No {
        why: "this chat works in workspace 'beta'".to_owned(),
    });

    for args in [
        &["clone", "widget", "-w", "alpha"][..],
        &["worktree", "add", "widget", "p1", "-w", "alpha"][..],
    ] {
        let ran = world.purlis(&root, args, &chat_3(&socket, &token));
        assert_eq!(ran.status.code(), Some(1), "{args:?}: {}", err(&ran));
        assert!(
            err(&ran).contains("the app did not run this for the chat: this chat works in"),
            "{}",
            err(&ran)
        );
    }
    assert!(!root.join("workspaces/alpha/repos/widget").exists());
}

#[test]
fn where_no_app_takes_the_ask_the_command_runs_here() {
    // An app with nothing that answers asks, and no app at all: the clone is this process's,
    // as it always was. Its git reads the HOME this test hands the binary.
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    world.widget();
    let root = world.plane("unanswered", "[\"forge\"]");
    let taken = Arc::new(Mutex::new(0));
    let count = Arc::clone(&taken);
    let (_reading, token, socket, _asked) = an_app(&world.base, move |_| {
        *count.lock().unwrap() += 1;
        Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        }
    });

    let ran = world.purlis(
        &root,
        &["clone", "widget", "-w", "alpha"],
        &chat_3(&socket, &token),
    );

    assert!(ran.status.success(), "{}", err(&ran));
    assert_eq!(*taken.lock().unwrap(), 1, "the app was not asked first");
    assert!(root.join("workspaces/alpha/repos/widget/.git").exists());

    let alone = world.plane("alone", "[\"forge\"]");
    let ran = world.purlis(&alone, &["clone", "widget", "-w", "alpha"], &[]);
    assert!(ran.status.success(), "{}", err(&ran));
    assert!(alone.join("workspaces/alpha/repos/widget/.git").exists());
}

/// The core's commit broker, for sandboxed chat 3 standing in branch folder `cwd`, as persona
/// `steward`: what the app's answerer calls with its record of the chat. Every other ask is
/// one nothing answers.
fn the_commit_broker(
    root: PathBuf,
    cwd: PathBuf,
) -> impl Fn(Ask) -> Answer + Send + Sync + 'static {
    let identity = git::Isolated::identity(Some("Tester"), Some("t@e.invalid"));
    move |ask| match ask {
        Ask::Commit(asked) => gitbroker::commit::answer(
            &root,
            &Asker {
                chat: 3,
                cwd: Some(cwd.clone()),
                persona: Some("steward".to_owned()),
                harnessed: true,
                unsandboxed: false,
                roots_git_held: false,
                policy: None,
                unreadable: Vec::new(),
                config: None,
                id: None,
                live: None,
            },
            &asked,
            &identity,
        ),
        _ => Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        },
    }
}

/// What git answers in `dir`, read by the test's own git.
fn git_said(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", err(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn a_sandboxed_chats_commit_in_its_branch_folder_is_made_by_the_app_and_its_answer_printed() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    world.widget();
    let root = world.plane("committing", "[\"forge\"]");
    // The clone and the branch folder, made from the person's own terminal.
    for args in [
        &["clone", "widget", "-w", "alpha"][..],
        &["worktree", "add", "widget", "p1", "-w", "alpha"][..],
    ] {
        let made = world.purlis(&root, args, &[]);
        assert!(made.status.success(), "{args:?}: {}", err(&made));
    }
    let piece = root.join("workspaces/alpha/.worktrees/widget/p1");
    let branch = git_said(&piece, &["branch", "--show-current"]);
    let before = git_said(&piece, &["rev-parse", "HEAD"]);
    std::fs::write(piece.join("notes.md"), "a note\n").unwrap();
    let (_reading, token, socket, asked) =
        an_app(&world.base, the_commit_broker(root.clone(), piece.clone()));
    let commit = |args: &[&str]| {
        support::purlis()
            .args(args)
            .current_dir(&piece)
            .env_clear()
            .env("CHARTER_ROOT", &root)
            .env("HOME", &world.home)
            .env("PATH", "/usr/bin:/bin")
            .env("USER", "tester")
            .env(SANDBOXED_ENV, "1")
            .envs(chat_3(&socket, &token))
            .output()
            .expect("the binary runs")
    };
    let printed = |out: &Output| {
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };

    // The command asks, the app commits on the folder's branch, and its answer is printed.
    let ran = commit(&["worktree", "commit", "-m", "notes: one", "notes.md"]);
    assert_eq!(ran.status.code(), Some(0), "{}", printed(&ran));
    let Ok(Ask::Commit(sent)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to commit: {}", printed(&ran));
    };
    assert_eq!(sent.chat, 3);
    assert_eq!(sent.message, "notes: one");
    assert_eq!(sent.stage, Stage::Paths(vec!["notes.md".to_owned()]));
    let after = git_said(&piece, &["rev-parse", "HEAD"]);
    assert_ne!(after, before, "nothing was committed: {}", printed(&ran));
    assert_eq!(git_said(&piece, &["branch", "--show-current"]), branch);
    assert_eq!(
        git_said(&piece, &["log", "-1", "--format=%s"]),
        "notes: one"
    );
    assert_eq!(
        git_said(&piece, &["show", "--format=", "--name-only", "HEAD"]),
        "notes.md"
    );
    assert!(
        printed(&ran).contains(&format!("committed {after} on {branch}: 1 file")),
        "{}",
        printed(&ran)
    );

    // A commit the app refuses is refused here in the app's words, and nothing is committed.
    let refused = commit(&["worktree", "commit", "-m", "again", "--all"]);
    assert_eq!(refused.status.code(), Some(1), "{}", printed(&refused));
    assert!(
        printed(&refused)
            .contains("the app did not run this for the chat: there is nothing to commit"),
        "{}",
        printed(&refused)
    );
    assert_eq!(git_said(&piece, &["rev-parse", "HEAD"]), after);
}

#[test]
fn a_commit_run_in_a_branch_folder_takes_its_paths_from_there_for_a_chat_above_it() {
    // #3927: the chat stands in its workspace, and the command runs in the branch folder
    // (`env -C <folder>`, or a shell standing in it): its paths are the folder's, as git's are.
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    world.widget();
    let root = world.plane("committing-from-above", "[\"forge\"]");
    for args in [
        &["clone", "widget", "-w", "alpha"][..],
        &["worktree", "add", "widget", "p1", "-w", "alpha"][..],
    ] {
        let made = world.purlis(&root, args, &[]);
        assert!(made.status.success(), "{args:?}: {}", err(&made));
    }
    let piece = root.join("workspaces/alpha/.worktrees/widget/p1");
    std::fs::create_dir_all(piece.join("docs")).unwrap();
    std::fs::write(piece.join("docs/one.md"), "one\n").unwrap();
    std::fs::write(piece.join("two.md"), "two\n").unwrap();
    std::fs::write(piece.join("three.md"), "three\n").unwrap();
    let (_reading, token, socket, asked) = an_app(
        &world.base,
        the_commit_broker(root.clone(), root.join("workspaces/alpha")),
    );
    let commit = |from: &Path, args: &[&str]| {
        support::purlis()
            .args(args)
            .current_dir(from)
            .env_clear()
            .env("CHARTER_ROOT", &root)
            .env("HOME", &world.home)
            .env("PATH", "/usr/bin:/bin")
            .env("USER", "tester")
            .env(SANDBOXED_ENV, "1")
            .envs(chat_3(&socket, &token))
            .output()
            .expect("the binary runs")
    };
    let printed = |out: &Output| {
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };

    // From below the folder's top, a path as git takes it there, and a message that starts
    // with two dashes.
    let ran = commit(
        &piece.join("docs"),
        &[
            "worktree",
            "commit",
            "-m",
            "--docs: one",
            "one.md",
            "../two.md",
        ],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", printed(&ran));
    let Ok(Ask::Commit(sent)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to commit: {}", printed(&ran));
    };
    assert_eq!(sent.message, "--docs: one");
    assert_eq!(
        sent.stage,
        Stage::Paths(vec!["docs/one.md".to_owned(), "two.md".to_owned()])
    );
    assert_eq!(
        git_said(&piece, &["show", "--format=", "--name-only", "HEAD"]),
        "docs/one.md\ntwo.md"
    );

    // From the top, a path spelled from the workspace still names the same file.
    let ran = commit(
        &piece,
        &[
            "worktree",
            "commit",
            "-m",
            "three",
            ".worktrees/widget/p1/three.md",
        ],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", printed(&ran));
    assert!(asked.recv_timeout(Duration::from_secs(5)).is_ok());
    assert_eq!(
        git_said(&piece, &["show", "--format=", "--name-only", "HEAD"]),
        "three.md"
    );

    // A path that leaves the folder is refused here, naming what works, and nothing is asked.
    let ran = commit(&piece, &["worktree", "commit", "-m", "x", "../p2/a.md"]);
    assert_eq!(ran.status.code(), Some(2), "{}", printed(&ran));
    assert!(
        printed(&ran).contains(".worktrees/widget/p1/<path>"),
        "{}",
        printed(&ran)
    );
    assert!(asked.recv_timeout(Duration::from_millis(200)).is_err());
}

const REMOVING: &str = "brokered_git::a_sandboxed_chats_finished_branch_folder_is_removed_by_the_app_when_nothing_would_be_lost";

#[test]
fn a_sandboxed_chats_finished_branch_folder_is_removed_by_the_app_when_nothing_would_be_lost() {
    // #3879: the chat's own cleanup after a merge stopped on the person. The app removes the
    // folder for it, never forced, and only where no change and no commit would be lost.
    purlis_core::unsteered!();
    let Some(base) = std::env::var_os(IN_THE_CHILD) else {
        let tmp = tempfile::tempdir().unwrap();
        let home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
        purlis_core::testrun::rerun(
            &[REMOVING],
            &[
                (IN_THE_CHILD, tmp.path().as_os_str()),
                ("HOME", home.as_os_str()),
            ],
        );
        return;
    };
    let world = World::at(Path::new(&base));
    world.widget();
    let root = world.plane("removing", "[\"forge\"]");
    for args in [
        &["clone", "widget", "-w", "alpha"][..],
        &["worktree", "add", "widget", "done", "-w", "alpha"][..],
        &["worktree", "add", "widget", "dirty", "-w", "alpha"][..],
    ] {
        let made = world.purlis(&root, args, &[]);
        assert!(made.status.success(), "{args:?}: {}", err(&made));
    }
    let pieces = root.join("workspaces/alpha/.worktrees/widget");
    std::fs::write(pieces.join("dirty/README.md"), "changed\n").unwrap();
    let workspace = root.join("workspaces/alpha");
    let (_reading, token, socket, asked) = an_app(
        &world.base,
        the_broker_at(root.clone(), workspace.clone(), &world.forge),
    );
    let remove = |from: &Path, args: &[&str]| {
        support::purlis()
            .args(args)
            .current_dir(from)
            .env_clear()
            .env("CHARTER_ROOT", &root)
            .env("HOME", &world.home)
            .env("PATH", "/usr/bin:/bin")
            .env("USER", "tester")
            .env(SANDBOXED_ENV, "1")
            .envs(chat_3(&socket, &token))
            .output()
            .expect("the binary runs")
    };

    // A clean folder holding no commit of its own: removed by the app.
    let ran = remove(
        &workspace,
        &["worktree", "remove", "widget", "done", "-w", "alpha"],
    );
    assert!(ran.status.success(), "{}", err(&ran));
    let Ok(Ask::Git(git)) = asked.recv_timeout(Duration::from_secs(5)) else {
        panic!("the app was not asked to remove: {}", err(&ran));
    };
    assert_eq!(git.workspace, "alpha");
    assert_eq!(
        git.work,
        GitWork::WorktreeRemove {
            repo: "widget".to_owned(),
            piece: "done".to_owned(),
            delete_branch: false,
        }
    );
    assert!(err(&ran).contains("Removed widget · done"), "{}", err(&ran));
    assert!(!pieces.join("done").exists());

    // An uncommitted change: refused by the app, naming it, and the folder stays.
    let ran = remove(&workspace, &["worktree", "remove", "widget", "dirty"]);
    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(err(&ran).contains("README.md"), "{}", err(&ran));
    assert!(asked.recv_timeout(Duration::from_secs(5)).is_ok());
    assert!(pieces.join("dirty/README.md").is_file());

    // Forced: the person's, from the window; nothing is asked of the app.
    let ran = remove(
        &workspace,
        &["worktree", "remove", "widget", "dirty", "--force"],
    );
    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(err(&ran).contains("from the window"), "{}", err(&ran));
    assert!(asked.recv_timeout(Duration::from_millis(200)).is_err());
    assert!(pieces.join("dirty/README.md").is_file());
}

#[test]
#[cfg(unix)]
fn a_chat_commits_with_the_command_with_and_without_the_sandbox() {
    /// The test's libtest path, as [`CREATING`].
    const COMMITTING: &str =
        "brokered_git::a_chat_commits_with_the_command_with_and_without_the_sandbox";
    // #4534: a chat with no sandbox commits by itself, with the repository's hooks, and pushes
    // through the app; a sandboxed one commits through the app, with none; and the project's
    // setting brings back the refusal of the first alone.
    purlis_core::unsteered!();
    let Some(base) = std::env::var_os(IN_THE_CHILD) else {
        let tmp = tempfile::tempdir().unwrap();
        let home = std::fs::canonicalize(tmp.path()).unwrap().join("home");
        purlis_core::testrun::rerun(
            &[COMMITTING],
            &[
                (IN_THE_CHILD, tmp.path().as_os_str()),
                ("HOME", home.as_os_str()),
            ],
        );
        return;
    };
    let world = World::at(Path::new(&base));
    world.widget();
    let root = world.plane("committing", "[\"forge\"]");
    for args in [
        &["clone", "widget", "-w", "alpha"][..],
        &["worktree", "add", "widget", "p1", "-w", "alpha"][..],
    ] {
        let made = world.purlis(&root, args, &[]);
        assert!(made.status.success(), "{args:?}: {}", err(&made));
    }
    let piece = root.join("workspaces/alpha/.worktrees/widget/p1");
    // A hook of the repository's own: a linked worktree runs its clone's.
    let hook_ran = world.base.join("hook-ran");
    let hook = root.join("workspaces/alpha/repos/widget/.git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(
        &hook,
        format!("#!/bin/sh\ntouch '{}'\n", hook_ran.display()),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let git_asks = the_broker_at(root.clone(), piece.clone(), &world.forge);
    let (at_root, at_piece) = (root.clone(), piece.clone());
    let (_reading, token, socket, asked) = an_app(&world.base, move |ask| match ask {
        Ask::Commit(commit) => gitbroker::commit::answer(
            &at_root,
            &Asker {
                chat: 3,
                cwd: Some(at_piece.clone()),
                persona: Some("steward".to_owned()),
                harnessed: true,
                unsandboxed: false,
                roots_git_held: false,
                policy: None,
                unreadable: Vec::new(),
                config: None,
                id: None,
                live: None,
            },
            &commit,
            &git::Isolated::identity(Some("Tester"), Some("t@e.invalid")),
        ),
        other => git_asks(other),
    });
    let run = |args: &[&str], sandboxed: bool| {
        let mut command = support::purlis();
        command
            .args(args)
            .current_dir(&piece)
            .env_clear()
            .env("CHARTER_ROOT", &root)
            .env("HOME", &world.home)
            .env("PATH", "/usr/bin:/bin")
            .env("USER", "tester")
            .envs(IDENTITY)
            .envs(chat_3(&socket, &token));
        if sandboxed {
            command.env(SANDBOXED_ENV, "1");
        }
        command.output().expect("the binary runs")
    };
    let head = || git_said(&piece, &["rev-parse", "HEAD"]);
    let write = |name: &str| std::fs::write(piece.join(name), "new\n").unwrap();
    let was_committed = |name: &str| {
        git_said(&piece, &["show", "--format=", "--name-only", "HEAD"])
            .lines()
            .any(|file| file == name)
    };

    // With no sandbox: its own git, the hook, and no commit asked of the app.
    write("a.txt");
    let ran = run(&["worktree", "commit", "-m", "plain", "a.txt"], false);
    assert!(ran.status.success(), "{}", err(&ran));
    assert!(was_committed("a.txt"));
    assert!(hook_ran.is_file(), "the repository's hook did not run");
    std::fs::remove_file(&hook_ran).unwrap();
    while let Ok(ask) = asked.try_recv() {
        assert!(!matches!(ask, Ask::Commit(_)), "{ask:?}");
    }

    // Nothing named differs: said, and no commit.
    let before = head();
    let ran = run(&["worktree", "commit", "-m", "again", "a.txt"], false);
    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(err(&ran).contains("nothing to commit"), "{}", err(&ran));
    assert_eq!(head(), before);

    // Its push is the app's, in a chat with no sandbox too.
    let ran = run(&["worktree", "push"], false);
    assert!(ran.status.success(), "{}", err(&ran));
    assert_eq!(
        git_said(
            &world.forge.join("acme/widget.git"),
            &["rev-parse", "refs/heads/p1"]
        ),
        head()
    );

    // Sandboxed: through the app, with no hook.
    write("b.txt");
    let ran = run(&["worktree", "commit", "-m", "brokered", "b.txt"], true);
    assert!(ran.status.success(), "{}", err(&ran));
    assert!(was_committed("b.txt"));
    assert!(!hook_ran.exists(), "a brokered commit ran a hook");

    // The setting: no sandbox is refused and nothing is made; sandboxed goes on as before.
    let manifest = root.join("charter.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!("{text}\n[plane]\nrefuse_unsandboxed_commit = true\n"),
    )
    .unwrap();
    write("c.txt");
    let before = head();
    let ran = run(&["worktree", "commit", "-m", "refused", "c.txt"], false);
    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(
        err(&ran).contains("refuse_unsandboxed_commit"),
        "{}",
        err(&ran)
    );
    assert_eq!(head(), before);
    let ran = run(&["worktree", "commit", "-m", "still", "c.txt"], true);
    assert!(ran.status.success(), "{}", err(&ran));
    assert!(was_committed("c.txt"));
}

// ---- #6995: an ask believes only the app the chat runs under ----

/// What the stand-in app answers every git ask in the cells below, so a run shows it was heard.
const HEARD_BY_THE_APP: &str = "heard by the app the chat runs under";

/// What a fake listener answers every line: that the app ran it, and it went well.
const FAKE_ALLOWED: &str = r#"{"said":{"lines":[],"code":0}}"#;

/// A listener that is not the app, at `socket`: a `python3` that answers every line with
/// [`FAKE_ALLOWED`] and writes `heard` beside it for each, until the test ends it. Waits until it
/// listens.
struct Fake {
    child: std::process::Child,
    heard: PathBuf,
}

impl Fake {
    fn at(socket: &Path) -> Fake {
        std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
        let heard = socket.with_extension("heard");
        let child = Command::new("python3")
            .args([
                "-c",
                "import socket, sys\n\
                 s = socket.socket(socket.AF_UNIX)\n\
                 s.bind(sys.argv[1]); s.listen(8)\n\
                 while True:\n\
                 \x20   c, _ = s.accept(); f = c.makefile('rwb')\n\
                 \x20   for line in f:\n\
                 \x20       open(sys.argv[3], 'a').write('heard\\n')\n\
                 \x20       f.write(sys.argv[2].encode() + b'\\n'); f.flush()\n\
                 \x20   c.close()\n",
            ])
            .arg(socket)
            .arg(FAKE_ALLOWED)
            .arg(&heard)
            .spawn()
            .expect("python3 runs");
        let until = std::time::Instant::now() + Duration::from_secs(10);
        while !socket.exists() {
            assert!(std::time::Instant::now() < until, "the fake never listened");
            std::thread::sleep(Duration::from_millis(20));
        }
        Fake { child, heard }
    }

    /// Whether anything was said to it.
    fn heard(&self) -> bool {
        self.heard.exists()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Read only by a test build of purlis: an ancestor's program is the app's only by its name, as
/// in a shipped build, never because it is one of the build's own test binaries
/// ([`hookwire::reach::program_of`]).
const ONLY_THE_APP: &str = "REACH_TRUSTS_ONLY_THE_APP";

/// Set in the copy of this test binary that plays the app: the folder its test works in.
const PLAYS_THE_APP: &str = "BROKERED_GIT_PLAYS_THE_APP";

/// **The folder a cell of the app's trust works in, where this process plays the app**: a copy
/// of this test binary named as the app's program is (`purlis-app`), in the folder of the
/// `purlis` it starts, runs `test` and answers `Some` there. Here, in the test that started it,
/// it asserts that copy passed and answers `None`. No test binary is trusted for the app by
/// being one ([`ONLY_THE_APP`]), so what the cell shows is what a shipped build does.
fn played_by_the_app(test: &str) -> Option<PathBuf> {
    if let Some(base) = std::env::var_os(PLAYS_THE_APP) {
        return Some(PathBuf::from(base));
    }
    let purlis = PathBuf::from(support::purlis().get_program());
    let build = std::fs::canonicalize(purlis.parent().unwrap()).unwrap();
    let fixture = tempfile::Builder::new()
        .prefix("reach-fixture-")
        .tempdir_in(&build)
        .unwrap();
    let app = fixture.path().join(purlis_core::names::APP_BINARY.write);
    std::fs::copy(std::env::current_exe().unwrap(), &app).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let out = purlis_core::forklock::output(
        support::command(&app)
            .args(["--exact", test, "--test-threads=1", "--nocapture"])
            .env(PLAYS_THE_APP, tmp.path()),
    )
    .unwrap();
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success() && said.contains("test result: ok. 1 passed"),
        "{said}"
    );
    None
}

/// An app that records each git ask's chat and answers [`HEARD_BY_THE_APP`].
fn a_hearing_app(base: &Path) -> (Reading, ChatToken, PathBuf, mpsc::Receiver<Ask>) {
    an_app(base, |_| Answer::No {
        why: HEARD_BY_THE_APP.to_owned(),
    })
}

/// The chat each git ask the app heard was asked for.
fn chats_heard(asked: &mpsc::Receiver<Ask>) -> Vec<u32> {
    asked
        .try_iter()
        .filter_map(|ask| match ask {
            Ask::Git(git) => Some(git.chat),
            _ => None,
        })
        .collect()
}

/// `purlis worktree add widget p1 -w alpha` in `cwd`, in a chat whose environment is `chat`,
/// trusting only a program named as the app's for the app ([`played_by_the_app`]).
fn worktree_add(world: &World, root: &Path, cwd: &Path, chat: &[(&str, &str)]) -> Output {
    support::purlis()
        .args(["worktree", "add", "widget", "p1", "-w", "alpha"])
        .current_dir(cwd)
        .env_clear()
        .env("CHARTER_ROOT", root)
        .env("HOME", &world.home)
        .env("PATH", "/usr/bin:/bin")
        .env("USER", "tester")
        .env(ONLY_THE_APP, "1")
        .envs(chat.iter().copied())
        .output()
        .expect("the binary runs")
}

#[test]
fn a_chat_s_ask_with_its_environment_emptied_still_reaches_the_app_it_runs_under() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::a_chat_s_ask_with_its_environment_emptied_still_reaches_the_app_it_runs_under",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = world.plane("scrubbed", "[\"forge\"]");
    let (_reading, _token, _socket, asked) = a_hearing_app(&world.base);

    // No socket, no chat number and no token: the app names the chat by this process's ancestry.
    let ran = worktree_add(&world, &root, &root, &[]);

    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(err(&ran).contains(HEARD_BY_THE_APP), "{}", err(&ran));
    assert_eq!(chats_heard(&asked), vec![3]);
    assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
}

#[test]
fn a_socket_the_environment_names_that_is_not_the_app_is_never_believed() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::a_socket_the_environment_names_that_is_not_the_app_is_never_believed",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = world.plane("faked", "[\"forge\"]");
    let (_reading, token, socket, asked) = a_hearing_app(&world.base);
    let fake = Fake::at(&world.base.join("fake/hooks.sock"));
    let fake_path = world.base.join("fake/hooks.sock");
    let env = [
        (SOCKET_ENV, fake_path.to_str().unwrap()),
        (CHAT_ENV, "3"),
        (TOKEN_ENV, token.expose()),
    ];
    let _ = socket;

    let ran = worktree_add(&world, &root, &root, &env);

    // Refused, never believed and never run here: the environment only ever tightens, so a
    // socket it names that the app does not hold leaves this chat no app at all.
    assert!(!fake.heard(), "the fake was told the chat's ask");
    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(
        err(&ran).contains(hookwire::reach::NO_APP_ANSWERED),
        "{}",
        err(&ran)
    );
    assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
    assert_eq!(chats_heard(&asked), Vec::<u32>::new());
}

#[test]
fn a_chat_named_by_its_environment_is_the_one_the_app_says_it_runs_in() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::a_chat_named_by_its_environment_is_the_one_the_app_says_it_runs_in",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = world.plane("forged", "[\"forge\"]");
    let socket = world.base.join("app").join("hooks.sock");
    let listener = Listener::bind(&world.base, &socket).expect("a socket");
    // Chat 3 is this test's; chat 7's program is another process, whose token the chat read.
    listener.tokens().issue_to_this_process(3).expect("a token");
    let mut other = Command::new("sleep").arg("30").spawn().unwrap();
    let sevens = listener.tokens().issue(7).expect("a token");
    listener.tokens().bind(
        7,
        hookwire::Program::of(other.id()).expect("the other chat's program"),
    );
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask).unwrap();
            Answer::No {
                why: HEARD_BY_THE_APP.to_owned(),
            }
        }),
    );

    let ran = worktree_add(
        &world,
        &root,
        &root,
        &[
            (SOCKET_ENV, socket.to_str().unwrap()),
            (CHAT_ENV, "7"),
            (TOKEN_ENV, sevens.expose()),
        ],
    );
    let _ = other.kill();
    let _ = other.wait();

    assert!(err(&ran).contains(HEARD_BY_THE_APP), "{}", err(&ran));
    assert_eq!(chats_heard(&asked), vec![3], "asked as the chat it runs in");
}

#[test]
fn a_socket_planted_in_a_plane_of_the_chat_s_making_is_never_believed() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::a_socket_planted_in_a_plane_of_the_chat_s_making_is_never_believed",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = world.plane("real", "[\"forge\"]");
    let (_reading, _token, _socket, asked) = a_hearing_app(&world.base);
    // A plane the chat made in its own folder, with a socket of its own where the app's would be.
    let planted = root.join("workspaces/alpha/planted");
    std::fs::create_dir_all(&planted).unwrap();
    std::fs::write(planted.join("charter.toml"), "schema = 1\n").unwrap();
    let fake = Fake::at(&planted.join(".purlis/app/hooks.sock"));

    let ran = worktree_add(&world, &root, &planted, &[]);

    assert!(!fake.heard(), "the planted socket was told the chat's ask");
    assert!(err(&ran).contains(HEARD_BY_THE_APP), "{}", err(&ran));
    assert_eq!(chats_heard(&asked), vec![3]);
}

/// `purlis worktree add widget p1 -w alpha` in `root`, started as an orphan, so no app runs
/// above it; its status and what it said.
fn orphaned_worktree_add(world: &World, root: &Path, chat: &[(&str, &str)]) -> (i32, String) {
    let mut command = support::purlis();
    command
        .args(["worktree", "add", "widget", "p1", "-w", "alpha"])
        .current_dir(root)
        .env_clear()
        .env("CHARTER_ROOT", root)
        .env("HOME", &world.home)
        .env("PATH", "/usr/bin:/bin")
        .env("USER", "tester")
        .env(ONLY_THE_APP, "1")
        .envs(chat.iter().copied());
    let ran = orphaned(&command, &world.base);
    (ran.status.code().unwrap_or(-1), err(&ran))
}

#[test]
fn a_chat_whose_app_is_gone_is_refused_and_nothing_runs_here_even_where_a_fake_answers() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    let root = world.plane("gone", "[\"forge\"]");
    let fake_path = world.base.join("fake/hooks.sock");
    let fake = Fake::at(&fake_path);

    for chat in [
        &[(SOCKET_ENV, fake_path.to_str().unwrap()), (CHAT_ENV, "3")][..],
        &[(CHAT_ENV, "3")][..],
    ] {
        let (code, said) = orphaned_worktree_add(&world, &root, chat);
        assert_eq!(code, 1, "{chat:?}: {said}");
        assert!(
            said.contains(hookwire::reach::NO_APP_ANSWERED),
            "{chat:?}: {said}"
        );
        assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
    }
    assert!(!fake.heard(), "the fake was told the chat's ask");
}

/// An app above the chat that never answers: a listener of the app's program, holding the
/// plane's socket, that takes each connection and says nothing until the test ends.
fn a_silent_app(base: &Path) -> (std::os::unix::net::UnixListener, PathBuf) {
    let socket = base.join("app").join("hooks.sock");
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    (listener, socket)
}

#[test]
fn an_emptied_environment_under_a_full_app_is_refused_and_never_run_here() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::an_emptied_environment_under_a_full_app_is_refused_and_never_run_here",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = world.plane("full", "[\"forge\"]");
    // Every place the app serves is taken: each connection is turned away unread.
    let socket = world.base.join("app").join("hooks.sock");
    let listener = Listener::bind(&world.base, &socket)
        .expect("a socket")
        .serving_at_most(32, 0);
    listener.tokens().issue_to_this_process(3).expect("a token");
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask).unwrap();
            Answer::No {
                why: HEARD_BY_THE_APP.to_owned(),
            }
        }),
    );

    // `env -i`: no socket, no chat number and no token.
    let ran = worktree_add(&world, &root, &root, &[]);

    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(
        err(&ran).contains(hookwire::reach::NO_APP_ANSWERED),
        "refused, never run here: {}",
        err(&ran)
    );
    assert_eq!(chats_heard(&asked), Vec::<u32>::new());
    assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
}

#[test]
fn an_emptied_environment_under_an_app_that_does_not_answer_is_refused_and_never_run_here() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::an_emptied_environment_under_an_app_that_does_not_answer_is_refused_and_never_run_here",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = world.plane("silent", "[\"forge\"]");
    let (_listener, _socket) = a_silent_app(&world.base);

    let ran = worktree_add(&world, &root, &root, &[]);

    assert_eq!(ran.status.code(), Some(1), "{}", err(&ran));
    assert!(
        err(&ran).contains(hookwire::reach::NO_APP_ANSWERED),
        "refused, never run here: {}",
        err(&ran)
    );
    assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn an_emptied_environment_in_a_pid_namespace_of_its_own_is_refused_and_never_run_here() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    let root = world.plane("unshared", "[\"forge\"]");
    let (_reading, _token, _socket, asked) = a_hearing_app(&world.base);
    // `unshare -Urpf --mount-proc env -i purlis …`: the first process of a new pid namespace,
    // whose parent reads as 0, with nothing in its environment.
    let mut probe = Command::new("unshare");
    probe.args(["-Urpf", "--mount-proc", "/bin/true"]);
    if !purlis_core::forklock::status(&mut probe).is_ok_and(|it| it.success()) {
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this host refuses an unprivileged pid namespace"
        );
        return;
    }
    let purlis = PathBuf::from(support::purlis().get_program());
    let out = purlis_core::forklock::output(
        Command::new("unshare")
            .args(["-Urpf", "--mount-proc", "env", "-i"])
            .arg(format!("CHARTER_ROOT={}", root.display()))
            .arg(format!("HOME={}", world.home.display()))
            .arg("PATH=/usr/bin:/bin")
            .arg(&purlis)
            .args(["worktree", "add", "widget", "p1", "-w", "alpha"])
            .current_dir(&root),
    )
    .unwrap();

    assert_eq!(out.status.code(), Some(1), "{}", err(&out));
    assert!(
        err(&out).contains(hookwire::reach::NO_APP_ANSWERED),
        "refused, never run here: {}",
        err(&out)
    );
    assert_eq!(chats_heard(&asked), Vec::<u32>::new());
    assert!(!root.join("workspaces/alpha/.worktrees/widget/p1").exists());
}

// ---- `worktree adopt`: the app decides that the person runs it (#7700) ----

/// A plane with workspace `alpha` holding clone `svc`, and its branch folder `p1` on
/// `fix/p1-other`, a branch not named as the folder: the folder `worktree adopt` is for.
fn a_folder_to_adopt(world: &World, name: &str) -> PathBuf {
    let root = world.base.join(name);
    let clone = root.join("workspaces/alpha/svc");
    std::fs::create_dir_all(&clone).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    world.git(&clone, &["init", "-q", "-b", "main", "."]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    world.git(&clone, &["add", "-A"]);
    world.git(&clone, &["commit", "-q", "-m", "one"]);
    let folder = root.join("workspaces/alpha/.worktrees/svc/p1");
    world.git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fix/p1-other",
            folder.to_str().unwrap(),
        ],
    );
    root
}

/// The core's broker as the app calls it for chat 3 standing at `root`: the window's shell tab
/// (no persona, no harness) where `shell_tab`, else a persona's chat running its harness.
fn the_adopting_broker(
    root: PathBuf,
    shell_tab: bool,
) -> impl Fn(Ask) -> Answer + Send + Sync + 'static {
    move |ask| match ask {
        Ask::Git(asked) => gitbroker::answer(
            &root,
            &Asker {
                chat: 3,
                cwd: Some(root.clone()),
                persona: (!shell_tab).then(|| "steward".to_owned()),
                harnessed: !shell_tab,
                unsandboxed: shell_tab,
                roots_git_held: false,
                policy: None,
                unreadable: Vec::new(),
                config: None,
                id: None,
                live: None,
            },
            &asked,
            &git::Isolated::identity(Some("Tester"), Some("t@e.invalid")),
            jiff::Timestamp::now(),
        ),
        _ => Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        },
    }
}

/// `purlis worktree adopt svc p1 -w alpha` at `root`, in a session whose environment is `env`,
/// trusting only a program named as the app's for the app ([`played_by_the_app`]).
fn worktree_adopt(world: &World, root: &Path, env: &[(&str, &str)]) -> Output {
    support::purlis()
        .args(["worktree", "adopt", "svc", "p1", "-w", "alpha"])
        .current_dir(root)
        .env_clear()
        .env("CHARTER_ROOT", root)
        .env("HOME", &world.home)
        .env("PATH", "/usr/bin:/bin")
        .env("USER", "tester")
        .env(ONLY_THE_APP, "1")
        .envs(IDENTITY)
        .envs(env.iter().copied())
        .output()
        .expect("the binary runs")
}

/// Everything `out` said, on either stream.
fn both(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), err(out))
}

/// Whether the app was asked to adopt a folder, by the ask's kind on the wire.
fn adopt_asked(asked: &mpsc::Receiver<Ask>) -> bool {
    asked.try_iter().any(|ask| {
        serde_json::to_string(&ask)
            .unwrap()
            .contains("\"worktree_adopt\"")
    })
}

#[test]
fn a_chat_that_marks_itself_a_shell_tab_and_drops_its_chat_marks_is_refused_adopt_by_the_app() {
    purlis_core::unsteered!();
    let Some(base) = played_by_the_app(
        "brokered_git::a_chat_that_marks_itself_a_shell_tab_and_drops_its_chat_marks_is_refused_adopt_by_the_app",
    ) else {
        return;
    };
    let world = World::at(&base);
    let root = a_folder_to_adopt(&world, "spoofed");
    let (_reading, _token, _socket, asked) =
        an_app(&world.base, the_adopting_broker(root.clone(), false));

    // The shell tab's mark set, and no socket, chat number or token: the app names the chat
    // by this process's ancestry, whatever its environment says.
    let ran = worktree_adopt(
        &world,
        &root,
        &[(purlis_core::shellguard::SHELL_TAB_ENV, "1")],
    );

    assert_eq!(ran.status.code(), Some(1), "{}", both(&ran));
    assert!(both(&ran).contains("only you do"), "{}", both(&ran));
    assert!(!both(&ran).contains("recorded"), "{}", both(&ran));
    assert!(adopt_asked(&asked), "the app was never asked");
}

#[test]
fn a_shell_tab_adopts_its_folder_through_the_app() {
    purlis_core::unsteered!();
    let Some(base) =
        played_by_the_app("brokered_git::a_shell_tab_adopts_its_folder_through_the_app")
    else {
        return;
    };
    let world = World::at(&base);
    let root = a_folder_to_adopt(&world, "tab");
    let (_reading, token, socket, asked) =
        an_app(&world.base, the_adopting_broker(root.clone(), true));

    let mut env = chat_3(&socket, &token).to_vec();
    env.push((purlis_core::shellguard::SHELL_TAB_ENV, "1"));
    let ran = worktree_adopt(&world, &root, &env);

    assert!(ran.status.success(), "{}", both(&ran));
    assert!(both(&ran).contains("fix/p1-other"), "{}", both(&ran));
    assert!(adopt_asked(&asked), "adopted here, not by the app");
}

/// Whether anything under `root` records a branch a folder was cut for: the note `worktree
/// adopt` writes, wherever the project's state keeps it.
fn adopted_anything(root: &Path) -> bool {
    fn walk(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                let path = entry.path();
                if path.file_name().and_then(|n| n.to_str())
                    == Some(purlis_core::worktree::record::BRANCH_NOTE)
                {
                    return true;
                }
                entry.file_type().is_ok_and(|kind| kind.is_dir()) && walk(&path)
            })
    }
    walk(root)
}

/// `purlis worktree adopt` from the shell tab's environment, chat 3's, under an app that
/// answers each git ask with `answer`.
fn adopt_under(
    test: &str,
    name: &str,
    answer: impl Fn(Ask) -> Answer + Send + Sync + 'static,
) -> Option<(Output, PathBuf, mpsc::Receiver<Ask>)> {
    let base = played_by_the_app(test)?;
    let world = World::at(&base);
    let root = a_folder_to_adopt(&world, name);
    let (_reading, token, socket, asked) = an_app(&world.base, answer);
    let mut env = chat_3(&socket, &token).to_vec();
    env.push((purlis_core::shellguard::SHELL_TAB_ENV, "1"));
    let ran = worktree_adopt(&world, &root, &env);
    Some((ran, root, asked))
}

#[test]
fn an_app_with_nothing_to_answer_adopt_refuses_it_and_nothing_is_adopted_here() {
    purlis_core::unsteered!();
    let Some((ran, root, asked)) = adopt_under(
        "brokered_git::an_app_with_nothing_to_answer_adopt_refuses_it_and_nothing_is_adopted_here",
        "unwired",
        |_| Answer::No {
            why: hookwire::NOTHING_ANSWERS.to_owned(),
        },
    ) else {
        return;
    };

    assert_eq!(ran.status.code(), Some(1), "{}", both(&ran));
    assert!(both(&ran).contains("nothing was adopted"), "{}", both(&ran));
    assert!(adopt_asked(&asked), "the app was never asked");
    assert!(!adopted_anything(&root), "adopted here: {}", both(&ran));
}

#[test]
fn an_app_that_drops_the_adopt_ask_refuses_it_and_nothing_is_adopted_here() {
    purlis_core::unsteered!();
    let Some((ran, root, asked)) = adopt_under(
        "brokered_git::an_app_that_drops_the_adopt_ask_refuses_it_and_nothing_is_adopted_here",
        "dropped",
        // A connection whose serving panics is closed with no answer: the asker reads its end.
        |ask| match ask {
            Ask::Git(_) => panic!("the app drops this connection"),
            _ => Answer::No {
                why: hookwire::NOTHING_ANSWERS.to_owned(),
            },
        },
    ) else {
        return;
    };

    assert_eq!(ran.status.code(), Some(1), "{}", both(&ran));
    assert!(both(&ran).contains("nothing was adopted"), "{}", both(&ran));
    assert!(adopt_asked(&asked), "the app was never asked");
    assert!(!adopted_anything(&root), "adopted here: {}", both(&ran));
}

#[test]
fn a_terminal_with_no_app_adopts_its_folder_here() {
    purlis_core::unsteered!();
    let tmp = tempfile::tempdir().unwrap();
    let world = World::at(tmp.path());
    let root = a_folder_to_adopt(&world, "terminal");
    let mut command = support::purlis();
    command
        .args(["worktree", "adopt", "svc", "p1", "-w", "alpha"])
        .current_dir(&root)
        .env_clear()
        .env("CHARTER_ROOT", &root)
        .env("HOME", &world.home)
        .env("PATH", "/usr/bin:/bin")
        .env("USER", "tester")
        .env(ONLY_THE_APP, "1")
        .envs(IDENTITY);

    // An orphan with no chat marks: no app above it, and nothing says it is a chat's.
    let ran = orphaned(&command, &world.base);

    assert_eq!(ran.status.code(), Some(0), "{}", both(&ran));
    assert!(both(&ran).contains("fix/p1-other"), "{}", both(&ran));
}
