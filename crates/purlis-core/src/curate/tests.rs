// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

#[test]
fn a_score_is_rounded_to_two_places_as_python_rounds_it() {
    assert_eq!(py_float(round2(2.0 / 3.0)), "0.67");
    assert_eq!(py_float(round2(1.0)), "1.0");
    assert_eq!(py_float(round2(0.5)), "0.5");
    // 0.125 is exact in binary, and Python's round goes to the even neighbour.
    assert_eq!(py_float(round2(0.125)), "0.12");
    assert_eq!(py_float(round2(0.375)), "0.38");
    // `str(float('inf'))` is `inf`, with nothing appended.
    assert_eq!(py_float(f64::INFINITY), "inf");
    assert_eq!(py_float(f64::NEG_INFINITY), "-inf");
}

#[test]
fn a_snapshot_is_never_a_rule_however_imperative_it_sounds() {
    assert_eq!(
        rule_score("Standing rule: never deploy. Deployed state is fine."),
        0
    );
    assert_eq!(
        rule_score("This is a standing rule. You must not deploy."),
        8
    );
}

fn day(text: &str) -> chrono::NaiveDate {
    text.parse().unwrap()
}

/// A store with a bit of everything curation looks for, and the plane it sits in.
///
/// Every expectation below it was read off Python's `curate.report` and
/// `curate.apply_safe` run over the same files on the same day.
fn store() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let mem = root.join("personas/p/memory");
    std::fs::create_dir_all(&mem).unwrap();
    for (name, text) in [
        // `a` and `b` are one body under two titles; `c` is near both of them.
        (
            "20250601-a.md",
            "# Widget build\nThe widget build passes on every runner\n",
        ),
        (
            "20250601-b.md",
            "# Widget build copy\nThe widget build passes on every runner\n",
        ),
        (
            "20250601-c.md",
            "# Widget build\nThe widget build passes on every runner today\n",
        ),
        // Exactly ninety days old on the day asked, and a strong rule.
        (
            "d.md",
            "# Dated\n_2025-06-03 · x_\nA standing rule: never guess.\n",
        ),
        // A rule signal of exactly two, which is enough, and of one, which is not.
        ("e.md", "# Habit\nAlways run the suite first.\n"),
        ("f.md", "# Caution\nDo not panic.\n"),
    ] {
        std::fs::write(mem.join(name), text).unwrap();
    }
    std::fs::write(
        mem.join(memstore::INDEX),
        "# Memory Index\n\n- [Widget build](20250601-a.md)\n- [Gone](gone.md)\n",
    )
    .unwrap();
    (dir, root, mem)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn a_report_finds_every_kind_of_candidate_the_python_report_finds() {
    let (_d, root, mem) = store();

    let rep = report(&root, &mem, 90, 0.5, day("2025-09-01")).unwrap();

    assert_eq!(
        rep,
        Report {
            total: 6,
            exact_dups: vec![names(&["20250601-a.md", "20250601-b.md"])],
            // The exact pair is tier 1's, and not proposed again as a near one.
            near_dups: vec![
                (0.83, "20250601-a.md".into(), "20250601-c.md".into()),
                (0.71, "20250601-b.md".into(), "20250601-c.md".into()),
            ],
            // Ninety-two days is past ninety; ninety is not.
            stale: vec![
                ("20250601-a.md".into(), "2025-06-01".into(), 92),
                ("20250601-b.md".into(), "2025-06-01".into(), 92),
                ("20250601-c.md".into(), "2025-06-01".into(), 92),
            ],
            orphans: names(&["gone.md"]),
            missing: names(&["20250601-b.md", "20250601-c.md", "d.md", "e.md", "f.md"]),
            rules: vec![
                ("d.md".into(), "Dated".into(), 7),
                ("e.md".into(), "Habit".into(), 2),
            ],
        }
    );
}

#[test]
fn only_the_exact_pair_itself_is_left_out_of_the_near_duplicates() {
    // `b` is near both halves of the exact pair `c`/`d`. The pair tier 1 handles is
    // dropped from the proposals; a pair that merely shares a name with it is not. Read
    // off Python's `curate.report` over the same three files.
    let (_d, root, _) = store();
    let mem = root.join("personas/q/memory");
    std::fs::create_dir_all(&mem).unwrap();
    for (name, text) in [
        (
            "b.md",
            "# B\nThe widget build passes on every runner today\n",
        ),
        ("c.md", "# C\nThe widget build passes on every runner\n"),
        ("d.md", "# D\nThe widget build passes on every runner\n"),
    ] {
        std::fs::write(mem.join(name), text).unwrap();
    }

    let rep = report(&root, &mem, 90, 0.5, day("2025-09-01")).unwrap();

    assert_eq!(rep.exact_dups, vec![names(&["c.md", "d.md"])]);
    assert_eq!(
        rep.near_dups,
        vec![
            (0.83, "b.md".into(), "c.md".into()),
            (0.83, "b.md".into(), "d.md".into()),
        ]
    );
}

#[test]
fn a_store_that_cannot_be_listed_is_named_rather_than_reported_empty() {
    let (_d, root, mem) = store();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&mem, std::fs::Permissions::from_mode(0o000)).unwrap();
        let listed = std::fs::read_dir(&mem).is_ok();
        let got = report(&root, &mem, 90, 0.5, day("2025-09-01"));
        let applied = apply_safe(&root, &mem, day("2025-09-01"));
        std::fs::set_permissions(&mem, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Root reads through a mode of 000, and the question then does not arise.
        if !listed {
            assert_eq!(got.unwrap_err().len(), 1);
            assert_eq!(applied.unwrap_err().len(), 1);
        }
    }
    let _ = (root, mem);
}

#[test]
fn applying_archives_the_redundant_copy_and_links_what_the_index_misses_once() {
    let (_d, root, mem) = store();

    assert_eq!(
        apply_safe(&root, &mem, day("2025-09-01")).unwrap(),
        [
            "archived exact-duplicate 20250601-b.md (kept 20250601-a.md)",
            "repaired index: linked 4 unindexed memories",
        ]
    );
    assert!(mem.join("archive/20250601-b.md").is_file());
    let index = std::fs::read_to_string(mem.join(memstore::INDEX)).unwrap();
    assert!(
        index.ends_with(
            "- [Widget build](20250601-c.md)\n- [Dated](d.md)\n- [Habit](e.md)\n\
             - [Caution](f.md)\n"
        ),
        "{index}"
    );

    assert_eq!(
        apply_safe(&root, &mem, day("2025-09-01")).unwrap(),
        Vec::<String>::new(),
        "nothing left that is safe to do"
    );
}

#[cfg(unix)]
#[test]
fn an_index_that_leaves_the_plane_is_reported_and_never_appended_to() {
    let (_d, root, mem) = store();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("elsewhere.md");
    std::fs::write(&target, "untouched\n").unwrap();
    std::fs::remove_file(mem.join(memstore::INDEX)).unwrap();
    std::os::unix::fs::symlink(&target, mem.join(memstore::INDEX)).unwrap();

    let actions = apply_safe(&root, &mem, day("2025-09-01")).unwrap();

    assert_eq!(actions.len(), 2, "{actions:?}");
    assert!(
        actions[1].starts_with("index NOT repaired — "),
        "{actions:?}"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "untouched\n");
}

#[test]
fn what_apply_would_do_is_said_before_it_is_done() {
    let mut rep = Report {
        exact_dups: vec![names(&["a.md", "b.md", "c.md"]), names(&["d.md", "e.md"])],
        missing: names(&["m1.md", "m2.md", "m3.md"]),
        ..Report::default()
    };
    assert_eq!(
        pending_auto(&rep),
        [
            "collapse 2 exact-duplicate groups — archives 3 redundant copies, reversible",
            "repair index: link 3 memories that exist but aren't listed \
             (m1.md, m2.md, m3.md)",
        ]
    );
    rep.missing.push("m4.md".into());
    rep.exact_dups.clear();
    assert_eq!(
        pending_auto(&rep),
        [
            "repair index: link 4 memories that exist but aren't listed \
          (m1.md, m2.md, m3.md, …)"
        ]
    );
    assert_eq!(pending_auto(&Report::default()), Vec::<String>::new());
}

#[test]
fn what_needs_a_person_is_proposed_in_pythons_words() {
    let long = "x".repeat(70);
    let rep = Report {
        near_dups: vec![
            (0.83, "a.md".into(), "c.md".into()),
            (1.0, "x.md".into(), "y.md".into()),
        ],
        stale: vec![
            ("old.md".into(), "2020-01-01".into(), 400),
            ("less.md".into(), "2021-01-01".into(), 300),
        ],
        rules: vec![("r.md".into(), long.clone(), 7)],
        orphans: names(&["g1.md", "g2.md", "g3.md", "g4.md", "g5.md", "g6.md"]),
        ..Report::default()
    };
    assert_eq!(
        proposals(&rep),
        [
            "merge near-duplicates (0.83): a.md + c.md → one canonical memory?".to_string(),
            "merge near-duplicates (1.0): x.md + y.md → one canonical memory?".to_string(),
            "archive 2 stale memories (age-based, oldest 400d — review first: age ≠ \
             obsolete)?"
                .to_string(),
            format!(
                "promote to charter? [r.md] \"{}\" (rule-signal 7)",
                &long[..60]
            ),
            "index lists 6 missing files — stale links to prune: g1.md, g2.md, g3.md, \
             g4.md, g5.md"
                .to_string(),
        ]
    );
    assert_eq!(proposals(&Report::default()), Vec::<String>::new());
}
