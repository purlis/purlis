// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A skill can only narrow a chat, manual-only is honoured on every harness, each skill carries a
//! compatibility badge per harness, and a skill that cannot be loaded is said (SK-8, #2771;
//! decisions 18, 41 and 58). Read through what each harness is started with: its arguments, its
//! environment, and the folder and briefing it is handed.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::skills::{Field, Fit, LISTED_ENV, PlaneSkills};

struct Here {
    _tmp: tempfile::TempDir,
    plugin: PathBuf,
    plane: PathBuf,
    sets: PathBuf,
}

fn skill(dir: &Path, name: &str, more: &str) {
    fs::create_dir_all(dir.join(name)).unwrap();
    fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name} for the team.\n{more}---\n\nbody\n"),
    )
    .unwrap();
}

/// What would widen a chat, and what narrows it.
const WIDENS: &str = "allowed-tools: Bash(rm:*)\nhooks:\n  Stop:\n    - hooks:\n        - type: command\n          command: ./x.sh\ndisallowed-tools: Write\n";

/// A bundle with one built-in skill and the opencode shim; a plane with a skill that asks for
/// what would widen a chat (and has a script), a manual-only one and a plain one.
fn here() -> Here {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let plugin = root.join("plugin");
    skill(&plugin.join("skills"), "secrets", "");
    let shim = purlis_core::opencode::shim_in(&plugin);
    fs::create_dir_all(shim.parent().unwrap()).unwrap();
    fs::write(&shim, "// the shim, a stand-in\n").unwrap();
    let plane = root.join("plane");
    let skills = plane.join("skills");
    skill(&skills, "deploy", WIDENS);
    fs::create_dir_all(skills.join("deploy/scripts")).unwrap();
    fs::write(skills.join("deploy/scripts/run.sh"), "#!/bin/sh\necho hi\n").unwrap();
    skill(&skills, "release", "disable-model-invocation: true\n");
    skill(&skills, "review", "");
    Here {
        _tmp: tmp,
        plugin,
        plane,
        sets: root.join("data/skill-sets"),
    }
}

/// What the host lays out for a chat at the plane root on `harness`, and what it is armed with.
fn host_armed(harness: Harness, here: &Here) -> (Vec<String>, Vec<(String, String)>) {
    let host = purlis_core::hoststart::Kit {
        binary: Some(PathBuf::from("/bin/purlis")),
        plugin: Some(here.plugin.clone()),
        git_hooks: None,
        skill_sets: Some(here.sets.clone()),
    };
    let linked = purlis_core::hoststart::plane_skills_laid_out(
        true,
        &here.plane,
        None,
        None,
        &host,
        harness,
    );
    let kit = Kit {
        binary: Path::new("/bin/purlis"),
        plugin: Some(&here.plugin),
        persona: None,
        skills: Some(PlaneSkills {
            plane: &here.plane,
            workspace: None,
            linked: linked.as_deref(),
            repo: &[],
        }),
    };
    match harness.state_hooks(
        kit,
        Some(&here.plane),
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) {
        StateHooks::ThisSessionOnly { args, env, .. } => (args, env),
        StateHooks::None => panic!("{harness:?} is armed"),
    }
}

fn var<'e>(env: &'e [(String, String)], name: &str) -> Option<&'e str> {
    env.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// The folder of the plane's skills a Claude Code chat is handed: its second `--plugin-dir`.
fn claude_folder(args: &[String]) -> PathBuf {
    let dirs: Vec<&String> = args
        .windows(2)
        .filter(|pair| pair[0] == "--plugin-dir")
        .map(|pair| &pair[1])
        .collect();
    assert_eq!(dirs.len(), 2, "{args:?}");
    PathBuf::from(dirs[1])
}

/// The folder of the plane's skills an opencode chat is handed: the second of its config's
/// skill directories.
fn opencode_folder(env: &[(String, String)]) -> PathBuf {
    let config: serde_json::Value =
        serde_json::from_str(var(env, purlis_core::opencode::CONFIG_ENV).unwrap()).unwrap();
    let dirs = config["plugin"][0][1]["skills"]
        .as_array()
        .expect("a list of directories");
    assert_eq!(dirs.len(), 2, "{dirs:?}");
    PathBuf::from(dirs[1].as_str().unwrap())
}

#[test]
fn a_claude_code_chat_is_never_handed_what_would_widen_it_and_keeps_what_narrows_it() {
    purlis_core::unsteered!();
    let here = here();
    let (args, _) = host_armed(Harness::ClaudeCode, &here);
    let skills = claude_folder(&args).join("skills");

    let deploy = skills.join("deploy");
    assert!(
        fs::symlink_metadata(&deploy).unwrap().is_dir(),
        "a folder of its own"
    );
    let handed = fs::read_to_string(deploy.join("SKILL.md")).unwrap();
    assert_eq!(
        handed,
        "---\nname: deploy\ndescription: Does deploy for the team.\ndisallowed-tools: Write\n---\n\nbody\n"
    );
    assert_eq!(
        fs::read_link(deploy.join("scripts")).unwrap(),
        here.plane.join("skills/deploy/scripts"),
        "its other files are the plane's own, linked"
    );
    // Claude Code reads manual-only itself, and a skill that asks for nothing to leave out is
    // linked as it lies.
    for plain in ["release", "review"] {
        assert_eq!(
            fs::read_link(skills.join(plain)).unwrap(),
            here.plane.join("skills").join(plain)
        );
    }
}

#[test]
fn an_opencode_chat_is_handed_a_manual_only_skill_without_its_description_and_nothing_that_widens()
{
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Opencode, &here);
    let skills = opencode_folder(&env);

    assert_eq!(
        fs::read_to_string(skills.join("release/SKILL.md")).unwrap(),
        "---\nname: release\ndisable-model-invocation: true\n---\n\nbody\n",
        "opencode lists only a skill with a description, and still runs one by name"
    );
    let deploy = fs::read_to_string(skills.join("deploy/SKILL.md")).unwrap();
    assert!(!deploy.contains("allowed-tools"), "{deploy}");
    assert!(!deploy.contains("hooks"), "{deploy}");
    assert_eq!(
        fs::read_link(skills.join("review")).unwrap(),
        here.plane.join("skills/review")
    );
}

#[test]
fn a_codex_chat_is_briefed_on_a_manual_only_skill_by_its_name_and_file_alone() {
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Codex, &here);
    assert!(var(&env, LISTED_ENV).is_some());

    let lookup = |name: &str| var(&env, name).map(str::to_owned);
    let text = purlis_core::skills::briefed(&here.plane, None, None, &lookup).expect("listed");
    assert!(text.contains("- `review` — Does review for the team."), "{text}");
    assert!(!text.contains("Does release"), "{text}");
    let release = here.plane.join("skills/release/SKILL.md");
    assert!(
        text.contains(&format!(
            "Only when the person names one, never by your own choice: `release` (`{}`)",
            release.display()
        )),
        "{text}"
    );
}

#[test]
fn a_set_laid_out_again_after_its_skill_changed_is_a_new_folder() {
    purlis_core::unsteered!();
    let here = here();
    let (args, _) = host_armed(Harness::ClaudeCode, &here);
    let first = claude_folder(&args);
    fs::write(
        here.plane.join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploys, changed.\nallowed-tools: Bash\n---\n",
    )
    .unwrap();
    let (args, _) = host_armed(Harness::ClaudeCode, &here);
    let again = claude_folder(&args);
    assert_ne!(first, again, "a running chat's folder never changes");
    assert_eq!(
        fs::read_to_string(again.join("skills/deploy/SKILL.md")).unwrap(),
        "---\nname: deploy\ndescription: Deploys, changed.\n---\n"
    );
}

#[test]
fn each_harness_badge_says_what_applies_what_is_left_out_and_what_is_carried_over() {
    let text = format!(
        "---\nname: deploy\ndescription: Deploys it.\nmodel: opus\ndisable-model-invocation: true\n{WIDENS}---\n"
    );
    let on = |harness: Harness| {
        purlis_core::skills::compat(&text, true, &harness.skill_frontmatter())
    };

    let claude = on(Harness::ClaudeCode);
    assert_eq!(claude.fit, Fit::Partial);
    assert_eq!(claude.dropped, [Field::AllowedTools, Field::Hooks]);
    assert_eq!(claude.ignored, Vec::<Field>::new());
    assert_eq!(claude.translated, Vec::<Field>::new());

    for harness in [Harness::Codex, Harness::Opencode] {
        let badge = on(harness);
        assert_eq!(badge.fit, Fit::Partial, "{harness:?}");
        assert_eq!(badge.dropped, [Field::AllowedTools, Field::Hooks]);
        assert_eq!(badge.translated, [Field::ManualOnly], "{harness:?}");
        assert_eq!(badge.ignored, [Field::Model, Field::DisallowedTools]);
        assert!(
            badge
                .lines()
                .contains(&"Narrowing not applied: the tools it takes away.".to_owned()),
            "{:?}",
            badge.lines()
        );
    }

    // A harness or repo skill purlis does not hand: it widens a Claude Code chat.
    let outside = purlis_core::skills::compat(
        &text,
        false,
        &Harness::ClaudeCode.skill_frontmatter(),
    );
    assert_eq!(outside.fit, Fit::Widens);
}

#[test]
fn a_manual_only_skill_costs_nothing_on_every_harness() {
    for harness in Harness::ALL {
        assert!(harness.skill_budget().manual_free, "{harness:?}");
    }
}

#[test]
fn a_skill_a_chat_would_have_loaded_that_cannot_be_is_said_with_its_level_and_why() {
    purlis_core::unsteered!();
    let here = here();
    fs::create_dir_all(here.plane.join("skills/half")).unwrap();
    fs::write(
        here.plane.join("skills/half/SKILL.md"),
        "---\nname: half\n---\n",
    )
    .unwrap();
    let env = purlis_core::harness_plugin::Env {
        chat: &[],
        home: None,
        process: false,
    };
    let said = purlis_core::skills::not_here(
        &here.plane,
        None,
        None,
        Some(&here.plugin.join("skills")),
        Harness::ClaudeCode,
        &env,
    )
    .expect("said");
    assert_eq!(
        said.broken,
        [purlis_core::skills::Broken {
            name: "half".to_owned(),
            level: "project".to_owned(),
            why: "not a skill: its SKILL.md names no `description`".to_owned(),
        }]
    );
    assert_eq!(said.missing, Vec::<String>::new());

    // A project skill granted to no workspace is no loss to a chat in one.
    fs::create_dir_all(here.plane.join("workspaces/ops")).unwrap();
    let in_ops = purlis_core::skills::not_here(
        &here.plane,
        Some("ops"),
        None,
        Some(&here.plugin.join("skills")),
        Harness::ClaudeCode,
        &env,
    );
    assert_eq!(in_ops, None);
}

#[test]
fn the_live_lint_checks_a_skill_against_the_project_and_this_machine() {
    purlis_core::unsteered!();
    let here = here();
    let level = purlis_core::skills::verbs::Level::project();
    let text = "---\nname: review\ndescription: Reviews a change for the team.\nowner: ops\n---\n\nThen use skills:deploy and skills:deplyo, and run !`purlis-no-such-tool-sk8 --x`.\n";
    let said: Vec<String> = purlis_core::skills::lint(&here.plane, &level, "review", text)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        said,
        [
            "its frontmatter's owner: is read by nothing that runs a skill",
            "it names skills:deplyo, and this project has no skill deplyo: a chat told to use \
             it finds none",
            "its ! command calls purlis-no-such-tool-sk8, which this machine does not have",
        ]
    );
}
