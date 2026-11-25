// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One removal ask for a persona and a branch folder** (T1.1-67, Q304 Q2), against a plane of
//! real git: what a chat's removal is refused for is raised as one ask that lists exactly what
//! the removal would lose, the press removes exactly that, and a press whose listing moved
//! refreshes the row and removes nothing. Every project runs under `[policy] template =
//! "strict"`, set explicitly (Q308).

use crate::support;

use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::Removing;
use purlis_core::windowonly::{self, Control};
use purlis_core::worktree;

/// A plane with the clone `thing` in workspace `alpha`, strict, and a persona `steward` holding
/// one memory.
fn a_plane() -> support::Fixture {
    let f = support::plane_with_clone("thing");
    std::fs::write(
        f.plane.join("charter.toml"),
        "schema = 1\n[policy]\ntemplate = \"strict\"\n",
    )
    .unwrap();
    let steward = f.plane.join("personas/steward");
    std::fs::create_dir_all(steward.join("memory")).unwrap();
    std::fs::write(
        steward.join("persona.md"),
        "---\nrole: steward\n---\n# steward\n",
    )
    .unwrap();
    std::fs::write(steward.join("memory/one.md"), "one\n").unwrap();
    f
}

/// Makes git ignore `patterns` in every folder of the clone.
fn ignore(f: &support::Fixture, patterns: &str) {
    let info = f.clone.join(".git/info");
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(info.join("exclude"), patterns).unwrap();
}

fn folder_of(f: &support::Fixture, piece: &str) -> std::path::PathBuf {
    worktree::add(&f.plane, &f.ws, &f.repo, piece, None)
        .expect("a piece is cut")
        .path
}

/// What the app raises for a chat's asked removal: the one control, read back.
fn raised(root: &Path) -> Vec<Control> {
    windowonly::every(root)
        .into_iter()
        .map(|one| one.control)
        .collect()
}

fn the_folder(piece: &str) -> Removing {
    Removing::BranchFolder {
        repo: "thing".to_owned(),
        piece: piece.to_owned(),
        force: false,
        ignored: false,
    }
}

#[test]
fn each_removal_refusal_raises_the_one_removal_ask() {
    let f = a_plane();
    let root = f.plane.as_path();
    let who = Some(f.ws.as_str());

    // B2: a persona. The row names it and lists what is in its folder.
    windowonly::ask_to_remove(
        root,
        4,
        None,
        &Removing::Persona {
            name: "steward".into(),
        },
    )
    .unwrap();
    // B3: a folder holding an uncommitted change, which a plain removal would discard.
    let forced = folder_of(&f, "forced");
    std::fs::write(forced.join("README.md"), "changed\n").unwrap();
    windowonly::ask_to_remove(root, 4, who, &the_folder("forced")).unwrap();
    // B4: a folder holding a file git ignores, which removing it deletes with it.
    ignore(&f, ".env\n");
    let ignoring = folder_of(&f, "ignoring");
    std::fs::write(ignoring.join(".env"), "SECRET=1\n").unwrap();
    windowonly::ask_to_remove(root, 4, who, &the_folder("ignoring")).unwrap();

    let asks = raised(root);
    assert_eq!(asks.len(), 3, "{asks:?}");
    let [persona, forced, ignoring] = &asks[..] else {
        unreachable!()
    };
    assert_eq!(persona.label(), "Remove persona");
    let Control::Remove { listed, exact, .. } = persona else {
        panic!("not a removal: {persona:?}");
    };
    assert!(*exact, "{listed:?}");
    assert!(
        listed.iter().any(|l| l == "personas/steward/persona.md"),
        "{listed:?}"
    );
    assert!(
        listed
            .iter()
            .any(|l| l == "personas/steward/memory: 1 memories"),
        "{listed:?}"
    );

    assert_eq!(forced.label(), "Remove and discard");
    let Control::Remove {
        listed,
        removing,
        workspace,
        ..
    } = forced
    else {
        panic!("not a removal: {forced:?}");
    };
    assert_eq!(workspace, "alpha");
    assert!(
        matches!(removing, Removing::BranchFolder { force: true, .. }),
        "{removing:?}"
    );
    assert!(listed.iter().any(|l| l.contains("README.md")), "{listed:?}");

    assert_eq!(ignoring.label(), "Remove with these files");
    let Control::Remove {
        listed, removing, ..
    } = ignoring
    else {
        panic!("not a removal: {ignoring:?}");
    };
    assert!(
        matches!(
            removing,
            Removing::BranchFolder {
                force: false,
                ignored: true,
                ..
            }
        ),
        "{removing:?}"
    );
    assert!(listed.iter().any(|l| l.contains(".env")), "{listed:?}");
    // What the row says is purlis's: the thing, exactly what goes, and what each answer does.
    let says = ignoring.says();
    assert!(says.contains("deletes exactly this"), "{says}");
    assert!(says.contains("Keep removes nothing"), "{says}");
    windowonly::project_ended(root);
}

#[test]
fn a_removal_press_removes_exactly_what_the_row_listed() {
    let f = a_plane();
    let root = f.plane.as_path();
    windowonly::ask_to_remove(
        root,
        4,
        None,
        &Removing::Persona {
            name: "steward".into(),
        },
    )
    .unwrap();
    let [shown] = &raised(root)[..] else {
        panic!("one ask");
    };

    let said = windowonly::remove_listed(root, shown).unwrap();

    assert!(
        said.contains("Removed persona directory personas/steward/"),
        "{said}"
    );
    assert!(!f.plane.join("personas/steward").exists());
    // The ask is cleared for the chat that waited on it.
    assert_eq!(windowonly::every(root), []);
}

#[test]
fn a_removal_press_whose_listing_changed_refreshes_and_removes_nothing() {
    let f = a_plane();
    let root = f.plane.as_path();

    // A persona that gained a memory since the row listed it.
    windowonly::ask_to_remove(
        root,
        4,
        None,
        &Removing::Persona {
            name: "steward".into(),
        },
    )
    .unwrap();
    let [shown] = raised(root)[..]
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| panic!("one"));
    std::fs::write(f.plane.join("personas/steward/memory/two.md"), "two\n").unwrap();
    let why = windowonly::remove_listed(root, &shown).unwrap_err();
    assert!(why.contains("changed since the window listed it"), "{why}");
    assert!(
        f.plane.join("personas/steward/memory/two.md").is_file(),
        "nothing was removed"
    );
    // The row now lists what it holds now, and pressing that one removes it.
    let [fresh] = raised(root)[..]
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| panic!("one"));
    assert_ne!(fresh, shown);
    let Control::Remove { listed, .. } = &fresh else {
        panic!("not a removal");
    };
    assert!(
        listed
            .iter()
            .any(|l| l == "personas/steward/memory: 2 memories"),
        "{listed:?}"
    );
    windowonly::remove_listed(root, &fresh).unwrap();
    assert!(!f.plane.join("personas/steward").exists());

    // A folder that gained a change, a commit and another ignored file since it was listed.
    ignore(&f, ".env\n*.log\n");
    let folder = folder_of(&f, "moving");
    std::fs::write(folder.join(".env"), "SECRET=1\n").unwrap();
    windowonly::ask_to_remove(root, 4, Some(&f.ws), &the_folder("moving")).unwrap();
    let [shown] = raised(root)[..]
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| panic!("one"));
    std::fs::write(folder.join("debug.log"), "later\n").unwrap();
    let why = windowonly::remove_listed(root, &shown).unwrap_err();
    assert!(why.contains("changed since the window listed it"), "{why}");
    assert!(folder.join(".env").is_file() && folder.join("debug.log").is_file());
    let [fresh] = raised(root)[..]
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| panic!("one"));
    let Control::Remove { listed, .. } = &fresh else {
        panic!("not a removal");
    };
    assert!(listed.iter().any(|l| l.contains("debug.log")), "{listed:?}");
    windowonly::project_ended(root);
}

#[test]
fn remove_anyway_is_offered_only_on_an_exact_listing() {
    let f = a_plane();
    let root = f.plane.as_path();
    let unlisted = Control::Remove {
        removing: the_folder("moving"),
        workspace: "alpha".to_owned(),
        listed: vec!["files git ignores, which git would not list".to_owned()],
        exact: false,
    };
    // Keep only: no press, and a press that arrives anyway removes nothing.
    assert!(!unlisted.pressable());
    let says = unlisted.says();
    assert!(says.contains("could not list everything"), "{says}");
    assert!(says.contains("Keep only"), "{says}");
    let why = windowonly::remove_listed(root, &unlisted).unwrap_err();
    assert!(why.contains("could not list everything"), "{why}");

    let exact = Control::Remove {
        removing: the_folder("moving"),
        workspace: "alpha".to_owned(),
        listed: vec!["ignored file: .env".to_owned()],
        exact: true,
    };
    assert!(exact.pressable());
    assert!(!exact.says().contains("Keep only"));
}

#[test]
fn a_removal_with_no_window_says_nobody_was_asked_and_writes_a_block() {
    let f = a_plane();
    let root = f.plane.as_path();
    let blocks: Arc<Mutex<Vec<u32>>> = Arc::default();
    windowonly::blocking(root, {
        let blocks = Arc::clone(&blocks);
        Arc::new(move |session, _stop| blocks.lock().unwrap().push(session))
    });
    let forced = folder_of(&f, "forced");
    std::fs::write(forced.join("README.md"), "changed\n").unwrap();

    for (session, removing, ws) in [
        (
            4,
            Removing::Persona {
                name: "steward".into(),
            },
            None,
        ),
        (5, the_folder("forced"), Some(f.ws.as_str())),
    ] {
        let said = windowonly::ask_to_remove(root, session, ws, &removing).unwrap();
        assert!(said.starts_with("Nobody was asked"), "{said}");
        assert!(!said.contains("asked the person in Asks"), "{said}");
        assert!(said.contains("nothing is removed until"), "{said}");
    }
    // One block each, written for the chat that was refused: the stop is in its Chat log.
    assert_eq!(*blocks.lock().unwrap(), [4, 5]);
    // Where a window listens, the same refusal is asked in Asks.
    windowonly::telling(root, Arc::new(|| {}));
    let said = windowonly::ask_to_remove(
        root,
        6,
        None,
        &Removing::Persona {
            name: "steward".into(),
        },
    )
    .unwrap();
    assert!(
        said.starts_with("purlis asked the person in Asks"),
        "{said}"
    );
    assert_eq!(
        blocks.lock().unwrap().len(),
        2,
        "no block where a window listens"
    );
    windowonly::project_ended(root);
}

#[test]
fn an_older_apps_answer_to_a_new_removing_variant_reads_as_older_with_restart_words() {
    use windowonly::AppSaid;
    let root = Path::new("/plane/acme");
    let older = windowonly::removal_refusal(root, "persona 'steward'", "x", AppSaid::Older);
    assert!(older.contains("different builds"), "{older}");
    assert!(older.contains("restart purlis"), "{older}");
    assert!(older.contains("update the purlis plugin"), "{older}");
    assert!(older.contains("nobody was asked"), "{older}");
    assert!(older.contains("Nothing was removed"), "{older}");
    // Never a refusal of the removal: none of the app's own words are in it.
    assert!(!older.contains("could not read this line"), "{older}");

    let unheard = windowonly::removal_refusal(
        root,
        "persona 'steward'",
        "remove it in Asks",
        AppSaid::Unheard,
    );
    assert!(
        unheard.contains("No purlis window answered, so nobody was asked"),
        "{unheard}"
    );
    assert!(unheard.contains("remove it in Asks"), "{unheard}");
}
