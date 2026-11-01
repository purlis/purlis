// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a grant may name (#1342): a host by the project's own hosts' rules, a folder outside
//! every denial class, and never the classes themselves, which get the way that works.

use std::path::PathBuf;

use super::*;
use crate::sandbox::{Access, Class, Denial};

/// A machine on disk: a project, a home, a folder you listed in Settings, the chat's own temp
/// folder, and what `PATH` and the harnesses hold — every path as the kernel names it.
struct Disk {
    _dir: tempfile::TempDir,
    at: PathBuf,
    root: PathBuf,
    chat: PathBuf,
    home: PathBuf,
    denied: Vec<Denial>,
    allowed: Vec<PathBuf>,
    refused: Vec<PathBuf>,
}

impl Disk {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let at = dir.path().canonicalize().expect("real");
        for folder in [
            "plane/workspaces/alpha/repo",
            "plane/workspaces/beta",
            "plane/.purlis/app",
            "home/Library/LaunchAgents",
            "home/.config/git",
            "home/.config/purlis/vaults",
            "home/.cache/pip",
            "home/.local/bin",
            "opt/tools/homebrew/bin",
            "tmp/chat",
        ] {
            std::fs::create_dir_all(at.join(folder)).expect("made");
        }
        let denial = |class, path: &str| Denial {
            class,
            path: at.join(path),
            access: Access::Write,
            named: None,
            reach: crate::sandbox::Reach::AndBelow,
            any_case: false,
        };
        Self {
            denied: vec![
                denial(Class::Vaults, "home/.config/purlis/vaults"),
                denial(Class::HumanPowers, "home/.config/purlis/purlisd"),
                denial(Class::Integrity, "plane/.purlis/app"),
            ],
            allowed: vec![at.join("opt/tools")],
            refused: vec![
                at.join("home/.local/bin"),
                at.join("opt/tools/homebrew/bin"),
                at.join("home/.claude"),
            ],
            root: at.join("plane"),
            chat: at.join("plane/workspaces/alpha/repo"),
            home: at.join("home"),
            _dir: dir,
            at,
        }
    }

    fn path(&self, below: &str) -> String {
        self.at.join(below).display().to_string()
    }

    fn place(&self) -> Place<'_> {
        Place {
            root: &self.root,
            chat: &self.chat,
            home: Some(&self.home),
            denied: &self.denied,
            allowed: &self.allowed,
            // No harness temp root: the fixture itself lives in one on some machines, and the
            // rule has its own test.
            harness_temp: &[],
            refused: &self.refused,
            folded: cfg!(target_os = "macos"),
        }
    }

    fn link(&self, at: &str, to: &str) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(self.at.join(to), self.at.join(at)).expect("linked");
    }
}

#[test]
fn a_folder_on_the_allowlist_is_granted_whole_clean_and_resolved() {
    let m = Disk::new();
    let here = m.place();
    for (typed, granted) in [
        // Another workspace of the same project.
        (
            "plane/workspaces/beta/sessions",
            "plane/workspaces/beta/sessions",
        ),
        // A folder you listed in Settings, and one inside it.
        ("opt/tools/cache/../x", "opt/tools/x"),
    ] {
        assert_eq!(
            write(&m.path(typed), &here),
            Ok(m.at.join(granted)),
            "{typed}"
        );
    }
}

#[test]
fn a_folder_whose_name_holds_a_character_that_draws_as_nothing_is_never_granted() {
    // #1709: a folder in this project whose name holds a character with no glyph (a format
    // character, a zero-width one, a control) could be read as another folder; purlis grants
    // only a name that reads as what it is, as it already refuses a control character.
    let m = Disk::new();
    let here = m.place();
    for name in [
        "out\u{202e}txt",
        "out\u{200b}",
        "\u{2066}out\u{2069}",
        "out\u{feff}",
        "out\u{7}",
        "out`x",
    ] {
        let typed = m.path(&format!("plane/workspaces/beta/{name}"));
        assert!(
            matches!(write(&typed, &here), Err(Refused::Not(_))),
            "{name:?}: {:?}",
            write(&typed, &here)
        );
        assert!(grantable(&typed, &here).is_err(), "{name:?}");
    }
    // A name in another script draws as itself, and is granted.
    let typed = m.path("plane/workspaces/beta/выход");
    assert_eq!(
        write(&typed, &here),
        Ok(m.at.join("plane/workspaces/beta/выход"))
    );
}

#[cfg(unix)]
#[test]
fn a_link_to_a_folder_whose_name_draws_as_nothing_is_never_granted() {
    // #1709: what is granted is the folder a name resolves to, so a plainly named link to a
    // folder whose name holds a character with no glyph is refused as that folder would be.
    let m = Disk::new();
    let here = m.place();
    std::fs::create_dir_all(m.at.join("plane/workspaces/beta/out\u{202e}txt")).expect("made");
    m.link(
        "plane/workspaces/beta/plain",
        "plane/workspaces/beta/out\u{202e}txt",
    );
    let typed = m.path("plane/workspaces/beta/plain");
    assert!(
        matches!(write(&typed, &here), Err(Refused::Not(_))),
        "{:?}",
        write(&typed, &here)
    );
    assert!(grantable(&typed, &here).is_err());
}

#[test]
fn a_folder_off_the_allowlist_gets_no_allow_and_says_why() {
    // D-1342-10 and D-1342-11: the home folder, the caches every project shares, the system
    // temp folders and the machine are where later code is loaded from, outside any sandbox.
    let m = Disk::new();
    let here = m.place();
    for typed in [
        // `~/.config` holds git's own config (core.hooksPath, core.fsmonitor): never grantable.
        "home/.config",
        "home/.config/git",
        "home/Library/LaunchAgents",
        "home/.ssh",
        // A cache shared by every project is no longer offered (D-1342-11).
        "home/.cache/pip",
        "home/Library/Caches/go-build",
        // No temp folder is on the allowlist (D-1342-14).
        "tmp/chat",
        "tmp/chat/build",
        "tmp/other",
        // On PATH, inside one, or holding one; and a harness's own home.
        "opt/tools/homebrew/bin",
        "home/.local/bin/x",
        "opt/tools/homebrew",
        "home/.claude/hooks",
    ] {
        // Refused either way: off the allowlist, or (#2071) a later-code name of its own.
        assert!(
            matches!(
                write(&m.path(typed), &here),
                Err(Refused::Outside(_) | Refused::Never(_))
            ),
            "{typed}: {:?}",
            write(&m.path(typed), &here)
        );
    }
    // The system temp folders, and a harness's temp root in one: never granted (a test's own
    // project may sit inside one, which then refuses them as holding the project instead).
    for typed in [
        "/tmp",
        "/private/tmp",
        "/private/tmp/claude-1/x",
        "/private/var/folders",
    ] {
        assert!(write(typed, &here).is_err(), "{typed}");
    }
    // The sentence names no folder: the window shows the chat's path apart from purlis's words.
    let why = write(&m.path("home/.config"), &here)
        .unwrap_err()
        .to_string();
    assert!(
        why.starts_with("purlis will not let a chat write that folder"),
        "{why}"
    );
    assert!(!why.contains(&m.at.display().to_string()), "{why}");
    // A folder you list in Settings is judged by every refusal but the allowlist.
    assert_eq!(
        grantable(&m.path("home/.cache/pip"), &here),
        Ok(m.at.join("home/.cache/pip"))
    );
    assert!(matches!(
        grantable(&m.path("home/.local/bin"), &here),
        Err(Refused::Outside(_))
    ));
    assert!(matches!(
        grantable(&m.path("home"), &here),
        Err(Refused::Not(_))
    ));
}

#[test]
fn a_link_is_judged_as_where_it_points_and_only_that_is_granted() {
    // D-1342-12 (review R1): a chat makes a link inside what it may be granted, pointing out.
    let m = Disk::new();
    m.link(
        "plane/workspaces/alpha/repo/cache",
        "home/Library/LaunchAgents",
    );
    m.link("opt/tools/agents", "home/Library/LaunchAgents");
    let here = m.place();
    for typed in [
        "plane/workspaces/alpha/repo/cache",
        "plane/workspaces/alpha/repo/cache/sub",
        "opt/tools/agents",
    ] {
        assert!(
            matches!(
                write(&m.path(typed), &here),
                Err(Refused::Outside(_) | Refused::Never(_))
            ),
            "{typed}: {:?}",
            write(&m.path(typed), &here)
        );
    }
    // A link to a folder that IS on the allowlist is granted as where it points, and only that.
    m.link("opt/tools/to-beta", "plane/workspaces/beta");
    assert_eq!(
        write(&m.path("opt/tools/to-beta"), &here),
        Ok(m.at.join("plane/workspaces/beta"))
    );
    // A folder granted, then swapped for a link, is dropped at the next start.
    let granted = write(&m.path("plane/workspaces/beta/out"), &here).expect("granted");
    assert_eq!(
        still_grantable(std::slice::from_ref(&granted), &here),
        std::slice::from_ref(&granted)
    );
    m.link("plane/workspaces/beta/out", "home/Library/LaunchAgents");
    assert!(still_grantable(std::slice::from_ref(&granted), &here).is_empty());
}

#[test]
fn a_folder_you_listed_is_compared_as_listed_and_a_link_in_its_place_widens_nothing() {
    // Review R8: a folder listed inside what a chat may write is swapped for a link out. The
    // allowlist keeps the spelling it was listed under, and the machine's ground drops it.
    let m = Disk::new();
    let listed = m.at.join("plane/workspaces/beta/tools");
    std::fs::create_dir_all(&listed).expect("made");
    let allowed = vec![listed.clone()];
    let here = Place {
        allowed: &allowed,
        ..m.place()
    };
    assert!(still_itself(&listed));
    std::fs::remove_dir(&listed).expect("removed");
    m.link("plane/workspaces/beta/tools", "home/Library/LaunchAgents");
    assert!(!still_itself(&listed), "it resolves elsewhere now");
    // What a block names through it is judged where it really is: LaunchAgents, off the list.
    assert!(matches!(
        write(&m.path("plane/workspaces/beta/tools/x.plist"), &here),
        Err(Refused::Outside(_) | Refused::Never(_))
    ));
    assert!(matches!(
        write(&m.path("home/Library/LaunchAgents"), &here),
        Err(Refused::Outside(_) | Refused::Never(_))
    ));
}

#[test]
fn a_harness_temp_root_is_refused_wherever_the_project_is() {
    // Never weakened (round 4): a listed folder in a harness's temp root grants nothing in it.
    let m = Disk::new();
    let allowed = vec![std::path::PathBuf::from("/private/tmp/claude-9/x")];
    let with = Place {
        allowed: &allowed,
        harness_temp: &HARNESS_TEMP,
        ..m.place()
    };
    assert!(matches!(
        write("/private/tmp/claude-9/x/y", &with),
        Err(Refused::Outside(_))
    ));
    let without = Place {
        allowed: &allowed,
        ..m.place()
    };
    assert_eq!(
        write("/private/tmp/claude-9/x/y", &without),
        Ok(std::path::PathBuf::from("/private/tmp/claude-9/x/y"))
    );
}

#[cfg(target_os = "macos")]
#[test]
fn on_macos_a_case_variant_of_a_refused_folder_is_refused_before_it_exists() {
    // Review R4: on a case-insensitive volume `HOMEBREW/BIN` is the folder on `PATH`.
    let m = Disk::new();
    let here = m.place();
    assert!(matches!(
        write(&m.path("opt/tools/HOMEBREW/BIN/new"), &here),
        Err(Refused::Outside(_))
    ));
    assert!(matches!(
        write(&m.path("plane/workspaces/beta/repo/.GIT/hooks"), &here),
        Err(Refused::Never(_))
    ));
}

#[test]
fn the_machine_the_home_the_project_and_what_holds_them_are_never_a_grant() {
    let m = Disk::new();
    let here = m.place();
    for typed in [
        "/".to_owned(),
        m.path("home"),
        m.path("home/"),
        m.path(""),
        m.path("plane"),
        m.path("plane/workspaces"),
        m.path("plane/workspaces/alpha/repo"),
        m.path("plane/workspaces/alpha/repo/.."),
        "relative/path".to_owned(),
        "~/cache".to_owned(),
        m.path("opt/tools/a\nb"),
        m.path("opt/tools/a`b"),
    ] {
        assert!(
            matches!(write(&typed, &here), Err(Refused::Not(_))),
            "{typed:?} was granted: {:?}",
            write(&typed, &here)
        );
    }
}

#[test]
fn every_denial_class_is_refused_with_the_way_that_works() {
    let m = Disk::new();
    let here = m.place();
    let never = |typed: &str| match write(&m.path(typed), &here) {
        Err(Refused::Never(why)) => why,
        other => panic!("{typed}: {other:?}"),
    };
    assert!(never("home/.config/purlis/vaults/forge").contains("purlis secret exec"));
    assert!(never("plane/.purlis/app").contains("purlis session record"));
    // purlis's state folder under the other spelling, which this project does not have.
    assert!(never("plane/.charter/handbacks").contains("purlis session record"));
    assert!(never("home/.config/purlis/purlisd").contains("only a person approves"));
    // A name a later program loads code from, wherever it is.
    for planted in [
        "plane/workspaces/beta/repo/.git/hooks",
        "tmp/chat/repo/.vscode",
        "opt/tools/.zshrc",
        "opt/tools/other/charter.toml",
    ] {
        assert!(never(planted).contains("later program"), "{planted}");
    }
}

#[test]
fn a_grant_judged_again_at_the_start_drops_what_a_class_now_covers() {
    let m = Disk::new();
    let here = m.place();
    let kept = still_grantable(
        &[
            m.at.join("opt/tools/x"),
            m.at.join("opt/tools/x"),
            m.at.join("plane/.purlis/app/x"),
            m.at.join("home"),
            m.at.join("home/.config"),
        ],
        &here,
    );
    assert_eq!(kept, [m.at.join("opt/tools/x")]);
}

#[test]
fn settings_names_what_a_folder_you_list_holds_that_later_code_loads() {
    let m = Disk::new();
    let here = m.place();
    let held = holds_later_code(&m.at.join("home/Library"), &here);
    assert!(
        held.iter().any(|one| one.ends_with("Library/LaunchAgents")),
        "{held:?}"
    );
    assert!(holds_later_code(&m.at.join("home/.cache/pip"), &here).is_empty());
}

#[test]
fn a_host_grant_takes_the_project_hosts_rules() {
    assert_eq!(
        host(" API.example.com ").map(|h| h.to_string()),
        Ok("api.example.com".to_owned())
    );
    assert_eq!(
        host("10.100.39.145:6443").map(|h| h.to_string()),
        Ok("10.100.39.145:6443".to_owned())
    );
    for typed in [
        "localhost",
        "169.254.169.254",
        "https://x.example/y",
        "x",
        "",
    ] {
        assert!(matches!(host(typed), Err(Refused::Not(_))), "{typed}");
    }
}

#[test]
fn one_chats_grants_hold_each_thing_once() {
    let mut grants = Grants::default();
    assert!(grants.is_empty());
    let api = What::Host(host("api.example.com").unwrap());
    let cache = What::Write(PathBuf::from("/opt/cache"));
    for what in [&api, &cache, &api, &cache] {
        grants.add(what);
    }
    assert_eq!(grants.hosts.len(), 1);
    assert_eq!(grants.writes, [PathBuf::from("/opt/cache")]);
}

#[test]
fn a_write_block_proposes_the_folder_it_was_refused_in() {
    let dir = tempfile::tempdir().expect("a folder");
    let real = dir.path().canonicalize().unwrap();
    assert_eq!(proposed_folder(&real), real, "a folder is itself");
    assert_eq!(
        proposed_folder(&real.join("not-there/file.lock")),
        real.join("not-there")
    );
}

#[test]
fn the_chat_is_told_what_was_allowed_and_to_retry() {
    let told = told(&What::Host(host("api.example.com").unwrap()), Level::Chat);
    // The target in backticks: it is what the block named, inside purlis's own sentence.
    assert_eq!(
        told,
        "⬢ purlis: reaching `api.example.com` — Allowed for this chat — retry."
    );
    let folder = super::told(&What::Write(PathBuf::from("/tmp/a b`c")), Level::You);
    assert_eq!(
        folder,
        "⬢ purlis: writing in `/tmp/a bc` — Allowed for every chat of this project on this \
         machine — retry."
    );
    assert_eq!(Level::of_word("you"), Some(Level::You));
    assert_eq!(Level::of_word("everyone"), None);
}

// ---- what a start compiles in --------------------------------------------------------------

use crate::sandbox::{Compiled, Machine, Os, Plane, claude, codex, local, opencode};

const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n";

/// A project folder with no manifest on disk (the policy is handed over as text), and a home
/// beside it.
struct Project {
    _dir: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
}

fn project() -> Project {
    let dir = tempfile::tempdir().expect("a folder");
    let real = dir.path().canonicalize().expect("real");
    let root = real.join("plane");
    let home = real.join("home");
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("project");
    std::fs::create_dir_all(&home).expect("home");
    Project {
        _dir: dir,
        root,
        home,
    }
}

fn compiled_with(project: &Project, os: Os, chat: &Grants) -> Compiled {
    // The fixture may sit in a harness's temp root on this machine: name none here.
    super::TEST_HARNESS_TEMP.with(|roots| roots.set(&[]));
    let plane = Plane::of(Some(ON));
    let policy = plane.said().policy.expect("on");
    Compiled::granted(
        &policy,
        &plane,
        &project.root,
        &Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(project.home.clone()),
            os,
        },
        None,
        chat,
    )
}

#[test]
fn a_chats_own_grants_reach_its_compiled_sandbox_and_no_other_start() {
    let project = project();
    let cache = project.root.join("workspaces/beta/out");
    let mut chat = Grants::default();
    chat.add(&What::Host(host("api.example.com").unwrap()));
    chat.add(&What::Write(cache.clone()));
    // One that a class covers is dropped as it is compiled, whoever granted it.
    chat.add(&What::Write(project.root.join(".purlis/app")));

    let granted = compiled_with(&project, Os::MacOs, &chat);
    assert!(granted.hosts.contains(&"api.example.com".to_owned()));
    assert_eq!(granted.writable, std::slice::from_ref(&cache));
    // Recorded with what the chat is held to, so a start compiled later can be told from this
    // one by the folders it may write (#1428).
    let applied = crate::sandbox::applied_of(
        crate::harness::Harness::ClaudeCode,
        crate::sandbox::compiler(crate::harness::Harness::ClaudeCode).expect("a compiler"),
        &granted,
        &project.root,
        &Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(project.home.clone()),
            os: Os::MacOs,
        },
    )
    .expect("compiles");
    assert_eq!(applied.confines().writable, std::slice::from_ref(&cache));

    // Another chat of the same project, started with no grant of its own: none of it.
    let other = compiled_with(&project, Os::MacOs, &Grants::default());
    assert!(!other.hosts.contains(&"api.example.com".to_owned()));
    assert!(other.writable.is_empty());
}

/// #1664: the proxy decides by the layer that lists a host. A preset's host is open, and a
/// host allowed for this chat alone is allowed at that scope, for this chat and no other.
#[test]
fn a_chats_compiled_reach_says_which_layer_lets_each_host_through() {
    use crate::sandbox::reach::{By, Decision};
    let project = project();
    let mut chat = Grants::default();
    chat.add(&What::Host(host("api.example.com").unwrap()));
    let granted = compiled_with(&project, Os::MacOs, &chat);
    let decide = |compiled: &Compiled, host: &str| compiled.reach.decide(host, 443, &[443], &[]);
    assert_eq!(decide(&granted, "api.anthropic.com"), Decision::Open);
    assert_eq!(
        decide(&granted, "api.example.com"),
        Decision::Allowed(By::Chat)
    );
    assert_eq!(decide(&granted, "unlisted.example.org"), Decision::Ask);
    assert_eq!(granted.reach.hosts().len(), granted.hosts.len());
    let other = compiled_with(&project, Os::MacOs, &Grants::default());
    assert_eq!(decide(&other, "api.example.com"), Decision::Ask);
}

#[test]
fn a_folder_you_granted_reaches_every_chat_here_until_it_is_revoked() {
    let project = project();
    let folder = project.home.join("tools/cache");
    // Off the allowlist until you list a folder that holds it (D-1342-10).
    local::grant_write(&project.root, &folder).expect("granted");
    let unlisted = compiled_with(&project, Os::MacOs, &Grants::default());
    assert!(unlisted.writable.is_empty(), "{:?}", unlisted.writable);
    std::fs::create_dir_all(project.home.join("tools")).expect("made");
    local::list_grantable(&project.root, &project.home.join("tools")).expect("listed");
    assert_eq!(
        local::granted_writes(&project.root),
        std::slice::from_ref(&folder)
    );
    let started = compiled_with(&project, Os::MacOs, &Grants::default());
    assert_eq!(started.writable, std::slice::from_ref(&folder));

    local::revoke_write(&project.root, &folder).expect("revoked");
    let after = compiled_with(&project, Os::MacOs, &Grants::default());
    assert!(after.writable.is_empty(), "{:?}", after.writable);
}

#[test]
fn claude_code_is_handed_a_granted_folder_under_its_denials_and_nothing_without_one() {
    let project = project();
    let none = claude::settings(&compiled_with(&project, Os::MacOs, &Grants::default()))
        .expect("compiles");
    assert!(
        none.sandbox["filesystem"].get("allowWrite").is_none(),
        "a chat with no grant is handed what it always was: {}",
        none.sandbox
    );

    let cache = project.root.join("workspaces/beta/out");
    let mut chat = Grants::default();
    chat.add(&What::Write(cache.clone()));
    let settings = claude::settings(&compiled_with(&project, Os::MacOs, &chat)).expect("compiles");
    assert_eq!(
        settings.sandbox["filesystem"]["allowWrite"],
        serde_json::json!([cache.display().to_string()])
    );
    // Its denials are all still there, and Claude Code holds a denial over an allow.
    assert!(
        settings.sandbox["filesystem"]["denyWrite"]
            .as_array()
            .is_some_and(|denied| !denied.is_empty())
    );
    assert_eq!(settings.sandbox["allowUnsandboxedCommands"], false);
    // Every later-code name is denied again under the granted folder, by absolute path: a
    // relative `**/<name>` is read against the chat's folder, not this one (review S2).
    let denied: Vec<&str> = settings.sandbox["filesystem"]["denyWrite"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|one| one.as_str())
        .collect();
    for name in [
        ".git/hooks",
        ".git/config",
        ".envrc",
        ".vscode",
        "charter.toml",
    ] {
        let glob = format!("{}/**/{name}", cache.display());
        assert!(
            denied.contains(&glob.as_str()),
            "{glob} is not denied: {denied:?}"
        );
        assert!(
            settings.deny.contains(&format!("Edit(/{glob})")),
            "the Edit tool is not held off {glob}"
        );
    }
}

#[test]
fn a_wrapped_chat_writes_a_granted_folder_with_every_denial_after_it() {
    let project = project();
    let cache = project.root.join("workspaces/beta/out");
    let mut chat = Grants::default();
    chat.add(&What::Write(cache.clone()));
    let compiled = compiled_with(&project, Os::MacOs, &chat);
    let cwd = project.root.join("workspaces/alpha");
    let tmp = project.home.join("tmp");
    let quoted = format!("(subpath \"{}\")", cache.display());

    let opencode = opencode::profile(
        &opencode::wrap(&compiled).expect("wraps"),
        &cwd,
        &tmp,
        &[4000],
        None,
    )
    .expect("a profile");
    let codex = codex::profile(
        &codex::wrap(&compiled).expect("wraps"),
        &cwd,
        &tmp,
        &[4000],
        None,
    )
    .expect("a profile");
    for profile in [opencode, codex] {
        let allowed = profile.find(&quoted).expect("the grant is written");
        let denial = profile
            .find("(deny file-write* file-link (subpath")
            .expect("the classes are denied");
        assert!(allowed < denial, "a denial comes after the grant");
        // The later-code names are denied at any depth of the granted folder too.
        assert!(
            profile.contains(&format!(
                "(deny file-write* file-link (regex \"^{}/(.*/)?\\\\.git/config(/.*)?$\"))",
                crate::sandbox::seatbelt::escaped(&cache.display().to_string())
                    .replace('\\', "\\\\")
            )),
            "{profile}"
        );
    }
}

// ---- a host allowed already (#1666's fold-in) ----------------------------------------------

#[test]
fn a_host_allowed_already_is_found_at_the_scope_that_holds_it() {
    let one = |typed: &str| super::super::hosts::Host::parse(typed).expect("a host");
    let none = super::super::policy::Locks::none();
    let yours = [one("pypi.example.com")];
    let project = [one("*.blob.core.windows.net")];
    assert_eq!(
        already(&one("pypi.example.com:443"), &yours, &project, &none),
        Some(Level::You)
    );
    assert_eq!(
        already(
            &one("results.blob.core.windows.net:443"),
            &yours,
            &project,
            &none
        ),
        Some(Level::Project)
    );
    assert_eq!(
        already(&one("elsewhere.example:443"), &yours, &project, &none),
        None
    );
    // One a policy locks out is not allowed: it reaches nothing while the policy stands.
    let locked = super::super::policy::Locks::parse(
        r#"{"sandbox": {"personal-hosts": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(
        already(&one("pypi.example.com:443"), &yours, &project, &locked),
        None
    );
}

/// qw384: a service on this Mac allowed for one chat reaches that chat's start alone, as one
/// port: its sandbox connects to it directly, and its proxy carries exactly that address and
/// port of this machine, never another.
#[test]
fn a_port_on_this_mac_allowed_for_a_chat_reaches_its_start_as_that_port_alone() {
    let project = project();
    let mut chat = Grants::default();
    chat.add(&What::Port(50795));
    chat.add(&What::Port(50795));
    assert_eq!(chat.ports, vec![50795]);
    assert!(!chat.is_empty());

    let granted = compiled_with(&project, Os::MacOs, &chat);
    assert_eq!(granted.widened.ports, vec![50795]);
    let own: [std::net::IpAddr; 0] = [];
    let loopback: std::net::IpAddr = "127.0.0.1".parse().unwrap();
    let v6: std::net::IpAddr = "::1".parse().unwrap();
    assert!(granted.reach.lists_exactly(loopback, 50795));
    assert!(granted.reach.lists_exactly(v6, 50795));
    assert!(
        granted
            .reach
            .decide("localhost", 50795, &[443], &own)
            .carries()
    );
    assert!(!granted.reach.lists_exactly(loopback, 50796));
    // Never a host a harness's own allowlist names.
    assert!(!granted.hosts.iter().any(|host| host.contains("50795")));

    let mut own_rules = crate::sandbox::seatbelt::Own::default();
    crate::sandbox::seatbelt::widen(&mut own_rules, &granted.widened).expect("widens");
    assert!(
        own_rules
            .after
            .contains(&"(allow network-outbound (remote ip \"localhost:50795\"))".to_owned())
    );

    let other = compiled_with(&project, Os::MacOs, &Grants::default());
    assert!(other.widened.ports.is_empty());
    assert!(!other.reach.lists_exactly(loopback, 50795));
}

/// #2019: a port a chat was allowed to serve on reaches its start alone: its sandbox makes and
/// binds its own socket for it in its folder, never a TCP port, and connects to the door on that
/// port, which becomes a named local port of its reach, carried for its browser by its literal
/// address; another port stays refused to the chat and its browser.
#[test]
fn a_port_a_chat_may_listen_on_is_bound_reached_and_named_for_its_browser_alone() {
    use crate::sandbox::browser_reach::{Lane, Refusal, Verdict};
    let project = project();
    let mut chat = Grants::default();
    chat.add(&What::Listen(3000));
    chat.add(&What::Listen(3000));
    assert_eq!(chat.listens, vec![3000]);
    assert!(!chat.is_empty());

    let granted = compiled_with(&project, Os::MacOs, &chat);
    assert_eq!(granted.widened.listens, vec![3000]);
    assert_eq!(granted.widened.ports, vec![3000]);
    let loopback: std::net::IpAddr = "127.0.0.1".parse().unwrap();
    assert!(granted.reach.lists_exactly(loopback, 3000));
    assert!(!granted.reach.lists_exactly(loopback, 3001));

    let mut own_rules = crate::sandbox::seatbelt::Own::default();
    crate::sandbox::seatbelt::widen(&mut own_rules, &granted.widened).expect("widens");
    let folder = std::path::Path::new("/data/serve/chat-1");
    crate::sandbox::seatbelt::served(&mut own_rules, folder, &granted.widened.listens)
        .expect("served");
    for rule in [
        "(allow network-bind (local unix-socket (path-literal \"/data/serve/chat-1/3000.sock\")))",
        "(allow network-inbound (local unix-socket (path-literal \"/data/serve/chat-1/3000.sock\")))",
        "(allow network-outbound (remote ip \"localhost:3000\"))",
    ] {
        assert!(own_rules.after.contains(&rule.to_owned()), "{rule}");
    }
    // No TCP port is ever bound: the door listens on the chat's port.
    assert!(
        !own_rules
            .after
            .iter()
            .any(|rule| rule.contains("network-bind") && !rule.contains("unix-socket"))
    );

    let own: [std::net::IpAddr; 1] = ["192.0.2.10".parse().unwrap()];
    let lane = Lane::of(granted.reach.clone());
    assert!(matches!(
        lane.decide("127.0.0.1", 3000, false, &own),
        Verdict::Carry { .. }
    ));
    assert!(matches!(
        lane.decide("::1", 3000, true, &own),
        Verdict::Carry { .. }
    ));
    assert_eq!(
        lane.decide("127.0.0.1", 3001, false, &own),
        Verdict::Refused(Refusal::ThisMachine)
    );

    // A chat allowed nothing listens on nothing, and its browser reaches no port of this Mac.
    let other = compiled_with(&project, Os::MacOs, &Grants::default());
    assert!(other.widened.listens.is_empty());
    assert_eq!(
        Lane::of(other.reach).decide("127.0.0.1", 3000, false, &own),
        Verdict::Refused(Refusal::ThisMachine)
    );
}

/// #2019: a port allowed to listen on for this project on this machine reaches every chat's
/// start there, and its Remove takes it off the next.
#[test]
fn a_port_to_listen_on_kept_for_this_project_reaches_every_chats_start() {
    let project = project();
    crate::sandbox::local::grant_listen(&project.root, 5173).expect("kept");
    assert_eq!(
        crate::sandbox::local::granted_listens(&project.root),
        vec![5173]
    );
    let granted = compiled_with(&project, Os::MacOs, &Grants::default());
    assert_eq!(granted.widened.listens, vec![5173]);
    crate::sandbox::local::revoke_listen(&project.root, 5173).expect("removed");
    let after = compiled_with(&project, Os::MacOs, &Grants::default());
    assert!(after.widened.listens.is_empty());
}

#[test]
fn a_listen_grant_is_told_and_audited_by_its_number() {
    let what = What::Listen(3000);
    assert_eq!(what.word(), "listen");
    assert_eq!(what.target(), "3000");
    assert!(what.machine_only());
    assert!(!what.machine_wide());
    assert_eq!(what.to_string(), "running a server on port 3000");
}

#[test]
fn a_port_grant_is_told_and_audited_by_its_number() {
    let what = What::Port(50795);
    assert_eq!(what.word(), "port");
    assert_eq!(what.target(), "50795");
    assert!(what.machine_only());
    assert!(!what.machine_wide());
    assert_eq!(
        what.to_string(),
        "connecting to the service on this Mac at port 50795"
    );
    assert_eq!(
        told(&what, Level::Chat),
        "⬢ purlis: connecting to the service on this Mac at port `50795` — Allowed for this chat \
         — retry."
    );
}

/// #2629: "Allow once" and "Allow for this chat" are the one-chat levels; every other level
/// reaches more than one chat.
#[test]
fn once_and_this_chat_are_the_one_chat_levels() {
    let one_chat: Vec<Level> = [
        Level::Once,
        Level::Chat,
        Level::You,
        Level::Machine,
        Level::Project,
    ]
    .into_iter()
    .filter(|level| level.one_chat())
    .collect();
    assert_eq!(one_chat, [Level::Once, Level::Chat]);
}

/// #2019: no door ever opens for a Claude Code chat, so a grant to serve on a port gives it
/// nothing: no port in its sandbox's settings, and none its proxy carries.
#[test]
fn a_claude_code_chat_is_given_nothing_for_a_grant_to_serve_on_a_port() {
    let project = project();
    let port = 51_999;
    let mut chat = Grants::default();
    chat.add(&What::Listen(port));
    let granted = compiled_with(&project, Os::MacOs, &chat);
    assert_eq!(granted.widened.listen_only, [port]);
    let applied = crate::sandbox::applied_of(
        crate::harness::Harness::ClaudeCode,
        crate::sandbox::compiler(crate::harness::Harness::ClaudeCode).expect("a compiler"),
        &granted,
        &project.root,
        &Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(project.home.clone()),
            os: Os::MacOs,
        },
    )
    .expect("compiles");
    let confines = applied.confines();
    assert!(!confines.widened.ports.contains(&port), "{confines:?}");
    assert!(confines.widened.listens.is_empty());
    assert!(confines.reach.withdrew(port));
}

// ---- the app's state in the data home (#3718) ------------------------------------------------

#[test]
fn the_app_state_and_the_data_home_that_holds_it_are_never_a_grant() {
    let mut m = Disk::new();
    let data = m.at.join("home/.local/share/purlis");
    std::fs::create_dir_all(data.join("app-state/0123456789abcdef")).expect("made");
    m.denied.extend(crate::sandbox::app_state_held(&data));
    let here = m.place();
    for typed in [
        "home/.local/share/purlis/app-state",
        "home/.local/share/purlis/app-state/0123456789abcdef",
        "home/.local/share/purlis/app-state/0123456789abcdef/skill-tries",
        "home/.local/share/purlis",
    ] {
        match write(&m.path(typed), &here) {
            Err(Refused::Never(why)) => {
                assert!(why.contains("purlis keeps its own state"), "{typed}: {why}");
            }
            other => panic!("{typed}: {other:?}"),
        }
    }
    // A folder of the data home beside it is judged as any other.
    assert!(!matches!(
        write(&m.path("home/.local/share/purlis/other"), &here),
        Err(Refused::Never(_))
    ));
}

#[test]
fn a_grant_above_the_data_home_leaves_the_app_state_denied_to_writes() {
    let project = project();
    // A data home below a folder a chat may be granted: the grant holds it, and the app state.
    let above = project.root.join("workspaces/beta/out");
    let data = above.join("data");
    let named_data = data.display().to_string();
    let machine = Machine {
        env: crate::secrets::Env::of(&[("PURLIS_DATA_HOME", named_data.as_str())]),
        home: Some(project.home.clone()),
        os: Os::MacOs,
    };
    assert_eq!(
        crate::sandbox::Homes::charter_data(&machine),
        Some(data.clone())
    );
    let app_state = data.join(crate::appstate::DIR);
    let mut chat = Grants::default();
    chat.add(&What::Write(above.clone()));
    super::TEST_HARNESS_TEMP.with(|roots| roots.set(&[]));
    let plane = Plane::of(Some(ON));
    let policy = plane.said().policy.expect("on");
    let compiled = Compiled::granted(&policy, &plane, &project.root, &machine, None, &chat);

    // Claude Code holds a denial over an allow.
    let settings = claude::settings(&compiled).expect("compiles");
    assert_eq!(
        settings.sandbox["filesystem"]["allowWrite"],
        serde_json::json!([above.display().to_string()])
    );
    let named = crate::sandbox::real(&app_state).display().to_string();
    assert!(
        settings.sandbox["filesystem"]["denyWrite"]
            .as_array()
            .is_some_and(|all| all.iter().any(|one| one == named.as_str())),
        "{named}: {}",
        settings.sandbox["filesystem"]["denyWrite"]
    );

    // A wrapped chat: every denial is written after the grant, and Seatbelt takes the last rule.
    let cwd = project.root.join("workspaces/alpha");
    let tmp = project.home.join("tmp");
    let quoted = format!("(subpath \"{}\")", above.display());
    let opencode = opencode::profile(
        &opencode::wrap(&compiled).expect("wraps"),
        &cwd,
        &tmp,
        &[4000],
        None,
    )
    .expect("a profile");
    let codex = codex::profile(
        &codex::wrap(&compiled).expect("wraps"),
        &cwd,
        &tmp,
        &[4000],
        None,
    )
    .expect("a profile");
    for profile in [opencode, codex] {
        let allowed = profile.find(&quoted).expect("the grant is written");
        for denial in crate::sandbox::app_state_held(&data) {
            for rule in crate::sandbox::seatbelt::denial_rules(&denial).expect("rules") {
                let at = profile
                    .find(&rule)
                    .unwrap_or_else(|| panic!("{rule} missing"));
                assert!(allowed < at, "{rule} comes before the grant");
            }
        }
    }
}

/// Where a held folder is judged: a project at the floor, or not, in `m`'s home.
fn ground_of(m: &Disk, floor: bool) -> Ground {
    Ground {
        root: m.at.join("plane"),
        chat: m.at.join("plane"),
        home: Some(m.at.join("home")),
        denied: Vec::new(),
        allowed: Vec::new(),
        refused: Vec::new(),
        harness_temp: &[],
        folded: false,
        floor,
    }
}

#[test]
fn a_held_folder_is_lifted_only_by_its_whole_name_below_the_home_at_the_floor() {
    let m = Disk::new();
    let ground = ground_of(&m, true);
    let gh = m.path("home/.config/gh");
    assert_eq!(lift(&gh, &ground), Ok(m.at.join("home/.config/gh")));
    // Below it, another copy of the name, or a name not on the list: never.
    for typed in [
        m.path("home/.config/gh/hosts.yml"),
        m.path("plane/.config/gh"),
        m.path("home/.config/git"),
        m.path("home/.zshrc"),
        m.path("home/Library/LaunchAgents"),
    ] {
        assert!(
            matches!(lift(&typed, &ground), Err(Refused::Never(_))),
            "{typed}"
        );
    }
    // The full sandbox keeps its refusal.
    assert!(matches!(
        lift(&gh, &ground_of(&m, false)),
        Err(Refused::Never(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_held_folder_reached_through_a_link_is_not_lifted() {
    let m = Disk::new();
    m.link("home/.kube", "tmp");
    assert!(matches!(
        lift(&m.path("home/.kube/kuberc"), &ground_of(&m, true)),
        Err(Refused::Not(_))
    ));
}

#[test]
fn only_what_this_chat_holds_for_itself_stays_lifted() {
    let m = Disk::new();
    let ground = ground_of(&m, true);
    let gh = m.at.join("home/.config/gh");
    let locks = crate::sandbox::policy::Locks::default();
    let mut chat = Grants::default();
    chat.add(&What::Write(gh.clone()));
    chat.add(&What::Write(m.at.join("plane/out")));
    assert_eq!(still_lifted(&chat, &locks, &ground), vec![gh.clone()]);
    // Not kept for a run alone, and not past the floor.
    let mut once = Grants::default();
    once.add_once(&What::Write(gh));
    assert!(still_lifted(&once, &locks, &ground).is_empty());
    assert!(still_lifted(&chat, &locks, &ground_of(&m, false)).is_empty());
}
