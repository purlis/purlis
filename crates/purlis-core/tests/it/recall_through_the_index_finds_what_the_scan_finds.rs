// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The project index (ADR 0079, #5498) is a faster way to answer recall, never a way to answer
//! a different question. On a fixture project with every kind of base (two workspaces, one of
//! them LOCAL, two personas, the shared store, nested refs), recall through the index finds
//! exactly the memories the file scan finds, for every reach. Only the order may differ: the
//! index ranks by BM25.
//!
//! Beside it, what makes the index safe to have: deleting it and building it again gives the
//! same answers, a secret-shaped span never reaches its file, and a link out of the project is
//! never indexed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use purlis_core::projectindex::{self, Writer};
use purlis_core::recall::{self, Ask, Workspaces};

/// A GitHub token's shape: the prefix and 36 characters. Not a real one, and put together at
/// run time so no token is written in this file.
fn token() -> String {
    ["ghp", "_", "Zq7Xw3Vb9Nc5Mk1Lp8Rt2Ys6Ud4Fh0Gj3Ka5"].concat()
}

fn memory(root: &Path, at: &str, title: &str, day: &str, body: &str) {
    let path = root.join(at);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!("# {title}\n\n_{day} 09:14 · persistent_\n\n{body}\n"),
    )
    .unwrap();
}

/// A project with a base of every kind, and a link out of it.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    // `scratch` is LOCAL: this clone's alone, ignored by git.
    std::fs::write(root.join(".gitignore"), "/workspaces/scratch/\n").unwrap();
    for persona in ["devops", "writer"] {
        std::fs::create_dir_all(root.join("personas").join(persona)).unwrap();
        std::fs::write(
            root.join("personas").join(persona).join("persona.md"),
            "---\nrole: a role\n---\n",
        )
        .unwrap();
    }
    let m = |at: &str, title: &str, day: &str, body: &str| memory(&root, at, title, day, body);
    m(
        "workspaces/alpha/memory/20260302-091400-deploy.md",
        "Deploy pipeline",
        "2026-03-02",
        "The deploy pipeline runs on mondays. snake_case_words stay whole.",
    );
    m(
        "workspaces/alpha/memory/20260303-091400-cafe.md",
        "Café opening hours",
        "2026-03-03",
        "The café opens at nine; deployment freezes on fridays.",
    );
    m(
        "workspaces/beta/memory/20260301-091400-mondays.md",
        "Mondays in beta",
        "2026-03-01",
        "Beta deploys on mondays too.",
    );
    m(
        "workspaces/scratch/memory/20260304-091400-local.md",
        "Local notes",
        "2026-03-04",
        "A LOCAL workspace's deploy notes.",
    );
    m(
        "personas/devops/memory/20260305-091400-oncall.md",
        "On-call rota",
        "2026-03-05",
        "Pager duty for the deploy pipeline.",
    );
    m(
        "personas/writer/memory/20260306-091400-style.md",
        "House style",
        "2026-03-06",
        "Sentence case; no deploy jargon.",
    );
    m(
        "personas/_shared/memory/20260307-091400-kiwi.md",
        "Shared kiwi",
        "2026-03-07",
        "Everyone knows the kiwi and the deploy word.",
    );
    // A memory with no date anywhere.
    std::fs::write(
        root.join("personas/_shared/memory/undated.md"),
        "# Undated deploy fact\n\nno stamp at all\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("personas/devops/refs/runbooks/deep")).unwrap();
    std::fs::write(
        root.join("personas/devops/refs/runbooks/deep/rollback.md"),
        "# Rollback\n\nHow to roll a deploy back.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("personas/devops/refs/top.md"),
        "# Top ref\n\nmondays and deploys\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("personas/_shared/refs")).unwrap();
    std::fs::write(
        root.join("personas/_shared/refs/glossary.md"),
        "# Glossary\n\nkiwi: a bird. deploy: to ship.\n",
    )
    .unwrap();
    // A link out of the project, whose target matches every query below.
    let outside = dir.path().join("outside.md");
    std::fs::write(
        &outside,
        "# Outside deploy secret\n\n_2026-03-08 09:14 · persistent_\n\noutsider mondays kiwi deploy\n",
    )
    .unwrap();
    for at in [
        "workspaces/alpha/memory/leak.md",
        "personas/_shared/memory/leak.md",
        "personas/devops/refs/leak.md",
    ] {
        std::os::unix::fs::symlink(&outside, root.join(at)).unwrap();
    }
    (dir, root)
}

/// Every reach a caller can ask for, by name.
fn reaches() -> Vec<(&'static str, Ask)> {
    let ask = |scopes: &[&str], workspaces: Option<Workspaces>, persona: Option<&str>| Ask {
        scopes: scopes.iter().map(|s| s.to_string()).collect(),
        workspaces,
        persona: persona.map(str::to_string),
        session: "s1".into(),
        query: None,
        limit: 0,
        since: None,
    };
    let default = &recall::DEFAULT_SCOPES[..];
    let alpha = || Some(Workspaces::One("alpha".into()));
    vec![
        ("the default scopes", ask(default, alpha(), Some("devops"))),
        (
            "every workspace",
            ask(default, Some(Workspaces::All), Some("devops")),
        ),
        ("one workspace", ask(&["workspace"], alpha(), None)),
        (
            "a LOCAL workspace",
            ask(
                &["workspace"],
                Some(Workspaces::One("scratch".into())),
                None,
            ),
        ),
        ("a persona", ask(&["persona"], None, Some("writer"))),
        ("the shared store", ask(&["shared"], None, None)),
        ("refs", ask(&["refs"], None, Some("devops"))),
        ("refs with no persona", ask(&["refs"], None, None)),
        ("at the project root", ask(default, None, Some("writer"))),
    ]
}

/// `cafe` and `case` are the scan's word split: `cafe` is not a prefix of `café`, and `case`
/// starts no word of `snake_case_words`, so neither finds those memories.
const QUERIES: [Option<&str>; 9] = [
    Some("deploy"),
    Some("dep"),
    Some("mondays kiwi"),
    Some("snake_case"),
    Some("café"),
    Some("cafe"),
    Some("case"),
    Some("the"),
    None,
];

/// What one answer found, as a set of `label path`. Order is the index's to choose.
fn found(got: &recall::Recalled) -> BTreeSet<String> {
    got.hits
        .iter()
        .map(|h| format!("{} {}", h.label, h.path.display()))
        .collect()
}

/// Every reach and query whose answer through the index differs from the scan's.
fn differences(root: &Path) -> Vec<String> {
    let mut differ = Vec::new();
    for (name, base) in reaches() {
        for query in QUERIES {
            for since in [None, Some("2026-03-04")] {
                let ask = Ask {
                    query: query.map(str::to_string),
                    since: since.map(|d| d.parse().unwrap()),
                    ..base.clone()
                };
                let scanned = recall::scan(root, &ask);
                let indexed = recall::recall(root, &ask);
                assert!(
                    indexed.index.is_some(),
                    "{name} {query:?} was not answered from the index"
                );
                if found(&scanned) != found(&indexed)
                    || (scanned.undated, scanned.undated_refs)
                        != (indexed.undated, indexed.undated_refs)
                {
                    differ.push(format!(
                        "{name}, {query:?} since {since:?}:\n  scan  {:?} ({}, {})\n  index {:?} ({}, {})",
                        found(&scanned),
                        scanned.undated,
                        scanned.undated_refs,
                        found(&indexed),
                        indexed.undated,
                        indexed.undated_refs,
                    ));
                }
            }
        }
    }
    differ
}

#[test]
fn for_every_reach_recall_through_the_index_finds_what_the_scan_finds() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let _writer = Writer::open(&root).unwrap();

    let differ = differences(&root);

    assert!(differ.is_empty(), "{}", differ.join("\n"));
}

#[test]
fn the_index_ranks_and_the_scan_still_answers_without_it() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let ask = Ask {
        query: Some("deploy".into()),
        ..reaches().remove(0).1
    };

    let without = recall::recall(&root, &ask);
    let _writer = Writer::open(&root).unwrap();
    let with = recall::recall(&root, &ask);

    assert!(
        without.index.is_none(),
        "no index yet: the files are scanned"
    );
    assert!(!without.hits.is_empty());
    assert!(with.index.is_some());
    let first_two: BTreeSet<&str> = with.hits[..2].iter().map(|h| h.title.as_str()).collect();
    assert_eq!(
        first_two,
        BTreeSet::from(["Deploy pipeline", "Undated deploy fact"]),
        "the two with the word in their title come first: {:?}",
        with.hits
    );
}

#[test]
fn deleting_every_index_file_and_starting_again_gives_the_same_answers() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let answers = |root: &Path| -> Vec<BTreeSet<String>> {
        reaches()
            .into_iter()
            .flat_map(|(_, base)| {
                QUERIES.map(|query| {
                    found(&recall::recall(
                        root,
                        &Ask {
                            query: query.map(str::to_string),
                            ..base.clone()
                        },
                    ))
                })
            })
            .collect()
    };
    let first = {
        let _writer = Writer::open(&root).unwrap();
        answers(&root)
    };

    std::fs::remove_dir_all(projectindex::home(&root)).unwrap();
    let scanned = answers(&root);
    let _writer = Writer::open(&root).unwrap();
    let rebuilt = answers(&root);

    assert_eq!(first, scanned, "with no index, the scan answers the same");
    assert_eq!(first, rebuilt, "a rebuilt index answers the same");
}

/// Every byte of every file the index keeps.
fn index_bytes(root: &Path) -> Vec<u8> {
    let mut all = Vec::new();
    let mut dirs = vec![projectindex::home(root)];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                all.extend(std::fs::read(path).unwrap());
            }
        }
    }
    all
}

fn holds(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[test]
fn a_secret_shaped_span_never_reaches_the_index() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let token = token();
    let token = token.as_str();
    memory(
        &root,
        "workspaces/alpha/memory/20260309-091400-token.md",
        &format!("Token {token}"),
        "2026-03-09",
        &format!("pasted by hand: {token} for the deploy"),
    );

    let writer = Writer::open(&root).unwrap();
    drop(writer);
    let got = recall::recall(
        &root,
        &Ask {
            query: Some(token[..12].to_string()),
            ..reaches().remove(0).1
        },
    );

    let bytes = index_bytes(&root);
    assert!(!bytes.is_empty());
    assert!(
        !holds(&bytes, &token.to_lowercase()) && !holds(&bytes, token),
        "the token is in the index's files"
    );
    assert!(
        !holds(&bytes, &token.to_lowercase()[4..16]),
        "a piece of the token is in the index's files"
    );
    assert!(got.index.is_some());
    assert!(got.hits.is_empty(), "{:?}", got.hits);
    let by_its_other_words = recall::recall(
        &root,
        &Ask {
            query: Some("pasted".into()),
            ..reaches().remove(0).1
        },
    );
    assert_eq!(
        by_its_other_words.hits.len(),
        1,
        "the memory itself is indexed: {:?}",
        by_its_other_words.hits
    );
}

#[test]
fn a_link_out_of_the_project_is_never_indexed() {
    purlis_core::unsteered!();
    let (_dir, root) = project();

    let writer = Writer::open(&root).unwrap();
    drop(writer);

    assert!(
        !holds(&index_bytes(&root), "outsider"),
        "the outside file's words are in the index"
    );
    for (name, base) in reaches() {
        let got = recall::recall(
            &root,
            &Ask {
                query: Some("outsider".into()),
                ..base
            },
        );
        assert!(got.index.is_some());
        assert!(got.hits.is_empty(), "{name}: {:?}", got.hits);
    }
}
