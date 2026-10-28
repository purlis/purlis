// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A persona's chat, dispatched into a workspace the project granted nothing, gets the persona's
//! own skills and the project skills its `skills:` list names, on Claude Code, Codex and
//! opencode, each by its own route; and a harness skill it names that this machine does not have
//! is said on the chat's tab (SK-5, #2768; ADR 0099 as amended 2026-10-21). Read through what each
//! harness is started with: its arguments and its environment.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::{As, Harness, Kit, StateHooks};
use purlis_core::skills::{PLANE_LISTED_ENV, PlaneSkills};

struct Here {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    plugin: PathBuf,
    plane: PathBuf,
    sets: PathBuf,
}

const WORKSPACE: &str = "smart-ide";
const PERSONA: &str = "ops";

fn skill(dir: &Path, name: &str) {
    fs::create_dir_all(dir.join(name)).unwrap();
    fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
    )
    .unwrap();
    // A file beside it, which a set's folder links to where it hands the skill.
    fs::write(dir.join(name).join("notes.md"), "notes\n").unwrap();
}

/// A bundle with the built-in skill `secrets` and the opencode shim; a plane with the project
/// skills `deploy` (granted to no workspace) and `review` (named by nobody), the workspace
/// `smart-ide`, and the persona `ops`, which owns `triage` and names `deploy`,
/// `charter:secrets` and a harness skill.
fn here() -> Here {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let plugin = root.join("plugin");
    skill(&plugin.join("skills"), "secrets");
    let shim = purlis_core::opencode::shim_in(&plugin);
    fs::create_dir_all(shim.parent().unwrap()).unwrap();
    fs::write(&shim, "// the shim, a stand-in\n").unwrap();
    let plane = root.join("plane");
    skill(&plane.join("skills"), "deploy");
    skill(&plane.join("skills"), "review");
    fs::create_dir_all(plane.join("workspaces").join(WORKSPACE)).unwrap();
    let persona = plane.join("personas").join(PERSONA);
    skill(&persona.join("skills"), "triage");
    fs::write(
        persona.join("persona.md"),
        "---\nrole: Ops\nskills: deploy, charter:secrets, superpowers:test-driven-development\n\
         ---\n\nOps.\n",
    )
    .unwrap();
    Here {
        _tmp: tmp,
        sets: root.join("data/skill-sets"),
        root,
        plugin,
        plane,
    }
}

/// What the host does before an adapter arms the persona's chat in the workspace, with the flag
/// on: lay out the folder of links, then arm the chat with it.
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
        Some(WORKSPACE),
        Some(PERSONA),
        &host,
        harness,
    );
    let kit = Kit {
        binary: Path::new("/bin/purlis"),
        plugin: Some(&here.plugin),
        persona: Some(As {
            root: &here.plane,
            persona: PERSONA,
        }),
        skills: Some(PlaneSkills {
            plane: &here.plane,
            workspace: Some(WORKSPACE),
            repo: &[],
            routed: None,
        }),
        graded: false,
        laid_out: linked.as_deref(),
    };
    let cwd = here.plane.join("workspaces").join(WORKSPACE);
    match harness.state_hooks(
        kit,
        Some(&cwd),
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

/// Each skill a set's `skills/` folder hands, by name, and the plane folder its files link to: a
/// folder of its own holding the `SKILL.md` purlis wrote, and a link to each of its files
/// (SK-8).
fn linked(skills: &Path) -> Vec<(String, PathBuf)> {
    let mut handed: Vec<(String, PathBuf)> = fs::read_dir(skills)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let own = entry.path();
            assert!(
                fs::symlink_metadata(&own).unwrap().is_dir(),
                "{}",
                own.display()
            );
            let file = fs::read_link(own.join("notes.md")).unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                file.parent().unwrap().to_path_buf(),
            )
        })
        .collect();
    handed.sort();
    handed
}

fn wanted(here: &Here) -> Vec<(String, PathBuf)> {
    vec![
        ("deploy".to_owned(), here.plane.join("skills/deploy")),
        (
            "triage".to_owned(),
            here.plane.join("personas/ops/skills/triage"),
        ),
    ]
}

#[test]
fn a_claude_code_chat_as_the_persona_loads_its_skills_in_a_workspace_granted_none() {
    purlis_core::unsteered!();
    let here = here();
    let (args, _) = host_armed(Harness::ClaudeCode, &here);

    let dirs: Vec<PathBuf> = args
        .windows(2)
        .filter(|pair| pair[0] == "--plugin-dir")
        .map(|pair| PathBuf::from(&pair[1]))
        .collect();
    assert_eq!(dirs.len(), 2, "{args:?}");
    assert_eq!(dirs[0], here.plugin);
    assert_eq!(
        linked(&dirs[1].join("skills")),
        wanted(&here),
        "its own skill and the project skill it names; never `review`"
    );
}

#[test]
fn an_opencode_chat_as_the_persona_is_handed_its_skills_folder() {
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Opencode, &here);

    let config: serde_json::Value =
        serde_json::from_str(var(&env, purlis_core::opencode::CONFIG_ENV).unwrap()).unwrap();
    // One folder: the built-in skills under their own names beside the persona's.
    let folder = PathBuf::from(
        config["plugin"][0][1]["skills"]
            .as_str()
            .expect("one directory"),
    );
    assert!(folder.starts_with(&here.sets), "{}", folder.display());
    let mut wanted = wanted(&here);
    wanted.push((
        "purlis-secrets".to_owned(),
        here.plugin.join("skills/secrets"),
    ));
    wanted.sort();
    assert_eq!(linked(&folder), wanted);
}

#[test]
fn a_codex_chat_as_the_persona_is_briefed_on_its_skills() {
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Codex, &here);
    let handed = PathBuf::from(var(&env, PLANE_LISTED_ENV).expect("the set's folder"));
    assert!(handed.starts_with(&here.sets), "{}", handed.display());

    // The chat runs as the persona: its environment names it, as every chat as a persona's does.
    let lookup = |name: &str| {
        if name == purlis_core::active::PERSONA_ENV {
            return Some(PERSONA.to_owned());
        }
        var(&env, name).map(str::to_owned)
    };
    let text =
        purlis_core::skills::briefed(&here.plane, Some(WORKSPACE), None, &lookup).expect("listed");
    for name in ["`deploy`", "`triage`", "`secrets`"] {
        assert!(text.contains(name), "{name} in {text}");
    }
    assert!(!text.contains("`review`"), "{text}");
}

#[test]
fn a_chat_on_no_persona_in_that_workspace_gets_none_of_them() {
    purlis_core::unsteered!();
    let here = here();
    let host = purlis_core::hoststart::Kit {
        binary: Some(PathBuf::from("/bin/purlis")),
        plugin: Some(here.plugin.clone()),
        git_hooks: None,
        skill_sets: Some(here.sets.clone()),
    };

    let linked = purlis_core::hoststart::plane_skills_laid_out(
        true,
        &here.plane,
        Some(WORKSPACE),
        None,
        &host,
        Harness::ClaudeCode,
    );

    assert_eq!(linked, None);
}

#[test]
fn a_harness_skill_the_persona_names_and_this_machine_lacks_is_said_on_the_tab() {
    purlis_core::unsteered!();
    let here = here();
    let config = here.root.join("claude-config");
    let chat = [("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];
    let env = purlis_core::harness_plugin::Env {
        chat: &chat,
        home: Some(here.root.join("home")),
        process: false,
    };
    let said = |harness| {
        purlis_core::skills::not_here(
            &here.plane,
            Some(WORKSPACE),
            Some(PERSONA),
            Some(&here.plugin.join("skills")),
            harness,
            &env,
            None,
        )
    };

    assert_eq!(
        said(Harness::ClaudeCode),
        Some(purlis_core::skills::NotHere {
            missing: vec!["superpowers:test-driven-development".to_owned()],
            no_built_in: Vec::new(),
            broken: Vec::new(),
            by_a_repo: Vec::new(),
            beside_a_built_in: Vec::new(),
        })
    );

    skill(
        &config.join("plugins/cache/superpowers/skills"),
        "test-driven-development",
    );
    fs::write(
        config.join("plugins/installed_plugins.json"),
        serde_json::json!({
            "version": 2,
            "plugins": {
                "superpowers@market": [
                    {"scope": "user", "installPath": config.join("plugins/cache/superpowers")}
                ]
            }
        })
        .to_string(),
    )
    .unwrap();

    assert_eq!(said(Harness::ClaudeCode), None, "installed now");
}

#[test]
fn on_codex_a_plugin_skill_is_found_in_its_cache_and_one_it_cannot_list_raises_nothing() {
    purlis_core::unsteered!();
    let here = here();
    let codex = here.root.join("codex-home");
    fs::create_dir_all(&codex).unwrap();
    fs::write(
        codex.join("config.toml"),
        "[plugins.\"superpowers@market\"]\nenabled = true\n",
    )
    .unwrap();
    let chat = [("CODEX_HOME".to_owned(), codex.display().to_string())];
    let env = purlis_core::harness_plugin::Env {
        chat: &chat,
        home: Some(here.root.join("home")),
        process: false,
    };
    let said = || {
        purlis_core::skills::not_here(
            &here.plane,
            Some(WORKSPACE),
            Some(PERSONA),
            Some(&here.plugin.join("skills")),
            Harness::Codex,
            &env,
            None,
        )
    };

    assert_eq!(
        said(),
        None,
        "its cache is not there: unchecked, never missing"
    );

    let version = codex.join("plugins/cache/market/superpowers/1.0.0");
    fs::create_dir_all(version.join(".codex-plugin")).unwrap();
    fs::write(
        version.join(".codex-plugin/plugin.json"),
        r#"{"name":"superpowers"}"#,
    )
    .unwrap();
    fs::create_dir_all(version.join("skills")).unwrap();
    assert_eq!(
        said().map(|not| not.missing),
        Some(vec!["superpowers:test-driven-development".to_owned()]),
        "listed, and not among them"
    );

    skill(&version.join("skills"), "test-driven-development");
    assert_eq!(said(), None);
}
