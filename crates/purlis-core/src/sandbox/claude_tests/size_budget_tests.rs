// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Claude Code chat's settings on a large plane stay within the size budget** (T1.1-71,
//! #6961, Sentry PURLIS-APP-J).
//!
//! Claude Code's auto mode hands what a chat's settings say to a check of its own, which takes
//! about a megabyte in all, its own lists among it. A plane with many workspaces and branch
//! folders had its chats' settings near 800 KB, and auto mode then switched off for them: every
//! action that needs the check was refused, and nobody was asked.
//!
//! So a chat's `--settings` is armed here as its start arms it: the sandbox decided for the plane,
//! compiled for the folder the chat opens in, armed by Claude Code's adapter, and given its hook
//! socket and proxy ports ([`crate::harness::claude::reporting_on`]). That word is what the start
//! writes to the chat's settings file ([`crate::hoststart::settings_in_a_file`]). The plane is
//! shaped like a large real one: many workspaces, each with clones and many branch folders (each
//! a repository whose git folder a chat is denied), personas, and the plane's state under both of
//! its spellings, under the `strict` template (Q308) at the full sandbox. On macOS each denied
//! path also has its `/System/Volumes/Data` name, and every path held under each spelling its
//! either-case copy.
//!
//! What is held to the budget is what auto mode sends its check: the permission rules and the
//! auto mode lists ([`crate::hoststart::auto_mode_weight`]). The sandbox is no part of it, so
//! every git folder pin is written wherever the chat opens, and the whole file is held to a
//! sanity cap of its own ([`crate::hoststart::HARNESS_SETTINGS_FILE_BYTES`]).

use std::fs;
use std::path::Path;
use std::process::Command;

use super::super::{Decided, Form, Machine, Os, decide};
use crate::harness::{Harness, Kit, StateHooks};

/// The most what auto mode counts of a chat's generated harness settings may weigh:
/// `limits.harness_settings_bytes`.
const BUDGET: usize = crate::hoststart::HARNESS_SETTINGS_BYTES;

/// The most a chat's whole generated harness settings file may weigh.
const FILE_CAP: usize = crate::hoststart::HARNESS_SETTINGS_FILE_BYTES;

/// What auto mode counts of a chat on the large plane, as serialised (`crate::hoststart::auto_mode_weight`), measured with the plane's folder stood in for ([`STAND_IN`]) and each rule's `root` counted (#7225; #7157 had 219,348 without them). **Per OS, because the plane weighs twice as much on macOS** (#7225 round 3): there every denied path under a firmlink (`/Users`, `/private`, ...) is also denied under its `/System/Volumes/Data` name (`firmlink_twin`), which Linux has no such name for. **On macOS this is 1,242 bytes under the 256,000 budget, and past the 128,000 where allow rules may be shed** (656 deny rules); **on Linux it is 11,795 bytes under the 128,000 and the tiers never fire** (333 deny rules). The figure that matters for the person's notice tiers is macOS's.
#[cfg(target_os = "macos")]
const COUNTED_TODAY: usize = 254_758;

/// [`COUNTED_TODAY`] on Linux (and any OS without firmlinks): measured on CI, ubuntu, 2026-11-19.
#[cfg(not(target_os = "macos"))]
const COUNTED_TODAY: usize = 116_205;

/// What the plane's folder is replaced by before the ratchet weighs a chat's settings, so the
/// weight does not move with the length of `TMPDIR`.
const STAND_IN: &str = "/plane";

/// Workspaces in the plane.
const WORKSPACES: usize = 12;

/// Branch folders in each workspace, cut from its first clone.
const BRANCH_FOLDERS: usize = 20;

/// Personas in the plane.
const PERSONAS: usize = 12;

/// git in `dir`, with nothing of the developer's own configuration.
fn git(dir: &Path, args: &[&str]) {
    let out = crate::forklock::output(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid"),
    )
    .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A large plane at `root`, under the `strict` template (Q308) at the full sandbox:
/// [`WORKSPACES`] workspaces of two clones and [`BRANCH_FOLDERS`] branch folders each,
/// [`PERSONAS`] personas, and the plane's state under both spellings. What is denied in the state
/// is denied by path, there or not, so its folders alone.
pub(crate) fn a_large_plane(root: &Path) {
    fs::write(root.join("charter.toml"), "").unwrap();
    fs::write(
        root.join("purlis.toml"),
        "[policy]\ntemplate = \"strict\"\n\n[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    )
    .unwrap();
    for state in [".purlis", ".charter"] {
        fs::create_dir_all(root.join(state).join("app")).unwrap();
    }
    for persona in (0..PERSONAS).map(|n| format!("persona-{n:02}")) {
        let at = root.join("personas").join(&persona);
        fs::create_dir_all(at.join("skills")).unwrap();
        fs::write(at.join("persona.md"), format!("# {persona}\n")).unwrap();
    }
    for workspace in (0..WORKSPACES).map(|n| format!("workspace-{n:02}")) {
        let at = root.join("workspaces").join(&workspace);
        fs::create_dir_all(at.join("skills")).unwrap();
        fs::write(
            at.join("workspace.json"),
            format!("{{\"name\": \"{workspace}\"}}\n"),
        )
        .unwrap();
        for clone in ["app", "infra"] {
            let repo = at.join(clone);
            fs::create_dir_all(&repo).unwrap();
            git(&repo, &["init", "-q", "-b", "main"]);
            git(&repo, &["commit", "-q", "--allow-empty", "-m", "start"]);
        }
        for piece in (0..BRANCH_FOLDERS).map(|n| format!("t{n:02}-a-piece-of-work-named-at-length"))
        {
            let folder = at.join(".worktrees/app").join(&piece);
            git(
                &at.join("app"),
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &piece,
                    folder.to_str().unwrap(),
                ],
            );
        }
    }
}

/// The `--settings` word a Claude Code chat opening in `cwd`, in the plane at `root`, is started
/// with, its hook socket and proxy ports in it.
pub(crate) fn the_settings_of_a_chat_in(root: &Path, cwd: &Path, home: &Path) -> String {
    let machine = Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(home.to_path_buf()),
        os: Os::MacOs,
    };
    let Some(Decided::Sandboxed(applied)) =
        decide(Harness::ClaudeCode, root, &machine, &|_| true, None, None).expect("decided")
    else {
        panic!("not sandboxed");
    };
    let Form::ClaudeCode(compiled) = applied.form_in(Some(cwd)) else {
        panic!("not Claude Code's own sandbox");
    };
    let plugin = root.join(".kit/plugin");
    let kit = Kit {
        skills: None,
        laid_out: None,
        binary: Path::new("/opt/purlis/bin/purlis"),
        plugin: Some(&plugin),
        persona: None,
        graded: false,
        judging: false,
        writes_its_piece: false,
        runs_a_shell: false,
        own: &[],
    };
    let StateHooks::ThisSessionOnly { args, .. } = Harness::ClaudeCode.state_hooks(
        kit,
        Some(cwd),
        &crate::harness_plugin::Chosen::new(),
        Some(&applied),
    ) else {
        panic!("not armed");
    };
    let socket = root.join(".purlis/app/hooks.sock");
    let args = crate::harness::claude::reporting_on(
        args,
        &compiled,
        Some(&socket),
        Some([50811, 50812]),
        None,
    )
    .expect("reports on its socket");
    let at = args
        .iter()
        .position(|word| word == "--settings")
        .expect("--settings");
    args[at + 1].clone()
}

#[test]
fn the_generated_settings_of_a_large_plane_stay_within_the_size_budget() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path().canonicalize().unwrap();
    let home = tempfile::tempdir().unwrap();
    a_large_plane(&root);
    let cwd =
        root.join("workspaces/workspace-00/.worktrees/app/t00-a-piece-of-work-named-at-length");

    let settings = the_settings_of_a_chat_in(&root, &cwd, home.path());

    // The plane's folder is in each rule several times (as the rule, its `canonical`, its
    // `pattern` and its `root`), and its length is the machine's: a long `TMPDIR` weighs more.
    // The ratchet measures with it stood in for by a fixed-length prefix.
    let normal = settings.replace(root.to_str().unwrap(), STAND_IN);
    let parsed: serde_json::Value = serde_json::from_str(&normal).expect("JSON");
    let stood_in = Path::new(STAND_IN).join(cwd.strip_prefix(&root).unwrap());
    let counted = crate::hoststart::auto_mode_weight(
        &parsed,
        &crate::hoststart::RulesOutside::default(),
        Some(&stood_in),
    );
    let deny = parsed["permissions"]["deny"].as_array().map_or(0, Vec::len);
    let deny_write = parsed["sandbox"]["filesystem"]["denyWrite"]
        .as_array()
        .map_or(0, Vec::len);
    let said = format!(
        "a chat on a plane of {WORKSPACES} workspaces and {} branch folders: {counted} bytes that \
         auto mode counts ({deny} deny rules), {} bytes in all ({deny_write} paths denied to \
         writes)",
        WORKSPACES * BRANCH_FOLDERS,
        settings.len()
    );
    println!("{said}");
    assert!(
        deny > 0,
        "the chat is sandboxed, and its tools held: {parsed}"
    );
    assert!(
        counted <= BUDGET,
        "{said}: auto mode counts more than the budget of {BUDGET} bytes \
         (`limits.harness_settings_bytes`)"
    );
    // A ratchet (#7157): the budget has room, so a rule added to every chat would pass it
    // unseen. This weight, measured with the plane's folder stood in for ([`STAND_IN`]), is
    // what the plane's chats are measured to send today; a change that moves it either way
    // says so here: a rise past a tenth is a re-bloat to look at, a fall past a tenth is a
    // rule lost (or a better count) to be written into `COUNTED_TODAY`. The figure is per OS
    // (`COUNTED_TODAY` says why: macOS's firmlink twins double the denied paths), each with
    // its own tenth either side, rather than one OS-independent measure: a measure that left
    // the twins out would stop weighing what a macOS chat really sends, which is what the
    // tiers act on.
    assert!(
        counted <= COUNTED_TODAY + COUNTED_TODAY / 10,
        "{said}: more than a tenth over the {COUNTED_TODAY} bytes this plane weighed when \
         #7157 measured it"
    );
    assert!(
        counted >= COUNTED_TODAY - COUNTED_TODAY / 10,
        "{said}: more than a tenth under the {COUNTED_TODAY} bytes this plane weighed when \
         #7157 measured it"
    );
    assert!(
        settings.len() <= FILE_CAP,
        "{said}: the file is past its sanity cap of {FILE_CAP} bytes"
    );
}

/// Whether `path`, or a folder above it, matches `glob` as Claude Code's sandbox reads a path
/// pattern: `*` within a name, `**` any folders or none, `?` one letter, `[...]` a class.
fn glob_holds(glob: &str, path: &str) -> bool {
    thread_local! {
        /// Each glob's pattern, made once: the large plane weighs thousands of paths.
        static MADE: std::cell::RefCell<std::collections::HashMap<String, Option<regex::Regex>>> =
            std::cell::RefCell::default();
    }
    MADE.with(|made| {
        made.borrow_mut()
            .entry(glob.to_owned())
            .or_insert_with(|| glob_pattern(glob))
            .as_ref()
            .is_some_and(|it| it.is_match(path))
    })
}

/// [`glob_holds`]'s pattern for `glob`, or `None` where it is none.
fn glob_pattern(glob: &str) -> Option<regex::Regex> {
    let mut pattern = String::from("^");
    let mut rest = glob;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("**") {
            pattern.push_str(".*");
            rest = after;
            continue;
        }
        match c {
            '*' => pattern.push_str("[^/]*"),
            '?' => pattern.push_str("[^/]"),
            '[' => match rest.find(']') {
                Some(end) => {
                    pattern.push_str(&rest[..=end]);
                    rest = &rest[end + 1..];
                    continue;
                }
                None => pattern.push_str(r"\["),
            },
            _ => pattern.push_str(&regex::escape(&c.to_string())),
        }
        rest = &rest[c.len_utf8()..];
    }
    pattern.push_str("(?:/.*)?$");
    regex::Regex::new(&pattern).ok()
}

/// **What Claude Code's sandbox decides of a command's write to `path`** in a chat opened in
/// `cwd` under `filesystem`, as its profile builder makes it (read in 2.1.291 and 2.1.300): a
/// write goes ahead inside the chat's folder, an `allowWrite` entry (a glob by what it matches)
/// or its shared temp folder, unless a `denyWrite` entry holds it, at or below; and every folder
/// above a plain `denyWrite` path is pinned, never written itself. A glob gets no folder above
/// it pinned.
fn writes(filesystem: &serde_json::Value, cwd: &str, path: &str) -> bool {
    let list = |key: &str| -> Vec<String> {
        filesystem[key]
            .as_array()
            .map(|it| {
                it.iter()
                    .filter_map(|one| one.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let glob = |it: &str| it.contains(['*', '?', '[']);
    let under = |inner: &str, outer: &str| {
        if glob(outer) {
            return glob_holds(outer, inner);
        }
        outer == "/" || inner == outer || inner.starts_with(&format!("{outer}/"))
    };
    let roots: Vec<String> = std::iter::once(cwd.to_owned())
        .chain(list("allowWrite"))
        .chain(super::claude::SHARED_TEMP.iter().map(|it| (*it).to_owned()))
        .collect();
    let denied = list("denyWrite");
    roots.iter().any(|root| under(path, root))
        && !denied.iter().any(|held| under(path, held))
        && !denied
            .iter()
            .filter(|held| !glob(held))
            .any(|held| held != path && under(held, path))
}

/// **A folder granted while the chat runs still finds every git folder pin** (the review of
/// #7005). Claude Code 2.1.325 adds a write root mid-chat three ways: an "allow all edits in this
/// folder during this session" answer in its own prompt, `/add-dir`, and a change to the
/// person's settings, which it reads again and builds its sandbox anew from. Each makes the
/// folder a write root as it is, with no pin of its own in it. So the settings purlis writes hold
/// every pin wherever the chat opens: on the large plane, a chat in one workspace's branch folder
/// granted another workspace's branch folder, or that workspace whole, writes in it but never
/// moves, replaces or rewrites its `.git`, nor moves a folder above it.
#[test]
fn a_folder_granted_mid_chat_still_finds_every_pin() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path().canonicalize().unwrap();
    let home = tempfile::tempdir().unwrap();
    a_large_plane(&root);
    let cwd =
        root.join("workspaces/workspace-00/.worktrees/app/t00-a-piece-of-work-named-at-length");
    let machine = Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(home.path().to_path_buf()),
        os: Os::MacOs,
    };
    let Some(Decided::Sandboxed(applied)) =
        decide(Harness::ClaudeCode, &root, &machine, &|_| true, None, None).expect("decided")
    else {
        panic!("not sandboxed");
    };
    let Form::ClaudeCode(settings) = applied.form_in(Some(&cwd)) else {
        panic!("not Claude Code's own sandbox");
    };
    let other = root.join("workspaces/workspace-07");
    let q1 = other.join(".worktrees/app/t03-a-piece-of-work-named-at-length");
    let cwd = cwd.display().to_string();
    // Each route by the root it adds: the folder the prompt or `/add-dir` names, and the
    // workspace whole, as a settings change may name it.
    for (route, granted) in [
        ("its own prompt, for the session", q1.clone()),
        ("/add-dir", q1.clone()),
        ("a live change to the person's settings", other.clone()),
    ] {
        let mut filesystem = settings.sandbox["filesystem"].clone();
        let allow = &mut filesystem["allowWrite"];
        if !allow.is_array() {
            *allow = serde_json::json!([]);
        }
        allow
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!(granted.display().to_string()));
        let at = |path: &Path| path.display().to_string();
        assert!(
            writes(&filesystem, &cwd, &at(&q1.join("src.rs"))),
            "{route}: the grant writes the folder"
        );
        for held in [
            q1.join(".git"),
            q1.clone(),
            q1.parent().unwrap().to_path_buf(),
            other.join(".worktrees"),
            other.join("app/.git/config"),
        ] {
            assert!(
                !writes(&filesystem, &cwd, &at(&held)),
                "{route}: {} is written",
                held.display()
            );
        }
    }
}

/// **A held path that ends in whitespace keeps its `/**` rule, and the rules below it**: Claude
/// Code reads a rule's path as gitignore reads a line, which drops whitespace at its end, so
/// `Read(//a/b )` holds `/a/b`, not `/a/b ` nor what is below it; only `Read(//a/b /**)` does.
#[test]
fn a_held_path_that_ends_in_whitespace_keeps_its_rules_below_it() {
    for rules in [
        vec!["Read(//a/b )", "Read(//a/b /**)"],
        vec!["Edit(//a/b\t)", "Edit(//a/b\t/c)"],
    ] {
        let rules: Vec<String> = rules.into_iter().map(str::to_owned).collect();
        assert_eq!(super::claude::fewest(rules.clone()), rules);
    }
}

/// **A path with `]` is a pattern, as every character [`PATTERN_CHARACTERS`] names is** (#7157):
/// Claude Code reads it as a glob, so a rule on it holds no path below, and a rule below it is
/// not folded into it.
///
/// [`PATTERN_CHARACTERS`]: crate::sandbox::PATTERN_CHARACTERS
#[test]
fn a_path_with_a_closing_bracket_is_a_pattern_and_folds_nothing() {
    for c in crate::sandbox::PATTERN_CHARACTERS {
        let rules: Vec<String> = vec![format!("Read(//a/b{c})"), format!("Read(//a/b{c}/c)")];
        assert_eq!(
            super::claude::fewest(rules.clone()),
            rules,
            "{c:?} is a pattern, so the rule below it stays"
        );
    }
}
