// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's write to the policy that governs it (M75-16, #7025): what is held, what is not.

use std::path::PathBuf;

use serde_json::json;

use super::*;

/// A plane root with its settings, and a branch folder below it that holds its own copy of the
/// policy.
struct Plane {
    _dir: tempfile::TempDir,
    root: PathBuf,
    branch: PathBuf,
}

const SETTINGS: &str = "[policy]\ntemplate = \"strict\"\n\n[other]\nkey = 1\n";

fn plane() -> Plane {
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    let branch = root.join(".worktrees/repo/branch");
    std::fs::create_dir_all(&branch).expect("a branch folder");
    std::fs::write(root.join("purlis.toml"), SETTINGS).expect("settings");
    std::fs::write(root.join("policy.cedar"), "// purlis-policy 1\n").expect("rules");
    std::fs::write(branch.join("policy.cedar"), "// purlis-policy 1\n").expect("a copy");
    Plane {
        _dir: dir,
        root,
        branch,
    }
}

impl Plane {
    fn tool(&self, tool: &str, input: serde_json::Value, from: &std::path::Path) -> Option<Held> {
        let path = input["file_path"].as_str().expect("a path").to_owned();
        by_tool(&self.root, &from.to_string_lossy(), tool, &input, &[path])
    }

    fn shell(&self, command: &str, from: &std::path::Path) -> Option<Held> {
        by_command(&self.root, &from.to_string_lossy(), command)
    }
}

#[test]
fn a_chats_write_to_the_policy_file_is_held() {
    let p = plane();
    let file = p.root.join("policy.cedar").display().to_string();
    // A file tool, by its absolute path, or by a path that climbs out of the branch folder.
    for (tool, input) in [
        ("Write", json!({"file_path": file, "content": "x"})),
        (
            "Edit",
            json!({"file_path": file, "old_string": "a", "new_string": "b"}),
        ),
        (
            "MultiEdit",
            json!({"file_path": "../../../policy.cedar", "edits": [{"old_string": "a", "new_string": "b"}]}),
        ),
    ] {
        assert_eq!(p.tool(tool, input, &p.branch), Some(Held::File), "{tool}");
    }
    // A shell command, whichever way it writes.
    let root = p.root.display();
    for command in [
        format!("echo 'permit (principal, action, resource);' >> {root}/policy.cedar"),
        format!("echo x>{root}/policy.cedar"),
        format!("tee {root}/policy.cedar < new.cedar"),
        format!("sed -i '' s/a/b/ {root}/policy.cedar"),
        format!("cp ../../../x/policy.cedar {root}/policy.cedar"),
        format!("mv policy.cedar {root}/policy.cedar"),
        format!("cd {root} && printf x > policy.cedar"),
        "cp ../../../../../policy.cedar ../../..".to_owned(),
        format!("git -C {root} checkout main -- policy.cedar"),
        format!("rm {root}/*.cedar"),
        format!("bash -c 'echo x > {root}/policy.cedar'"),
        format!("echo x > $ROOT/policy.cedar # {root}"),
    ] {
        assert_eq!(p.shell(&command, &p.branch), Some(Held::File), "{command}");
    }
    // The refusal names the rule, and the way that stays open.
    let said = Held::File.said();
    assert!(
        said.contains(RULE) && said.contains("purlis worktree commit"),
        "{said}"
    );
}

#[test]
fn a_chats_write_to_the_policy_file_is_held_only_at_the_plane_root() {
    let p = plane();
    // The branch folder's own copy is the chat's to edit and commit.
    let copy = p.branch.join("policy.cedar").display().to_string();
    assert_eq!(
        p.tool(
            "Write",
            json!({"file_path": copy, "content": "x"}),
            &p.branch
        ),
        None
    );
    for command in [
        "echo x >> policy.cedar",
        "sed -i '' s/a/b/ policy.cedar",
        "git add policy.cedar && git commit -m 'edit policy.cedar'",
        // Only reading the root's.
        &format!("cat {}/policy.cedar", p.root.display()),
        &format!("grep -n permit {}/policy.cedar", p.root.display()),
        &format!("git -C {} diff policy.cedar", p.root.display()),
        "ls",
    ] {
        assert_eq!(p.shell(command, &p.branch), None, "{command}");
    }
}

#[test]
fn a_chats_write_to_the_policy_table_is_held() {
    let p = plane();
    let file = p.root.join("purlis.toml").display().to_string();
    // A new file, an edit and a whole write that each leave `[policy]` other than it is.
    for (tool, input) in [
        (
            "Write",
            json!({"file_path": file, "content": "[policy]\ntemplate = \"open\"\n"}),
        ),
        (
            "Edit",
            json!({"file_path": file, "old_string": "strict", "new_string": "open"}),
        ),
        (
            "Edit",
            json!({"file_path": file, "old_string": "[other]", "new_string": "[policy.rules]\n\"a\" = \"off\"\n\n[other]"}),
        ),
        // One that leaves a file that does not parse, or that the tool could not make.
        ("Write", json!({"file_path": file, "content": "[policy\n"})),
        (
            "Edit",
            json!({"file_path": file, "old_string": "no such text", "new_string": "x"}),
        ),
    ] {
        assert_eq!(
            p.tool(tool, input.clone(), &p.branch),
            Some(Held::Table),
            "{tool} {input}"
        );
    }
    // The local settings file is the You level's, held the same way.
    let local = p.root.join("purlis.local.toml").display().to_string();
    assert_eq!(
        p.tool(
            "Write",
            json!({"file_path": local, "content": "[policy]\ntemplate = \"open\"\n"}),
            &p.branch
        ),
        Some(Held::Table)
    );
    // A shell command that writes the settings file.
    let root = p.root.display();
    for command in [
        format!("sed -i '' s/strict/open/ {root}/purlis.toml"),
        format!("echo '[policy]' >> {root}/purlis.toml"),
        format!("cd {root} && tee charter.toml < x"),
    ] {
        assert_eq!(p.shell(&command, &p.branch), Some(Held::Table), "{command}");
    }
    assert!(Held::Table.said().contains(RULE));
}

#[test]
fn a_write_to_the_settings_that_leaves_the_policy_table_alone_is_not_held() {
    let p = plane();
    let file = p.root.join("purlis.toml").display().to_string();
    for input in [
        json!({"file_path": file, "old_string": "key = 1", "new_string": "key = 2"}),
        json!({"file_path": file, "content": format!("{SETTINGS}\n[third]\na = 1\n")}),
    ] {
        let tool = if input.get("content").is_some() {
            "Write"
        } else {
            "Edit"
        };
        assert_eq!(p.tool(tool, input, &p.branch), None, "{tool}");
    }
    // Reading the settings, and a branch folder's own copy, are no write of the root's.
    let root = p.root.display();
    for command in [
        format!("cat {root}/purlis.toml"),
        "echo x >> purlis.toml".to_owned(),
        "git commit -m 'edit charter.toml and policy.cedar'".to_owned(),
    ] {
        assert_eq!(p.shell(&command, &p.branch), None, "{command}");
    }
}

#[test]
fn a_command_that_names_no_policy_file_is_never_read() {
    let p = plane();
    // Not one of these is parsed for a policy file: the screen finds none in the text.
    for command in [
        "rm -rf *",
        "echo $(date) > out.txt",
        "bash -c 'ls'",
        "cp a b",
    ] {
        assert_eq!(p.shell(command, &p.branch), None, "{command}");
    }
}

#[test]
fn a_command_that_names_a_policy_file_and_cannot_be_read_is_held() {
    let p = plane();
    for command in [
        "echo \"unbalanced > policy.cedar",
        "echo $(cat x) > policy.cedar",
        "cat <<EOF > policy.cedar\nx\nEOF",
        "xargs -I{} cp {} policy.cedar < list",
    ] {
        assert_eq!(p.shell(command, &p.branch), Some(Held::File), "{command}");
    }
}

#[test]
fn a_redirection_in_front_of_the_command_or_after_a_reader_is_a_write() {
    let p = plane();
    let root = p.root.display();
    for command in [
        format!("> {root}/policy.cedar"),
        format!(">{root}/policy.cedar"),
        format!(">> {root}/policy.cedar printf x"),
        format!("2>/dev/null > {root}/policy.cedar"),
        format!("cat x > {root}/policy.cedar"),
        format!("cat x >| {root}/policy.cedar"),
        format!("cat x &> {root}/policy.cedar"),
        format!("cat x >& {root}/policy.cedar"),
        format!("cat x 2>& {root}/policy.cedar"),
        "cat x > $ROOT/policy.cedar".to_owned(),
        "cat x > ~/policy.cedar".to_owned(),
        "cat x > \"$R\"/policy.cedar".to_owned(),
    ] {
        assert_eq!(p.shell(&command, &p.branch), Some(Held::File), "{command}");
    }
    assert_eq!(
        p.shell("> policy.cedar", &p.root),
        Some(Held::File),
        "a bare redirection at the root"
    );
    // Output sent elsewhere, or a descriptor copied, writes no policy file.
    for command in [
        format!("cat {root}/policy.cedar > out.txt"),
        format!("cat {root}/policy.cedar 2>&1"),
        "> policy.cedar".to_owned(),
    ] {
        assert_eq!(p.shell(&command, &p.branch), None, "{command}");
    }
}

#[test]
fn a_git_reader_that_writes_with_output_is_a_write() {
    let p = plane();
    let root = p.root.display();
    for command in [
        format!("git -C {root} diff --output=policy.cedar"),
        format!("git -C {root} show HEAD --output policy.cedar"),
        format!("git -C {root} log --out=policy.cedar"),
        format!("git diff --output={root}/policy.cedar"),
        format!("git -C {root} diff --output=purlis.toml"),
    ] {
        assert!(p.shell(&command, &p.branch).is_some(), "{command}");
    }
    // The same readers without it stay readers.
    assert_eq!(
        p.shell(&format!("git -C {root} diff policy.cedar"), &p.branch),
        None
    );
}

#[test]
fn a_settings_file_copied_into_the_root_is_held_like_the_rules() {
    let p = plane();
    let root = p.root.display();
    for command in [
        format!("cp x/purlis.toml {root}/"),
        format!("mv ../x/purlis.local.toml {root}"),
        format!("git -C {root} checkout main -- purlis.toml"),
    ] {
        assert_eq!(p.shell(&command, &p.branch), Some(Held::Table), "{command}");
    }
    // The branch folder's own copy is the chat's.
    assert_eq!(p.shell("git checkout main -- purlis.toml", &p.branch), None);
    assert_eq!(p.shell("cp x/purlis.toml .", &p.branch), None);
}

#[test]
fn a_spelling_in_other_case_is_the_same_file_where_the_file_system_folds_case() {
    let p = plane();
    let root = p.root.display().to_string();
    let upper = format!("{root}/POLICY.cedar");
    let settings = format!("{root}/Purlis.toml");
    let from = p.branch.to_string_lossy().into_owned();
    let tool = |fold: bool, file: &str| {
        by_tool_folding(
            fold,
            &p.root,
            &from,
            "Write",
            &json!({"file_path": file, "content": "x"}),
            &[file.to_owned()],
        )
    };
    assert_eq!(tool(true, &upper), Some(Held::File));
    assert_eq!(tool(true, &settings), Some(Held::Table));
    assert_eq!(tool(false, &upper), None);
    for command in [
        format!("echo x > {upper}"),
        format!("echo x > {root}/policy.CEDAR"),
        format!("cp a {root}/POLICY.CEDAR"),
    ] {
        assert_eq!(
            by_command_folding(true, &p.root, &from, &command),
            Some(Held::File),
            "{command}"
        );
        assert_eq!(
            by_command_folding(false, &p.root, &from, &command),
            None,
            "{command}"
        );
    }
    assert_eq!(
        by_command_folding(
            true,
            &p.root,
            &from,
            &format!("sed -i '' s/a/b/ {settings}")
        ),
        Some(Held::Table)
    );
}

#[test]
fn a_tool_that_is_not_a_file_tool_is_held_on_the_path_alone() {
    let p = plane();
    let from = p.branch.to_string_lossy().into_owned();
    for (file, held) in [("policy.cedar", Held::File), ("purlis.toml", Held::Table)] {
        let file = p.root.join(file).display().to_string();
        let held_at_it = by_tool(&p.root, &from, "NotebookEdit", &json!({}), &[file]);
        assert_eq!(held_at_it, Some(held));
    }
    let copy = p.branch.join("policy.cedar").display().to_string();
    assert_eq!(
        by_tool(&p.root, &from, "NotebookEdit", &json!({}), &[copy]),
        None
    );
}

#[cfg(unix)]
#[test]
fn a_link_to_the_policy_file_is_the_policy_file() {
    let p = plane();
    let real = p.root.join("policy.cedar");
    std::os::unix::fs::symlink(&real, p.branch.join("link.cedar")).expect("a link");
    // By a file tool, and by a shell command that writes through it.
    assert_eq!(
        p.tool(
            "Write",
            json!({"file_path": "link.cedar", "content": "x"}),
            &p.branch
        ),
        Some(Held::File)
    );
    for command in [
        "echo x > link.cedar",
        "cat x > link.cedar",
        "tee link.cedar < x",
    ] {
        assert_eq!(p.shell(command, &p.branch), Some(Held::File), "{command}");
    }
    assert_eq!(p.shell("cat link.cedar", &p.branch), None);
}

#[test]
fn a_shell_write_with_cat_into_the_policy_file_is_held() {
    let p = plane();
    let root = p.root.display();
    for command in [
        format!("cat x > {root}/policy.cedar"),
        format!("cat a b >> {root}/policy.cedar"),
        format!("cd {root} && cat x > policy.cedar"),
    ] {
        assert_eq!(p.shell(&command, &p.branch), Some(Held::File), "{command}");
    }
}
