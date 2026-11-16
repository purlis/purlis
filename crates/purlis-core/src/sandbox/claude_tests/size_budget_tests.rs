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
//! its spellings, at the full sandbox. On macOS each denied path also has its
//! `/System/Volumes/Data` name, and every path held under each spelling its either-case copy.

use std::fs;
use std::path::Path;
use std::process::Command;

use super::super::{Decided, Form, Machine, Os, decide};
use crate::harness::{Harness, Kit, StateHooks};

/// The most a chat's generated harness settings may weigh: `limits.harness_settings_bytes`.
const BUDGET: usize = crate::hoststart::HARNESS_SETTINGS_BYTES;

/// Workspaces in the plane.
const WORKSPACES: usize = 12;

/// Branch folders in each workspace, cut from its first clone.
const BRANCH_FOLDERS: usize = 20;

/// Personas in the plane.
const PERSONAS: usize = 12;

/// No Claude Code settings of this machine's, the person's or an administrator's: what is
/// measured here is purlis's alone.
fn no_settings_of_this_machines() {
    super::super::claude::TEST_PERSONS_FILES.with(|it| *it.borrow_mut() = Some(Vec::new()));
}

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

/// A large plane at `root`, at the full sandbox: [`WORKSPACES`] workspaces of two clones and
/// [`BRANCH_FOLDERS`] branch folders each, [`PERSONAS`] personas, and the plane's state under
/// both spellings. What is denied in the state is denied by path, there or not, so its folders
/// alone.
pub(crate) fn a_large_plane(root: &Path) {
    fs::write(root.join("charter.toml"), "").unwrap();
    fs::write(
        root.join("purlis.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
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
    no_settings_of_this_machines();
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
    let args =
        crate::harness::claude::reporting_on(args, &compiled, Some(&socket), Some([50811, 50812]))
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

    let parsed: serde_json::Value = serde_json::from_str(&settings).expect("JSON");
    let deny = parsed["permissions"]["deny"].as_array().map_or(0, Vec::len);
    let deny_write = parsed["sandbox"]["filesystem"]["denyWrite"]
        .as_array()
        .map_or(0, Vec::len);
    let said = format!(
        "a chat on a plane of {WORKSPACES} workspaces and {} branch folders: {} bytes, {deny} \
         deny rules, {deny_write} paths denied to writes",
        WORKSPACES * BRANCH_FOLDERS,
        settings.len()
    );
    println!("{said}");
    assert!(
        deny > 0,
        "the chat is sandboxed, and its tools held: {parsed}"
    );
    assert!(
        settings.len() <= BUDGET,
        "{said}: more than the budget of {BUDGET} bytes (`limits.harness_settings_bytes`)"
    );
}

/// **What Claude Code's sandbox decides of a command's write to `path`** in a chat opened in
/// `cwd` under `filesystem`, as its profile builder makes it (read in 2.1.291 and 2.1.300): a
/// write goes ahead inside the chat's folder, an `allowWrite` entry or its shared temp folder,
/// unless a `denyWrite` path holds it, at or below; and every folder above a `denyWrite` path is
/// pinned, never written itself. Only plain paths are weighed: a glob is the same on both sides
/// of what this module compares.
fn writes(filesystem: &serde_json::Value, cwd: &str, path: &str) -> bool {
    let list = |key: &str| -> Vec<String> {
        filesystem[key]
            .as_array()
            .map(|it| {
                it.iter()
                    .filter_map(|one| one.as_str().map(str::to_owned))
                    .filter(|one| !one.contains(['*', '?', '[']))
                    .collect()
            })
            .unwrap_or_default()
    };
    let under = |inner: &str, outer: &str| {
        outer == "/" || inner == outer || inner.starts_with(&format!("{outer}/"))
    };
    let roots: Vec<String> = std::iter::once(cwd.to_owned())
        .chain(list("allowWrite"))
        .chain(super::claude::SHARED_TEMP.iter().map(|it| (*it).to_owned()))
        .collect();
    let denied = list("denyWrite");
    roots.iter().any(|root| under(path, root))
        && !denied.iter().any(|held| under(path, held))
        && !denied.iter().any(|held| held != path && under(held, path))
}

/// **The pins left out change no write a command may make** (the person's ruling, 2026-11-16,
/// #6961): on the large plane, every git folder pin, a file below it, each folder above it and a
/// file beside it, each written as before by a command in the chat's folder, with each pin
/// [`super::claude::Settings::written_in`] left out put back.
#[test]
fn the_pins_left_out_change_no_write_a_command_may_make() {
    no_settings_of_this_machines();
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
    let Form::ClaudeCode(after) = applied.form_in(Some(&cwd)) else {
        panic!("not Claude Code's own sandbox");
    };
    let mut before = after.sandbox["filesystem"].clone();
    let listed = before["denyWrite"].as_array_mut().expect("a list");
    for pin in &after.pins {
        if !listed.iter().any(|it| it == pin) {
            listed.push(serde_json::json!(pin));
        }
    }
    let left_out = before["denyWrite"].as_array().map_or(0, Vec::len)
        - after.sandbox["filesystem"]["denyWrite"]
            .as_array()
            .map_or(0, Vec::len);
    assert!(
        left_out > 400,
        "most of the plane's pins are left out: {left_out}"
    );

    let cwd = cwd.display().to_string();
    let mut corpus: Vec<String> = Vec::new();
    for pin in &after.pins {
        corpus.push(pin.clone());
        corpus.push(format!("{pin}/config"));
        corpus.push(format!("{pin}-beside"));
        let mut above = Path::new(pin).parent();
        while let Some(folder) = above.filter(|it| it.starts_with(&root)) {
            corpus.push(folder.display().to_string());
            above = folder.parent();
        }
    }
    corpus.push(format!("{cwd}/a-file"));
    corpus.sort();
    corpus.dedup();
    let changed: Vec<&String> = corpus
        .iter()
        .filter(|path| {
            writes(&before, &cwd, path) != writes(&after.sandbox["filesystem"], &cwd, path)
        })
        .collect();
    assert!(
        changed.is_empty(),
        "{} of {} writes decided otherwise: {changed:?}",
        changed.len(),
        corpus.len()
    );
    assert!(
        writes(&after.sandbox["filesystem"], &cwd, &format!("{cwd}/a-file")),
        "the chat still writes its own folder"
    );
}

/// **A pin under a write root of the person's own settings is kept** (the orchestrator's ruling
/// A', 2026-11-16, #6961): Claude Code lets a command write each folder the person's user or
/// managed settings name (`sandbox.filesystem.allowWrite`, an Edit allow rule,
/// `permissions.additionalDirectories`), so a git folder pin there holds what it held before.
/// What cannot be placed exactly, a relative Edit rule or a file that cannot be read, keeps them
/// all.
#[test]
fn a_pin_under_a_write_root_of_the_persons_own_settings_is_kept() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path().canonicalize().unwrap();
    let home = tempfile::tempdir().unwrap();
    let home = home.path().canonicalize().unwrap();
    a_large_plane(&root);
    let cwd =
        root.join("workspaces/workspace-00/.worktrees/app/t00-a-piece-of-work-named-at-length");
    let pin = |workspace: &str| {
        root.join("workspaces")
            .join(workspace)
            .join(".worktrees/app/t05-a-piece-of-work-named-at-length/.git")
            .display()
            .to_string()
    };
    let settings_dir = home.join(".claude");
    fs::create_dir_all(&settings_dir).unwrap();
    let user = settings_dir.join("settings.json");
    let pinned_with = |settings: &str| -> Vec<String> {
        fs::write(&user, settings).unwrap();
        super::super::claude::TEST_PERSONS_FILES
            .with(|it| *it.borrow_mut() = Some(vec![user.clone()]));
        let machine = Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(home.clone()),
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
        settings.sandbox["filesystem"]["denyWrite"]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|it| it.as_str().map(str::to_owned))
            .collect()
    };
    let workspace = |name: &str| root.join("workspaces").join(name).display().to_string();

    let none = pinned_with("{}");
    assert!(!none.contains(&pin("workspace-03")), "nothing writes there");

    let allowed = pinned_with(&format!(
        r#"{{"sandbox": {{"filesystem": {{"allowWrite": ["{}"]}}}}}}"#,
        workspace("workspace-03")
    ));
    assert!(allowed.contains(&pin("workspace-03")), "under allowWrite");
    assert!(!allowed.contains(&pin("workspace-04")), "and no other");

    let edit = pinned_with(&format!(
        r#"{{"permissions": {{"allow": ["Edit(/{}/**)"]}}}}"#,
        workspace("workspace-04")
    ));
    assert!(
        edit.contains(&pin("workspace-04")),
        "under an Edit allow rule"
    );
    assert!(!edit.contains(&pin("workspace-03")));

    let added = pinned_with(&format!(
        r#"{{"permissions": {{"additionalDirectories": ["{}"]}}}}"#,
        workspace("workspace-05")
    ));
    assert!(
        added.contains(&pin("workspace-05")),
        "under an additional directory"
    );

    for anywhere in [
        r#"{"permissions": {"allow": ["Edit(src/**)"]}}"#,
        "not JSON",
    ] {
        let all = pinned_with(anywhere);
        assert!(
            ["workspace-03", "workspace-04", "workspace-11"]
                .iter()
                .all(|it| all.contains(&pin(it))),
            "{anywhere}: every pin kept"
        );
    }
}
