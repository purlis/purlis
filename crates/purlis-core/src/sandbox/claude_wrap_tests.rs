// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Claude Code chat on a runner, wrapped whole by purlis (#4083): what starts it, what its
//! profile holds, read by the wrap's own reader, and that a person's own Linux keeps its own.

use std::path::{Path, PathBuf};

use purlis_linux_sandbox::profile::{FileOp, Kind, Op, Profile, Remote};
use purlis_session_protocol::hostlink::{LinkKey, Pinned};

use super::backend::{LANDLOCK, Missing};
use super::claude_wrap::{self, Wrap};
use super::signin::ProfileFolders;
use super::{Applied, At, Form, Level, Machine, NotStarted, Os, Words, for_start};
use crate::harness::Harness;

const FULL: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n";
const FLOOR: &str = "schema = 1\n";

struct Place {
    _home: tempfile::TempDir,
    home: PathBuf,
    machine: Machine,
    plane: tempfile::TempDir,
}

/// A project saying `manifest` on a Linux machine, paired as a runner where `paired` says so.
fn place(manifest: &str, paired: bool) -> Place {
    let held = tempfile::tempdir().expect("a home");
    let home = super::real(held.path());
    let config = home.join(".config");
    std::fs::create_dir_all(&config).expect("a config home");
    if paired {
        let desktop = Pinned {
            device: "01JA0DESKT0P00000000000000".into(),
            key: LinkKey::generate().expect("a key").public(),
        };
        crate::runnerlink::pair(&config, &desktop).expect("paired");
    }
    let plane = tempfile::tempdir().expect("a project");
    std::fs::write(plane.path().join("purlis.toml"), manifest).expect("the manifest");
    let machine = Machine {
        env: crate::secrets::Env::of(&[(
            crate::machine::HOME_VAR,
            config.to_str().expect("a path"),
        )]),
        home: Some(home.clone()),
        os: Os::Linux,
    };
    Place {
        _home: held,
        home,
        machine,
        plane,
    }
}

fn only_landlock(program: &str) -> bool {
    program == LANDLOCK
}

/// What a chat of Claude Code starts under at `place`, on its profile's folder `config`.
fn started(place: &Place, config: &Path) -> Applied {
    let mut applied = for_start(
        Harness::ClaudeCode,
        place.plane.path(),
        &place.machine,
        &only_landlock,
    )
    .expect("it starts")
    .expect("a sandbox in force");
    applied.on_profile(&ProfileFolders {
        config: Some(config.to_path_buf()),
        ..ProfileFolders::default()
    });
    applied
}

fn wrap_of(applied: &Applied) -> Wrap {
    let Form::ClaudeWrapped(wrap) = applied.form() else {
        panic!("not wrapped: {:?}", applied.form());
    };
    wrap.clone()
}

/// The profile of a chat in `cwd`, read as the wrap reads it, reporting on `socket`.
fn profile_of(wrap: &Wrap, cwd: &Path, tmp: &Path, socket: &Path) -> Profile {
    let written =
        claude_wrap::profile(wrap, cwd, tmp, &[4040, 4041], Some(socket)).expect("written");
    Profile::parse(&written).expect("it reads")
}

fn writes(profile: &Profile, path: &Path) -> bool {
    profile.allows(&Op::File {
        what: FileOp::WriteData,
        path,
        kind: Kind::Regular,
    })
}

fn reads(profile: &Profile, path: &Path) -> bool {
    profile.allows(&Op::File {
        what: FileOp::ReadData,
        path,
        kind: Kind::Regular,
    })
}

#[test]
fn a_claude_code_chat_on_a_runner_needs_the_kernel_and_no_program() {
    for (manifest, level) in [(FULL, Level::Full), (FLOOR, Level::Floor)] {
        let place = place(manifest, true);
        let applied = started(&place, &place.home.join("work"));
        assert!(matches!(applied.form(), Form::ClaudeWrapped(_)));
        assert_eq!(applied.level(), level);
        assert_eq!(applied.backend_word(), Some("landlock"));
        assert!(applied.through_purlis_proxy());
    }
}

#[test]
fn a_runner_whose_kernel_cannot_apply_the_wrap_refuses_the_chat_in_words() {
    let place = place(FULL, true);
    let refused = for_start(
        Harness::ClaudeCode,
        place.plane.path(),
        &place.machine,
        &|_| false,
    );
    assert!(matches!(
        refused,
        Err(NotStarted::NoBackend(Missing::Landlock))
    ));
}

#[test]
fn on_a_person_s_own_linux_a_claude_code_chat_keeps_its_own_sandbox() {
    let place = place(FULL, false);
    let refused = for_start(
        Harness::ClaudeCode,
        place.plane.path(),
        &place.machine,
        &only_landlock,
    );
    assert!(
        matches!(
            &refused,
            Err(NotStarted::NoBackend(Missing::Programs { absent, .. }))
                if absent == &["bwrap", "socat"]
        ),
        "{refused:?}"
    );
    let applied = for_start(
        Harness::ClaudeCode,
        place.plane.path(),
        &place.machine,
        &|_| true,
    )
    .expect("it starts")
    .expect("a sandbox in force");
    assert!(matches!(applied.form(), Form::ClaudeCode(_)));
    assert!(!applied.through_purlis_proxy(), "it waits for its answer");
}

#[test]
fn a_pairing_that_cannot_be_read_is_no_runner() {
    let place = place(FULL, true);
    std::fs::write(
        crate::machine::dir(&place.home.join(".config")).join(crate::runnerlink::PEERS_FILE),
        "not what purlis writes",
    )
    .expect("written over");
    assert!(!place.machine.is_a_runner());
}

#[test]
fn only_linux_has_runners() {
    let place = place(FULL, true);
    let machine = Machine {
        os: Os::MacOs,
        ..place.machine.clone()
    };
    assert!(!machine.is_a_runner());
    assert!(!machine.purlis_wraps(Harness::ClaudeCode));
}

#[test]
fn claude_code_s_own_sandbox_is_off_and_its_tools_keep_their_rules() {
    let place = place(FULL, true);
    let home = place.home.join("work");
    let applied = started(&place, &home);
    let settings = wrap_of(&applied).settings();
    assert_eq!(settings.sandbox, serde_json::json!({ "enabled": false }));
    assert!(settings.is_off());
    assert!(settings.made_empty.is_empty());
    assert!(settings.deny.iter().any(|rule| rule == "WebFetch"));
    // Its sign-in is read by the harness, and the tools it offers a model are still denied it.
    let sign_in = format!("Read(/{}/.credentials.json)", home.display());
    assert!(settings.deny.contains(&sign_in), "{:?}", settings.deny);
}

#[test]
fn an_older_claude_code_goes_through_the_proxy_all_the_same() {
    let place = place(FULL, true);
    let mut applied = started(&place, &place.home.join("work"));
    applied.answered("2.1.100 (Claude Code)");
    assert!(applied.through_purlis_proxy());
    assert!(applied.older_notice().is_none());
}

#[test]
fn what_waits_for_a_chat_is_not_denied_since_its_hooks_run_inside() {
    let place = place(FULL, true);
    let wrap = wrap_of(&started(&place, &place.home.join("work")));
    assert!(
        wrap.denied
            .iter()
            .filter(|denial| denial.path.ends_with(crate::handback::DIR_NAME))
            .all(|denial| denial.access != super::Access::ReadWrite),
        "{:?}",
        wrap.denied
    );
}

#[test]
fn what_only_its_own_sandbox_needed_is_left_out() {
    let place = place(FLOOR, true);
    let repo = place.plane.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("a repository");
    let pin = super::real(&repo).join(".git/config");
    let wrap = wrap_of(&started(&place, &place.home.join("work")));
    let rule = format!("Edit(/{})", pin.display());
    assert!(!wrap.rules.deny.contains(&rule), "{:?}", wrap.rules.deny);
    assert!(wrap.settings().made_empty.is_empty());
}

#[test]
fn the_whole_line_runs_in_the_wrap_with_its_traffic_and_temp_folder_its_own() {
    let place = place(FULL, true);
    let home = place.home.join("work");
    let applied = started(&place, &home);
    let cwd = place.plane.path().join("w");
    std::fs::create_dir(&cwd).expect("a folder");
    let confinement = applied.confine().expect("confined").expect("a proxy");
    let tmp = confinement.tmp().expect("a temp folder").to_path_buf();
    let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
    let line = applied
        .line(
            Words {
                program: "/opt/claude".to_owned(),
                command: words("--model m"),
                armed: Vec::new(),
                charters: words("--resume s"),
            },
            &At {
                cwd: Some(&cwd),
                confinement: Some(&confinement),
                purlis: Some(Path::new("/opt/purlis")),
                ..At::default()
            },
        )
        .expect("it starts");
    assert_eq!(line.program, "/opt/purlis");
    assert_eq!(line.args[0], purlis_linux_sandbox::COMMAND);
    let at = line
        .args
        .iter()
        .position(|word| word == "/opt/claude")
        .expect("the harness on the line");
    assert_eq!(line.args[at + 1..], ["--model", "m", "--resume", "s"]);
    let has = |key: &str, value: &str| {
        line.env
            .iter()
            .any(|(name, set)| name == key && (value.is_empty() || set == value))
    };
    assert!(has("HTTPS_PROXY", &confinement.proxy_url()));
    assert!(has("TMPDIR", tmp.to_str().expect("a path")));
    assert!(has(claude_wrap::TMP_ENV, tmp.to_str().expect("a path")));
    assert!(has(claude_wrap::QUIET_ENV, "1"));
    assert!(has(crate::hookwire::WRAPPED_ENV, "1"));
    assert!(applied.ssh_route(Some(&confinement)).is_some());
    assert!(applied.claude_ssh_route(Some(&confinement)).is_none());
}

#[test]
fn a_chat_with_no_config_folder_purlis_can_hold_is_not_started() {
    let place = place(FULL, true);
    let real = place.home.join("elsewhere");
    std::fs::create_dir(&real).expect("a folder");
    let link = place.home.join("linked");
    std::os::unix::fs::symlink(&real, &link).expect("a link");
    let applied = started(&place, &link);
    let cwd = place.plane.path();
    let confinement = applied.confine().expect("confined").expect("a proxy");
    let refused = applied
        .line(
            Words {
                program: "/opt/claude".to_owned(),
                command: Vec::new(),
                armed: Vec::new(),
                charters: Vec::new(),
            },
            &At {
                cwd: Some(cwd),
                confinement: Some(&confinement),
                purlis: Some(Path::new("/opt/purlis")),
                ..At::default()
            },
        )
        .expect_err("refused");
    assert!(refused.contains("config folder"), "{refused}");
}

#[test]
fn its_config_folder_is_written_only_where_a_turn_writes_at_either_level() {
    for manifest in [FULL, FLOOR] {
        let place = place(manifest, true);
        let home = place.home.join("work");
        let applied = started(&place, &home);
        let cwd = super::real(place.plane.path());
        let tmp = place.home.join("tmp");
        let socket = place.home.join("run/hooks.sock");
        let profile = profile_of(&wrap_of(&applied), &cwd, &tmp, &socket);
        for written in [
            "projects/-w/one.jsonl",
            "projects/-w/memory/MEMORY.md",
            "shell-snapshots/one.sh",
            "todos/one.json",
            "history.jsonl",
            ".credentials.json",
        ] {
            assert!(writes(&profile, &home.join(written)), "{written}");
        }
        for held in [
            ".claude.json",
            "settings.json",
            "settings.local.json",
            "CLAUDE.md",
            "hooks/a.sh",
            "skills/a/SKILL.md",
            "commands/a.md",
            "agents/a.md",
            "plugins/installed_plugins.json",
            "projects",
            "something-new",
        ] {
            assert!(!writes(&profile, &home.join(held)), "{held}");
        }
        assert!(reads(&profile, &home.join(".credentials.json")));
        assert!(reads(&profile, &home.join("settings.json")));
        // Neither the folder nor one above it moves, and no link is made where it writes.
        for path in [home.as_path(), place.home.as_path()] {
            assert!(
                !profile.allows(&Op::File {
                    what: FileOp::Unlink,
                    path,
                    kind: Kind::Directory,
                }),
                "{path:?}"
            );
        }
        assert!(!profile.allows(&Op::File {
            what: FileOp::Create,
            path: &home.join("projects/-w/link"),
            kind: Kind::Symlink,
        }));
        // Its hook socket is reached, and its folder never written.
        assert!(profile.allows(&Op::Connect(Remote::Unix(Some(&socket)))));
        assert!(!writes(&profile, &socket));
    }
}

#[test]
fn another_profile_s_folder_is_neither_read_nor_written() {
    for manifest in [FULL, FLOOR] {
        let place = place(manifest, true);
        let profiles = super::Homes::of(&place.machine).profiles.expect("profiles");
        let home = profiles.join("work/config");
        let other = profiles.join("private/config/.credentials.json");
        let applied = started(&place, &home);
        let wrap = wrap_of(&applied);
        assert_eq!(wrap.home.as_deref(), Some(home.as_path()));
        let profile = profile_of(
            &wrap,
            &super::real(place.plane.path()),
            &place.home.join("tmp"),
            &place.home.join("run/hooks.sock"),
        );
        assert!(!reads(&profile, &other));
        assert!(!writes(&profile, &other));
        assert!(reads(&profile, &home.join(".credentials.json")));
    }
}

#[test]
fn a_chat_whose_folder_holds_its_config_folder_is_not_wrapped() {
    let place = place(FULL, true);
    let cwd = super::real(place.plane.path());
    let home = cwd.join(".claude-profile");
    let wrap = wrap_of(&started(&place, &home));
    let refused = claude_wrap::profile(&wrap, &cwd, &place.home.join("tmp"), &[4040], None);
    assert_eq!(refused, Err(claude_wrap::OVERLAP));
}

#[test]
fn a_chat_is_readied_by_purlis_before_the_wrap_and_keeps_what_the_file_held() {
    let place = place(FULL, true);
    let home = place.home.join("work");
    std::fs::create_dir_all(home.join("projects/-w")).expect("a folder");
    std::os::unix::fs::symlink("/etc", home.join("projects/-w/planted")).expect("a link");
    std::fs::write(
        home.join(claude_wrap::STATE_FILE),
        r#"{"oauthAccount":{"emailAddress":"a"},"projects":{"/x":{"allowedTools":[]}}}"#,
    )
    .expect("its state");
    let cwd = place.plane.path();
    claude_wrap::prepare(&wrap_of(&started(&place, &home)), cwd);
    assert!(home.join("todos").is_dir());
    assert!(!home.join("projects/-w/planted").is_symlink());
    let state: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(claude_wrap::STATE_FILE)).expect("it reads"),
    )
    .expect("JSON");
    assert_eq!(state["oauthAccount"]["emailAddress"], "a");
    assert_eq!(state["hasCompletedOnboarding"], true);
    assert_eq!(
        state["projects"]["/x"]["allowedTools"],
        serde_json::json!([])
    );
    let folder = super::real(cwd).display().to_string();
    assert_eq!(state["projects"][&folder]["hasTrustDialogAccepted"], true);
}

#[test]
fn a_state_file_purlis_cannot_read_is_left_as_it_is() {
    let place = place(FULL, true);
    let home = place.home.join("work");
    std::fs::create_dir_all(&home).expect("a folder");
    std::fs::write(home.join(claude_wrap::STATE_FILE), "not JSON").expect("its state");
    claude_wrap::prepare(&wrap_of(&started(&place, &home)), place.plane.path());
    assert_eq!(
        std::fs::read_to_string(home.join(claude_wrap::STATE_FILE)).expect("it reads"),
        "not JSON"
    );
}

#[test]
fn a_wrapped_chat_is_told_as_one() {
    let wrapped = |name: &str| (name == crate::hookwire::WRAPPED_ENV).then(|| "1".to_owned());
    assert!(super::wrapped_whole_in(&wrapped));
    assert!(!super::wrapped_whole_in(&|_| None));
    let harness_hook = |name: &str| match name {
        crate::hookwire::SANDBOXED_ENV | crate::hookwire::WRAPPED_ENV => Some("1".to_owned()),
        crate::hookwire::HARNESS_ENV => Some("claude-code".to_owned()),
        _ => None,
    };
    assert!(super::writes_are_sandboxed_in(
        &harness_hook,
        super::Started::ByTheHarness
    ));
}
