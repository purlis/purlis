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
use purlis_core::pieces::Who;
use purlis_core::windowonly::{self, Asking, Control, Removal};
use purlis_core::worktree;

/// The person's own `Who`, as the window's press names them.
fn person() -> Who {
    Who::here(None, None, None)
}

/// Chat `session`'s ask for `removing`, as the app records the chat: no chat holds a folder, and
/// the ask is not raised from inside the folder's claim. What the chat is told.
fn ask(
    root: &Path,
    session: u32,
    workspace: Option<&str>,
    removing: &Removing,
) -> Result<String, String> {
    let who = Who::here(None, Some(session.to_string()), None);
    let asking = Asking {
        session,
        who: &who,
        live: Some(&[]),
        claimed: false,
    };
    match windowonly::ask_to_remove(root, &asking, workspace, removing) {
        Removal::Asked(told) | Removal::Ran(told) => told,
    }
}

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
    ask(
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
    ask(root, 4, who, &the_folder("forced")).unwrap();
    // B4: a folder holding a file git ignores, which removing it deletes with it.
    ignore(&f, ".env\n");
    let ignoring = folder_of(&f, "ignoring");
    std::fs::write(ignoring.join(".env"), "SECRET=1\n").unwrap();
    ask(root, 4, who, &the_folder("ignoring")).unwrap();

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
    ask(
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

    let said = windowonly::remove_listed(root, shown, &person()).unwrap();

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
    ask(
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
    let why = windowonly::remove_listed(root, &shown, &person()).unwrap_err();
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
    windowonly::remove_listed(root, &fresh, &person()).unwrap();
    assert!(!f.plane.join("personas/steward").exists());

    // A folder that gained a change, a commit and another ignored file since it was listed.
    ignore(&f, ".env\n*.log\n");
    let folder = folder_of(&f, "moving");
    std::fs::write(folder.join(".env"), "SECRET=1\n").unwrap();
    ask(root, 4, Some(&f.ws), &the_folder("moving")).unwrap();
    let [shown] = raised(root)[..]
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| panic!("one"));
    std::fs::write(folder.join("debug.log"), "later\n").unwrap();
    let why = windowonly::remove_listed(root, &shown, &person()).unwrap_err();
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
        digest: String::new(),
    };
    // Keep only: no press, and a press that arrives anyway removes nothing.
    assert!(!unlisted.pressable());
    let says = unlisted.says();
    assert!(says.contains("could not list everything"), "{says}");
    assert!(says.contains("Keep only"), "{says}");
    let why = windowonly::remove_listed(root, &unlisted, &person()).unwrap_err();
    assert!(why.contains("could not list everything"), "{why}");

    let exact = Control::Remove {
        removing: the_folder("moving"),
        workspace: "alpha".to_owned(),
        listed: vec!["ignored file: .env".to_owned()],
        exact: true,
        digest: "d".to_owned(),
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
        let said = ask(root, session, ws, &removing).unwrap();
        assert!(said.starts_with("Nobody was asked"), "{said}");
        assert!(!said.contains("asked the person in Asks"), "{said}");
        assert!(said.contains("nothing is removed until"), "{said}");
    }
    // One block each, written for the chat that was refused: the stop is in its Chat log.
    assert_eq!(*blocks.lock().unwrap(), [4, 5]);
    // Where a window listens, the same refusal is asked in Asks.
    windowonly::telling(root, Arc::new(|| {}));
    let said = ask(
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

/// The folder `piece` cut, holding `count` uncommitted files `a00`, `a01`, and so on.
fn with_files(f: &support::Fixture, piece: &str, count: usize) -> std::path::PathBuf {
    let folder = folder_of(f, piece);
    for n in 0..count {
        std::fs::write(folder.join(format!("a{n:02}")), "x\n").unwrap();
    }
    folder
}

/// The one ask raised for the chat, read back.
fn the_one(root: &Path) -> Control {
    let [one] = raised(root)[..]
        .to_vec()
        .try_into()
        .unwrap_or_else(|_| panic!("one ask"));
    one
}

#[test]
fn a_listing_git_would_not_give_whole_is_produced_by_the_code_and_keep_clears_it() {
    // A file git quotes with an escape cannot be read back to stamp, so the listing is not exact:
    // the code produces it (not a hand-built control), the row offers Keep only, and Keep clears
    // exactly that row, which a control built `exact: true` never matched.
    let f = a_plane();
    let root = f.plane.as_path();
    let folder = folder_of(&f, "quoted");
    std::fs::write(folder.join("caf\u{e9}.txt"), "x\n").unwrap();

    let loss = purlis_core::removalask::folder_loss(root, &f.ws, "thing", "quoted").unwrap();
    assert!(!loss.listing.exact, "{:?}", loss.listing);
    ask(root, 4, Some(&f.ws), &the_folder("quoted")).unwrap();
    let shown = the_one(root);
    let Control::Remove { exact, .. } = &shown else {
        panic!("not a removal: {shown:?}");
    };
    assert!(!*exact);
    assert!(!shown.pressable());

    windowonly::keep_listed(root, &shown);

    assert_eq!(windowonly::every(root), []);
    assert!(folder.is_dir(), "Keep removes nothing");
    windowonly::project_ended(root);
}

#[test]
fn an_entry_swapped_past_the_eighth_moves_the_listing_though_its_lines_do_not() {
    let f = a_plane();
    let root = f.plane.as_path();
    let folder = with_files(&f, "long", 10);
    ask(root, 4, Some(&f.ws), &the_folder("long")).unwrap();
    let shown = the_one(root);
    let Control::Remove { listed, exact, .. } = &shown else {
        panic!("not a removal");
    };
    // Eight are named and the rest counted: that is all the row says, and it is exact.
    assert!(*exact, "{listed:?}");
    assert!(listed.iter().any(|l| l == "and 2 more uncommitted"));

    std::fs::remove_file(folder.join("a09")).unwrap();
    std::fs::write(folder.join("a10"), "x\n").unwrap();
    let why = windowonly::remove_listed(root, &shown, &person()).unwrap_err();

    assert!(why.contains("changed since the window listed it"), "{why}");
    assert!(folder.is_dir(), "nothing was removed");
    let fresh = the_one(root);
    let Control::Remove {
        listed: now,
        digest,
        ..
    } = &fresh
    else {
        panic!("not a removal");
    };
    // The same lines, a different thing.
    assert_eq!(now, listed);
    let Control::Remove { digest: then, .. } = &shown else {
        unreachable!()
    };
    assert_ne!(digest, then);
    windowonly::project_ended(root);
}

#[test]
fn an_edit_to_a_file_already_listed_moves_the_listing() {
    let f = a_plane();
    let root = f.plane.as_path();
    let folder = folder_of(&f, "edited");
    std::fs::write(folder.join("README.md"), "changed\n").unwrap();
    ask(root, 4, Some(&f.ws), &the_folder("edited")).unwrap();
    let shown = the_one(root);

    // Its size moves.
    std::fs::write(folder.join("README.md"), "changed again\n").unwrap();
    let why = windowonly::remove_listed(root, &shown, &person()).unwrap_err();
    assert!(why.contains("changed since the window listed it"), "{why}");
    assert!(folder.is_dir());

    // And an edit that keeps its size is seen by the time it was written.
    let shown = the_one(root);
    std::fs::write(folder.join("README.md"), "CHANGED AGAIN\n").unwrap();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(folder.join("README.md"))
        .unwrap()
        .set_modified(later)
        .unwrap();
    let why = windowonly::remove_listed(root, &shown, &person()).unwrap_err();
    assert!(why.contains("changed since the window listed it"), "{why}");
    assert!(folder.is_dir());
    windowonly::project_ended(root);
}

#[test]
fn a_persona_listing_names_its_generated_sub_agent_and_the_grants_that_name_it() {
    let f = a_plane();
    let root = f.plane.as_path();
    let before = purlis_core::removalask::persona_listing(root, "steward").unwrap();
    assert!(
        !before.lines.iter().any(|l| l.contains(".claude/agents")),
        "{before:?}"
    );

    // The file purlis generates for the persona, in the shape it writes it.
    let marker = purlis_core::names::SYNC_AGENTS_MARKER
        .spellings()
        .next()
        .unwrap();
    let agents = f.plane.join(".claude/agents");
    std::fs::create_dir_all(&agents).unwrap();
    std::fs::write(
        agents.join("steward.md"),
        format!(
            "---\nname: steward\n---\n<!-- {marker} from steward — edit the persona, not this \
             file. -->\n\nThis sub-agent acts as the **steward** persona — steward.\n"
        ),
    )
    .unwrap();
    purlis_core::sandbox::local::grant_dispatch(root, "steward", "builder").unwrap();

    let now = purlis_core::removalask::persona_listing(root, "steward").unwrap();
    assert!(
        now.lines.iter().any(|l| l == ".claude/agents/steward.md"),
        "{now:?}"
    );
    assert!(
        now.lines
            .iter()
            .any(|l| l.contains("dispatch grants on this machine")),
        "{now:?}"
    );
    assert_ne!(now.digest, before.digest);
    assert!(now.exact);
}

#[test]
fn a_line_with_a_new_removing_arm_does_not_parse_as_an_older_apps_but_a_workspace_still_does() {
    // What an app built before T1.1-67 reads: `Removing` with its one arm. A line naming a
    // persona or a branch folder is not one it can read, which it answers as an unknown line,
    // and the chat is told the builds differ (`AppSaid::Older`); a workspace's line is as it was.
    #[derive(Debug, PartialEq, serde::Deserialize)]
    #[serde(rename_all = "snake_case", tag = "what")]
    enum Older {
        Workspace { name: String },
    }
    let wire = |removing: &Removing| serde_json::to_string(removing).unwrap();

    let workspace = Removing::Workspace {
        name: "alpha".to_owned(),
    };
    assert_eq!(
        serde_json::from_str::<Older>(&wire(&workspace)).unwrap(),
        Older::Workspace {
            name: "alpha".to_owned()
        }
    );
    for new in [
        Removing::Persona {
            name: "steward".to_owned(),
        },
        the_folder("piece"),
        Removing::BranchFolder {
            repo: "thing".to_owned(),
            piece: "piece".to_owned(),
            force: true,
            ignored: true,
        },
    ] {
        let line = wire(&new);
        assert!(
            serde_json::from_str::<Older>(&line).is_err(),
            "an older app read {line}"
        );
        // And this build reads every line it writes.
        assert_eq!(serde_json::from_str::<Removing>(&line).unwrap(), new);
    }
    // A line from an older purlis, with no flags, is this build's plain removal.
    assert_eq!(
        serde_json::from_str::<Removing>(r#"{"what":"branch_folder","repo":"thing","piece":"p"}"#)
            .unwrap(),
        Removing::BranchFolder {
            repo: "thing".to_owned(),
            piece: "p".to_owned(),
            force: false,
            ignored: false,
        }
    );
}
