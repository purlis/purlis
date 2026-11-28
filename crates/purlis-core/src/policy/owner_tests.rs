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
fn the_host_computes_context_owner_from_what_the_act_touches() {
    let m = machine();
    let root = plane(&m);
    dispatched(&root);
    assert!(record_owner(&root, &branch("work"), TASK).expect("recorded"));
    let standing = answers::standing(&root);
    let work = Touched::from(branch("work"));

    assert_eq!(of_request(&standing, Some(TASK), Some(&work)), Owner::Actor);
    assert_eq!(of_request(&standing, Some(ROOT), Some(&work)), Owner::Task);
    assert_eq!(
        of_request(&standing, Some(SIBLING), Some(&work)),
        Owner::Other
    );
    let opened = Touched::OpenedBy(ROOT.to_owned());
    assert_eq!(
        of_request(&standing, Some(TASK), Some(&opened)),
        Owner::Asker
    );
    // Nothing named (the merge hold, until #7035), no actor, or a branch no record names: "none".
    assert_eq!(of_request(&standing, Some(TASK), None), Owner::Nobody);
    assert_eq!(of_request(&standing, None, Some(&work)), Owner::Nobody);
    assert_eq!(of_request(&standing, Some(""), Some(&work)), Owner::Nobody);
    let theirs = Touched::from(branch("theirs"));
    assert_eq!(
        of_request(&standing, Some(TASK), Some(&theirs)),
        Owner::Nobody
    );
    // Another forge's branch of the same name is another record.
    let elsewhere = Touched::from(Owned::Branch {
        host: "gitlab.com",
        repo: "acme/app",
        branch: "work",
    });
    assert_eq!(
        of_request(&standing, Some(TASK), Some(&elsewhere)),
        Owner::Nobody
    );

    // The host's context carries the value, on every action that declares it, and `made_by`
    // reads "none" until M81 fills it.
    let asker = crate::policy::decide::Asker {
        chat: 7,
        of: crate::policy::levels::Chat {
            workspace: Some("web"),
            persona: None,
        },
        sandbox: "floor",
        attended: true,
        harness_mode: "default",
        depth: 1,
        via: "hook",
        start: None,
    };
    for action in [
        "read",
        "write",
        "push",
        "merge",
        "reach",
        "browse",
        "close",
        "comment.update",
    ] {
        let context = asker.context(action, None, Owner::Task);
        assert_eq!(context["owner"], "task", "{action}: {context}");
    }
    assert_eq!(
        asker.context("comment.update", None, Owner::Task)["made_by"],
        "none"
    );
    assert!(
        asker
            .context("cut", None, Owner::Task)
            .get("owner")
            .is_none()
    );
}

#[test]
fn a_record_a_chat_edited_never_makes_it_an_owner() {
    let m = machine();
    let root = plane(&m);
    assert!(record_owner(&root, &branch("work"), STRANGER).expect("recorded"));
    // The branch folder the piece log below claims: `workspaces/web/.worktrees/app/piece`.
    let folder = root.join("workspaces/web/.worktrees/app/piece");
    assert!(record_owner(&root, &Owned::Folder(&folder), STRANGER).expect("recorded"));
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Other);

    // The files a chat can reach, and that once said who owns a branch or a folder, name it:
    // the forge route's own list and the piece log. Each is written as its own writer writes it,
    // and read back by its own reader, so the check is of a claim purlis does read elsewhere.
    let config = m.dir.path().join("config");
    crate::forge::createdbranches::record(
        &config,
        TASK,
        &[("github.com".into(), "acme/app".into(), "work".into())],
    )
    .expect("written");
    assert_eq!(
        crate::forge::createdbranches::created_by(&config, TASK, "github.com", "acme/app"),
        ["work"],
        "the forge route's list names TASK as the branch's maker"
    );
    std::fs::create_dir_all(root.join("workspaces/web")).expect("a workspace");
    let who = crate::pieces::Who {
        session: None,
        persona: None,
        chat: Some(TASK.to_owned()),
        host: "here".to_owned(),
        log: "device".to_owned(),
    };
    crate::pieces::record(
        &root,
        "web",
        crate::pieces::Event::Claimed,
        "app",
        "piece",
        None,
        &who,
        jiff::Timestamp::now(),
    )
    .expect("a claim in the piece log");
    let claims = crate::pieces::claims(&root, "web");
    assert_eq!(
        claims
            .get(&("app".to_owned(), "piece".to_owned()))
            .and_then(|claim| claim.get("chat"))
            .and_then(serde_json::Value::as_str),
        Some(TASK),
        "the piece log names TASK as the folder's claimant: {claims:?}"
    );
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Other);
    assert_eq!(owner_of(&root, TASK, &Owned::Folder(&folder)), Owner::Other);

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

// ---- review round 2: a record taken away or put back (F1, F4; Q339.8) -----------------------

/// The sealed store's folder: where the manifest and each kind's folder are.
fn store_dir() -> PathBuf {
    let files = owner_files();
    files
        .first()
        .and_then(|file| file.parent())
        .and_then(Path::parent)
        .expect("a store with an owner record")
        .to_path_buf()
}

/// Whether the store lists an owner record that does not load (Q339.8).
fn lists_a_lost_owner_record(root: &Path) -> bool {
    answers::standing(root)
        .no_answer
        .iter()
        .any(|lost| lost.starts_with("owner "))
}

/// A copy of the store's folder, to put back later as an older one.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for item in std::fs::read_dir(from).expect("a folder").flatten() {
        let path = item.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(item.file_name()));
        } else {
            std::fs::copy(&path, to.join(item.file_name())).expect("copied");
        }
    }
}

#[test]
fn a_deleted_owner_record_is_not_replaced_by_a_second_claim() {
    let m = machine();
    let root = plane(&m);
    dispatched(&root);
    assert!(record_owner(&root, &branch("work"), STRANGER).expect("recorded"));
    let files = owner_files();
    let record = files
        .iter()
        .find(|file| std::fs::read_to_string(file).is_ok_and(|text| text.contains("acme/app#work")))
        .expect("the branch's record");
    std::fs::remove_file(record).expect("deleted");

    // The decision reads "none", and the plane is not in the deny state for it.
    let standing = answers::standing(&root);
    assert!(!standing.is_deny(), "{:?}", standing.state);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Nobody);
    assert!(lists_a_lost_owner_record(&root));

    // A second chat's claim does not become the owner in its place, nor does any new record.
    let refused = record_owner(&root, &branch("work"), TASK).expect_err("refused");
    assert!(refused.to_string().contains("owner record"), "{refused}");
    assert!(record_owner(&root, &branch("another"), TASK).is_err());
    assert!(record_dispatch(&root, STRANGER, TASK).is_err());
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Nobody);
    assert_eq!(owner_of(&root, TASK, &branch("another")), Owner::Nobody);

    // A commit that goes through keeps the listing: the tamper leaves its trace.
    assert_eq!(hand_over(&root, GRANDCHILD).expect("a commit"), 0);
    let_this_chat(&press(), &root, &branch("theirs"), SIBLING).expect("let");
    assert!(lists_a_lost_owner_record(&root), "the listing was dropped");
    assert!(record_owner(&root, &branch("work"), TASK).is_err());
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Nobody);
}

#[test]
fn an_edited_owner_record_is_not_replaced_by_a_second_claim() {
    let m = machine();
    let root = plane(&m);
    assert!(record_owner(&root, &branch("work"), STRANGER).expect("recorded"));
    for file in owner_files() {
        let text = std::fs::read_to_string(&file).expect("a record");
        std::fs::write(&file, text.replace(STRANGER, SIBLING)).expect("edited");
    }
    assert!(!answers::standing(&root).is_deny());
    assert_eq!(owner_of(&root, SIBLING, &branch("work")), Owner::Nobody);
    assert!(lists_a_lost_owner_record(&root));

    assert!(record_owner(&root, &branch("work"), TASK).is_err());
    assert!(record_owner(&root, &branch("work"), SIBLING).is_err());
    let_this_chat(&press(), &root, &branch("theirs"), SIBLING).expect("let");
    assert!(lists_a_lost_owner_record(&root), "the listing was dropped");
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Nobody);
    assert_eq!(owner_of(&root, SIBLING, &branch("work")), Owner::Nobody);
}

#[test]
fn an_old_record_put_back_after_a_hand_over_counts_for_nothing() {
    let m = machine();
    let root = plane(&m);
    dispatched(&root);
    assert!(record_owner(&root, &branch("work"), GRANDCHILD).expect("recorded"));
    let old = owner_files();
    let kept: Vec<(PathBuf, Vec<u8>)> = old
        .iter()
        .map(|file| (file.clone(), std::fs::read(file).expect("a record")))
        .collect();
    assert_eq!(hand_over(&root, GRANDCHILD).expect("handed over"), 1);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Actor);

    // The grandchild's own record, put back: the manifest does not list it, so it is ignored.
    for (file, bytes) in &kept {
        std::fs::write(file, bytes).expect("put back");
    }
    let standing = answers::standing(&root);
    assert!(!standing.is_deny(), "{:?}", standing.state);
    assert_eq!(owner_of(&root, GRANDCHILD, &branch("work")), Owner::Asker);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Actor);
}

#[test]
fn an_older_store_put_back_is_the_deny_state_and_names_no_owner() {
    let m = machine();
    let root = plane(&m);
    dispatched(&root);
    assert!(record_owner(&root, &branch("work"), GRANDCHILD).expect("recorded"));
    let store = store_dir();
    let older = m.dir.path().join("older-store");
    copy_dir(&store, &older);
    assert_eq!(hand_over(&root, GRANDCHILD).expect("handed over"), 1);

    // The whole store as it was before the hand-over, its manifest with it: the keyring's
    // counter is past it, so it is the deny state, and nothing in it is an owner.
    std::fs::remove_dir_all(&store).expect("taken away");
    copy_dir(&older, &store);
    let standing = answers::standing(&root);
    assert!(
        standing
            .causes()
            .iter()
            .any(|cause| matches!(cause, answers::Deny::ManifestOlder(_))),
        "{:?}",
        standing.state
    );
    assert_eq!(owner_of(&root, GRANDCHILD, &branch("work")), Owner::Nobody);
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Nobody);
    assert!(record_owner(&root, &branch("work"), GRANDCHILD).is_err());
}

// ---- review round 2: the owner reaches the decision (F4, F5) --------------------------------

#[test]
fn the_decision_reads_the_owner_of_what_the_sealed_request_touches() {
    use crate::policy::decide::{self, ChatFacts, Outcome, Sealed};
    use crate::policy::force::tests::{HEADER, STRICT, plane as plane_with};
    let m = machine();
    let rule = format!(
        "{HEADER}@id(\"ports-only-its-own\")\n\
         forbid (principal, action == Action::\"reach\", resource) \
         when {{ context.owner != \"self\" }};\n"
    );
    let root = plane_with(&m, &[("purlis.toml", STRICT), ("policy.cedar", &rule)]);
    answers::register(&root).expect("registered");
    let facts = ChatFacts {
        workspace: None,
        persona: None,
        sandboxed: false,
        level: None,
        attended: true,
        harness_mode: None,
    };
    let floor = crate::enforcement::Resolved::parse("ports=enforce");
    let reach = |chat: &str, touched: Option<Touched>| {
        let sealed = Sealed {
            chat: Some(chat.to_owned()),
            touched,
            ..Sealed::of(&root)
        };
        decide::for_chat(
            &root,
            4,
            Some(&facts),
            &floor,
            &sealed,
            "reach",
            ("Port", "8080"),
        )
        .expect("a chat that is open")
    };
    let forbidden = |said: &decide::Decision| {
        said.after_mode == Outcome::Deny && said.ids().contains(&"ports-only-its-own")
    };

    // The server TASK opened, reached by TASK: "self", so the rule does not forbid it.
    let own = reach(TASK, Some(Touched::OpenedBy(TASK.to_owned())));
    assert!(!forbidden(&own), "{own:?}");
    // By another chat, or with nothing named: "other" and "none", forbidden.
    let theirs = reach(STRANGER, Some(Touched::OpenedBy(TASK.to_owned())));
    assert!(forbidden(&theirs), "{theirs:?}");
    let unnamed = reach(TASK, None);
    assert!(forbidden(&unnamed), "{unnamed:?}");
}

// ---- review round 3: the press that mends the deny state, and the way out of a lost record ----

/// A plane whose project policy the press can read (`HEAD` holds it), registered as the app opens
/// it, with the line dispatched, `TASK`'s record of "work", `GRANDCHILD`'s of "theirs", a record
/// of "gone" and a never.
fn plane_with_records(m: &Machine) -> PathBuf {
    use crate::policy::force::tests::{STRICT, plane as plane_with};
    let root = plane_with(m, &[("purlis.toml", STRICT)]);
    answers::register(&root).expect("registered");
    dispatched(&root);
    assert!(record_owner(&root, &branch("work"), TASK).expect("recorded"));
    assert!(record_owner(&root, &branch("theirs"), GRANDCHILD).expect("recorded"));
    assert!(record_owner(&root, &branch("gone"), STRANGER).expect("recorded"));
    answers::commit(&root, |draft| draft.add(&press(), answers::NEVER, "rm -rf")).expect("a never");
    root
}

/// [`plane_with_records`], the record of "gone" then deleted: a lost listing.
fn plane_with_a_lost_record(m: &Machine) -> PathBuf {
    let root = plane_with_records(m);
    delete_the_record_of(&branch("gone"));
    assert!(lists_a_lost_owner_record(&root));
    root
}

/// Deletes the sealed file of the record of `owned`, as a chat that writes the store could.
fn delete_the_record_of(owned: &Owned<'_>) {
    let key = owned.key();
    let file = owner_files()
        .into_iter()
        .find(|file| std::fs::read_to_string(file).is_ok_and(|text| text.contains(&key)))
        .expect("the record's file");
    std::fs::remove_file(file).expect("deleted");
}

/// **The person's Accept, as the window makes it**: the press the plane raises now, accepted.
fn accept_the_press(root: &Path) {
    let raised = crate::policy::press::of(root).expect("a press waits");
    crate::policy::press::accept(root, &press(), &raised.digest).expect("the press");
    let after = answers::standing(root);
    assert!(
        !after.is_deny(),
        "the press mended nothing: {:?}",
        after.state
    );
}

/// The ways a plane comes into the deny state with its manifest still this clone's and as new as
/// the keyring says: what the press mends while every owner record it listed stays.
fn deny_with_the_manifest_current(root: &Path, cause: &str) {
    let store = store_dir();
    match cause {
        "past-the-cap" => answers::commit(root, |draft| {
            draft.add_narrowing(answers::PAST_THE_CAP, "a commit");
        })
        .map(|_| ())
        .expect("recorded"),
        "never-fails-seal" | "never-missing" => {
            let never = std::fs::read_dir(store.join(answers::NEVER))
                .expect("the nevers")
                .flatten()
                .next()
                .expect("a never")
                .path();
            if cause == "never-missing" {
                std::fs::remove_file(never).expect("deleted");
            } else {
                let text = std::fs::read_to_string(&never).expect("a never");
                std::fs::write(&never, text.replace("rm -rf", "rm -r")).expect("edited");
            }
        }
        other => panic!("no such cause: {other}"),
    }
    assert!(answers::standing(root).is_deny(), "{cause}");
}

#[test]
fn the_accept_that_mends_the_deny_state_keeps_every_owner_record_and_the_lost_listing() {
    for cause in ["past-the-cap", "never-fails-seal", "never-missing"] {
        let m = machine();
        let root = plane_with_a_lost_record(&m);
        deny_with_the_manifest_current(&root, cause);

        accept_the_press(&root);

        // The records that loaded stay, as they were.
        assert_eq!(
            owner_of(&root, TASK, &branch("work")),
            Owner::Actor,
            "{cause}"
        );
        assert_eq!(
            owner_of(&root, TASK, &branch("theirs")),
            Owner::Task,
            "{cause}"
        );
        // The lost listing stays, so no chat is recorded in the lost record's place.
        assert!(
            lists_a_lost_owner_record(&root),
            "{cause}: the listing was dropped"
        );
        assert!(
            record_owner(&root, &branch("gone"), TASK).is_err(),
            "{cause}"
        );
        assert!(
            record_owner(&root, &branch("work"), STRANGER).is_err(),
            "{cause}"
        );
        assert_eq!(
            owner_of(&root, TASK, &branch("work")),
            Owner::Actor,
            "{cause}"
        );
        assert_eq!(
            owner_of(&root, TASK, &branch("gone")),
            Owner::Nobody,
            "{cause}"
        );
    }
}

#[test]
fn the_accept_after_the_store_was_put_back_or_taken_away_brings_no_old_owner_back() {
    for cause in ["older-copy", "manifest-gone", "git-swapped"] {
        let m = machine();
        let root = plane_with_records(&m);
        let store = store_dir();
        let older = m.dir.path().join("older-store");
        copy_dir(&store, &older);
        assert_eq!(hand_over(&root, GRANDCHILD).expect("handed over"), 1);
        assert_eq!(
            owner_of(&root, TASK, &branch("theirs")),
            Owner::Actor,
            "{cause}"
        );
        match cause {
            "older-copy" => {
                std::fs::remove_dir_all(&store).expect("taken away");
                copy_dir(&older, &store);
            }
            "manifest-gone" => std::fs::remove_file(store.join("manifest.json")).expect("gone"),
            "git-swapped" => {
                std::fs::rename(root.join(".git"), root.join(".git-old")).expect("moved");
                std::fs::create_dir(root.join(".git")).expect("a new .git");
                copy_dir(&root.join(".git-old"), &root.join(".git"));
                std::fs::remove_dir_all(root.join(".git-old")).expect("removed");
            }
            other => panic!("no such cause: {other}"),
        }
        assert!(answers::standing(&root).is_deny(), "{cause}");

        accept_the_press(&root);

        // What the keyring says is newer is not known: nothing an older or missing manifest
        // named is an owner, the grandchild's handed-over branch included…
        assert_eq!(
            owner_of(&root, GRANDCHILD, &branch("theirs")),
            Owner::Nobody,
            "{cause}"
        );
        assert_eq!(
            owner_of(&root, TASK, &branch("work")),
            Owner::Nobody,
            "{cause}"
        );
        // …and no first claim is free until the person forgets the lost records.
        assert!(lists_a_lost_owner_record(&root), "{cause}");
        assert!(
            record_owner(&root, &branch("theirs"), GRANDCHILD).is_err(),
            "{cause}"
        );
        assert!(
            record_owner(&root, &branch("work"), STRANGER).is_err(),
            "{cause}"
        );
        assert!(record_dispatch(&root, STRANGER, TASK).is_err(), "{cause}");

        assert_eq!(
            forget_lost_records(&press(), &root).expect("forgotten"),
            1,
            "{cause}"
        );
        assert!(!lists_a_lost_owner_record(&root), "{cause}");
        assert!(
            record_owner(&root, &branch("work"), STRANGER).expect("recorded"),
            "{cause}"
        );
    }
}

#[test]
fn forgetting_the_lost_owner_records_frees_their_keys_and_keeps_the_rest() {
    let m = machine();
    let root = plane_with_a_lost_record(&m);
    assert_eq!(forget_lost_records(&press(), &root).expect("forgotten"), 1);
    assert!(!lists_a_lost_owner_record(&root));
    // The record that loaded is kept.
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Actor);
    // The lost record's key is free for a first claim, and a new record can be made again.
    assert!(record_owner(&root, &branch("gone"), TASK).expect("recorded"));
    assert_eq!(owner_of(&root, TASK, &branch("gone")), Owner::Actor);
    assert!(record_owner(&root, &branch("new"), SIBLING).expect("recorded"));
    // Nothing more to forget: the press changes nothing and says so by its count.
    assert_eq!(
        forget_lost_records(&press(), &root).expect("nothing lost"),
        0
    );
}

#[test]
fn a_record_put_back_after_the_person_forgot_it_counts_for_nothing() {
    // A task that deletes its own record before its hand-over and puts it back afterwards: once
    // the person forgot the lost listing, the file put back is listed nowhere and ignored.
    let m = machine();
    let root = plane(&m);
    assert!(record_owner(&root, &branch("mine"), STRANGER).expect("recorded"));
    let key = branch("mine").key();
    let (file, bytes) = owner_files()
        .into_iter()
        .find_map(|file| {
            let bytes = std::fs::read(&file).ok()?;
            String::from_utf8_lossy(&bytes)
                .contains(&key)
                .then_some((file, bytes))
        })
        .expect("the record's file");
    std::fs::remove_file(&file).expect("deleted");
    forget_lost_records(&press(), &root).expect("forgotten");
    std::fs::write(&file, &bytes).expect("put back");
    assert_eq!(owner_of(&root, STRANGER, &branch("mine")), Owner::Nobody);
    assert!(!lists_a_lost_owner_record(&root));
    assert!(record_owner(&root, &branch("mine"), TASK).expect("recorded"));
    assert_eq!(owner_of(&root, TASK, &branch("mine")), Owner::Actor);
}

#[test]
fn an_owner_record_handed_to_the_mend_is_refused_and_changes_nothing() {
    // The mend keeps the owner records the store holds itself; one handed in by its caller would
    // be an owner made by a press other than "Let this chat push it".
    let m = machine();
    let root = plane(&m);
    assert!(record_owner(&root, &branch("work"), TASK).expect("recorded"));
    let handed = answers::Entry::new(
        answers::OWNER,
        &Fact::Owns {
            of: branch("work").key(),
            chat: STRANGER.to_owned(),
        }
        .text(),
    );
    assert!(answers::recommit(&root, &press(), &[handed]).is_err());
    assert_eq!(owner_of(&root, TASK, &branch("work")), Owner::Actor);
    assert_eq!(owner_of(&root, STRANGER, &branch("work")), Owner::Other);
}
