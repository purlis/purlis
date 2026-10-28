// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project skill reaches a chat at the plane root on Claude Code, Codex and opencode, each by
//! its own route, and nothing is written where a harness looks for skills (SK-2, #2765; ADR 0063
//! as amended 2026-10-20, ADR 0099). Read through what each harness is started with: its
//! arguments and its environment.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::skills::{LISTED_ENV, PLANE_LISTED_ENV, PlaneSkills};

struct Here {
    _tmp: tempfile::TempDir,
    plugin: PathBuf,
    plane: PathBuf,
    sets: PathBuf,
}

fn skill(dir: &Path, name: &str) {
    fs::create_dir_all(dir.join(name)).unwrap();
    fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
    )
    .unwrap();
    fs::write(dir.join(name).join("notes.md"), "notes\n").unwrap();
}

/// A bundle with one built-in skill and the opencode shim, a plane with one project skill, and
/// an empty folder for the sets, each apart.
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
    fs::create_dir_all(plane.join("workspaces/smart-ide")).unwrap();
    Here {
        _tmp: tmp,
        plugin,
        plane,
        sets: root.join("data/skill-sets"),
    }
}

fn kit<'a>(here: &'a Here, skills: Option<PlaneSkills<'a>>, laid_out: Option<&'a Path>) -> Kit<'a> {
    Kit {
        binary: Path::new("/bin/purlis"),
        plugin: Some(&here.plugin),
        persona: None,
        skills,
        graded: false,
        laid_out,
    }
}

/// What the host does before an adapter arms a chat, with the flag on: lay out the folder of
/// links (`hoststart::plane_skills_laid_out`), then arm the chat with it.
fn host_armed(
    harness: Harness,
    here: &Here,
    workspace: Option<&str>,
    cwd: &Path,
) -> (Vec<String>, Vec<(String, String)>) {
    host_armed_on(true, harness, here, workspace, cwd)
}

/// [`host_armed`] with the flag `on` or off.
fn host_armed_on(
    on: bool,
    harness: Harness,
    here: &Here,
    workspace: Option<&str>,
    cwd: &Path,
) -> (Vec<String>, Vec<(String, String)>) {
    let host = purlis_core::hoststart::Kit {
        binary: Some(PathBuf::from("/bin/purlis")),
        plugin: Some(here.plugin.clone()),
        git_hooks: None,
        skill_sets: Some(here.sets.clone()),
    };
    let linked = purlis_core::hoststart::plane_skills_laid_out(
        on,
        &here.plane,
        workspace,
        None,
        &host,
        harness,
    );
    let skills = on.then_some(PlaneSkills {
        plane: &here.plane,
        workspace,
        repo: &[],
        routed: None,
    });
    armed(harness, kit(here, skills, linked.as_deref()), cwd)
}

/// The directories an opencode chat's config hands its shim, as a list.
fn opencode_skill_dirs(env: &[(String, String)]) -> Vec<PathBuf> {
    let config: serde_json::Value =
        serde_json::from_str(var(env, purlis_core::opencode::CONFIG_ENV).unwrap()).unwrap();
    match &config["plugin"][0][1]["skills"] {
        serde_json::Value::Null => Vec::new(),
        serde_json::Value::String(dir) => vec![PathBuf::from(dir)],
        dirs => dirs
            .as_array()
            .expect("a list of directories")
            .iter()
            .map(|dir| PathBuf::from(dir.as_str().unwrap()))
            .collect(),
    }
}

fn armed(harness: Harness, kit: Kit<'_>, cwd: &Path) -> (Vec<String>, Vec<(String, String)>) {
    match harness.state_hooks(
        kit,
        Some(cwd),
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) {
        StateHooks::ThisSessionOnly { args, env, .. } => (args, env),
        StateHooks::None => panic!("{harness:?} is armed"),
    }
}

fn plugin_dirs(args: &[String]) -> Vec<PathBuf> {
    args.windows(2)
        .filter(|pair| pair[0] == "--plugin-dir")
        .map(|pair| PathBuf::from(&pair[1]))
        .collect()
}

fn var<'e>(env: &'e [(String, String)], name: &str) -> Option<&'e str> {
    env.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Every path under `dir`, so a test can tell nothing was written there.
fn tree(dir: &Path) -> Vec<PathBuf> {
    let mut all = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(next) = todo.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                todo.push(path.clone());
            }
            all.push(path);
        }
    }
    all.sort();
    all
}

#[test]
fn a_claude_code_chat_at_the_plane_root_loads_the_project_skill_as_a_plugin_of_links() {
    purlis_core::unsteered!();
    let here = here();
    let (args, _) = host_armed(Harness::ClaudeCode, &here, None, &here.plane);

    let dirs = plugin_dirs(&args);
    assert_eq!(dirs.len(), 2, "{args:?}");
    assert_eq!(dirs[0], here.plugin, "the bundle first, as before");
    let linked = &dirs[1];
    assert!(linked.starts_with(&here.sets), "{}", linked.display());
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(linked.join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["name"], "skills");
    assert_eq!(
        fs::read_link(linked.join("skills/deploy/notes.md")).unwrap(),
        here.plane.join("skills/deploy/notes.md"),
        "its files are links to the plane's own, never copies"
    );
    assert_eq!(
        fs::read_to_string(linked.join("skills/deploy/SKILL.md")).unwrap(),
        "---\nname: deploy\ndescription: Does deploy.\n---\n\nbody\n",
        "the SKILL.md purlis writes for it"
    );
    assert_eq!(
        fs::read_dir(linked.join("skills")).unwrap().count(),
        1,
        "the project's skills only: the built-in ones ride the bundle"
    );
}

#[test]
fn an_opencode_chat_at_the_plane_root_is_handed_one_folder_of_the_built_in_and_the_project_skills()
{
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Opencode, &here, None, &here.plane);

    let skills = opencode_skill_dirs(&env);
    assert_eq!(skills.len(), 1, "{skills:?}");
    let skills = &skills[0];
    assert!(skills.starts_with(&here.sets), "{}", skills.display());
    assert_eq!(
        fs::read_link(skills.join("deploy/notes.md")).unwrap(),
        here.plane.join("skills/deploy/notes.md")
    );
    let mut names: Vec<String> = fs::read_dir(skills)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["deploy", "purlis-secrets"], "no bare `secrets`");
    assert_eq!(
        fs::read_to_string(skills.join("purlis-secrets/SKILL.md")).unwrap(),
        "---\nname: purlis-secrets\ndescription: Does secrets.\n---\n\nbody\n",
        "its name is its folder's, as the skill format asks"
    );
    assert_eq!(
        fs::read_link(skills.join("purlis-secrets/notes.md")).unwrap(),
        here.plugin.join("skills/secrets/notes.md"),
        "its files are the bundle's own"
    );
}

#[test]
fn an_opencode_chat_in_a_clone_with_its_own_secrets_skill_gets_both_by_distinct_names() {
    purlis_core::unsteered!();
    // opencode reads a clone's `.agents/skills` itself, every skill by its bare name: the
    // clone's `secrets` and purlis's `purlis-secrets` are two skills, and neither is the other.
    let here = here();
    let clone = here.plane.join("workspaces/smart-ide/api");
    fs::create_dir_all(clone.join(".git")).unwrap();
    skill(&clone.join(".agents/skills"), "secrets");

    let (_, env) = host_armed_on(false, Harness::Opencode, &here, None, &clone);
    let dirs = opencode_skill_dirs(&env);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    let ours = fs::read_to_string(dirs[0].join("purlis-secrets/SKILL.md")).unwrap();
    assert!(ours.contains("name: purlis-secrets\n"), "{ours}");
    assert!(
        !dirs[0].join("secrets").exists(),
        "no bare `secrets` from purlis"
    );

    let repo = purlis_core::skills::repo_skills_of_chat(
        Harness::Opencode,
        &here.plane,
        "smart-ide",
        &clone,
    );
    let set = purlis_core::skills::for_chat(
        kit(
            &here,
            Some(PlaneSkills {
                plane: &here.plane,
                workspace: Some("smart-ide"),
                repo: &repo,
                routed: None,
            }),
            None,
        ),
        Harness::Opencode,
    );
    let mut names: Vec<(&str, purlis_core::skills::Origin)> = set
        .effective
        .iter()
        .map(|e| (e.skill.name.as_str(), e.origin))
        .collect();
    names.sort_by_key(|(name, _)| *name);
    assert_eq!(
        names,
        [
            ("purlis-secrets", purlis_core::skills::Origin::BuiltIn),
            ("secrets", purlis_core::skills::Origin::Repo),
        ]
    );
}

#[test]
fn a_codex_chat_at_the_plane_root_is_briefed_on_the_project_skill_from_its_folder_in_the_plane() {
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Codex, &here, None, &here.plane);

    let built_in = here.plugin.join("skills").display().to_string();
    assert_eq!(var(&env, LISTED_ENV), Some(built_in.as_str()));
    let handed = PathBuf::from(var(&env, PLANE_LISTED_ENV).expect("the set's folder"));
    assert!(handed.starts_with(&here.sets), "{}", handed.display());

    let lookup = |name: &str| var(&env, name).map(str::to_owned);
    let text = purlis_core::skills::briefed(&here.plane, None, None, &lookup).expect("listed");
    assert!(text.contains("`deploy`"), "{text}");
    assert!(
        text.contains(&handed.join("skills/deploy/SKILL.md").display().to_string()),
        "the SKILL.md purlis wrote for it: {text}"
    );
    assert!(text.contains("`secrets`"), "{text}");
}

#[test]
fn with_the_flag_off_every_harness_is_armed_with_the_built_in_skills_alone() {
    purlis_core::unsteered!();
    let here = here();
    // Claude Code and Codex as before: nothing laid out.
    let (claude, _) = host_armed_on(false, Harness::ClaudeCode, &here, None, &here.plane);
    assert_eq!(plugin_dirs(&claude), std::slice::from_ref(&here.plugin));

    let (_, codex) = host_armed_on(false, Harness::Codex, &here, None, &here.plane);
    assert_eq!(var(&codex, PLANE_LISTED_ENV), None);
    let lookup = |name: &str| var(&codex, name).map(str::to_owned);
    let text = purlis_core::skills::briefed(&here.plane, None, None, &lookup).expect("listed");
    assert!(!text.contains("`deploy`"), "{text}");
    assert!(text.contains("`secrets`"), "{text}");
    assert!(!here.sets.exists(), "nothing laid out");

    // opencode is handed the built-in skills under their own names, in a folder of their own.
    let (_, opencode) = host_armed_on(false, Harness::Opencode, &here, None, &here.plane);
    let dirs = opencode_skill_dirs(&opencode);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    assert!(dirs[0].starts_with(&here.sets), "{}", dirs[0].display());
    let names: Vec<String> = fs::read_dir(&dirs[0])
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["purlis-secrets"], "the built-in skills alone");
}

#[test]
fn a_chat_in_a_workspace_the_skill_was_not_granted_to_gets_the_built_in_skills_alone() {
    purlis_core::unsteered!();
    let here = here();
    let cwd = here.plane.join("workspaces/smart-ide");

    let (claude, _) = host_armed(Harness::ClaudeCode, &here, Some("smart-ide"), &cwd);
    assert_eq!(plugin_dirs(&claude), std::slice::from_ref(&here.plugin));
    let (_, codex) = host_armed(Harness::Codex, &here, Some("smart-ide"), &cwd);
    assert_eq!(var(&codex, PLANE_LISTED_ENV), None);
    assert!(!here.sets.exists(), "nothing laid out");
}

#[test]
fn an_adapter_lays_nothing_out_itself_and_with_no_folder_hands_opencode_no_skills_folder() {
    purlis_core::unsteered!();
    // ADR 0063 as amended 2026-10-20: an adapter only turns the set into words. The host lays
    // the folder out; an adapter handed none writes none and names none.
    let here = here();
    let no_folder = Some(PlaneSkills {
        plane: &here.plane,
        workspace: None,
        repo: &[],
        routed: None,
    });

    let (claude, _) = armed(
        Harness::ClaudeCode,
        kit(&here, no_folder, None),
        &here.plane,
    );
    let (_, opencode) = armed(Harness::Opencode, kit(&here, no_folder, None), &here.plane);

    assert_eq!(plugin_dirs(&claude), std::slice::from_ref(&here.plugin));
    assert_eq!(
        opencode_skill_dirs(&opencode),
        Vec::<PathBuf>::new(),
        "opencode is never handed the bundle's bare names"
    );
    assert!(!here.sets.exists(), "nothing laid out by an adapter");
}

#[test]
fn arming_every_harness_writes_nothing_in_the_plane_and_two_chats_share_one_folder() {
    purlis_core::unsteered!();
    let here = here();
    let before = tree(&here.plane);

    let (first, _) = host_armed(Harness::ClaudeCode, &here, None, &here.plane);
    host_armed(Harness::Opencode, &here, None, &here.plane);
    host_armed(Harness::Codex, &here, None, &here.plane);
    let (second, _) = host_armed(Harness::ClaudeCode, &here, None, &here.plane);

    assert_eq!(
        tree(&here.plane),
        before,
        "no harness skills folder (`.claude/skills`, `.agents/skills`, `.codex/skills`, \
         `.opencode/`) or anything else is written in the plane"
    );
    assert_eq!(plugin_dirs(&first), plugin_dirs(&second));
    assert_eq!(
        fs::read_dir(&here.sets)
            .unwrap()
            // Beside the sets' folders: the lock a tidy takes turns on (SK-24).
            .filter(|entry| entry.as_ref().unwrap().file_name() != ".lock")
            .count(),
        2,
        "one set, one folder: Claude Code's and Codex's, and opencode's, which holds the \
         built-in skills too"
    );
}

#[test]
fn a_new_project_skill_is_laid_out_in_a_folder_of_its_own_and_the_old_one_is_left_alone() {
    purlis_core::unsteered!();
    let here = here();
    let (first, _) = host_armed(Harness::ClaudeCode, &here, None, &here.plane);
    skill(&here.plane.join("skills"), "review");

    let (second, _) = host_armed(Harness::ClaudeCode, &here, None, &here.plane);

    let (old, new) = (&plugin_dirs(&first)[1], &plugin_dirs(&second)[1]);
    assert_ne!(old, new);
    assert_eq!(fs::read_dir(old.join("skills")).unwrap().count(), 1);
    assert_eq!(fs::read_dir(new.join("skills")).unwrap().count(), 2);
}
