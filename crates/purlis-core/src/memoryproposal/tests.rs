// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A memory or todo proposed for a store that is not the chat's own (T1.1-68, C1 and C2), and,
//! by the person's ruling of 2026-11-25, a memory forgotten or edited there.

use super::*;

fn now() -> jiff::civil::DateTime {
    "2026-11-25T12:00:00".parse().expect("a stamp")
}

/// A project with workspaces `alpha` and `beta` and personas `steward` and `devops`.
fn a_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
    }
    for persona in ["steward", "devops"] {
        std::fs::create_dir_all(root.join("personas").join(persona)).unwrap();
        std::fs::write(
            root.join("personas").join(persona).join("persona.md"),
            format!("---\nname: {persona}\n---\n# {persona}\n"),
        )
        .unwrap();
    }
    (dir, root)
}

fn remember(text: &str) -> Write {
    Write::PersonaRemember {
        text: text.to_owned(),
        title: None,
        shared: false,
    }
}

fn devops() -> Target {
    Target::Persona("devops".to_owned())
}

fn beta() -> Target {
    Target::Workspace("beta".to_owned())
}

/// The entries of a store's folder, `MEMORY.md` apart.
fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name != crate::memstore::INDEX && name != crate::memstore::ARCHIVE)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn memory_dir(root: &Path, target: &Target) -> std::path::PathBuf {
    match target {
        Target::Persona(name) => root.join("personas").join(name).join("memory"),
        Target::Workspace(name) => root.join("workspaces").join(name).join("memory"),
    }
}

/// Adds one memory to `target` by the proposal's own Apply, and says its slug.
fn an_entry(root: &Path, target: &Target, text: &str) -> String {
    let write = match target {
        Target::Persona(_) => remember(text),
        Target::Workspace(_) => Write::WorkspaceRemember {
            text: text.to_owned(),
            title: None,
        },
    };
    let before = files(&memory_dir(root, target));
    let held = shown(root, write, target).expect("held").write;
    apply(root, 7, &held, target, None, now()).expect("written");
    let added: Vec<String> = files(&memory_dir(root, target))
        .into_iter()
        .filter(|name| !before.contains(name))
        .collect();
    assert_eq!(added.len(), 1, "{added:?}");
    added[0].strip_suffix(".md").unwrap_or(&added[0]).to_owned()
}

fn forget(slug: &str) -> Write {
    Write::Forget {
        of: Store::Persona,
        slug: slug.to_owned(),
    }
}

fn edit(slug: &str, title: Option<&str>, text: Option<&str>) -> Write {
    Write::EditMemory {
        of: Store::Persona,
        slug: slug.to_owned(),
        title: title.map(str::to_owned),
        text: text.map(str::to_owned),
    }
}

#[test]
fn a_remember_and_a_todo_are_proposable_to_a_store_that_is_there() {
    let (_dir, root) = a_project();
    assert!(shown(&root, remember("run the drill"), &devops()).is_ok());
    let note = Write::WorkspaceRemember {
        text: "beta uses blue".to_owned(),
        title: None,
    };
    assert!(shown(&root, note, &beta()).is_ok());
    let todo = Write::Todo {
        text: "rotate the key".to_owned(),
    };
    assert!(shown(&root, todo, &beta()).is_ok());
}

/// **Move and optimize stay refused**, and so does every other write that is not a verb the
/// person ruled on: archive, unarchive, a todo's close and a workspace's vision.
#[test]
fn every_other_write_stays_refused() {
    let (_dir, root) = a_project();
    let refused = [
        Write::ArchiveMemory {
            of: Store::Persona,
            slug: "x".to_owned(),
        },
        Write::UnarchiveMemory {
            of: Store::Persona,
            slug: "x".to_owned(),
            restore_as: None,
        },
        Write::MoveMemory {
            from: Store::Persona,
            slug: "x".to_owned(),
            to: Store::Shared,
        },
        Write::WorkspaceOptimize { stale_days: 30 },
        Write::PersonaOptimize {
            of: Store::Persona,
            with_shared: false,
            stale_days: 30,
        },
        Write::TodoClose {
            how: crate::wscmd::todo::Close::Done,
            slug: "x".to_owned(),
        },
        Write::WorkspaceVision {
            text: "new".to_owned(),
        },
    ];
    for write in refused {
        assert!(!proposable_kind(&write), "{write:?}");
        for target in [devops(), beta()] {
            assert_eq!(
                shown(&root, write.clone(), &target),
                Err(NOT_PROPOSABLE.to_owned()),
                "{write:?} to {target:?}"
            );
        }
    }
}

#[test]
fn a_write_that_does_not_fit_its_target_or_names_no_store_is_refused() {
    let (_dir, root) = a_project();
    // A persona's memory aimed at a workspace, and a todo aimed at a persona.
    assert!(shown(&root, remember("x"), &beta()).is_err());
    let todo = Write::Todo {
        text: "x".to_owned(),
    };
    assert!(shown(&root, todo, &devops()).is_err());
    // Shared memory is reached with --shared, never by name.
    let shared = Write::PersonaRemember {
        text: "x".to_owned(),
        title: None,
        shared: true,
    };
    let underscore = Target::Persona("_shared".to_owned());
    assert!(shown(&root, shared, &underscore).is_err());
    assert!(shown(&root, remember("x"), &underscore).is_err());
    // A store that is not there.
    assert!(shown(&root, remember("x"), &Target::Persona("ghost".to_owned())).is_err());
    for name in ["../alpha", "ghost"] {
        let note = Write::WorkspaceRemember {
            text: "x".to_owned(),
            title: None,
        };
        assert!(shown(&root, note, &Target::Workspace(name.to_owned())).is_err());
    }
    // A forget naming the wrong kind of store for its target.
    let slug = an_entry(&root, &devops(), "run the drill");
    let wrong = Write::Forget {
        of: Store::Workspace,
        slug,
    };
    assert!(shown(&root, wrong, &devops()).is_err());
}

#[test]
fn a_text_that_draws_as_something_else_is_not_proposed() {
    let (_dir, root) = a_project();
    // A right-to-left override makes what is read differ from what is written.
    let why = shown(&root, remember("safe \u{202e}evil"), &devops()).expect_err("refused");
    assert_eq!(why, DRAWN_OTHERWISE);
}

#[test]
fn a_text_is_held_trimmed_so_apply_writes_what_was_read() {
    let (_dir, root) = a_project();
    let held = shown(&root, remember("  run the drill  \n"), &devops()).expect("held");
    assert_eq!(held.write, remember("run the drill"));
    assert_eq!((held.before.as_str(), held.state), ("", None));
}

#[test]
fn what_the_person_is_shown_names_the_target_and_the_whole_text() {
    let said = said(&remember("run the drill"), &devops());
    assert!(said.starts_with("In persona devops: "), "{said}");
    assert!(said.contains("run the drill"), "{said}");
}

#[test]
fn apply_writes_a_persona_memory_to_the_named_persona_and_no_other() {
    let (_dir, root) = a_project();
    let held = shown(&root, remember("run the drill"), &devops()).unwrap();
    let said = apply(&root, 7, &held.write, &devops(), None, now()).expect("written");
    assert!(said.contains("personas/devops/memory/"), "{said}");
    assert_eq!(files(&root.join("personas/devops/memory")).len(), 1);
    assert!(files(&root.join("personas/steward/memory")).is_empty());
    assert!(files(&root.join("personas/_shared/memory")).is_empty());
}

#[test]
fn apply_writes_a_workspace_memory_and_a_todo_to_the_named_workspace_only() {
    let (_dir, root) = a_project();
    let note = Write::WorkspaceRemember {
        text: "beta uses blue".to_owned(),
        title: None,
    };
    let held = shown(&root, note, &beta()).unwrap();
    let said = apply(&root, 7, &held.write, &beta(), None, now()).expect("written");
    assert!(said.contains("workspaces/beta/memory/"), "{said}");
    let todo = Write::Todo {
        text: "rotate the key".to_owned(),
    };
    let held = shown(&root, todo, &beta()).unwrap();
    let said = apply(&root, 7, &held.write, &beta(), None, now()).expect("written");
    assert!(said.contains("workspaces/beta/todos/"), "{said}");
    assert!(!root.join("workspaces/alpha/memory").exists());
    assert!(!root.join("workspaces/alpha/todos").exists());
}

#[test]
fn apply_judges_again_so_a_store_gone_since_is_refused_and_nothing_is_written() {
    let (_dir, root) = a_project();
    let todo = Write::Todo {
        text: "rotate the key".to_owned(),
    };
    let held = shown(&root, todo, &beta()).unwrap();
    std::fs::remove_dir_all(root.join("workspaces/beta")).unwrap();
    let refused = apply(&root, 7, &held.write, &beta(), None, now());
    assert!(matches!(refused, Err(NotMade::Refused(_))), "{refused:?}");
    assert!(!root.join("workspaces/beta").exists(), "nothing recreated");
}

// ---- forget and edit (the person's ruling of 2026-11-25) --------------------------------------

#[test]
fn a_forget_proposal_shows_the_entry_being_removed() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");

    let one = shown(&root, forget(&slug), &devops()).expect("held");

    assert!(
        one.before.contains("Run the drill monthly"),
        "{}",
        one.before
    );
    assert_eq!(one.after, "", "nothing is left");
    assert!(one.state.is_some());
    assert_eq!(
        one.write,
        forget(&slug),
        "held under the entry's exact name"
    );
    assert_eq!(
        said(&one.write, &devops()),
        format!("In persona devops: Forget memory {slug}")
    );
    // Shown, not done.
    assert_eq!(files(&memory_dir(&root, &devops())).len(), 1);
}

#[test]
fn a_forget_of_a_slug_typed_short_is_held_under_the_exact_name() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");
    let tail = slug.rsplit('-').next().unwrap().to_owned();

    let one = shown(&root, forget(&tail), &devops()).expect("held");

    assert_eq!(one.write, forget(&slug));
}

#[test]
fn the_press_of_a_forget_removes_exactly_the_entry_shown() {
    let (_dir, root) = a_project();
    let keep = an_entry(&root, &devops(), "Keep this memory about backups");
    // Entries are named by the second they were made in.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let gone = an_entry(&root, &devops(), "Forget this one about the drill");
    assert_ne!(keep, gone);
    let one = shown(&root, forget(&gone), &devops()).unwrap();

    let said = apply(&root, 7, &one.write, &devops(), one.state.as_deref(), now()).expect("done");

    assert!(said.contains("Forgot"), "{said}");
    let left = files(&memory_dir(&root, &devops()));
    assert_eq!(
        left,
        vec![format!("{keep}.md")],
        "only the entry shown went"
    );
}

#[test]
fn an_edit_proposal_shows_the_entry_before_and_after() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");

    let one = shown(
        &root,
        edit(&slug, Some("Drills"), Some("Run the drill weekly")),
        &devops(),
    )
    .expect("held");

    assert!(
        one.before.contains("Run the drill monthly"),
        "{}",
        one.before
    );
    assert!(one.after.contains("# Drills"), "{}", one.after);
    assert!(one.after.contains("Run the drill weekly"), "{}", one.after);
    assert!(!one.after.contains("monthly"), "{}", one.after);
    // The stamp line the entry had is kept, as an edit keeps it.
    if let Some(stamp) = one.before.lines().find(|line| line.starts_with('_')) {
        assert!(one.after.contains(stamp), "{}", one.after);
    }
    assert_eq!(
        said(&one.write, &devops()),
        format!("In persona devops: Rewrite memory {slug}")
    );
    // Shown, not done.
    let file = memory_dir(&root, &devops()).join(format!("{slug}.md"));
    assert_eq!(std::fs::read_to_string(file).unwrap(), one.before);
}

/// **What the person was shown is what is written**: the file after the press is exactly the
/// text the proposal showed.
#[test]
fn the_press_of_an_edit_writes_exactly_the_text_shown() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");
    let one = shown(
        &root,
        edit(&slug, None, Some("Run the drill weekly")),
        &devops(),
    )
    .unwrap();

    let said = apply(&root, 7, &one.write, &devops(), one.state.as_deref(), now()).expect("done");

    assert!(said.contains("Edited"), "{said}");
    let file = memory_dir(&root, &devops()).join(format!("{slug}.md"));
    assert_eq!(std::fs::read_to_string(file).unwrap(), one.after);
}

#[test]
fn an_edit_that_gives_only_a_title_keeps_the_text_in_what_it_shows() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");

    let titled = shown(&root, edit(&slug, Some("New title"), None), &devops()).unwrap();

    assert!(titled.after.contains("# New title"), "{}", titled.after);
    assert!(
        titled.after.contains("Run the drill monthly"),
        "{}",
        titled.after
    );
}

#[test]
fn a_forget_or_an_edit_of_a_memory_that_changed_since_it_was_shown_is_stale_and_writes_nothing() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");
    let file = memory_dir(&root, &devops()).join(format!("{slug}.md"));
    let forgets = shown(&root, forget(&slug), &devops()).unwrap();
    let edits = shown(&root, edit(&slug, None, Some("Run it weekly")), &devops()).unwrap();
    // The person (or another chat) edited it since.
    let changed = format!("{}\nAnd also on Fridays.\n", forgets.before.trim_end());
    std::fs::write(&file, &changed).unwrap();

    for one in [&forgets, &edits] {
        let stale = apply(&root, 7, &one.write, &devops(), one.state.as_deref(), now());
        assert_eq!(stale, Err(NotMade::Stale), "{:?}", one.write);
    }
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        changed,
        "untouched"
    );
}

#[test]
fn a_forget_of_a_memory_gone_since_it_was_shown_writes_nothing() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &devops(), "Run the drill monthly");
    let one = shown(&root, forget(&slug), &devops()).unwrap();
    std::fs::remove_file(memory_dir(&root, &devops()).join(format!("{slug}.md"))).unwrap();

    let refused = apply(&root, 7, &one.write, &devops(), one.state.as_deref(), now());

    assert!(matches!(refused, Err(NotMade::Refused(_))), "{refused:?}");
}

#[test]
fn a_forget_or_an_edit_of_an_entry_that_is_not_there_is_not_proposed() {
    let (_dir, root) = a_project();
    assert!(shown(&root, forget("no-such-entry"), &devops()).is_err());
    assert!(shown(&root, edit("no-such-entry", None, Some("x")), &devops()).is_err());
}

#[test]
fn a_forget_and_an_edit_of_a_workspace_entry_are_proposable_the_same_way() {
    let (_dir, root) = a_project();
    let slug = an_entry(&root, &beta(), "Beta uses blue");
    let forgets = Write::Forget {
        of: Store::Workspace,
        slug: slug.clone(),
    };
    let edits = Write::EditMemory {
        of: Store::Workspace,
        slug: slug.clone(),
        title: None,
        text: Some("Beta uses green".to_owned()),
    };

    let edit_shown = shown(&root, edits, &beta()).expect("held");
    assert!(edit_shown.after.contains("Beta uses green"));
    let state = edit_shown.state.as_deref();
    apply(&root, 7, &edit_shown.write, &beta(), state, now()).expect("edited");
    let file = memory_dir(&root, &beta()).join(format!("{slug}.md"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), edit_shown.after);

    let forget_shown = shown(&root, forgets, &beta()).expect("held");
    assert!(forget_shown.before.contains("Beta uses green"));
    let state = forget_shown.state.as_deref();
    apply(&root, 7, &forget_shown.write, &beta(), state, now()).expect("forgotten");
    assert!(!file.exists());
}

#[test]
fn the_ask_a_chat_sends_reads_back_as_it_was_sent() {
    let ask = crate::hookwire::Ask::ProposeMemory(Box::new(crate::hookwire::ProposeMemoryAsk {
        chat: 7,
        write: remember("run the drill"),
        target: devops(),
    }));
    let line = serde_json::to_string(&ask).expect("a line");
    let back: crate::hookwire::Ask = serde_json::from_str(&line).expect("read back");
    assert_eq!(back, ask);
    let line = serde_json::to_string(&beta()).expect("a line");
    assert_eq!(
        serde_json::from_str::<Target>(&line).expect("read back"),
        beta()
    );
}
