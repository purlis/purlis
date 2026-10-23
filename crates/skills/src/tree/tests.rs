// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The names purlis writes from a source's tree, and the folders it reads as skills.

use super::*;

/// One `ls-tree -r -z` record, its blob's size written as its object's name so the test's
/// `size_of` reads it back.
fn record(mode: &str, kind: &str, size: &str, path: &str) -> Vec<u8> {
    let size: u64 = size.parse().unwrap_or(0);
    let mut out = format!("{mode} {kind} {size:040x}\t").into_bytes();
    out.extend_from_slice(path.as_bytes());
    out.push(0);
    out
}

fn listing(records: &[Vec<u8>]) -> Vec<u8> {
    records.concat()
}

/// The files and left-out entries of `listing`.
fn read(listing: &[u8]) -> (Vec<Entry>, Vec<LeftOut>) {
    read_listing(records(listing).unwrap(), |oid| {
        u64::from_str_radix(oid, 16).ok()
    })
}

#[test]
fn a_plain_name_is_printable_ascii_and_never_a_way_out() {
    for good in ["SKILL.md", "run.sh", ".hidden", "a b", "x-1_2.txt"] {
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
        ".github",
        ".GitKeep",
        "caf\u{e9}",
        "a\u{0}b",
        "tab\t",
    ] {
        assert!(!plain_name(bad), "{bad:?}");
    }
}

#[test]
fn links_submodules_and_bad_names_are_left_out_with_why() {
    let (entries, left) = read(&listing(&[
        record("100644", "blob", "10", "skills/a/SKILL.md"),
        record("100755", "blob", "10", "skills/a/run.sh"),
        record("120000", "blob", "9", "skills/a/escape"),
        record("160000", "commit", "-", "skills/a/vendor"),
        record("100644", "blob", "3", "skills/a/../../evil"),
        record("100644", "blob", "3", "skills/a/.git/hooks/post-checkout"),
        record("100644", "blob", "3", "skills/a/.gitattributes"),
        record("100644", "blob", "2000000", "skills/a/big.bin"),
    ]));
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
    let (entries, left) = read(&listing(&[
        record("100644", "blob", "1", "a/Scripts/x.sh"),
        record("100644", "blob", "1", "a/scripts/y.sh"),
        record("100644", "blob", "1", "a/SKILL.md"),
        record("100644", "blob", "1", "a/skill.md"),
    ]));
    let written: Vec<String> = entries.iter().map(Entry::shown).collect();
    assert_eq!(written, vec!["a/Scripts/x.sh", "a/SKILL.md"]);
    assert!(left.iter().all(|l| l.why == Why::Case));
    assert_eq!(left.len(), 2);
}

#[test]
fn skill_folders_are_found_under_the_folder_and_a_nested_one_belongs_to_its_outer() {
    let (entries, _) = read(&listing(&[
        record("100644", "blob", "1", "README.md"),
        record("100644", "blob", "1", "skills/deploy/SKILL.md"),
        record("100644", "blob", "1", "skills/deploy/inner/SKILL.md"),
        record("100644", "blob", "1", "skills/review/SKILL.md"),
        record("100644", "blob", "1", "other/x/SKILL.md"),
    ]));
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
    let (entries, left) = read(&listing(&[
        record("100644", "blob", "1", "SKILL.md"),
        record("120000", "blob", "1", "link"),
    ]));
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
fn a_listing_git_would_not_write_is_refused_whole() {
    assert_eq!(records(b"100644 blob abc\tx\0"), Err(Unread::NotGit));
    assert_eq!(records(b"100644 blob\tx\0"), Err(Unread::NotGit));
    assert_eq!(records(b"no tab here\0"), Err(Unread::NotGit));
}

#[test]
fn a_tree_past_the_most_entries_is_refused_before_any_is_read() {
    let one = record("100644", "blob", "1", "a/SKILL.md");
    let many = one.repeat(100_000);
    assert_eq!(records(&many), Err(Unread::TooManyEntries));
}

#[test]
fn the_most_entries_a_tree_may_list_fold_in_one_pass() {
    // Under the entry cap's ceiling: the fold is a set, so this is quick whatever the count.
    let names: Vec<Vec<u8>> = (0..MOST_ENTRIES)
        .map(|n| record("100644", "blob", "1", &format!("d{}/f{n}", n % 100)))
        .collect();
    let started = std::time::Instant::now();
    let (entries, left) = read(&listing(&names));
    assert_eq!(entries.len(), MOST_ENTRIES);
    assert!(left.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    // And a case clash is still caught among them.
    let mut clashing = names.clone();
    clashing.push(record("100644", "blob", "1", "D7/f7"));
    clashing.remove(0);
    let (_, left) = read(&listing(&clashing));
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].why, Why::Case);
}

#[test]
fn a_blob_the_fetch_left_out_is_too_large() {
    let (entries, left) = read_listing(
        records(&record("100644", "blob", "5", "a/big.bin")).unwrap(),
        |_| None,
    );
    assert!(entries.is_empty());
    assert_eq!(left[0].why, Why::TooLarge);
}
