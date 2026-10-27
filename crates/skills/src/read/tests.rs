// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;

/// The skills directory the app ships.
fn bundled() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills")
}

fn skill(dir: &Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

#[test]
fn every_bundled_skill_is_read_with_its_name_description_and_file() {
    let skills = read(&bundled());
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    for name in [
        "safe-remove",
        "compact",
        "add-curation-action",
        "handoff",
        "secrets",
        "smart-close",
        "theme",
    ] {
        assert!(names.contains(&name), "{name} is missing from {names:?}");
    }
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "listed by name");
    // Every folder in the bundle is a skill a harness would load: none is dropped.
    let folders = std::fs::read_dir(bundled()).unwrap().count();
    assert_eq!(skills.len(), folders, "{names:?}");
    let safe = skills.iter().find(|s| s.name == "safe-remove").unwrap();
    assert!(
        safe.description
            .starts_with("Remove a purlis workspace or persona")
    );
    assert_eq!(safe.path, bundled().join("safe-remove/SKILL.md"));
}

#[test]
fn a_folder_that_is_not_a_whole_skill_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    skill(
        dir.path(),
        "whole",
        "---\nname: whole\ndescription: Does it.\n---\n\nbody\n",
    );
    skill(dir.path(), "nameless", "---\ndescription: Does it.\n---\n");
    skill(dir.path(), "undescribed", "---\nname: undescribed\n---\n");
    skill(dir.path(), "no-frontmatter", "# just a page\n");
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();
    std::fs::write(
        dir.path().join("stray.md"),
        "---\nname: stray\ndescription: x\n---\n",
    )
    .unwrap();

    let names: Vec<String> = read(dir.path()).into_iter().map(|s| s.name).collect();

    assert_eq!(names, ["whole"]);
}

#[test]
fn a_directory_that_is_not_there_has_no_skills() {
    assert!(read(Path::new("/nonexistent/charter/skills")).is_empty());
    assert_eq!(in_bundle(Path::new("/nonexistent/charter/plugin")), None);
}

#[test]
fn the_bundle_holds_its_skills_in_its_own_skills_folder() {
    let plugin = bundled().parent().unwrap().to_path_buf();
    assert_eq!(in_bundle(&plugin), Some(plugin.join("skills")));
}

#[test]
fn the_listing_names_each_skill_when_to_use_it_and_where_to_read_it() {
    let skills = vec![
        Skill {
            name: "compact".to_owned(),
            description: "Compact a memory. Use when asked to tidy.".to_owned(),
            path: PathBuf::from("/app/plugin/skills/compact/SKILL.md"),
        },
        Skill {
            name: "safe-remove".to_owned(),
            description: "Remove a workspace safely.".to_owned(),
            path: PathBuf::from("/app/plugin/skills/safe-remove/SKILL.md"),
        },
    ];

    let text = listing(&skills).expect("a listing");

    assert!(text.starts_with("⬢ **purlis's built-in skills**"), "{text}");
    // The model is told what a skill is here: a file to read and follow, by its name.
    assert!(text.contains("read its `SKILL.md`"), "{text}");
    assert!(text.contains(
        "- `compact` — Compact a memory. Use when asked to tidy. \
         (`/app/plugin/skills/compact/SKILL.md`)"
    ));
    assert!(text.ends_with(
        "- `safe-remove` — Remove a workspace safely. \
         (`/app/plugin/skills/safe-remove/SKILL.md`)"
    ));
}

#[test]
fn no_skills_is_no_listing_not_an_empty_one() {
    assert_eq!(listing(&[]), None);
}

#[test]
fn a_manual_only_skill_is_listed_by_its_name_and_file_alone_to_run_only_when_named() {
    let tmp = tempfile::tempdir().unwrap();
    skill(
        tmp.path(),
        "release",
        "---\nname: release\ndescription: Cuts a release.\ndisable-model-invocation: true\n---\n",
    );
    skill(
        tmp.path(),
        "review",
        "---\nname: review\ndescription: Reviews a change.\n---\n",
    );
    let text = listing(&read(tmp.path())).expect("a listing");
    assert!(text.contains("- `review` — Reviews a change."), "{text}");
    assert!(!text.contains("Cuts a release"), "{text}");
    let release = tmp.path().join("release/SKILL.md");
    assert!(
        text.ends_with(&format!(
            "\n- Only when the person names one, never by your own choice: `release` (`{}`)",
            release.display()
        )),
        "{text}"
    );
}

#[test]
fn no_built_in_skill_asks_for_what_would_widen_a_chat_and_each_is_handed_whole() {
    for skill in read(&bundled()) {
        let text = std::fs::read_to_string(&skill.path).unwrap();
        let read =
            crate::handed::parse(&text).unwrap_or_else(|why| panic!("{}: {why}", skill.name));
        assert!(
            !read.fields().iter().any(|field| field.widens()),
            "{} widens a chat",
            skill.name
        );
    }
}

// ---- links (SK-36) --------------------------------------------------------------------------

/// A folder outside every skill, holding a whole `SKILL.md` and a file a load would name.
#[cfg(unix)]
fn outside(dir: &Path) -> PathBuf {
    let outside = dir.join("outside");
    skill(
        dir,
        "outside",
        "---\nname: outside\ndescription: Not a skill here.\n---\n",
    );
    std::fs::write(outside.join("secret.txt"), "no\n").unwrap();
    outside
}

#[cfg(unix)]
#[test]
fn a_skill_folder_that_is_a_link_out_of_the_level_is_left_out_as_a_link_and_never_read() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = outside(tmp.path());
    let level = tmp.path().join("skills");
    skill(
        &level,
        "whole",
        "---\nname: whole\ndescription: Does it.\n---\n",
    );
    std::os::unix::fs::symlink(&outside, level.join("linked")).unwrap();
    // A link to a file beside the folders is a file, which is nothing.
    std::os::unix::fs::symlink(outside.join("secret.txt"), level.join("stray.md")).unwrap();

    assert_eq!(
        scan(tmp.path(), Path::new("skills")),
        [
            Err((level.join("linked"), Missing::Link)),
            Ok(Skill {
                name: "whole".to_owned(),
                description: "Does it.".to_owned(),
                path: level.join("whole/SKILL.md"),
            }),
        ]
    );
    let names: Vec<String> = read(&level).into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["whole"]);
    // And were it handed on by its path, nothing of what it leads to is named.
    assert_eq!(
        crate::SkillFolder::open(tmp.path(), &level.join("linked/SKILL.md")).err(),
        Some(crate::Refusal::FolderLink)
    );
}

#[cfg(unix)]
#[test]
fn a_skill_md_that_is_a_link_or_has_a_second_name_is_left_out_as_a_link() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = outside(tmp.path());
    let level = tmp.path().join("skills");
    std::fs::create_dir_all(level.join("borrowed")).unwrap();
    std::os::unix::fs::symlink(outside.join("SKILL.md"), level.join("borrowed/SKILL.md")).unwrap();
    // A hard link reaches past the folder as a link does.
    std::fs::create_dir_all(level.join("twinned")).unwrap();
    std::fs::hard_link(outside.join("SKILL.md"), level.join("twinned/SKILL.md")).unwrap();

    assert_eq!(
        scan(tmp.path(), Path::new("skills")),
        [
            Err((level.join("borrowed"), Missing::Link)),
            Err((level.join("twinned"), Missing::Link)),
        ]
    );
    assert_eq!(
        crate::read_skill_md(tmp.path(), &level.join("twinned/SKILL.md")),
        Err(crate::Refusal::FileLink)
    );
}

#[cfg(unix)]
#[test]
fn a_level_any_part_of_whose_path_is_a_link_is_left_out_as_a_link_whatever_it_leads_to() {
    let tmp = tempfile::tempdir().unwrap();
    let elsewhere = tmp.path().join("elsewhere");
    skill(
        &elsewhere.join("skills"),
        "far",
        "---\nname: far\ndescription: Far away.\n---\n",
    );
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(root.join("workspaces")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("workspaces/linked")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("skills"), root.join("skills")).unwrap();

    for rel in ["skills", "workspaces/linked/skills"] {
        assert_eq!(
            scan(&root, Path::new(rel)),
            [Err((root.join(rel), Missing::Link))],
            "{rel}"
        );
    }

    let plugin = tmp.path().join("plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("skills"), plugin.join(DIR_IN_BUNDLE)).unwrap();
    assert_eq!(
        in_bundle(&plugin),
        None,
        "a bundle's linked skills folder is not read"
    );
}

#[test]
fn a_skill_md_larger_than_purlis_reads_is_left_out_saying_the_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let body = "x".repeat(usize::try_from(crate::MOST_BYTES).unwrap());
    skill(
        &tmp.path().join("skills"),
        "huge",
        &format!("---\nname: huge\ndescription: Huge.\n---\n{body}"),
    );

    let scanned = scan(tmp.path(), Path::new("skills"));
    assert_eq!(
        scanned,
        [Err((tmp.path().join("skills/huge"), Missing::TooLarge))]
    );
    let said = crate::Reason::NotASkill(Missing::TooLarge).to_string();
    assert!(said.contains("(256 KiB)"), "{said}");
    assert!(!said.contains("not a skill"), "{said}");
}

#[test]
fn a_level_of_more_names_than_purlis_reads_is_refused_with_a_sentence() {
    // SK-52 (#3205): the scan side of SK-44's limit. A plane level is the chat's to fill; past
    // the most names purlis reads of one folder it reads none of them and says so once.
    let tmp = tempfile::tempdir().unwrap();
    let level = tmp.path().join("skills");
    skill(
        &level,
        "whole",
        "---\nname: whole\ndescription: Does it.\n---\n",
    );
    for n in 0..crate::held::MOST_NAMES {
        std::fs::write(level.join(format!("f{n}")), "").unwrap();
    }

    let scanned = scan(tmp.path(), Path::new("skills"));

    assert_eq!(scanned, [Err((level, Missing::Crowded))]);
    assert_eq!(
        crate::Reason::NotASkill(Missing::Crowded).to_string(),
        "a folder of more than 2000 names, the most purlis reads of one, so none of its skills \
         is read"
    );
}

#[test]
fn a_level_of_exactly_the_most_names_purlis_reads_is_scanned() {
    let tmp = tempfile::tempdir().unwrap();
    let level = tmp.path().join("skills");
    skill(
        &level,
        "whole",
        "---\nname: whole\ndescription: Does it.\n---\n",
    );
    for n in 1..crate::held::MOST_NAMES {
        std::fs::write(level.join(format!("f{n}")), "").unwrap();
    }

    let names: Vec<String> = scan(tmp.path(), Path::new("skills"))
        .into_iter()
        .map(|one| one.map(|skill| skill.name).map_err(|(_, why)| why).unwrap())
        .collect();

    assert_eq!(names, ["whole"]);
}
