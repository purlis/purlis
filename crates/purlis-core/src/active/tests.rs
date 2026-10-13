// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::collections::HashMap;
use std::fs;

/// A plane with every rung of both ladders set to a DIFFERENT value, so removing one
/// proves which of the others was underneath it. A rung that names the same workspace as
/// the rung below proves nothing at all.
struct Rig {
    dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("charter.toml"),
            "[workspace]\ndefault = \"w-plane\"\n\n[persona]\ndefault = \"p-plane\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("workspaces/w-cwd/repo")).unwrap();
        fs::create_dir_all(root.join(".charter/sessions")).unwrap();
        fs::create_dir_all(root.join(".charter/terminals")).unwrap();
        fs::write(root.join("workspaces/.default"), "w-declared\n").unwrap();
        fs::write(root.join(".charter/sessions/sid.workspace"), "w-session\n").unwrap();
        fs::write(
            root.join(".charter/terminals/tid.workspace"),
            "w-terminal\n",
        )
        .unwrap();
        for who in [
            "p-plane",
            "p-committed",
            "p-session",
            "p-terminal",
            "p-active",
        ] {
            fs::create_dir_all(root.join("personas").join(who)).unwrap();
            fs::write(
                root.join("personas").join(who).join("persona.md"),
                "# who\n",
            )
            .unwrap();
        }
        fs::write(root.join("personas/.default"), "p-committed\n").unwrap();
        fs::write(root.join(".charter/sessions/sid.persona"), "p-session\n").unwrap();
        fs::write(root.join(".charter/terminals/tid.persona"), "p-terminal\n").unwrap();
        fs::write(root.join(".charter/active-persona"), "p-active\n").unwrap();
        Self { dir }
    }

    fn root(&self) -> PathBuf {
        fs::canonicalize(self.dir.path()).unwrap()
    }

    fn ids(&self) -> Ids {
        Ids {
            session: Some("sid".to_string()),
            terminal: Some("tid".to_string()),
        }
    }
}

fn ask<'a>(root: &'a Path, cwd: &'a Path, ids: &'a Ids) -> Asking<'a> {
    Asking {
        root,
        cwd,
        flag: None,
        ids,
        env: None,
    }
}

// --- the workspace ladder, one rung at a time ------------------------------------- //

#[test]
fn the_flag_outranks_the_environment_the_tree_and_every_pointer() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let cwd = root.join("workspaces/w-cwd/repo");

    let mut asking = ask(&root, &cwd, &ids);
    asking.flag = Some("w-flag");
    asking.env = Some("w-env");

    let found = workspace(&asking);
    assert_eq!(found.name, "w-flag");
    assert_eq!(found.rung, WorkspaceRung::Flag);
}

#[test]
fn with_no_flag_the_environment_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let cwd = root.join("workspaces/w-cwd/repo");

    let mut asking = ask(&root, &cwd, &ids);
    asking.env = Some("w-env");

    let found = workspace(&asking);
    assert_eq!(found.name, "w-env");
    assert_eq!(found.rung, WorkspaceRung::Environment);
}

#[test]
fn an_environment_holding_only_whitespace_is_unset_and_the_tree_decides() {
    // charter#1055: `export CHARTER_WORKSPACE=$(…)` over a command that printed a space.
    // Taken as a name it hid every rung below it and `source` named the variable as what
    // decided, so the operator saw their own choice reported over a broken export.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let cwd = root.join("workspaces/w-cwd/repo");

    let mut asking = ask(&root, &cwd, &ids);
    asking.env = Some("   ");

    assert_eq!(workspace(&asking).rung, WorkspaceRung::Cwd);
    assert!(blank_in_environment(Some("   ")));
    assert!(!blank_in_environment(Some("")));
    assert!(!blank_in_environment(None));
}

#[test]
fn with_no_flag_and_no_environment_the_tree_you_stand_in_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let cwd = root.join("workspaces/w-cwd/repo");

    let found = workspace(&ask(&root, &cwd, &ids));
    assert_eq!(found.name, "w-cwd");
    assert_eq!(found.rung, WorkspaceRung::Cwd);
}

#[test]
fn the_workspace_directory_itself_is_in_that_workspace() {
    // SI-1: the app starts a workspace's chats in `workspaces/<ws>` and files them under
    // `<ws>` (`Plane::workspace_of`), so the ladder answers the same for that directory.
    // It used to be "a container and not a tree" and fall through to the pointers, which
    // is how a chat the app started in a workspace was asked which workspace it was in.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let cwd = root.join("workspaces/w-cwd");

    let found = workspace(&ask(&root, &cwd, &ids));
    assert_eq!(found.name, "w-cwd");
    assert_eq!(found.rung, WorkspaceRung::Cwd);
}

#[test]
fn the_workspaces_directory_itself_is_in_no_workspace() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let cwd = root.join("workspaces");

    assert_eq!(workspace_of_tree(&root, &cwd), None);
    assert_eq!(
        workspace(&ask(&root, &cwd, &ids)).rung,
        WorkspaceRung::SessionPointer
    );
}

// --- the plane root ------------------------------------------------------------------ //

#[test]
fn a_chat_started_at_the_plane_root_is_in_no_workspace() {
    assert!(at_plane_root(None, None, Some(PLANE_ROOT_ON)));
    assert!(!at_plane_root(None, Some("w-env"), Some(PLANE_ROOT_ON)));
}

#[test]
fn a_named_workspace_outranks_the_plane_root() {
    // `-w` is how a root chat acts on one workspace, and `CHARTER_WORKSPACE=<name>` set
    // on one command is the same request spelled in the environment.
    assert!(!at_plane_root(Some("w-flag"), None, Some(PLANE_ROOT_ON)));
    assert!(!at_plane_root(None, Some("w-env"), Some(PLANE_ROOT_ON)));
    // A blank one names nothing, so it outranks nothing.
    assert!(at_plane_root(Some(""), Some("  "), Some(PLANE_ROOT_ON)));
}

#[test]
fn only_the_one_word_marks_a_plane_root_chat() {
    // Fail toward no change: anything else is a chat the ladder answers as it always did.
    for other in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some(" 1"),
        Some("yes"),
    ] {
        assert!(!at_plane_root(None, None, other), "{other:?}");
    }
}

#[test]
fn the_refusal_at_the_plane_root_names_the_flag_and_the_workspaces() {
    let rig = Rig::new();
    let root = rig.root();
    fs::create_dir_all(root.join("workspaces/beta")).unwrap();

    for by in [PlaneRoot::Launched, PlaneRoot::Standing] {
        let said = plane_root_refusal(&root, by);
        assert!(said.contains("plane root"), "{said}");
        assert!(said.contains("-w <workspace>"), "{said}");
        assert!(
            said.contains("`beta`") && said.contains("`w-cwd`"),
            "{said}"
        );
    }
    // Only a session nothing launched there can be moved, so only its refusal says how.
    assert!(!plane_root_refusal(&root, PlaneRoot::Launched).contains("workspace use"));
    assert!(plane_root_refusal(&root, PlaneRoot::Standing).contains("purlis workspace use"));
}

#[test]
fn a_place_is_a_workspace_name_or_the_plane_root_and_nothing_else() {
    for place in [Place::Workspace("alpha".into()), Place::PlaneRoot] {
        assert_eq!(Place::read(place.word()), Some(place.clone()));
    }
    // The plane root's word is not a name any reader of the old field would join on.
    assert!(!contain::workspace_name_ok(Place::PLANE_ROOT));
    for not_one in [
        "",
        "../x",
        ".default",
        "Plane root",
        "plane  root",
        "plane-root ",
    ] {
        assert_eq!(Place::read(not_one), None, "{not_one:?}");
    }
    assert_eq!(Place::PlaneRoot.workspace(), None);
    assert_eq!(Place::Workspace("a".into()).workspace(), Some("a"));
}

// --- standing at the plane root (SI-1b) ------------------------------------------- //

/// The rig with neither pointer, which is a session nothing has chosen a workspace for.
fn unchosen(rig: &Rig) -> PathBuf {
    let root = rig.root();
    fs::remove_file(root.join(".charter/sessions/sid.workspace")).unwrap();
    fs::remove_file(root.join(".charter/terminals/tid.workspace")).unwrap();
    root
}

#[test]
fn standing_anywhere_in_the_plane_outside_every_workspace_is_the_plane_root() {
    // The defect: a chat started in `docs/` was in no workspace the window drew, and was
    // quizzed for one. Anywhere under the plane that is not a workspace's is the root.
    let rig = Rig::new();
    let root = unchosen(&rig);
    fs::create_dir_all(root.join("docs/adr")).unwrap();
    let ids = rig.ids();

    for cwd in [
        root.clone(),
        root.join("docs"),
        root.join("docs/adr"),
        root.join(".charter"),
        root.join("workspaces"),
    ] {
        assert_eq!(
            plane_root(&ask(&root, &cwd, &ids), None),
            Some(PlaneRoot::Standing),
            "{cwd:?}"
        );
    }
}

#[test]
fn the_planes_defaults_do_not_speak_for_a_session_standing_at_its_root() {
    // `workspaces/.default` and `[workspace] default` are both set in the rig. They answer
    // for a caller standing nowhere in the plane; a session in it is somewhere.
    let rig = Rig::new();
    let root = unchosen(&rig);
    let ids = rig.ids();

    assert_eq!(
        plane_root(&ask(&root, &root, &ids), None),
        Some(PlaneRoot::Standing)
    );
    // The ladder itself is unchanged: asked alone it still ends on a default.
    assert_eq!(
        workspace(&ask(&root, &root, &ids)).rung,
        WorkspaceRung::DeclaredDefault
    );
}

#[test]
fn a_session_that_chose_a_workspace_is_not_at_the_plane_root() {
    let rig = Rig::new();
    let root = rig.root();
    let ids = rig.ids();

    // Its own pointer, the terminal's, `-w` and `$CHARTER_WORKSPACE` are each a choice.
    assert_eq!(plane_root(&ask(&root, &root, &ids), None), None);
    fs::remove_file(root.join(".charter/sessions/sid.workspace")).unwrap();
    assert_eq!(plane_root(&ask(&root, &root, &ids), None), None);
    fs::remove_file(root.join(".charter/terminals/tid.workspace")).unwrap();
    let mut flagged = ask(&root, &root, &ids);
    flagged.flag = Some("w-flag");
    assert_eq!(plane_root(&flagged, None), None);
    let mut pinned = ask(&root, &root, &ids);
    pinned.env = Some("w-env");
    assert_eq!(plane_root(&pinned, None), None);
    // And a workspace's own tree is that workspace, whatever else is true.
    let inside = root.join("workspaces/w-cwd/repo");
    assert_eq!(plane_root(&ask(&root, &inside, &ids), None), None);
}

#[test]
fn standing_outside_the_plane_is_not_its_root() {
    // A script with `$CHARTER_ROOT` set, run from anywhere else: the plane's defaults are
    // for exactly this caller.
    let rig = Rig::new();
    let root = unchosen(&rig);
    let elsewhere = tempfile::tempdir().unwrap();
    let ids = rig.ids();

    assert_eq!(plane_root(&ask(&root, elsewhere.path(), &ids), None), None);
}

#[test]
fn the_launchers_mark_outranks_every_rung_but_a_named_workspace() {
    // `$CHARTER_PLANE_ROOT_SESSION=1` is the app's word for where it started the chat, so
    // a pointer or a tree does not move it; `-w` and `$CHARTER_WORKSPACE` still name one.
    let rig = Rig::new();
    let root = rig.root();
    let ids = rig.ids();
    let inside = root.join("workspaces/w-cwd/repo");

    for cwd in [&root, &inside] {
        assert_eq!(
            plane_root(&ask(&root, cwd, &ids), Some(PLANE_ROOT_ON)),
            Some(PlaneRoot::Launched)
        );
    }
    let mut flagged = ask(&root, &root, &ids);
    flagged.flag = Some("w-flag");
    assert_eq!(plane_root(&flagged, Some(PLANE_ROOT_ON)), None);
}

#[test]
fn a_hook_reads_the_launchers_mark_from_its_environment() {
    let rig = Rig::new();
    let root = unchosen(&rig);
    let ids = rig.ids();
    let marked = |name: &str| (name == PLANE_ROOT_ENV).then(|| PLANE_ROOT_ON.to_string());
    let none = |_: &str| None;

    let inside = root.join("workspaces/w-cwd/repo");
    assert_eq!(
        plane_root_in(&ask(&root, &inside, &ids), &marked),
        Some(PlaneRoot::Launched)
    );
    assert_eq!(
        plane_root_in(&ask(&root, &root, &ids), &none),
        Some(PlaneRoot::Standing)
    );
}

#[test]
fn with_no_tree_to_stand_in_the_sessions_own_pointer_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());

    let found = workspace(&ask(&root, &root, &ids));
    assert_eq!(found.name, "w-session");
    assert_eq!(found.rung, WorkspaceRung::SessionPointer);
}

#[test]
fn with_no_session_pointer_the_terminals_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    fs::remove_file(root.join(".charter/sessions/sid.workspace")).unwrap();

    let found = workspace(&ask(&root, &root, &ids));
    assert_eq!(found.name, "w-terminal");
    assert_eq!(found.rung, WorkspaceRung::TerminalPointer);
}

#[test]
fn a_session_with_no_id_reads_no_pointer_of_its_own() {
    // Absence is representable: the alternative charter had was a `nosession` sentinel,
    // a SHARED key that every id-less invocation wrote into.
    let rig = Rig::new();
    let root = rig.root();
    let ids = Ids {
        session: None,
        terminal: Some("tid".to_string()),
    };

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "w-terminal");
}

#[test]
fn with_neither_pointer_the_nominated_default_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());

    let found = workspace(&ask(&root, &root, &ids));
    assert_eq!(found.name, "w-declared");
    assert_eq!(found.rung, WorkspaceRung::DeclaredDefault);
}

#[test]
fn with_nothing_nominated_the_planes_declared_default_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join("workspaces/.default")).unwrap();

    let found = workspace(&ask(&root, &root, &ids));
    assert_eq!(found.name, "w-plane");
    assert_eq!(found.rung, WorkspaceRung::PlaneDefault);
    // And it is not a CHOICE: the picker fires exactly where nobody decided.
    assert_eq!(chosen_workspace(&ask(&root, &root, &ids)), None);
}

#[test]
fn with_a_plane_that_declares_nothing_the_answer_is_the_built_in() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join("workspaces/.default")).unwrap();
    fs::write(root.join("charter.toml"), "").unwrap();

    let found = workspace(&ask(&root, &root, &ids));
    assert_eq!(found.name, "default");
    assert_eq!(found.rung, WorkspaceRung::BuiltIn);
}

// --- what a rung may not say -------------------------------------------------------- //

#[test]
fn a_nominated_default_that_climbs_out_of_the_plane_is_no_rung() {
    // charter#442: `workspaces/.default` is committable, so the value is a teammate's,
    // and `resolve()` hands whatever it returns to a join onto `workspaces/`.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::write(root.join("workspaces/.default"), "../../esc\n").unwrap();

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "w-plane");
}

#[test]
fn a_plane_default_that_climbs_out_of_the_plane_is_no_rung() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join("workspaces/.default")).unwrap();
    fs::write(
        root.join("charter.toml"),
        "[workspace]\ndefault = \"../../esc\"\n",
    )
    .unwrap();

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "default");
}

#[test]
fn a_session_pointer_that_climbs_out_of_the_plane_is_no_rung() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    fs::write(root.join(".charter/sessions/sid.workspace"), "../../esc\n").unwrap();

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "w-terminal");
}

#[test]
fn a_nominated_default_reached_through_a_link_out_of_the_plane_is_no_rung() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("elsewhere"), "w-smuggled\n").unwrap();
    let link = root.join("workspaces/.default");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(outside.path().join("elsewhere"), &link).unwrap();

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "w-plane");
}

#[test]
fn a_session_pointer_that_is_a_link_is_no_rung() {
    // charter's own path under `.charter/`, created by charter: a link there has no
    // honest use, and this is the gate `reopen`, `hookwire` and `cistate` already keep.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("elsewhere"), "w-smuggled\n").unwrap();
    let link = root.join(".charter/sessions/sid.workspace");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(outside.path().join("elsewhere"), &link).unwrap();

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "w-terminal");
}

#[test]
fn an_id_that_is_not_a_path_segment_reads_no_pointer() {
    // The guard sits in FRONT of the join, not behind `session_id`'s sanitiser: `Ids` is
    // a public struct a caller can build — the app answering for another chat is exactly
    // that caller — so an id that never went through the sanitiser reaches the join.
    let rig = Rig::new();
    let root = rig.root();
    let ids = Ids {
        session: Some("..".to_string()),
        terminal: Some("../sessions/sid".to_string()),
    };

    assert_eq!(workspace(&ask(&root, &root, &ids)).name, "w-declared");
}

// --- the persona ladder, one rung at a time ----------------------------------------- //

#[test]
fn the_persona_flag_outranks_the_environment_and_every_pointer() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let mut asking = ask(&root, &root, &ids);
    asking.flag = Some("p-flag");
    asking.env = Some("p-env");

    let found = persona(&asking);
    assert_eq!(found.name.as_deref(), Some("p-flag"));
    assert_eq!(found.rung, PersonaRung::Flag);
}

#[test]
fn with_no_persona_flag_the_environment_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let mut asking = ask(&root, &root, &ids);
    asking.env = Some("p-env");

    let found = persona(&asking);
    assert_eq!(found.name.as_deref(), Some("p-env"));
    assert_eq!(found.rung, PersonaRung::Environment);
}

#[test]
fn a_persona_environment_holding_only_whitespace_is_unset() {
    // charter#1048, the same defect one noun over.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    let mut asking = ask(&root, &root, &ids);
    asking.env = Some(" \t ");

    assert_eq!(persona(&asking).rung, PersonaRung::SessionPointer);
}

#[test]
fn with_no_flag_and_no_environment_the_sessions_pointer_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name.as_deref(), Some("p-session"));
    assert_eq!(found.rung, PersonaRung::SessionPointer);
}

#[test]
fn with_no_session_pointer_the_terminals_decides_for_a_persona_too() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    fs::remove_file(root.join(".charter/sessions/sid.persona")).unwrap();

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name.as_deref(), Some("p-terminal"));
    assert_eq!(found.rung, PersonaRung::TerminalPointer);
}

#[test]
fn with_neither_pointer_the_plane_wide_active_file_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name.as_deref(), Some("p-active"));
    assert_eq!(found.rung, PersonaRung::ActiveFile);
}

#[test]
fn with_nothing_selected_the_planes_declared_front_door_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join(".charter/active-persona")).unwrap();

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name.as_deref(), Some("p-plane"));
    assert_eq!(found.rung, PersonaRung::PlaneDefault);
}

#[test]
fn with_nothing_declared_the_legacy_committed_default_decides() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join(".charter/active-persona")).unwrap();
    fs::write(
        root.join("charter.toml"),
        "[workspace]\ndefault = \"w-plane\"\n",
    )
    .unwrap();

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name.as_deref(), Some("p-committed"));
    assert_eq!(found.rung, PersonaRung::CommittedDefault);
}

#[test]
fn a_plane_with_no_front_door_has_no_persona_and_says_so() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join(".charter/active-persona")).unwrap();
    fs::remove_file(root.join("personas/.default")).unwrap();
    fs::write(root.join("charter.toml"), "").unwrap();

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name, None);
    assert_eq!(found.rung, PersonaRung::Nothing);
}

#[test]
fn a_committed_rung_naming_a_persona_the_plane_does_not_have_is_no_rung() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join(".charter/active-persona")).unwrap();
    fs::write(
        root.join("charter.toml"),
        "[persona]\ndefault = \"p-deleted\"\n",
    )
    .unwrap();

    // Falls to `personas/.default`, which names one that IS there.
    assert_eq!(
        persona(&ask(&root, &root, &ids)).name.as_deref(),
        Some("p-committed")
    );
}

#[test]
fn a_pointer_naming_a_persona_the_plane_does_not_have_still_wins() {
    // The asymmetry is charter's and is the point: a pointer `persona remove` left
    // behind must be VISIBLE, not silently replaced by the plane's front door.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), rig.ids());
    fs::write(root.join(".charter/sessions/sid.persona"), "p-deleted\n").unwrap();

    let found = persona(&ask(&root, &root, &ids));
    assert_eq!(found.name.as_deref(), Some("p-deleted"));
    assert_eq!(found.rung, PersonaRung::SessionPointer);
}

#[test]
fn the_shared_store_is_not_a_front_door_any_committed_rung_may_name() {
    // `_shared` is the store every persona READS, not a persona anybody adopts, and
    // `contain::persona_name_ok` admits it by name for the callers that mean the store.
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join(".charter/active-persona")).unwrap();
    fs::create_dir_all(root.join("personas/_shared")).unwrap();
    fs::write(root.join("personas/_shared/persona.md"), "# shared\n").unwrap();
    fs::write(
        root.join("charter.toml"),
        "[persona]\ndefault = \"_shared\"\n",
    )
    .unwrap();

    assert_eq!(
        persona(&ask(&root, &root, &ids)).name.as_deref(),
        Some("p-committed")
    );
}

#[test]
fn a_legacy_flat_persona_still_answers_a_committed_rung() {
    let rig = Rig::new();
    let (root, ids) = (rig.root(), Ids::default());
    fs::remove_file(root.join(".charter/active-persona")).unwrap();
    fs::write(root.join("personas/p-flat.md"), "# flat\n").unwrap();
    fs::write(
        root.join("charter.toml"),
        "[persona]\ndefault = \"p-flat\"\n",
    )
    .unwrap();

    assert_eq!(
        persona(&ask(&root, &root, &ids)).name.as_deref(),
        Some("p-flat")
    );
}

#[test]
fn every_rung_is_labelled_the_way_charter_labels_it() {
    // These are the words `workspace.source` and `persona._resolved` return, and they end
    // up on a status line beside the name. Pinned because the next caller (M2.2's
    // `recall`, and the app's own header) will print them, and a label that drifts
    // explains the active workspace by naming a rung that did not decide it.
    use PersonaRung as P;
    use WorkspaceRung as W;
    assert_eq!(
        [
            W::Flag,
            W::Environment,
            W::Cwd,
            W::SessionPointer,
            W::TerminalPointer,
            W::DeclaredDefault,
            W::PlaneDefault,
            W::BuiltIn,
        ]
        .map(W::label),
        [
            "--workspace",
            "$PURLIS_WORKSPACE or $CHARTER_WORKSPACE",
            "cwd",
            "session",
            "terminal",
            "declared default",
            "default (nothing selected)",
            "default (nothing selected)",
        ]
    );
    assert_eq!(
        [
            P::Flag,
            P::Environment,
            P::SessionPointer,
            P::TerminalPointer,
            P::ActiveFile,
            P::PlaneDefault,
            P::CommittedDefault,
            P::Nothing,
        ]
        .map(P::label),
        [
            "--persona",
            "$PURLIS_PERSONA or $CHARTER_PERSONA",
            "session",
            "terminal",
            "active-file",
            "charter.toml",
            "committed-default",
            "none",
        ]
    );
}

// --- the two ids -------------------------------------------------------------------- //

fn env_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn reader(map: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
    move |name: &str| map.get(name).cloned()
}

#[test]
fn a_frames_session_id_shadows_the_harnesss_own() {
    let map = env_of(&[
        ("PURLIS_SESSION_ID", "alpha.1"),
        ("CLAUDE_CODE_SESSION_ID", "9f2c"),
    ]);
    assert_eq!(Ids::of(&reader(&map)).session.as_deref(), Some("alpha.1"));
}

#[test]
fn claude_codes_own_id_answers_when_charter_sets_none() {
    let map = env_of(&[("CLAUDE_CODE_SESSION_ID", "9f2c-d1")]);
    assert_eq!(Ids::of(&reader(&map)).session.as_deref(), Some("9f2c-d1"));
}

#[test]
fn a_charter_session_id_of_whitespace_does_not_fall_through_to_claude_codes() {
    // Python's `a or b` chooses on the RAW value and sanitises after, so whitespace in
    // the first variable is TAKEN and becomes nothing. Stripping before the choice would
    // silently key this session on a different conversation's id.
    let map = env_of(&[
        ("PURLIS_SESSION_ID", "  "),
        ("CLAUDE_CODE_SESSION_ID", "9f2c"),
    ]);
    assert_eq!(Ids::of(&reader(&map)).session, None);
}

#[test]
fn a_session_id_becomes_a_filename_so_what_cannot_be_one_is_deleted() {
    let map = env_of(&[("PURLIS_SESSION_ID", "a/../b c")]);
    assert_eq!(Ids::of(&reader(&map)).session.as_deref(), Some("a..bc"));
}

#[test]
fn a_pane_id_keeps_its_shape_and_replaces_what_it_cannot_keep() {
    // Replaced with `-`, never deleted: two panes whose ids differ only in a character
    // charter dislikes must not collapse onto one pointer.
    let map = env_of(&[("TMUX_PANE", "%3")]);
    assert_eq!(Ids::of(&reader(&map)).terminal.as_deref(), Some("-3"));
}

#[test]
fn the_pane_variables_are_tried_in_charters_order() {
    let map = env_of(&[
        ("TERM_SESSION_ID", "w0t1p0"),
        ("TMUX_PANE", "%3"),
        ("STY", "1234.pts-0"),
    ]);
    assert_eq!(Ids::of(&reader(&map)).terminal.as_deref(), Some("w0t1p0"));
}

#[test]
fn a_window_id_is_not_a_pane_id_and_names_no_terminal() {
    // It identifies a WINDOW, which holds many tabs and splits, so every session in it
    // wrote and read one another's pointer. Measured exactly that way.
    let map = env_of(&[("WINDOWID", "4194311")]);
    assert_eq!(Ids::of(&reader(&map)).terminal, None);
}

#[test]
fn an_empty_pane_variable_is_skipped_for_the_next_one() {
    let map = env_of(&[("TERM_SESSION_ID", ""), ("TMUX_PANE", "%7")]);
    assert_eq!(Ids::of(&reader(&map)).terminal.as_deref(), Some("-7"));
}

/// Python's `str()` of each scalar `tomllib` hands back, read off CPython. (A float Rust
/// would print without an exponent where Python uses one, `1e20`, is left out on
/// purpose: the two disagree there, and it is not this test's to settle.)
#[test]
fn a_manifest_value_is_rendered_the_way_python_str_renders_it() {
    let v = |text: &str| -> toml::Value {
        let doc: toml::Table = format!("v = {text}").parse().unwrap();
        doc["v"].clone()
    };
    assert_eq!(python_str(&v("\"alpha\"")).as_deref(), Some("alpha"));
    assert_eq!(python_str(&v("7")).as_deref(), Some("7"));
    assert_eq!(python_str(&v("-7")).as_deref(), Some("-7"));
    assert_eq!(python_str(&v("true")).as_deref(), Some("True"));
    assert_eq!(python_str(&v("false")).as_deref(), Some("False"));
    assert_eq!(python_str(&v("2.0")).as_deref(), Some("2.0"));
    assert_eq!(python_str(&v("1.5")).as_deref(), Some("1.5"));
    assert_eq!(python_str(&v("inf")).as_deref(), Some("inf"));
    assert_eq!(python_str(&v("[1]")), None);
    assert_eq!(python_str(&v("{ a = 1 }")), None);
}

#[test]
fn the_planes_declared_default_is_what_python_would_name_or_else_default() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let with = |manifest: &str| {
        fs::write(root.join(crate::plane::MANIFEST), manifest).unwrap();
        plane_default_workspace(root)
    };
    assert_eq!(with("[workspace]\ndefault = \" alpha \"\n"), "alpha");
    assert_eq!(with("[workspace]\ndefault = 7\n"), "7");
    assert_eq!(with("[workspace]\ndefault = true\n"), "True");
    assert_eq!(with("[workspace]\ndefault = \"../esc\"\n"), "default");
    assert_eq!(with("[workspace]\ndefault = [\"a\"]\n"), "default");
    assert_eq!(with(""), "default");
}

#[test]
fn a_default_workspace_says_when_nothing_can_persist_because_there_is_no_pane() {
    let pane = Ids {
        session: None,
        terminal: Some("w0t1p0".into()),
    };
    let none = Ids::default();
    for rung in [WorkspaceRung::PlaneDefault, WorkspaceRung::BuiltIn] {
        assert_eq!(
            workspace_source(&none, rung),
            "default (no pane id — nothing persists between sessions)"
        );
        assert_eq!(workspace_source(&pane, rung), "default (nothing selected)");
    }
    assert_eq!(workspace_source(&none, WorkspaceRung::Flag), "--workspace");
    assert_eq!(
        workspace_source(&pane, WorkspaceRung::SessionPointer),
        "session"
    );
}

#[test]
fn a_rung_file_is_read_up_to_charters_size_bound_and_not_one_byte_past_it() {
    let dir = tempfile::tempdir().unwrap();
    let at_bound = dir.path().join("at");
    let past = dir.path().join("past");
    let padded = |len: u64| {
        let mut text = String::from("alpha");
        text.push_str(&" ".repeat(usize::try_from(len).unwrap() - text.len()));
        text
    };
    fs::write(&at_bound, padded(MAX_RUNG_BYTES)).unwrap();
    fs::write(&past, padded(MAX_RUNG_BYTES + 1)).unwrap();

    assert_eq!(read_a_name(&at_bound, false).as_deref(), Some("alpha"));
    assert_eq!(read_a_name(&past, false), None);
    assert_eq!(read_a_name(&past, true), None);
    // Not a file at all.
    assert_eq!(read_a_name(dir.path(), true), None);
}

#[test]
fn the_planes_own_default_persona_is_the_declared_one_else_the_committed_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    for who in ["declared", "committed"] {
        fs::create_dir_all(root.join("personas").join(who)).unwrap();
        fs::write(root.join("personas").join(who).join("persona.md"), "# p\n").unwrap();
    }
    fs::write(root.join("personas/.default"), "committed\n").unwrap();
    assert_eq!(plane_default_persona(&root).as_deref(), Some("committed"));

    fs::write(
        root.join(crate::plane::MANIFEST),
        "[persona]\ndefault = \"declared\"\n",
    )
    .unwrap();
    assert_eq!(plane_default_persona(&root).as_deref(), Some("declared"));
}

/// Set in a child this test binary starts, which then prints what [`Ids::from_env`]
/// answered there and nothing else.
const IDS_CHILD: &str = "CHARTER_TEST_ACTIVE_IDS_CHILD";

/// Every variable a rung reads, so the child answers from what the test hands it and
/// never from whatever terminal the suite happens to run in.
const IDS_VARS: [&str; 6] = [
    SESSION_ID_ENV,
    CONVERSATION_ENV,
    "TERM_SESSION_ID",
    "TMUX_PANE",
    "STY",
    "SSH_TTY",
];

const ASK_IDS: &str = "active::tests::this_process_is_asked_for_its_own_ids";

#[test]
fn this_process_is_asked_for_its_own_ids() {
    if std::env::var_os(IDS_CHILD).is_some() {
        let ids = Ids::from_env();
        println!("ids<{:?}|{:?}>", ids.session, ids.terminal);
        return;
    }
    // The process's own environment, and not a named one: `from_env` reads the real
    // variables, which a test can only set by starting a process with them.
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", ASK_IDS, "--nocapture", "--test-threads=1"])
        .env(IDS_CHILD, "1")
        .stdin(std::process::Stdio::null());
    for name in IDS_VARS {
        for spelling in crate::envvar::spellings(name) {
            child.env_remove(spelling);
        }
    }
    let out = crate::forklock::output(
        child
            .env(SESSION_ID_ENV, "frame 1")
            .env("TERM_SESSION_ID", "w0t1p0"),
    )
    .unwrap();
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(
        printed.contains(r#"ids<Some("frame1")|Some("w0t1p0")>"#),
        "{printed}"
    );
}

/// With no pane variable at all, the pane is the controlling terminal's device — the
/// rung that answers under the app, which gives each chat a pty and sets none of them.
#[cfg(unix)]
#[test]
fn with_no_pane_variable_the_pane_is_the_terminal_the_process_reads() {
    use crate::engine::{AlacrittyEngine, Size};
    use crate::session::{Session, Spec};
    const SIZE: Size = Size {
        columns: 200,
        rows: 24,
    };
    let mut spec = Spec::new(std::env::current_exe().unwrap(), SIZE).args([
        "--exact",
        ASK_IDS,
        "--nocapture",
        "--test-threads=1",
    ]);
    spec.env.push((IDS_CHILD.into(), "1".into()));
    spec.env_without = IDS_VARS
        .iter()
        .flat_map(|name| crate::envvar::spellings(name))
        .map(Into::into)
        .collect();
    let session = Session::spawn(spec, Box::new(AlacrittyEngine::new(SIZE, 1000))).expect("a pty");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let lines = session.screen().lines;
        if let Some(line) = lines.iter().find(|l| l.contains("ids<")) {
            // `/dev/ttys003` on macOS, `/dev/pts/3` on Linux, with charter's `-` for
            // every character it does not keep.
            assert!(line.contains(r#"ids<None|Some("-dev-"#), "{lines:#?}");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the child never answered: {lines:#?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
