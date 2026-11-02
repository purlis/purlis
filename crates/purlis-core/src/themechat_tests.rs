// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A plane with `charter.toml` and the named workspaces.
fn plane(workspaces: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    for ws in workspaces {
        let d = dir.path().join("workspaces").join(ws);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("workspace.md"), "# ws\n").unwrap();
    }
    dir
}

fn card(id: &str, from: From) -> Card {
    Card {
        id: id.to_owned(),
        name: "Dusk".to_owned(),
        from,
        preview: false,
        appearances: Vec::new(),
        options: Vec::new(),
        refused: false,
        said: Vec::new(),
        warnings: Vec::new(),
        floors: Vec::new(),
        follows: None,
        duplicable: false,
        folder: true,
        stamp: String::new(),
    }
}

#[test]
fn new_theme_is_purlis_s_own_create_a_theme_on_the_level_the_gallery_is_at() {
    let dir = plane(&["web"]);
    let root = dir.path();
    let offered = |subject: &Subject| {
        curation::resolve(root, subject)
            .unwrap()
            .actions
            .into_iter()
            .find(|one| one.id == "charter/create-theme")
            .unwrap()
    };

    let in_workspace = chat(root, Some("web"), Asked::New).unwrap();
    assert_eq!(
        in_workspace,
        offered(&Subject {
            kind: Kind::Workspace,
            name: Some("web".into())
        })
    );
    assert_eq!(in_workspace.label, "Create a theme");
    assert_eq!(in_workspace.cwd, root.join("workspaces/web"));
    assert_eq!(
        in_workspace.prompt,
        "Use the purlis-theme skill to create a theme for the workspace web."
    );

    let at_root = chat(root, None, Asked::New).unwrap();
    assert_eq!(
        at_root,
        offered(&Subject {
            kind: Kind::Plane,
            name: None
        })
    );
    assert_eq!(at_root.cwd, root);
    assert!(
        at_root
            .prompt
            .starts_with("Use the purlis-theme skill to create a theme for the plane "),
        "{}",
        at_root.prompt
    );
}

#[test]
fn edit_with_a_chat_names_the_theme_by_the_id_the_list_gives_it_and_works_where_new_does() {
    let dir = plane(&["web"]);
    let root = dir.path();
    let edit = chat(root, Some("web"), Asked::Edit(&card("dusk", From::Project))).unwrap();
    assert_eq!(edit.label, EDIT_LABEL);
    assert_eq!(
        edit.prompt,
        "Use the purlis-theme skill to edit the theme \"project:dusk\" for the workspace web."
    );
    let new = chat(root, Some("web"), Asked::New).unwrap();
    assert_eq!(
        (&edit.cwd, &edit.runner, &edit.source),
        (&new.cwd, &new.runner, &new.source)
    );

    for (one, workspace, id) in [
        (
            card("dusk", From::Workspace),
            Some("web"),
            "workspace:web:dusk",
        ),
        (card("dusk", From::Machine), None, "machine:dusk"),
        (
            card("purlis-classic", From::BuiltIn),
            None,
            "purlis-classic",
        ),
        (
            card(
                "solarized/Solarized Dark",
                From::Extension {
                    extension: "solarized".into(),
                },
            ),
            None,
            "solarized/Solarized Dark",
        ),
    ] {
        assert_eq!(listed_id(&one, workspace), id);
        let said = chat(root, workspace, Asked::Edit(&one)).unwrap();
        assert!(
            said.prompt.contains(&format!("\"{id}\"")),
            "{}",
            said.prompt
        );
    }
}

#[test]
fn a_theme_whose_id_is_not_plainly_spelled_opens_nothing() {
    let dir = plane(&["web"]);
    let root = dir.path();
    for id in [
        "ext/Two\nlines",
        "ext/a \"quoted\" one",
        "ext/`tick`",
        &format!("ext/{}", "a".repeat(100)),
    ] {
        let one = card(
            id,
            From::Extension {
                extension: "ext".into(),
            },
        );
        let refused = chat(root, None, Asked::Edit(&one)).unwrap_err();
        assert!(refused.contains("no chat was opened"), "{refused}");
        assert!(!refused.contains(id), "the id is not repeated: {refused}");
    }
}

#[test]
fn a_workspace_the_plane_does_not_have_opens_nothing() {
    let dir = plane(&["web"]);
    assert_eq!(
        chat(dir.path(), Some("gone"), Asked::New).unwrap_err(),
        "no workspace 'gone'"
    );
}

#[test]
fn every_prompt_reads_whole_in_every_harness_it_can_be_typed_into() {
    let long = crate::curation::LONG_SUBJECT_NAME;
    let dir = plane(&[long]);
    let root = dir.path();
    let theme = card(&"d".repeat(32), From::Workspace);
    for asked in [Asked::New, Asked::Edit(&theme)] {
        let said = chat(root, Some(long), asked).unwrap();
        for harness in crate::harness::Harness::ALL {
            if harness.ready_to_type().is_none() {
                continue;
            }
            assert_eq!(
                harness.why_drawn_as_a_placeholder(&curation::pasted(&said.prompt)),
                None,
                "{}",
                said.prompt
            );
        }
    }
}
