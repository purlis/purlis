// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A skill can only narrow a chat, manual-only is honoured on every harness, each skill carries a
//! compatibility badge per harness, and a skill that cannot be loaded is said (SK-8, #2771;
//! decisions 18, 41 and 58). Read through what each harness is started with: its arguments, its
//! environment, and the folder and briefing it is handed.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::skills::{Fit, LISTED_ENV, Origin, PlaneSkills};

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
        graded: false,
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
    assert_eq!(
        fs::read_to_string(deploy.join("SKILL.md")).unwrap(),
        "---\nname: deploy\ndescription: Does deploy for the team.\ndisallowed-tools: Write\n---\n\nbody\n"
    );
    assert!(
        fs::symlink_metadata(deploy.join("scripts"))
            .unwrap()
            .is_dir(),
        "a folder of the skill's is a folder of its own too"
    );
    assert_eq!(
        fs::read_link(deploy.join("scripts/run.sh")).unwrap(),
        here.plane.join("skills/deploy/scripts/run.sh"),
        "each file is the plane's own, linked"
    );
    // Claude Code reads manual-only itself: the description stays.
    assert!(
        fs::read_to_string(skills.join("release/SKILL.md"))
            .unwrap()
            .contains("description: Does release for the team.\n")
    );
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
    assert!(!deploy.contains("\nallowed-tools:"), "{deploy}");
    assert!(!deploy.contains("\nhooks:"), "{deploy}");
    assert!(deploy.contains("\ndisallowed-tools: Write\n"), "{deploy}");
}

#[cfg(unix)]
#[test]
fn nothing_is_handed_through_a_link_and_only_the_top_skill_md_is_the_skills() {
    purlis_core::unsteered!();
    let here = here();
    let outside = here.plane.parent().unwrap().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "no\n").unwrap();
    let skills = here.plane.join("skills");
    // A link to outside, inside a skill's folder, and a nested skill.
    std::os::unix::fs::symlink(outside.join("secret.txt"), skills.join("review/secret.txt"))
        .unwrap();
    std::os::unix::fs::symlink(&outside, skills.join("review/outside")).unwrap();
    fs::create_dir_all(skills.join("review/inner")).unwrap();
    fs::write(
        skills.join("review/inner/SKILL.md"),
        "---\nname: inner\ndescription: Nested.\nallowed-tools: Bash\n---\n",
    )
    .unwrap();
    fs::write(skills.join("review/inner/notes.md"), "notes\n").unwrap();
    // A skill whose folder is a link, and one whose SKILL.md is.
    fs::create_dir_all(outside.join("away")).unwrap();
    fs::write(
        outside.join("away/SKILL.md"),
        "---\nname: away\ndescription: Lives elsewhere.\n---\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.join("away"), skills.join("away")).unwrap();
    fs::create_dir_all(skills.join("pointed")).unwrap();
    fs::write(
        outside.join("pointed.md"),
        "---\nname: pointed\ndescription: Points elsewhere.\n---\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.join("pointed.md"), skills.join("pointed/SKILL.md"))
        .unwrap();

    let (args, _) = host_armed(Harness::ClaudeCode, &here);
    let handed = claude_folder(&args).join("skills");
    let review = handed.join("review");
    assert!(
        fs::symlink_metadata(review.join("secret.txt")).is_err(),
        "no link to a link"
    );
    assert!(
        fs::symlink_metadata(review.join("outside")).is_err(),
        "no link to a link"
    );
    assert!(
        fs::symlink_metadata(review.join("inner/SKILL.md")).is_err(),
        "no nested skill"
    );
    assert_eq!(
        fs::read_link(review.join("inner/notes.md")).unwrap(),
        skills.join("review/inner/notes.md")
    );
    for walked in walk(&handed) {
        if let Ok(target) = fs::read_link(&walked) {
            assert!(
                target.starts_with(&skills),
                "{} -> {}",
                walked.display(),
                target.display()
            );
        }
    }
    for refused in ["away", "pointed"] {
        assert!(
            fs::symlink_metadata(handed.join(refused)).is_err(),
            "{refused} handed"
        );
    }

    // And the chat's tab says why each was handed to no chat.
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
        None,
    )
    .expect("said");
    let mut broken: Vec<(String, String)> = said
        .broken
        .iter()
        .map(|one| (one.name.clone(), one.why.to_string()))
        .collect();
    broken.sort();
    const LINK: &str = "a link, so purlis doesn't read it";
    assert_eq!(
        broken,
        [
            // Discovery leaves both out before anything is handed (SK-36).
            ("away".to_owned(), LINK.to_owned()),
            ("pointed".to_owned(), LINK.to_owned()),
        ]
    );
}

/// Every path under `dir`, never through a link.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut all = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if fs::symlink_metadata(&path).unwrap().is_dir() {
            all.extend(walk(&path));
        }
        all.push(path);
    }
    all
}

#[test]
fn a_skill_whose_frontmatter_purlis_cannot_read_is_handed_to_no_chat_and_said() {
    purlis_core::unsteered!();
    let here = here();
    skill(
        &here.plane.join("skills"),
        "respelled",
        "allowedTools: Bash\n",
    );
    let (args, _) = host_armed(Harness::ClaudeCode, &here);
    assert!(fs::symlink_metadata(claude_folder(&args).join("skills/respelled")).is_err());
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
        None,
    )
    .expect("said");
    assert_eq!(said.broken.len(), 1, "{said:?}");
    assert_eq!(said.broken[0].name, "respelled");
    assert!(said.broken[0].why.to_string().contains("widen the chat"));
}

#[test]
fn a_codex_chat_is_briefed_on_a_manual_only_skill_by_its_name_and_file_alone() {
    purlis_core::unsteered!();
    let here = here();
    let (_, env) = host_armed(Harness::Codex, &here);
    assert!(var(&env, LISTED_ENV).is_some());
    let handed_deploy = PathBuf::from(var(&env, purlis_core::skills::PLANE_LISTED_ENV).unwrap())
        .join("skills/deploy/SKILL.md");

    let lookup = |name: &str| var(&env, name).map(str::to_owned);
    let text = purlis_core::skills::briefed(&here.plane, None, None, &lookup).expect("listed");
    assert!(
        text.contains("- `review` — Does review for the team."),
        "{text}"
    );
    assert!(!text.contains("Does release"), "{text}");
    assert!(
        text.contains(&format!("(`{}`)", handed_deploy.display())),
        "the narrowed SKILL.md: {text}"
    );
    assert!(
        !fs::read_to_string(&handed_deploy)
            .unwrap()
            .contains("\nallowed-tools:"),
        "and it widens nothing"
    );
    let handed = PathBuf::from(var(&env, purlis_core::skills::PLANE_LISTED_ENV).unwrap());
    let release = handed.join("skills/release/SKILL.md");
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
    purlis_core::unsteered!();
    let text = format!(
        "---\nname: deploy\ndescription: Deploys it.\nmodel: opus\ndisable-model-invocation: true\n{WIDENS}---\n"
    );
    let on = |harness: Harness, origin: Origin| {
        purlis_core::skills::compat(&text, origin, &harness.skill_frontmatter())
    };
    let left_out = "Left out, because a skill may only narrow a chat: the tools it pre-approves \
                    and the commands it adds to the chat's events.";

    let claude = on(Harness::ClaudeCode, Origin::Project);
    assert_eq!(claude.fit, Fit::Partial);
    assert_eq!(claude.lines(), [left_out]);

    for harness in [Harness::Codex, Harness::Opencode] {
        let badge = on(harness, Origin::Project);
        assert_eq!(badge.fit, Fit::Partial, "{harness:?}");
        assert_eq!(
            badge.lines(),
            [
                left_out,
                "Carried over by purlis's equivalent: running only when asked for.",
                "Narrowing not applied: the tools it takes away.",
                "Ignored: the model it asks for.",
            ],
            "{harness:?}"
        );
    }

    // A harness or repo skill purlis does not hand: it widens a Claude Code chat.
    let outside = on(Harness::ClaudeCode, Origin::Repo);
    assert_eq!(outside.fit, Fit::Widens);
    assert!(outside.widens());
}

#[test]
fn a_manual_only_skill_costs_nothing_on_every_harness() {
    purlis_core::unsteered!();
    for harness in Harness::ALL {
        assert!(
            matches!(
                harness.skill_frontmatter().manual,
                purlis_core::skills::ManualOnly::Read
                    | purlis_core::skills::ManualOnly::Listing
                    | purlis_core::skills::ManualOnly::WithoutDescription
            ),
            "{harness:?}"
        );
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
        None,
    )
    .expect("said");
    assert_eq!(
        said.broken,
        [purlis_core::skills::Broken {
            name: "half".to_owned(),
            origin: Origin::Project,
            local: false,
            why: purlis_core::skills::BrokenWhy::LeftOut(purlis_core::skills::Reason::NotASkill(
                purlis_core::skills::Missing::Description
            )),
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
        None,
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
