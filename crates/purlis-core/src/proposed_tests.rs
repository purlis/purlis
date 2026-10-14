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
