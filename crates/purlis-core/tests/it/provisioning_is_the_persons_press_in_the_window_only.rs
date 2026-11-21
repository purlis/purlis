// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Provisioning is the person's press, in the window** (ADR 0128 D5).
//!
//! Signing in, pinning a host's application, choosing the account that serves a repo, trusting a
//! clone and the second token's sign-in are never done from a chat, the command line or a socket.
//! Three things hold that, and each is checked here:
//!
//! 1. **A chat's process tree is refused.** The one place the window claims the host's scope
//!    ([`provision::on_the_persons_press`]) answers the refusal inside a process that carries a
//!    chat's marks, and the scope outside one. Every act that provisions takes that scope, so a
//!    caller that cannot claim it cannot call.
//! 2. **No link serves a provisioning command**, and each is in the list of the window's own.
//! 3. **No source but the window's calls a provisioning entry point.** The `purlis` binary, the
//!    session host and every module of the core outside the forge's own are read for the names.
//!    **This third check is a lint, not the wall**: it reads text, so a spelling it does not
//!    know (a macro, a re-export under another name, a path built by `concat!`) slips past it. It
//!    does read a `use` group over several lines, an alias or a glob of the module, a function
//!    named as a value (`.map(for_ci_push)`) and a method call (`route.enrol(`). What holds the
//!    line whatever the spelling is the first check: every act takes a [`HostScope`], and a
//!    chat's process tree cannot claim one.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use purlis_core::forge::provision::{
    PROVISIONING_COMMANDS, PROVISIONING_ENTRY_POINTS, REFUSAL, on_the_persons_press,
};
use purlis_session_protocol::ui;

const EXPECT: &str = "PROVISIONING_EXPECTED";

#[test]
fn inside_a_chats_process_tree_a_provisioning_act_is_refused_with_where_to_do_it() {
    purlis_core::unsteered!();
    let child = "provisioning_is_the_persons_press_in_the_window_only::a_child_claims_the_persons_press_only_outside_a_chat";
    for chat_mark in ["PURLIS_CHAT", "PURLIS_SESSION_ID"] {
        purlis_core::testrun::rerun(
            &[child],
            &[
                (chat_mark, OsStr::new("7")),
                (EXPECT, OsStr::new("refused")),
            ],
        );
    }
    purlis_core::testrun::rerun(&[child], &[(EXPECT, OsStr::new("pressed"))]);
}

/// The child: claims the person's press as the window does; in any run this binary did not start
/// for the test it does nothing.
#[test]
fn a_child_claims_the_persons_press_only_outside_a_chat() {
    purlis_core::unsteered!();
    let Ok(expected) = std::env::var(EXPECT) else {
        return;
    };
    match (expected.as_str(), on_the_persons_press()) {
        ("refused", Err(said)) => {
            assert_eq!(said, REFUSAL);
            assert!(said.contains("window"), "{said}");
        }
        ("pressed", Ok(_)) => {}
        (wanted, got) => panic!("wanted {wanted}, got {:?}", got.map(|_| "a scope")),
    }
}

#[test]
fn every_provisioning_command_is_the_windows_alone_and_no_link_serves_one() {
    purlis_core::unsteered!();
    for command in PROVISIONING_COMMANDS {
        assert!(
            ui::window_only(command),
            "{command} provisions, so it must be one of the commands only the window runs"
        );
        assert!(
            !ui::served_on_a_link(command),
            "{command} provisions and is served on a link"
        );
    }
    // The commands this slice adds are among them.
    for command in [
        "forge_use_application",
        "forge_choose_project_account",
        "forge_trust_clone_repo",
        "forge_ci_permission_start",
        "forge_ci_permission_wait",
    ] {
        assert!(PROVISIONING_COMMANDS.contains(&command), "{command}");
    }
}

/// Whether `module_file` declares `mod <name>;` right after a `#[cfg(test)]`.
fn declares_cfg_test_mod(module_file: &Path, name: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(module_file) else {
        return false;
    };
    let wanted = format!("mod {name};");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines.iter().enumerate().any(|(at, line)| {
        (*line == wanted || (line.starts_with("pub") && line.ends_with(&wanted)))
            && at > 0
            && lines[at - 1].contains("cfg(test)")
    })
}

/// Whether the module `name` that lives in `dir` (as `dir/name.rs` or `dir/name/`) is declared as
/// a test module (`#[cfg(test)] mod name;`) by the module that owns `dir`. A file that is merely
/// *named* with "tests" is not a test module and is scanned.
fn is_cfg_test_module(dir: &Path, name: &str) -> bool {
    let mut owners = vec![dir.join("mod.rs"), dir.join("lib.rs"), dir.join("main.rs")];
    if let (Some(parent), Some(dir_name)) = (dir.parent(), dir.file_name()) {
        owners.push(parent.join(format!("{}.rs", dir_name.to_string_lossy())));
    }
    owners
        .iter()
        .any(|owner| declares_cfg_test_mod(owner, name))
}

/// Every `.rs` file under `dir`, but not a `#[cfg(test)]` module's.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name == "target" || is_cfg_test_module(dir, &name) {
                continue;
            }
            sources(&path, out);
        } else if let Some(stem) = name.strip_suffix(".rs")
            && !is_cfg_test_module(dir, stem)
        {
            out.push(path);
        }
    }
}

/// The lines of `text` that are code: a comment line holds a name without calling it.
fn code(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| !l.trim_start().starts_with("//"))
}

/// The sources of one directory, which must exist and hold at least `at_least` of them: a scan
/// of a folder that was renamed or moved reads nothing and must not pass for a clean one. The
/// floor sits just under the real count, so a folder that lost files fails too.
fn scanned(root: &Path, dir: &str, at_least: usize) -> Vec<PathBuf> {
    let at = root.join(dir);
    assert!(
        at.is_dir(),
        "the scanned directory {} is missing",
        at.display()
    );
    let mut found = Vec::new();
    sources(&at, &mut found);
    assert!(
        found.len() >= at_least,
        "the scan found {} sources in {dir}, and wants at least {at_least}",
        found.len()
    );
    found
}

/// The core's own files that name a provisioning entry point, **each named, by exact path** (a new
/// file in `src/` is not exempt until it is added here, with the reason it calls one). Every one
/// must still name one: [`allowed_files_still_name_an_entry_point`].
const ALLOWED_IN_CORE: &[&str] = &[
    "src/forge/accounts.rs",
    "src/forge/accounts/workflow.rs",
    "src/forge/confirmed.rs",
    "src/forge/device.rs",
    "src/forge/gitcredential/vouch.rs",
    "src/forge/provision.rs",
    // Re-exports `pin_with_owner` from its own module.
    "src/forge/registered.rs",
    // Defines `with_forge_id`, the one way a repo's forge-read id enters the proxy.
    "src/forge/proxy/repo.rs",
    "src/forge/refresh.rs",
    "src/forge/registered/owner.rs",
    "src/forge/signin.rs",
    "src/windowonly.rs",
];

/// The window's own files, under `app/src-tauri/src`, that call a provisioning entry point: the
/// presses. Any other file of the app naming one is a source that is not the window's press.
const ALLOWED_IN_APP: &[&str] = &[
    "clonevouch.rs",
    "createproject.rs",
    "forgeaccounts.rs",
    "forgepkce.rs",
    // The forge route the app runs for each run of a chat (one login, slice K). A run starts by
    // the app (the person started the chat): the route enrols it with its proxy and its gate, and
    // reads the person's sign-in as the app, in its own process. A chat's process tree has no
    // `HostScope`, so none of this is reachable from one.
    "forgeroute/mod.rs",
    // The route's seams: the person's confirmed repos (recorded by their press, with the id the
    // forge gave) and the gate over them. It records nothing itself.
    "forgeroute/prod.rs",
    // Held acts: the app reads and writes the forge as the person only for their press of an ask,
    // and a CI-file push's base is fetched for the ask the chat's push raised.
    "forgewiring.rs",
    // Lists the window's commands, `vouch_clone` among them, for the handler.
    "ipc_commands.rs",
    "onelogin.rs",
    "remotequestion.rs",
    "staleclones.rs",
];

/// The entries the scan reads: `(root, directory, floor)`. The floor is just under the number of
/// non-test sources there today.
const SCANNED: &[(&str, &str, usize)] = &[
    ("crates", "purlis-cli/src", 45),
    ("crates", "host/src", 20),
    ("crates", "session-protocol/src", 19),
    ("crates", "purlis-core/src", 1070),
    ("app", "src-tauri/src", 275),
];

fn roots() -> (PathBuf, PathBuf) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    (manifest.join(".."), manifest.join("../../app"))
}

/// Whether `line` names the entry point `entry` (as [`PROVISIONING_ENTRY_POINTS`] spells it), by
/// its symbol: a bare name is matched as an identifier, never as part of a longer one; a path
/// (`registered::pin(`) is matched as that path, or as an import of its last segment from its
/// module. A substring of another name is not a hit.
fn names_entry(line: &str, entry: &str) -> bool {
    let entry = entry.trim_end_matches('(');
    let is_use = {
        let line = line.trim_start();
        line.starts_with("use ") || line.starts_with("pub use ")
    };
    match entry.rsplit_once("::") {
        // A bare name is a call, an import or a function named as a value; a string that holds the
        // name is none of them.
        None => names_outside_a_string(line, entry),
        Some((module, symbol)) => {
            names_symbol(line, entry)
                || (is_use
                    && names_symbol(line, module)
                    && (names_symbol(line, symbol)
                        // A glob (`workflow::*`, `workflow::{*, x}`) or the module under another name
                        // (`workflow as w`) brings the symbol in without spelling it.
                        || line.contains('*')
                        || line.contains(" as ")))
                // A method call: `route.enrol(` is `ChatRoute::enrol(` on a value.
                || (METHOD_ENTRIES.contains(&symbol)
                    && module.starts_with(char::is_uppercase)
                    && line.contains(&format!(".{symbol}(")))
        }
    }
}

/// The entries that are methods of a value, which a caller spells `value.name(`. An associated
/// function (`HostScope::claim(`) is called by its path alone and is not in this list.
const METHOD_ENTRIES: &[&str] = &["enrol"];

/// Whether `line` names `symbol` as an identifier, not inside a string literal.
fn names_outside_a_string(line: &str, symbol: &str) -> bool {
    line.match_indices(symbol).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + symbol.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !part(before) && !part(after) && before != Some('"') && after != Some('"')
    })
}

/// The statements of `text` that are code, one string each: a `use` group that runs over several
/// lines is one statement, so its names are read together with the module they come from.
fn statements(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<String> = None;
    for line in code(text) {
        match open.as_mut() {
            Some(held) => {
                held.push(' ');
                held.push_str(line.trim());
                if line.contains(';') {
                    out.extend(open.take());
                }
            }
            None => {
                let trimmed = line.trim_start();
                let starts = trimmed.starts_with("use ") || trimmed.starts_with("pub use ");
                if starts && !line.contains(';') {
                    open = Some(line.trim().to_string());
                } else {
                    out.push(line.to_string());
                }
            }
        }
    }
    out.extend(open);
    out
}

/// Every entry point a file names in its code, as `(entry, line)`.
fn entry_points_named(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    PROVISIONING_ENTRY_POINTS
        .iter()
        .filter(|entry| {
            statements(&text)
                .iter()
                .any(|line| names_entry(line, entry))
        })
        .map(|entry| (*entry).to_string())
        .collect()
}

fn allowed(path: &Path, root: &Path, allowed: &[&str]) -> bool {
    allowed.iter().any(|ok| path == root.join(ok))
}

#[test]
fn no_source_but_the_windows_and_the_named_files_calls_a_provisioning_entry_point() {
    purlis_core::unsteered!();
    let (crates, app) = roots();
    let core = crates.join("purlis-core");
    let app_src = app.join("src-tauri/src");
    let mut calls = Vec::new();
    // Each directory is held to its own count, so one that emptied cannot hide behind the others.
    for (root, dir, at_least) in SCANNED {
        let base = if *root == "crates" { &crates } else { &app };
        for path in scanned(base, dir, *at_least) {
            let permitted = match *root {
                "crates" => allowed(&path, &core, ALLOWED_IN_CORE),
                _ => allowed(&path, &app_src, ALLOWED_IN_APP),
            };
            if permitted {
                continue;
            }
            for name in entry_points_named(&path) {
                calls.push(format!("{} calls {name}", path.display()));
            }
        }
    }
    assert!(
        calls.is_empty(),
        "a source that is not the window's provisions:\n{}",
        calls.join("\n")
    );
}

#[test]
fn every_file_allowed_to_call_a_provisioning_entry_point_still_does() {
    purlis_core::unsteered!();
    let (crates, app) = roots();
    for (root, files) in [
        (crates.join("purlis-core"), ALLOWED_IN_CORE),
        (app.join("src-tauri/src"), ALLOWED_IN_APP),
    ] {
        for ok in files {
            let path = root.join(ok);
            assert!(path.is_file(), "{ok} is allowed and is not a file");
            assert!(
                !entry_points_named(&path).is_empty(),
                "{ok} is allowed to call a provisioning entry point and names none: take it off \
                 the list"
            );
        }
    }
}

/// Whether `line` names `symbol` as an identifier (an import, a path or a call), however it is
/// spelled: `workflow::for_ci_push(`, `use ...::{for_ci_push, ...}`, `as`-renamed or bare.
fn names_symbol(line: &str, symbol: &str) -> bool {
    line.match_indices(symbol).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + symbol.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}

#[test]
fn only_the_window_names_the_ci_push_credential_and_nothing_reads_the_second_token_by_name() {
    purlis_core::unsteered!();
    let (crates, app) = roots();
    let core = crates.join("purlis-core");
    let owners = [
        core.join("src/forge/accounts/workflow.rs"),
        core.join("src/forge/provision.rs"),
    ];
    let mut bad = Vec::new();
    for (root, dir, at_least) in SCANNED {
        let base = if *root == "crates" { &crates } else { &app };
        for path in scanned(base, dir, *at_least) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let shown = path.display();
            // `for_ci_push` is defined in `workflow.rs` and listed in `provision.rs`; the window and
            // slice F2's held push call it, from the app.
            let owns_it = owners.contains(&path);
            let in_app = *root == "app";
            for line in code(&text) {
                if !owns_it && !in_app && names_symbol(line, "for_ci_push") {
                    bad.push(format!("{shown} names for_ci_push: {}", line.trim()));
                }
                // The keyring item of the second token is read by its name in `workflow.rs` alone,
                // whether it is spelled as the literal or as the scope or item constant.
                let calls_keyring = ["get(", "exists(", "delete(", "set("]
                    .iter()
                    .any(|call| line.contains(call));
                let names_item = line.contains("\"workflow\"")
                    || line.contains("workflow::SCOPE")
                    || line.contains("workflow::ITEM");
                if path != owners[0] && calls_keyring && names_item {
                    bad.push(format!("{shown} reads the workflow item: {}", line.trim()));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn the_symbol_scan_sees_an_import_a_rename_and_a_call_and_not_a_longer_name() {
    purlis_core::unsteered!();
    for hit in [
        "use purlis_core::forge::accounts::workflow::for_ci_push;",
        "use x::workflow::{keep, for_ci_push as push};",
        "    workflow::for_ci_push(&scope, k, &a)",
        "let f = for_ci_push;",
    ] {
        assert!(names_symbol(hit, "for_ci_push"), "{hit}");
    }
    for miss in ["let x = for_ci_push_later(1);", "let my_for_ci_push = 1;"] {
        assert!(!names_symbol(miss, "for_ci_push"), "{miss}");
    }
}

#[test]
fn an_entry_is_matched_by_its_symbol_and_not_by_a_substring_of_another_name() {
    purlis_core::unsteered!();
    for hit in [
        "let r = confirmed::confirm(&s, h, c, &cand, &a, id);",
        "use purlis_core::forge::confirmed::{confirm, Candidate};",
        "forget_by_key(&scope, &root, key)",
        "let f = registered::unpin;",
    ] {
        let entry = PROVISIONING_ENTRY_POINTS
            .iter()
            .find(|entry| names_entry(hit, entry));
        assert!(entry.is_some(), "{hit}");
    }
    for miss in [
        "let r = confirmed::confirm_trusted_later(1);",
        "let my_forget_by_key = 1;",
        "unregistered::pin_it(1)",
        "xconfirmed::confirm(1)",
    ] {
        assert!(
            PROVISIONING_ENTRY_POINTS
                .iter()
                .all(|entry| !names_entry(miss, entry)),
            "{miss}"
        );
    }
}

#[test]
fn the_scan_reads_a_use_group_over_lines_an_alias_a_glob_a_function_value_and_a_method_call() {
    purlis_core::unsteered!();
    let hit = |text: &str| {
        let found = statements(text).iter().any(|line| {
            PROVISIONING_ENTRY_POINTS
                .iter()
                .any(|e| names_entry(line, e))
        });
        assert!(found, "{text}");
    };
    // A use group split over lines.
    hit("use purlis_core::forge::accounts::workflow::{\n    SCOPE,\n    keep,\n};");
    hit("use purlis_core::forge::accounts::workflow::{\n    SCOPE,\n    for_ci_push as push,\n};");
    // The module under another name, and a glob of it.
    hit("use purlis_core::forge::accounts::workflow as w;");
    hit("use purlis_core::forge::confirmed::*;");
    // A function named as a value.
    hit("let f = items.iter().map(for_ci_push);");
    // A method call on a value.
    hit("route.enrol(&chat, clones);");
    hit("let sentinel = proxy.enrol(&chat, repos);");
    hit("let repo = repo.with_forge_id(id);");
    // What is not one: a string, a longer name, a comment.
    for miss in [
        "const NAME: &str = \"vouch_clone\";",
        "let x = other.enrolment(1);",
        "// route.enrol(&chat, clones);",
    ] {
        let found = statements(miss).iter().any(|line| {
            PROVISIONING_ENTRY_POINTS
                .iter()
                .any(|e| names_entry(line, e))
        });
        assert!(!found, "{miss}");
    }
}

#[test]
fn a_file_named_with_tests_is_scanned_unless_its_module_is_declared_cfg_test() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("mod.rs"),
        "#[cfg(test)]\nmod tests;\nmod testsupport;\n",
    )
    .unwrap();
    for name in ["tests.rs", "testsupport.rs", "tests_helpers.rs"] {
        std::fs::write(dir.path().join(name), "fn f() {}\n").unwrap();
    }
    let mut found = Vec::new();
    sources(dir.path(), &mut found);
    let names: Vec<String> = found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(!names.contains(&"tests.rs".to_string()), "{names:?}");
    assert!(names.contains(&"testsupport.rs".to_string()), "{names:?}");
    assert!(names.contains(&"tests_helpers.rs".to_string()), "{names:?}");
}

#[test]
fn the_scan_finds_a_provisioning_call_where_there_is_one() {
    purlis_core::unsteered!();
    // The control: the window's own file, which the scan skips, does call them.
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/src/onelogin.rs");
    assert!(
        !entry_points_named(&app).is_empty(),
        "the window's file calls a provisioning entry point"
    );
}
