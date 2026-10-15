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
    assert_eq!(proposed.content, "[plane]\nmode = \"pr\"\n");
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

/// The size cap is a boundary, not a cut: a file of [`MOST_BYTES`] is proposed whole, one byte
/// more is not proposed at all.
#[test]
fn a_proposal_is_never_cut_at_the_size_cap() {
    let (_dir, root) = project();
    let rel = Path::new("personas/devops/persona.md");
    let at_most = "x".repeat(MOST_BYTES);
    let whole = read_and_apply(&root, rel, |_| Ok(at_most.clone())).expect("whole");
    assert_eq!(whole.content.len(), MOST_BYTES);
    assert_eq!(
        read_and_apply(&root, rel, |_| Ok(format!("{at_most}x"))),
        Err(UNPREPARED.to_owned())
    );
    std::fs::write(root.join(rel), format!("{at_most}x")).expect("a long file");
    assert_eq!(
        read_and_apply(&root, rel, |now| Ok(now.unwrap_or_default().to_owned())),
        Err(UNPREPARED.to_owned())
    );
}
