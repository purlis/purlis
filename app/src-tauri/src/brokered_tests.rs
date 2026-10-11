//! What the app answers a chat's brokered memory verb with (#1408): the lines its command says,
//! made in the chat's own stores as the app records them.

use super::*;
use purlis_core::brokered::{Asker, Store, Write};
use purlis_core::repocmd::Say;

fn now() -> chrono::NaiveDateTime {
    "2026-10-06T12:00:00".parse().expect("a stamp")
}

/// A project with workspace `alpha` and personas `steward` and `devops`, each persona holding
/// one memory under the same name: that name.
fn a_project() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    let mut slug = String::new();
    for persona in ["steward", "devops"] {
        std::fs::create_dir_all(root.join("personas").join(persona)).unwrap();
        std::fs::write(
            root.join("personas").join(persona).join("persona.md"),
            format!("---\nname: {persona}\n---\n# {persona}\n"),
        )
        .unwrap();
        let path = purlis_core::brokered::remember_persona(
            &root,
            persona,
            "The forge is self-hosted",
            None,
            now(),
        )
        .expect("remembered");
        slug = path.file_stem().unwrap().to_string_lossy().into_owned();
    }
    (dir, root, slug)
}

/// Chat 4 as the app records it: in `alpha`, as `persona`.
fn chat_as(persona: Option<&str>) -> Asker {
    Asker {
        chat: 4,
        place: Place::Workspace("alpha".to_owned()),
        persona: persona.map(str::to_owned),
    }
}

#[test]
fn a_memory_verb_is_answered_with_what_its_command_says_and_made_in_the_chats_own_persona() {
    let (_dir, root, slug) = a_project();

    let answer = answer(
        &root,
        &chat_as(Some("steward")),
        &Write::ArchiveMemory {
            of: Store::Persona,
            slug: slug.clone(),
        },
        now(),
    );

    let Answer::Said { lines, code } = answer else {
        panic!("not answered with lines: {answer:?}");
    };
    assert_eq!(code, 0, "{lines:?}");
    assert!(
        matches!(&lines[..], [Say::Done(line), ..] if line.starts_with(&format!("Archived '{slug}' from steward's memory"))),
        "{lines:?}"
    );
    assert!(
        !root
            .join(format!("personas/steward/memory/{slug}.md"))
            .exists()
    );
    assert!(
        root.join(format!("personas/devops/memory/{slug}.md"))
            .is_file(),
        "another persona's memory was touched"
    );
}

#[test]
fn a_chat_on_no_persona_is_refused_a_persona_memory_verb() {
    let (_dir, root, slug) = a_project();

    let answer = answer(
        &root,
        &chat_as(None),
        &Write::Forget {
            of: Store::Persona,
            slug: slug.clone(),
        },
        now(),
    );

    assert!(
        matches!(&answer, Answer::No { why } if why.contains("runs as no persona")),
        "{answer:?}"
    );
    for persona in ["steward", "devops"] {
        assert!(
            root.join(format!("personas/{persona}/memory/{slug}.md"))
                .is_file()
        );
    }
}

#[test]
fn a_one_file_write_is_still_answered_as_written() {
    let (_dir, root, _) = a_project();
    let answer = answer(
        &root,
        &chat_as(None),
        &Write::Todo {
            text: "Port the picker".to_owned(),
        },
        now(),
    );
    assert!(
        matches!(&answer, Answer::Written { to, path } if to == "alpha" && path.starts_with("workspaces/alpha/todos/")),
        "{answer:?}"
    );
}
