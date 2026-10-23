// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The names purlis writes from a source's tree, and the folders it reads as skills.

use super::*;

/// One `ls-tree -r -z -l` record.
fn record(mode: &str, kind: &str, size: &str, path: &str) -> Vec<u8> {
    let mut out = format!("{mode} {kind} {:040x} {size:>7}\t", 1).into_bytes();
    out.extend_from_slice(path.as_bytes());
    out.push(0);
    out
}

fn listing(records: &[Vec<u8>]) -> Vec<u8> {
    records.concat()
}

#[test]
fn a_plain_name_is_printable_ascii_and_never_a_way_out() {
    for good in ["SKILL.md", "run.sh", ".github", "a b", "x-1_2.txt"] {
        assert!(plain_name(good), "{good}");
    }
    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        "c:",
        "x.",
        "x ",
        ".git",
        ".GIT",
        ".Git",
        "git~1",
        ".gitattributes",
        ".GitModules",
        ".gitignore",
        "caf\u{e9}",
        "a\u{0}b",
        "tab\t",
    ] {
        assert!(!plain_name(bad), "{bad:?}");
    }
}

#[test]
fn links_submodules_and_bad_names_are_left_out_with_why() {
    let (entries, left) = read_listing(&listing(&[
        record("100644", "blob", "10", "skills/a/SKILL.md"),
        record("100755", "blob", "10", "skills/a/run.sh"),
        record("120000", "blob", "9", "skills/a/escape"),
        record("160000", "commit", "-", "skills/a/vendor"),
        record("100644", "blob", "3", "skills/a/../../evil"),
        record("100644", "blob", "3", "skills/a/.git/hooks/post-checkout"),
        record("100644", "blob", "3", "skills/a/.gitattributes"),
        record("100644", "blob", "2000000", "skills/a/big.bin"),
    ]))
    .unwrap();
    let written: Vec<String> = entries.iter().map(Entry::shown).collect();
    assert_eq!(written, vec!["skills/a/SKILL.md", "skills/a/run.sh"]);
    assert!(entries[1].executable);
    let why: Vec<(&str, Why)> = left.iter().map(|l| (l.path.as_str(), l.why)).collect();
    assert_eq!(
        why,
        vec![
            ("skills/a/escape", Why::Link),
            ("skills/a/vendor", Why::Submodule),
            ("skills/a/../../evil", Why::Name),
            ("skills/a/.git/hooks/post-checkout", Why::Name),
            ("skills/a/.gitattributes", Why::Name),
            ("skills/a/big.bin", Why::TooLarge),
        ]
    );
}

#[test]
fn of_two_names_one_disk_would_fold_together_the_first_is_kept() {
    let (entries, left) = read_listing(&listing(&[
        record("100644", "blob", "1", "a/Scripts/x.sh"),
        record("100644", "blob", "1", "a/scripts/y.sh"),
        record("100644", "blob", "1", "a/SKILL.md"),
        record("100644", "blob", "1", "a/skill.md"),
    ]))
    .unwrap();
    let written: Vec<String> = entries.iter().map(Entry::shown).collect();
    assert_eq!(written, vec!["a/Scripts/x.sh", "a/SKILL.md"]);
    assert!(left.iter().all(|l| l.why == Why::Case));
    assert_eq!(left.len(), 2);
}

#[test]
fn skill_folders_are_found_under_the_folder_and_a_nested_one_belongs_to_its_outer() {
    let (entries, _) = read_listing(&listing(&[
        record("100644", "blob", "1", "README.md"),
        record("100644", "blob", "1", "skills/deploy/SKILL.md"),
        record("100644", "blob", "1", "skills/deploy/inner/SKILL.md"),
        record("100644", "blob", "1", "skills/review/SKILL.md"),
        record("100644", "blob", "1", "other/x/SKILL.md"),
    ]))
    .unwrap();
    let folder = folder_parts("skills/").unwrap();
    let found = skill_folders(&entries, &folder);
    assert_eq!(
        found,
        vec![
            vec!["skills".to_owned(), "deploy".to_owned()],
            vec!["skills".to_owned(), "review".to_owned()],
        ]
    );
    let files: Vec<Vec<String>> = files_of(&entries, &found[0])
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        files,
        vec![
            vec!["SKILL.md".to_owned()],
            vec!["inner".to_owned(), "SKILL.md".to_owned()],
        ]
    );
}

#[test]
fn a_source_that_is_one_skill_is_its_own_folder() {
    let (entries, left) = read_listing(&listing(&[
        record("100644", "blob", "1", "SKILL.md"),
        record("120000", "blob", "1", "link"),
    ]))
    .unwrap();
    assert_eq!(skill_folders(&entries, &[]), vec![Vec::<String>::new()]);
    assert_eq!(left_in(&left, &[]).len(), 1);
}

#[test]
fn a_folder_with_a_way_out_names_nothing() {
    assert_eq!(folder_parts(""), Some(vec![]));
    assert_eq!(folder_parts("a/b"), Some(vec!["a".into(), "b".into()]));
    assert_eq!(folder_parts("../x"), None);
    assert_eq!(folder_parts(".git"), None);
}

#[test]
fn a_plain_frontmatter_names_its_skill_and_anything_ambiguous_is_refused() {
    assert_eq!(
        plain_identity(
            "---\nname: deploy\ndescription: Ships it.\nallowed-tools: [Read]\n---\nbody\n"
        ),
        Ok(("deploy".into(), "Ships it.".into()))
    );
    for bad in [
        // No fence, or no closing one.
        "name: deploy\n",
        "---\nname: deploy\ndescription: x\n",
        // A second name: YAML and the line reader would each pick one.
        "---\nname: deploy\nname: evil\ndescription: x\n---\n",
        // Quoted keys, anchors, aliases and flow maps.
        "---\n\"name\": evil\nname: deploy\ndescription: x\n---\n",
        "---\nname: &a deploy\ndescription: *a\n---\n",
        "---\n{name: evil, description: x}\n---\n",
        // A `---` inside a value ends the line reader's block early.
        "---\nname: deploy\ndescription: a---b\n---\n",
        // A value YAML reads otherwise than a line does.
        "---\nname: deploy\ndescription: |\n  evil\n---\n",
        "---\nname: deploy # evil\ndescription: x\n---\n",
    ] {
        assert!(plain_identity(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_listing_git_would_not_write_is_refused_whole() {
    assert!(read_listing(b"100644 blob abc 10\tx\0").is_err());
    assert!(read_listing(b"100644 blob\tx\0").is_err());
    assert!(read_listing(&record("100644", "blob", "ten", "x")).is_err());
}
