// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The host-held owner facts** (#7748): `context.owner` comes from records only the app seals,
//! so a chat cannot forge it. The records are sealed for real, in a machine of the test's own.

use std::path::{Path, PathBuf};

use super::*;
use crate::policy::force::tests::{Machine, machine};
use crate::secrets::seal;

/// The person's first chat, and the line below it: `TASK` and `SIBLING` are its tasks,
/// `GRANDCHILD` is `TASK`'s, and `COUSIN` is `SIBLING`'s.
const ROOT: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S00";
const TASK: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S0A";
const SIBLING: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S0B";
const GRANDCHILD: &str = "01J9ZQ3V5N8X4T2K7M6P0R1SAA";
const COUSIN: &str = "01J9ZQ3V5N8X4T2K7M6P0R1SBA";
/// A chat of another lineage.
const STRANGER: &str = "01J9ZQ3V5N8X4T2K7M6P0R1SZZ";

fn branch(name: &str) -> Owned<'_> {
    Owned::Branch {
        host: "github.com",
        repo: "acme/app",
        branch: name,
    }
}

/// A plane on `machine`, with a `.git`, registered as the app opens it.
fn plane(machine: &Machine) -> PathBuf {
    let root = machine.dir.path().join("plane");
    std::fs::create_dir_all(root.join(".git")).expect("a plane");
    let root = std::fs::canonicalize(&root).expect("a real path");
    answers::register(&root).expect("registered");
    root
}

/// The line above, dispatched as the app records each dispatch.
fn dispatched(root: &Path) {
    for (task, by) in [
        (TASK, ROOT),
        (SIBLING, ROOT),
        (GRANDCHILD, TASK),
        (COUSIN, SIBLING),
    ] {
        assert!(record_dispatch(root, task, by).expect("recorded"));
    }
}

/// The person's press in the window.
#[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
fn press() -> Press {
    Press::of_the_persons_press()
}

/// Every owner record's file in the sealed store.
fn owner_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for item in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = item.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == answers::OWNER)
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &crate::sandbox::local::config_home().expect("the test's config home"),
        &mut out,
    );
    out
}

fn owner_of(root: &Path, actor: &str, owned: &Owned<'_>) -> Owner {
    let records = Records::read(root);
    owner(actor, records.holder(owned), &records)
}

#[test]
fn the_owner_values_are_self_task_asker_other_or_none() {
    let words: Vec<&str> = Owner::ALL.into_iter().map(Owner::word).collect();
    assert_eq!(words, ["self", "task", "asker", "other", "none"]);

    let m = machine();
    let root = plane(&m);
    dispatched(&root);
    for (chat, folder) in [
        (TASK, "task"),
        (ROOT, "root"),
        (GRANDCHILD, "grandchild"),
        (SIBLING, "sibling"),
        (COUSIN, "cousin"),
        (STRANGER, "stranger"),
    ] {
        let path = root.join(folder);
        assert!(record_owner(&root, &Owned::Folder(&path), chat).expect("recorded"));
    }
    let records = Records::read(&root);
    let of = |actor: &str, folder: &str| {
        let path = root.join(folder);
        owner(actor, records.holder(&Owned::Folder(&path)), &records)
    };

    // The actor's own line, up and down, at any depth.
    assert_eq!(of(TASK, "task"), Owner::Actor);
    assert_eq!(of(TASK, "grandchild"), Owner::Task);
    assert_eq!(of(ROOT, "grandchild"), Owner::Task);
    assert_eq!(of(TASK, "root"), Owner::Asker);
    assert_eq!(of(GRANDCHILD, "root"), Owner::Asker);
    // A sibling and a cousin are reached only through a common ancestor: never lineage.
    assert_eq!(of(TASK, "sibling"), Owner::Other);
    assert_eq!(of(GRANDCHILD, "cousin"), Owner::Other);
    assert_eq!(of(TASK, "stranger"), Owner::Other);
    // No record: the person's, or not recorded.
    assert_eq!(of(TASK, "nobody's"), Owner::Nobody);
    assert_eq!(owner(TASK, Some(""), &records), Owner::Nobody);

    // A port's server or a browser, by the host's registry's opener.
    assert_eq!(owner(GRANDCHILD, Some(TASK), &records), Owner::Asker);
    assert_eq!(owner(TASK, Some(COUSIN), &records), Owner::Other);
}

#[test]
fn a_record_a_chat_edited_never_makes_it_an_owner() {
    let m = machine();
    let root = plane(&m);
    assert!(record_owner(&root, &branch("work"), STRANGER).expect("recorded"));
    let folder = root.join("piece");
    assert!(record_owner(&root, &Owned::Folder(&folder), STRANGER).expect("recorded"));
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Other);

    // The files a chat can reach, and that once said who owns a branch or a folder, name it:
    // the forge route's own list and the piece log. Neither is read.
    let config = m.dir.path().join("config");
    crate::forge::createdbranches::record(
        &config,
        TASK,
        &[("github.com".into(), "acme/app".into(), "work".into())],
    )
    .expect("written");
    std::fs::write(
        root.join("pieces.jsonl"),
        format!("{{\"event\":\"claimed\",\"chat\":\"{TASK}\"}}\n"),
    )
    .expect("written");
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Other);

    // An unsandboxed chat edits the sealed records to name itself: they fail their seal and are
    // no record, so the owner is "none", never "self".
    let files = owner_files();
    assert_eq!(files.len(), 2, "{files:?}");
    for file in &files {
        let text = std::fs::read_to_string(file).expect("a record");
        std::fs::write(file, text.replace(STRANGER, TASK)).expect("edited");
    }
    let standing = answers::standing(&root);
    assert!(!standing.is_deny(), "{:?}", standing.state);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Nobody);
    assert_eq!(
        owner_of(&root, TASK, &Owned::Folder(&folder)),
        Owner::Nobody
    );

    // A process that is not the app cannot seal a record, and verifies none.
    seal::set_for_this_test(None);
    assert!(record_owner(&root, &branch("mine"), TASK).is_err());
    assert_eq!(owner_of(&root, TASK, &branch("mine")), Owner::Nobody);
}

#[test]
fn an_owner_record_moves_only_by_hand_over_or_the_persons_press() {
    let m = machine();
    let root = plane(&m);
    dispatched(&root);
    assert!(record_owner(&root, &branch("work"), GRANDCHILD).expect("recorded"));

    // A second record, whoever asks for it, moves nothing: the first stands.
    assert!(!record_owner(&root, &branch("work"), TASK).expect("asked"));
    assert!(!record_owner(&root, &branch("work"), STRANGER).expect("asked"));
    // Nor does a dispatch record written again, or an answer the person's press adds or takes
    // out by the owner kind.
    assert!(!record_dispatch(&root, GRANDCHILD, STRANGER).expect("asked"));
    let text = owner_files()
        .iter()
        .map(|file| std::fs::read_to_string(file).expect("a record"))
        .find(|text| text.contains("branch github.com/acme/app#work"))
        .expect("the branch's record");
    let entry: serde_json::Value = serde_json::from_str(&text).expect("an entry");
    let recorded = entry["text"].as_str().expect("its text").to_owned();
    assert!(
        answers::commit(&root, |draft| draft.remove(
            &press(),
            answers::OWNER,
            &recorded
        ))
        .is_err()
    );
    assert!(
        answers::commit(&root, |draft| {
            draft.add(
                &press(),
                answers::OWNER,
                &recorded.replace(GRANDCHILD, STRANGER),
            );
        })
        .is_err()
    );
    assert_eq!(owner_of(&root, GRANDCHILD, &branch("work")), Owner::Actor);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Task);

    // A finished task's hand-over gives it to the chat that dispatched it, and to nobody else.
    assert_eq!(hand_over(&root, GRANDCHILD).expect("handed over"), 1);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Actor);
    assert_eq!(owner_of(&root, GRANDCHILD, &branch("work")), Owner::Asker);
    assert_eq!(owner_of(&root, SIBLING, &branch("work")), Owner::Other);
    // A chat no dispatch record names an asker for hands nothing over.
    assert!(record_owner(&root, &branch("theirs"), STRANGER).expect("recorded"));
    assert_eq!(hand_over(&root, STRANGER).expect("asked"), 0);
    assert_eq!(owner_of(&root, STRANGER, &branch("theirs")), Owner::Actor);

    // The person's press of "Let this chat push it" gives it to the chat named.
    let_this_chat(&press(), &root, &branch("work"), SIBLING).expect("let");
    assert_eq!(owner_of(&root, SIBLING, &branch("work")), Owner::Actor);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Other);
}
