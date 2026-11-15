// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Files and egress are enforced on each harness** (R-353, Q225; the harness matrix's sandbox
//! row, ADR 0124): a write outside the chat's folders and a connection to a host nothing lists
//! are refused, on Claude Code by its own sandbox for Bash, by deny rules for its file tools
//! where a denial class holds the path, and by denying its web tools, and on Codex and opencode
//! by purlis's wrap round the whole process. What each compiles is read whole, as the chat would
//! be started with it, at the full sandbox; and at Basic sandbox, the floor, Claude Code's
//! denial classes are held and its web tools allowed, its proxy carrying every host (Q254).
//!
//! Each test is a pin, and its doc comment names the mutations it turns red (Q208, Q253).

use std::path::{Path, PathBuf};

use super::*;

/// The sandbox on, with no toolchains preset, so no package cache widens what a chat writes.
const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

/// A host no preset, project or person lists.
const UNLISTED: &str = "unlisted.example";

/// A project whose committed file turns the sandbox on, its root as the kernel names it, and a
/// home of its own: what a chat is compiled for.
struct Project {
    _plane: tempfile::TempDir,
    _home: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
}

impl Project {
    fn new() -> Self {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::write(plane.path().join("charter.toml"), ON).expect("charter.toml");
        let home = tempfile::tempdir().expect("a home");
        Self {
            root: plane.path().canonicalize().expect("a real path"),
            home: home.path().canonicalize().expect("a real path"),
            _plane: plane,
            _home: home,
        }
    }

    fn machine(&self) -> Machine {
        Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(self.home.clone()),
            os: Os::MacOs,
        }
    }

    /// A folder outside every folder the chat may write: in the home, beside nothing it holds.
    fn outside(&self) -> PathBuf {
        self.home.join("elsewhere")
    }

    /// What a denial class holds in the project: its vaults.
    fn vaults(&self) -> String {
        self.root.join(".charter/vaults").display().to_string()
    }
}

/// Whether `applied`'s proxy carries a connection to [`UNLISTED`]: what refuses it, wherever the
/// chat's network goes through purlis's proxy.
fn carries_the_unlisted_host(applied: &Applied) -> bool {
    applied
        .confines()
        .decides()
        .decide_on_any_port(UNLISTED, 443, &[])
        .carries()
}

/// The strings of a list in `value`, none where it is not there.
fn strings(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The entries of `profile`'s one `file-write*` allow, each as written there: from its first
/// line to the one that closes it, alone or after its last entry.
fn write_allows(profile: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    for line in profile
        .lines()
        .skip_while(|line| *line != "(allow file-write*")
        .skip(1)
    {
        let line = line.trim();
        if line == ")" {
            break;
        }
        match line.strip_suffix("))") {
            Some(last) => {
                entries.push(&line[..last.len() + 1]);
                break;
            }
            None => entries.push(line),
        }
    }
    entries
}

/// R-353 on Claude Code at the full sandbox: Bash runs in its sandbox, which lets it write no
/// folder outside the chat's and connects only to listed hosts, strictly; its file tools are
/// denied what a denial class holds (the vaults here); and its web tools are denied, since no
/// setting holds them to the sandbox's hosts. A file tool's write elsewhere outside the chat's
/// folders is Claude Code's own permission check's, not a deny rule purlis writes, so this test
/// claims no deny for it.
///
/// A pin of `claude::rules`; its mutations, each red here: `strictAllowlist` set to false;
/// `allowUnsandboxedCommands` set to true; `WEB_TOOLS` dropped from the deny chain; the vaults'
/// `deny_write` (and their `Edit` rules) dropped.
#[test]
fn files_and_egress_are_enforced_on_claude_code() {
    let project = Project::new();
    let applied =
        compiled_anyway(Harness::ClaudeCode, &project.root, &project.machine()).expect("compiles");
    let Form::ClaudeCode(settings) = applied.form() else {
        panic!(
            "compiled for Claude Code's own sandbox: {:?}",
            applied.form()
        );
    };
    let sandbox = &settings.sandbox;

    // Bash runs in its sandbox: on, never a command let out of it, and the chat refused rather
    // than started where the sandbox cannot run.
    assert_eq!(sandbox["enabled"], true, "{sandbox}");
    assert_eq!(sandbox["failIfUnavailable"], true, "{sandbox}");
    assert_eq!(sandbox["allowUnsandboxedCommands"], false, "{sandbox}");

    // A Bash write outside the chat's folders is refused by its sandbox: nothing it is told to
    // let a command write covers the folder, and what a denial class holds is denied by path.
    let outside = project.outside();
    let allowed = strings(&sandbox["filesystem"]["allowWrite"]);
    assert!(
        !allowed
            .iter()
            .any(|folder| outside.starts_with(Path::new(folder))),
        "{allowed:?} lets a command write {}",
        outside.display()
    );
    let vaults = project.vaults();
    assert!(
        strings(&sandbox["filesystem"]["denyWrite"]).contains(&vaults),
        "{sandbox}"
    );

    // A connection to an unlisted host is refused by its sandbox: only listed hosts, strictly,
    // and no local port but purlis's proxy's, which refuses the host too.
    assert_eq!(sandbox["network"]["strictAllowlist"], true, "{sandbox}");
    assert_eq!(sandbox["network"]["allowLocalBinding"], false, "{sandbox}");
    let listed = strings(&sandbox["network"]["allowedDomains"]);
    assert!(!listed.iter().any(|host| host == UNLISTED), "{listed:?}");
    assert!(!carries_the_unlisted_host(&applied));

    // A file tool's write where a denial class holds the path is refused by a deny rule: its
    // Edit tool runs in Claude Code's own process, outside the sandbox.
    for rule in [format!("Edit(/{vaults})"), format!("Edit(/{vaults}/**)")] {
        assert!(
            settings.deny.contains(&rule),
            "{rule} in {:?}",
            settings.deny
        );
    }

    // And its web tools are denied: no setting holds them to the sandbox's hosts.
    for tool in claude::WEB_TOOLS {
        assert!(
            settings.deny.iter().any(|rule| rule == tool),
            "{tool} in {:?}",
            settings.deny
        );
    }
}

/// R-353 on a harness purlis wraps whole: the harness itself is the program `sandbox-exec`
/// starts under purlis's profile, which denies by default, lets the chat write no folder outside
/// its own, and lets it connect to nothing but its own proxy's two ports, whose proxy refuses an
/// unlisted host. So the same write and connection from the harness's own process, its hooks
/// and its tools among them, are refused.
///
/// A pin of the Seatbelt profile (`seatbelt`); its mutations, each red on Codex and on opencode:
/// `BASE` set to `(allow default)`; a bare `(allow network-outbound)`; the home written as a
/// `file-write*` subpath.
fn files_and_egress_are_enforced_by_the_wrap(harness: Harness) {
    let project = Project::new();
    let applied = compiled_anyway(harness, &project.root, &project.machine()).expect("compiles");
    assert!(
        matches!(applied.form(), Form::Codex(_) | Form::Opencode(_)),
        "{harness:?} is wrapped by purlis: {:?}",
        applied.form()
    );
    let confinement = applied.confine().expect("confined").expect("a wrap");
    let line = applied
        .line(
            Words {
                program: "harness".to_owned(),
                command: Vec::new(),
                armed: Vec::new(),
                charters: Vec::new(),
            },
            &At {
                cwd: Some(&project.root),
                hook_socket: None,
                confinement: Some(&confinement),
                no_opt_out: false,
                purlis: None,
            },
        )
        .expect("starts");

    // The whole process: the harness is what the wrap runs, under its profile.
    assert_eq!(line.program, backend::SANDBOX_EXEC, "{harness:?}");
    assert_eq!(line.args[0], "-p", "{harness:?}");
    assert_eq!(line.args.get(2).map(String::as_str), Some("harness"));
    let profile = &line.args[1];
    assert!(
        profile.starts_with("(version 1)\n(deny default)\n"),
        "{harness:?}: {profile}"
    );
    assert!(!profile.contains("(allow default)"), "{harness:?}");

    // The write: no folder the profile lets it write holds the folder outside.
    let outside = project.outside();
    let writes = write_allows(profile);
    assert!(
        !writes.is_empty(),
        "{harness:?}: no write allow in {profile}"
    );
    for allow in writes {
        let folder = allow
            .strip_prefix("(subpath \"")
            .and_then(|rest| rest.strip_suffix("\")"));
        if let Some(folder) = folder {
            assert!(
                !outside.starts_with(folder),
                "{harness:?} writes {} through {allow}",
                outside.display()
            );
        }
    }
    assert!(
        !profile.contains("(allow file-write* (subpath \"/\"))"),
        "{harness:?}"
    );

    // The connection: out only to its own proxy's ports, which its environment names, and the
    // proxy refuses the host.
    let ports = confinement.proxy_ports();
    for allow in profile
        .lines()
        .filter(|line| line.starts_with("(allow network-outbound"))
    {
        assert!(
            ports.iter().any(|port| {
                allow == format!("(allow network-outbound (remote ip \"localhost:{port}\"))")
            }),
            "{harness:?} connects past its proxy: {allow}"
        );
    }
    assert!(
        line.env
            .contains(&("HTTPS_PROXY".to_owned(), confinement.proxy_url())),
        "{harness:?}: {:?}",
        line.env
    );
    assert!(!carries_the_unlisted_host(&applied), "{harness:?}");
}

#[test]
fn files_and_egress_are_enforced_on_codex() {
    files_and_egress_are_enforced_by_the_wrap(Harness::Codex);
}

#[test]
fn files_and_egress_are_enforced_on_opencode() {
    files_and_egress_are_enforced_by_the_wrap(Harness::Opencode);
}

/// Q254, the floor's matrix cell (ADR 0124): at Basic sandbox, a project that has not turned
/// the sandbox on, Claude Code's denial classes are held as at the full sandbox, for Bash by its
/// sandbox and for its file tools by deny rules, and its web tools are allowed: no host is
/// listed, and the chat's proxy carries every host, an unlisted one too. "Web tools denied" is
/// the full sandbox's claim ([`files_and_egress_are_enforced_on_claude_code`]), never the
/// floor's.
///
/// A pin of `claude::floor`; its mutations, each red here: `WEB_TOOLS` added to the floor's
/// deny; the vaults' `deny_write` (or their `Edit` rules) dropped at the floor; an
/// `allowedDomains` list or `strictAllowlist` set at the floor; the floor's proxy narrowed from
/// carrying every host.
#[test]
fn at_basic_sandbox_claude_codes_denial_classes_are_held_and_its_web_tools_allowed() {
    let project = Project::new();
    std::fs::write(project.root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    let machine = project.machine();
    let applied = match decide(
        Harness::ClaudeCode,
        &project.root,
        &machine,
        &|_| true,
        None,
        None,
    )
    .expect("decided")
    {
        Some(Decided::Sandboxed(applied)) => applied,
        other => panic!("not the floor: {other:?}"),
    };
    assert_eq!(applied.level(), Level::Floor);
    let Form::ClaudeCode(settings) = applied.form() else {
        panic!(
            "compiled for Claude Code's own sandbox: {:?}",
            applied.form()
        );
    };
    let sandbox = &settings.sandbox;
    assert_eq!(sandbox["enabled"], true, "{sandbox}");
    assert_eq!(sandbox["allowUnsandboxedCommands"], false, "{sandbox}");

    // The denial classes are held: every path one holds is denied to a command, and to a file
    // tool by a deny rule, as at the full sandbox.
    let deny_write = strings(&sandbox["filesystem"]["denyWrite"]);
    let vaults = project.vaults();
    assert!(deny_write.contains(&vaults), "{vaults} in {deny_write:?}");
    for rule in [format!("Edit(/{vaults})"), format!("Edit(/{vaults}/**)")] {
        assert!(
            settings.deny.contains(&rule),
            "{rule} in {:?}",
            settings.deny
        );
    }
    let deny_read = strings(&sandbox["filesystem"]["denyRead"]);
    for denial in Denied::of(&project.root, &machine)
        .paths
        .iter()
        .filter(|denial| denial.access == Access::ReadWrite)
    {
        let path = denial.path.display().to_string();
        assert!(deny_read.contains(&path), "{path} in {deny_read:?}");
    }

    // Its web tools are allowed, and nothing limits the hosts a command reaches: its proxy
    // carries every one.
    for tool in claude::WEB_TOOLS {
        assert!(
            !settings.deny.iter().any(|rule| rule == tool),
            "{tool} in {:?}",
            settings.deny
        );
    }
    assert!(
        sandbox["network"].get("allowedDomains").is_none(),
        "{sandbox}"
    );
    assert!(
        sandbox["network"].get("strictAllowlist").is_none(),
        "{sandbox}"
    );
    assert!(carries_the_unlisted_host(&applied));
}
