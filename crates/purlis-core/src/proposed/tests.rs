// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A proposed change (#1975): which files, and the change as the tool would make it.

use super::*;
use serde_json::json;

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("real").join("plane");
    // Only folders any sandbox lets a test make: the judgement reads the disk only for links,
    // so a protected name need not be there to be judged.
    for folder in ["workspaces/alpha/repo", "personas/devops"] {
        std::fs::create_dir_all(root.join(folder)).expect("folders");
    }
    std::fs::write(root.join("purlis.toml"), "[plane]\n").expect("manifest");
    (dir, root)
}

/// PC-1, PC-11: purlis's own configuration, in the plane, and nothing else.
#[test]
fn only_purlis_s_own_configuration_in_the_plane_is_proposable() {
    let (_dir, root) = project();
    for yes in [
        "purlis.toml",
        "charter.toml",
        "workspaces/alpha/purlis.toml",
        "personas/devops/mcp.json",
        "personas/devops/persona.md",
        ".claude/skills/review/SKILL.md",
        "workspaces/alpha/.claude/skills/x/SKILL.md",
        "personas/devops/.claude/skills/y/SKILL.md",
    ] {
        assert_eq!(
            proposable(&root, &root.join(yes)),
            Ok(PathBuf::from(yes)),
            "{yes}"
        );
    }
    for no in [
        // Files that run as code the moment they are there.
        "workspaces/alpha/repo/.git/hooks/pre-commit",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".vscode/tasks.json",
        "package.json",
        ".envrc",
        // A repo clone's own files go through its own review.
        "workspaces/alpha/repo/purlis.toml",
        "workspaces/alpha/repo/.claude/skills/z/SKILL.md",
        // purlis's state, and anything else of a persona or a workspace.
        ".purlis/app/sandbox.json",
        "personas/devops/memory/x.md",
        "workspaces/alpha/workspace.md",
        "personas/.hidden/persona.md",
        // Outside the plane.
        "../elsewhere/purlis.toml",
    ] {
        assert!(proposable(&root, &root.join(no)).is_err(), "{no}");
    }
    assert!(proposable(&root, Path::new("/etc/purlis.toml")).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_anywhere_on_the_way_is_never_proposable() {
    let (dir, root) = project();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a folder");
    std::os::unix::fs::symlink(&elsewhere, root.join("personas/linked")).expect("a link");
    assert!(proposable(&root, &root.join("personas/linked/persona.md")).is_err());
    std::os::unix::fs::symlink(elsewhere.join("x"), root.join("personas/devops/mcp.json"))
        .expect("a link");
    assert!(proposable(&root, &root.join("personas/devops/mcp.json")).is_err());
}

/// PC-2: the change is the one the tool would make, or refused in its own terms.
#[test]
fn an_edit_is_worked_out_as_the_tool_would_make_it() {
    let now = "[plane]\nmode = \"pr\"\n";
    assert_eq!(
        apply_edit(
            "Write",
            &json!({"file_path": "x", "content": "new\n"}),
            Some(now)
        ),
        Ok("new\n".to_owned())
    );
    assert_eq!(
        apply_edit(
            "Edit",
            &json!({"file_path": "x", "old_string": "\"pr\"", "new_string": "\"pr-merge\""}),
            Some(now)
        ),
        Ok("[plane]\nmode = \"pr-merge\"\n".to_owned())
    );
    assert_eq!(
        apply_edit(
            "MultiEdit",
            &json!({"file_path": "x", "edits": [
                {"old_string": "[plane]", "new_string": "[plane] # x"},
                {"old_string": "pr", "new_string": "local", "replace_all": true}
            ]}),
            Some(now)
        ),
        Ok("[plane] # x\nmode = \"local\"\n".to_owned())
    );
    assert!(
        apply_edit(
            "Edit",
            &json!({"file_path": "x", "old_string": "nothing", "new_string": "y"}),
            Some(now)
        )
        .unwrap_err()
        .contains("not in the file")
    );
    assert!(
        apply_edit(
            "Edit",
            &json!({"file_path": "x", "old_string": "a", "new_string": "b"}),
            Some("a a")
        )
        .unwrap_err()
        .contains("2 times")
    );
}

#[test]
fn a_hook_s_edit_of_a_proposable_file_is_a_proposal_against_the_file_as_it_is() {
    let (_dir, root) = project();
    let payload = json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": "purlis.toml", "old_string": "[plane]", "new_string": "[plane]\nmode = \"pr\""},
    });
    let proposed = from_edit(&root, &payload, &root)
        .expect("ours")
        .expect("a proposal");
    assert_eq!(proposed.path, PathBuf::from("purlis.toml"));
    assert_eq!(proposed.after, "[plane]\nmode = \"pr\"\n");
    // What travels is the tool's own edit, never the file.
    assert_eq!(
        proposed.change,
        Change::Edits {
            edits: vec![Edit {
                old: "[plane]".to_owned(),
                new: "[plane]\nmode = \"pr\"".to_owned(),
                every: false,
            }]
        }
    );
    assert_eq!(proposed.base, base_of(Some(b"[plane]\n")));

    // A file that is not there yet is proposed whole, with no base.
    let new = json!({
        "tool_name": "Write",
        "tool_input": {"file_path": root.join("personas/devops/mcp.json"), "content": "{}\n"},
    });
    let proposed = from_edit(&root, &new, &root)
        .expect("ours")
        .expect("a proposal");
    assert_eq!(proposed.base, None);

    // Anything else is not this module's.
    let other = json!({"tool_name": "Edit", "tool_input": {"file_path": "src/main.rs"}});
    assert_eq!(from_edit(&root, &other, &root), None);
    let read = json!({"tool_name": "Read", "tool_input": {"file_path": "purlis.toml"}});
    assert_eq!(from_edit(&root, &read, &root), None);
}

/// PC-1: below `.claude/skills`, skill files alone; and no name in another case, and no `..`
/// that leaves the skills.
#[test]
fn below_the_skills_nothing_a_program_loads_as_its_own_is_proposable() {
    let (_dir, root) = project();
    for no in [
        ".claude/skills/x/.envrc",
        ".claude/skills/x/.vscode/tasks.json",
        ".claude/skills/x/.git/hooks/pre-commit",
        ".claude/skills/x/.git/config",
        ".claude/skills/x/package.json",
        ".claude/skills/x/hooks/run.sh",
        ".claude/skills/.hidden/SKILL.md",
        "workspaces/alpha/.claude/skills/x/.envrc",
        ".claude/skills/../settings.json",
        "Purlis.toml",
        "Personas/devops/persona.md",
        "personas/devops/Persona.md",
    ] {
        assert!(proposable(&root, &root.join(no)).is_err(), "{no}");
    }
    assert!(proposable(&root, &root.join(".claude/skills/x/scripts/run.py")).is_ok());
}

/// A proposal is read only from the file its name says: never through a second name, never a
/// pipe, and never what a chat could not be told why, in words that depend on the file.
#[cfg(unix)]
#[test]
fn a_second_name_or_a_pipe_at_a_proposable_path_is_never_read() {
    let (dir, root) = project();
    let elsewhere = dir.path().join("secret.json");
    std::fs::write(&elsewhere, "{\"token\": \"x\"}").expect("a file");
    std::fs::hard_link(&elsewhere, root.join("personas/devops/mcp.json")).expect("a hard link");
    let edit = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": "personas/devops/mcp.json", "old_string": "{", "new_string": "["},
    });
    assert_eq!(
        from_edit(&root, &edit, &root),
        Some(Err(UNPREPARED.to_owned()))
    );

    let fifo = root.join("personas/devops/persona.md");
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&fifo))
        .expect("mkfifo runs");
    assert!(made.success(), "a pipe");
    let started = std::time::Instant::now();
    let write = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {"file_path": "personas/devops/persona.md", "content": "x"},
    });
    assert_eq!(
        from_edit(&root, &write, &root),
        Some(Err(UNPREPARED.to_owned()))
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "never waited on"
    );
}

/// Why an edit could not be made is never said: one fixed sentence, whatever the file holds.
#[test]
fn an_edit_that_cannot_be_made_is_said_in_fixed_words() {
    let (_dir, root) = project();
    let missing = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": "purlis.toml", "old_string": "absent", "new_string": "x"},
    });
    let twice = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": "purlis.toml", "old_string": "a", "new_string": "b"},
    });
    std::fs::write(root.join("purlis.toml"), "[plane]\nname = \"a a\"\n").expect("manifest");
    for edit in [missing, twice] {
        assert_eq!(
            from_edit(&root, &edit, &root),
            Some(Err(UNPREPARED.to_owned()))
        );
    }
}

/// What Claude Code itself would refuse is refused: an edit that changes nothing, no edits, and
/// a MultiEdit one of whose edits fails.
#[test]
fn an_edit_claude_code_would_refuse_is_refused() {
    let now = Some("a b c");
    for (tool, input) in [
        (
            "Edit",
            serde_json::json!({"file_path": "x", "old_string": "a", "new_string": "a"}),
        ),
        (
            "MultiEdit",
            serde_json::json!({"file_path": "x", "edits": []}),
        ),
        (
            "MultiEdit",
            serde_json::json!({"file_path": "x", "edits": [
                {"old_string": "a", "new_string": "z"},
                {"old_string": "missing", "new_string": "y"}
            ]}),
        ),
    ] {
        assert!(apply_edit(tool, &input, now).is_err(), "{tool} {input}");
    }
}

/// The size cap is a boundary, not a cut: a whole content of [`MOST_BYTES`] is proposed whole,
/// one byte more is not proposed at all; and an edit carries only its own text, so a file far
/// larger than that is proposed as its edit, up to [`MOST_FILE_BYTES`].
#[test]
fn a_proposal_is_never_cut_and_an_edit_carries_only_its_own_text() {
    let (_dir, root) = project();
    let rel = Path::new("personas/devops/persona.md");
    let at_most = "x".repeat(MOST_BYTES);
    let whole = read_and_apply(
        &root,
        rel,
        Change::Whole {
            content: at_most.clone(),
        },
    )
    .expect("whole");
    assert_eq!(whole.after.len(), MOST_BYTES);
    assert_eq!(
        read_and_apply(
            &root,
            rel,
            Change::Whole {
                content: format!("{at_most}x")
            }
        ),
        Err(UNPREPARED.to_owned())
    );

    // A persona of 200 KiB, one line of it changed.
    let large = format!(
        "---\nname: devops\n---\n{}",
        "words ".repeat(200 * 1024 / 6)
    );
    std::fs::write(root.join(rel), &large).expect("a large persona");
    let edit = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": "personas/devops/persona.md", "old_string": "name: devops", "new_string": "name: ops"},
    });
    let proposed = from_edit(&root, &edit, &root)
        .expect("ours")
        .expect("a proposal");
    assert!(proposed.change.carried() < 32);
    assert_eq!(
        proposed.after,
        large.replacen("name: devops", "name: ops", 1)
    );

    // A file past what a proposal may change is not read at all.
    std::fs::write(root.join(rel), "x".repeat(MOST_FILE_BYTES + 1)).expect("a longer file");
    assert_eq!(
        from_edit(&root, &edit, &root),
        Some(Err(UNPREPARED.to_owned()))
    );
}

/// PC-3, PC-8: the app makes the change to the file as it reads it, only while it is what the
/// proposal was worked out against, and writes exactly that, keeping the file's permissions.
#[cfg(unix)]
#[test]
fn the_app_makes_and_writes_the_change_only_while_the_file_is_what_it_was() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = project();
    let rel = Path::new("purlis.toml");
    std::fs::set_permissions(root.join(rel), std::fs::Permissions::from_mode(0o640))
        .expect("a mode");
    let change = Change::Edits {
        edits: vec![Edit {
            old: "[plane]".to_owned(),
            new: "[plane]\nmode = \"pr\"".to_owned(),
            every: false,
        }],
    };
    let base = base_of(Some(b"[plane]\n"));
    let prepared = prepare(&root, rel, Some(base.as_deref()), &change).expect("made");
    assert_eq!(prepared.before.as_deref(), Some("[plane]\n"));
    write_pinned(&root, rel, base.as_deref(), &prepared.after).expect("written");
    assert_eq!(
        std::fs::read_to_string(root.join(rel)).expect("read"),
        "[plane]\nmode = \"pr\"\n"
    );
    assert_eq!(
        std::fs::metadata(root.join(rel))
            .expect("there")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    // The file changed since: stale, at both steps, and nothing written.
    assert_eq!(
        prepare(&root, rel, Some(base.as_deref()), &change),
        Err(NotMade::Stale)
    );
    assert_eq!(
        write_pinned(&root, rel, base.as_deref(), "x"),
        Err(NotMade::Stale)
    );
    // A new file in a skill folder that is not there yet is made with it.
    let skill = Path::new(".claude/skills/review/SKILL.md");
    write_pinned(&root, skill, None, "# Review\n").expect("a new skill");
    assert_eq!(
        std::fs::read_to_string(root.join(skill)).expect("read"),
        "# Review\n"
    );
    // One a chat made since it was proposed as new: stale.
    assert_eq!(write_pinned(&root, skill, None, "x"), Err(NotMade::Stale));
}

/// A proposal never makes a persona or a workspace (the dispatcher's decision, to be
/// confirmed): a file in one that is not there is refused, and a new skill folder is made.
#[test]
fn a_proposal_makes_no_new_persona_or_workspace() {
    let (_dir, root) = project();
    for no in [
        "personas/newcomer/persona.md",
        "personas/newcomer/mcp.json",
        "workspaces/newcomer/purlis.toml",
        "personas/newcomer/.claude/skills/x/SKILL.md",
    ] {
        assert_eq!(
            proposable(&root, &root.join(no)),
            Err(NO_NEW_FOLDER.to_owned()),
            "{no}"
        );
    }
    for yes in [
        "personas/devops/persona.md",
        ".claude/skills/new-skill/SKILL.md",
        "workspaces/alpha/.claude/skills/new/SKILL.md",
        "personas/devops/.claude/skills/new/scripts/run.py",
    ] {
        assert!(proposable(&root, &root.join(yes)).is_ok(), "{yes}");
    }
}

/// The app's write makes no folder but a skill's, and its new file's name is drawn new each
/// time: one left behind never blocks it.
#[cfg(unix)]
#[test]
fn the_app_s_write_makes_no_persona_and_is_not_blocked_by_a_leftover() {
    let (_dir, root) = project();
    assert_eq!(
        write_pinned(&root, Path::new("personas/newcomer/persona.md"), None, "x"),
        Err(NotMade::Stale)
    );
    assert!(!root.join("personas/newcomer").exists());
    // A leftover of the old, guessable name beside the file changes nothing.
    std::fs::write(
        root.join("personas/devops/.persona.md.purlis-apply-1"),
        "left",
    )
    .expect("a leftover");
    write_pinned(
        &root,
        Path::new("personas/devops/persona.md"),
        None,
        "# Devops\n",
    )
    .expect("written");
    assert_eq!(
        std::fs::read_to_string(root.join("personas/devops/persona.md")).expect("read"),
        "# Devops\n"
    );
    let left: Vec<_> = std::fs::read_dir(root.join("personas/devops"))
        .expect("a folder")
        .filter_map(Result::ok)
        .map(|one| one.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("purlis-apply") && name != ".persona.md.purlis-apply-1")
        .collect();
    assert!(left.is_empty(), "no new file left behind: {left:?}");
}

/// #3467: purlis proposes a change to the project's two settings files alone, and only to the
/// one there by the name the project spells it; a chat's own judgement is not widened.
#[test]
fn purlis_proposes_only_to_the_projects_two_settings_files() {
    let (_dir, root) = project();
    std::fs::write(root.join("charter.local.toml"), "a = 1\n").expect("local");
    for yes in ["purlis.toml", "charter.local.toml"] {
        assert_eq!(
            proposable_by_purlis(&root, &root.join(yes)).as_deref(),
            Ok(Path::new(yes)),
            "{yes}"
        );
    }
    for no in [
        "charter.toml",
        "workspaces/alpha/purlis.toml",
        "personas/devops/persona.md",
        "../purlis.toml",
    ] {
        assert!(proposable_by_purlis(&root, &root.join(no)).is_err(), "{no}");
    }
    assert!(
        proposable(&root, &root.join("charter.local.toml")).is_err(),
        "no chat proposes to the Local file"
    );
}

#[cfg(unix)]
#[test]
fn purlis_proposes_to_no_settings_file_that_is_a_link() {
    let (dir, root) = project();
    let elsewhere = dir.path().join("elsewhere.toml");
    std::fs::write(&elsewhere, "a = 1\n").expect("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, root.join("charter.local.toml")).expect("link");
    assert!(proposable_by_purlis(&root, &root.join("charter.local.toml")).is_err());
}

/// #3486: a workspace's manifest, where the workspace and its manifest are there.
#[test]
fn purlis_proposes_to_a_workspaces_manifest_that_is_there() {
    let (_dir, root) = project();
    let manifest = "workspaces/alpha/workspace.json";
    assert!(
        proposable_by_purlis(&root, &root.join(manifest)).is_err(),
        "not there yet"
    );
    std::fs::write(root.join(manifest), "{}\n").expect("its manifest");
    assert_eq!(
        proposable_by_purlis(&root, &root.join(manifest)).as_deref(),
        Ok(Path::new(manifest))
    );
    for no in [
        "workspaces/beta/workspace.json",
        "workspaces/alpha/repo/workspace.json",
        "workspaces/.hidden/workspace.json",
        "workspaces/alpha/purlis.toml",
    ] {
        assert!(proposable_by_purlis(&root, &root.join(no)).is_err(), "{no}");
    }
}

#[cfg(unix)]
#[test]
fn purlis_proposes_to_no_manifest_in_a_workspace_that_is_a_link() {
    let (dir, root) = project();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("elsewhere");
    std::fs::write(elsewhere.join("workspace.json"), "{}\n").expect("a manifest");
    std::os::unix::fs::symlink(&elsewhere, root.join("workspaces/gamma")).expect("link");
    assert!(proposable_by_purlis(&root, &root.join("workspaces/gamma/workspace.json")).is_err());
}

/// #3486: purlis's change to a workspace's manifest is written by the manifest's own writer,
/// only while it is what the change was worked out against.
#[test]
fn purlis_writes_a_workspaces_manifest_through_its_own_writer_and_only_unchanged() {
    let (_dir, root) = project();
    let rel = Path::new("workspaces/alpha/workspace.json");
    let before = "{\"name\": \"alpha\", \"settings\": {\"x\": 1}}";
    std::fs::write(root.join(rel), before).expect("its manifest");
    let after = "{\n  \"name\": \"alpha\"\n}\n";
    let base = base_of(Some(before.as_bytes()));
    assert_eq!(
        write_by_purlis(&root, rel, base.as_deref(), Some(before), after),
        Ok(())
    );
    assert_eq!(
        std::fs::read_to_string(root.join(rel)).expect("read"),
        after,
        "a hand's manifest is written as shown"
    );
    let base = base_of(Some(after.as_bytes()));
    std::fs::write(root.join(rel), "{\"name\": \"alpha\", \"moved\": 1}").expect("moved");
    assert_eq!(
        write_by_purlis(&root, rel, base.as_deref(), Some(after), "{}\n"),
        Err((NotMade::Stale, None))
    );
}
