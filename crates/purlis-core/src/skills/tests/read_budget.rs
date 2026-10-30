// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #3459: the lint reads a skill's other files within a budget of bytes, one lint's and one
//! list's, and reads the files of the person's own skills as it reads their `SKILL.md`.

use std::path::Path;

use super::super::*;

/// A script of exactly [`purlis_skills::MOST_BYTES`] bytes that calls `tool`: the most of one
/// file the lint reads.
fn a_whole_script(tool: &str) -> String {
    let mut text = format!("#!/bin/sh\n{tool} --x\n");
    let most = usize::try_from(purlis_skills::MOST_BYTES).unwrap();
    while text.len() + 64 < most {
        text.push_str(&format!("# {}\n", "x".repeat(60)));
    }
    text.push_str(&"#".repeat(most - text.len() - 1));
    text.push('\n');
    assert_eq!(text.len(), most);
    text
}

/// A plane at `plane` with the project skill `name`, each of `scripts` a whole script calling
/// the program of its name. Its `SKILL.md`'s text.
fn a_skill_of_scripts(plane: &Path, name: &str, scripts: &[&str]) -> String {
    let folder = plane.join("skills").join(name);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let text = format!("---\nname: {name}\ndescription: Runs the scripts it holds.\n---\n");
    std::fs::write(folder.join("SKILL.md"), &text).unwrap();
    for tool in scripts {
        std::fs::write(folder.join(format!("{tool}.sh")), a_whole_script(tool)).unwrap();
    }
    text
}

#[test]
fn one_lint_reads_at_most_its_bytes_of_a_skills_files_and_says_how_many_it_left_unread() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let tools = [
        "purlis-3459-a",
        "purlis-3459-b",
        "purlis-3459-c",
        "purlis-3459-d",
        "purlis-3459-e",
    ];
    let text = a_skill_of_scripts(&plane, "many", &tools);

    let said: Vec<String> = lint(&plane, &verbs::Level::project(), "many", &text)
        .iter()
        .map(ToString::to_string)
        .collect();

    // Four whole scripts are the one lint's MiB: the fifth, last by name, is not read.
    for tool in &tools[..4] {
        assert!(
            said.iter().any(|line| line.contains(tool)),
            "{tool}: {said:?}"
        );
    }
    assert!(!said.iter().any(|line| line.contains(tools[4])), "{said:?}");
    assert!(
        said.contains(
            &"1 of its files was not read for this lint: purlis reads at most 1 MiB of one \
              skill's files"
                .to_owned()
        ),
        "{said:?}"
    );
}

#[test]
fn one_list_reads_at_most_its_bytes_across_every_skill_it_lints() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    a_skill_of_scripts(&plane, "first", &["purlis-3459-f1", "purlis-3459-f2"]);
    a_skill_of_scripts(
        &plane,
        "second",
        &["purlis-3459-s1", "purlis-3459-s2", "purlis-3459-s3"],
    );
    let set = in_the_studio(None, &plane, None, None);
    let mut rows = rows_of(&set, &plane, &[], Reading::Whole);
    let known = Known::of(&plane, None);
    // What is left of the list's when it comes to these two: three whole scripts' worth.
    known.reads.set(3 * purlis_skills::MOST_BYTES);

    with_plane_lint(&mut rows, &set, &known, &[]);

    let lint_of = |name: &str| {
        rows.iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("{name} listed"))
            .lint
            .clone()
    };
    let first = lint_of("first");
    let second = lint_of("second");
    assert!(
        first.iter().any(|line| line.contains("purlis-3459-f2")),
        "{first:?}"
    );
    assert!(
        !first.iter().any(|line| line.contains("not read")),
        "{first:?}"
    );
    assert!(
        second.iter().any(|line| line.contains("purlis-3459-s1")),
        "{second:?}"
    );
    assert!(
        !second
            .iter()
            .any(|line| line.contains("purlis-3459-s2") || line.contains("purlis-3459-s3")),
        "{second:?}"
    );
    assert!(
        second.contains(
            &"2 of its files were not read for this lint: purlis reads at most 16 MiB of files \
              for all the skills in one list"
                .to_owned()
        ),
        "{second:?}"
    );
    assert_eq!(known.reads.get(), 0);
}

#[test]
fn a_list_starts_with_its_whole_budget_of_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        Known::of(tmp.path(), None).reads.get(),
        purlis_skills::MOST_LIST_BYTES
    );
}

#[cfg(unix)]
#[test]
fn a_file_that_is_not_text_spends_what_was_read_of_it() {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let text = a_skill_of_scripts(&plane, "pictures", &[]);
    let folder = plane.join("skills/pictures");
    let most = usize::try_from(purlis_skills::MOST_BYTES).unwrap();
    for n in 0..4 {
        std::fs::write(folder.join(format!("a{n}.png")), vec![0xff_u8; most]).unwrap();
    }
    std::fs::write(folder.join("z.sh"), a_whole_script("purlis-3459-z")).unwrap();

    let said: Vec<String> = lint(&plane, &verbs::Level::project(), "pictures", &text)
        .iter()
        .map(ToString::to_string)
        .collect();

    assert!(
        !said.iter().any(|line| line.contains("purlis-3459-z")),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|line| line.starts_with("1 of its files was not read")),
        "{said:?}"
    );
}

#[cfg(unix)]
#[test]
fn the_persons_own_skills_other_files_are_read_with_a_hard_link_as_its_skill_md_is() {
    // A person's own skill linked in from the Nix store or by `cp -al`: its script is a hard
    // link too, and the lint reads it as it reads its `SKILL.md`. The same file in a project
    // skill is still refused.
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::write(plane.join("charter.toml"), "[plane]\n").unwrap();
    let store = tmp.path().join("store/nix");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join("SKILL.md"),
        "---\nname: nix\ndescription: Comes from the store, linked in by the person.\n---\n",
    )
    .unwrap();
    std::fs::write(store.join("run.sh"), "#!/bin/sh\npurlis-3459-twin --x\n").unwrap();
    let config = tmp.path().join("claude-config");
    let own = config.join("skills/nix");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::hard_link(store.join("SKILL.md"), own.join("SKILL.md")).unwrap();
    std::fs::hard_link(store.join("run.sh"), own.join("run.sh")).unwrap();
    let chat = [("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(tmp.path().join("home")),
        process: false,
    };

    let list = studio_list(None, &plane, None, None, &env);

    let row = list
        .harnesses
        .iter()
        .find(|one| one.harness == Harness::ClaudeCode)
        .and_then(|one| one.rows.iter().find(|row| row.name == "nix"))
        .expect("nix listed");
    assert!(
        row.lint
            .iter()
            .any(|line| line.contains("purlis-3459-twin")),
        "{:?}",
        row.lint
    );
}

#[cfg(unix)]
#[test]
fn the_file_beside_the_persons_own_skill_is_read_with_a_hard_link_and_a_repos_is_not() {
    // Codex's sidecar keeps a skill it finds itself out of its model's choice (SK-39). Beside
    // the person's own skill it is read as its `SKILL.md` is, a hard link and all; beside a
    // repo's it is still refused, so that skill stays listed.
    let tmp = tempfile::tempdir().unwrap();
    let skills = tmp.path().join("skills");
    let folder = skills.join("deploy");
    std::fs::create_dir_all(folder.join("agents")).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        "---\nname: deploy\ndescription: Does deploy.\n---\n",
    )
    .unwrap();
    let store = tmp.path().join("openai.yaml");
    std::fs::write(
        &store,
        "interface:\n  display_name: Deploy\npolicy:\n  allow_implicit_invocation: false\n",
    )
    .unwrap();
    std::fs::hard_link(&store, folder.join("agents/openai.yaml")).unwrap();
    let path = folder.join("SKILL.md");
    let said = |origin: Origin| {
        badges((&skills, &path), origin, &[Harness::Codex])[0]
            .compat
            .lines()
    };

    assert_eq!(
        said(Origin::Harness),
        [
            "Everything it asks for applies.",
            "Runs only when asked for by name."
        ]
    );
    assert_eq!(said(Origin::Repo), ["Everything it asks for applies."]);
}
