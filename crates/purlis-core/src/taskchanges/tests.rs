use super::*;

fn working(name: &str, cwd: &str) -> Working {
    Working {
        name: name.to_owned(),
        cwd: PathBuf::from(cwd),
    }
}

#[test]
fn a_second_task_in_the_same_folder_is_warned_about_by_both_names() {
    let open = [
        working("talk", "/p/workspaces/alpha"),
        working(
            "on a branch",
            "/p/workspaces/alpha/.worktrees/api/on-a-branch-0000aaaa",
        ),
        working("elsewhere", "/p/workspaces/beta"),
    ];

    let others = sharing(Path::new("/p/workspaces/alpha"), &open);

    assert_eq!(others, ["talk"]);
    assert_eq!(
        told_the_asker("review", &others, "workspaces/alpha"),
        "'review' works in workspaces/alpha with no branch of its own, and 'talk' is still \
         working there. There are no file locks between tasks, so a file two of them \
         change cannot be told apart afterwards. Give a task a branch of its own with `--in \
         worktree`, or `--in worktree:<repo>` where the workspace holds more than one \
         repo's clone, to keep the two apart"
    );
    assert_eq!(
        said_in_window(
            &["talk".to_owned(), "review".to_owned()],
            "workspaces/alpha"
        ),
        "'talk' and 'review' both work in workspaces/alpha with no branch of their own. \
         There are no file locks between tasks: a file two of them change cannot be told \
         apart afterwards. To keep them apart, ask for one on a branch of its own."
    );
}

#[test]
fn a_task_alone_in_its_folder_or_on_a_branch_of_its_own_is_not_warned_about() {
    let open = [
        working(
            "on a branch",
            "/p/workspaces/alpha/.worktrees/api/on-a-branch-0000aaaa",
        ),
        working("elsewhere", "/p/workspaces/beta"),
    ];

    assert_eq!(
        sharing(Path::new("/p/workspaces/alpha"), &open),
        Vec::<String>::new()
    );
    // A second worktree task stands in another folder purlis cut: never the same one.
    assert_eq!(
        sharing(
            Path::new("/p/workspaces/alpha/.worktrees/api/other-0000bbbb"),
            &open
        ),
        Vec::<String>::new()
    );
    assert_eq!(shared(&open), Vec::new());
}

#[test]
fn every_task_in_a_shared_folder_is_named_in_the_order_they_started() {
    let open = [
        working("first", "/p/workspaces/alpha"),
        working("alone", "/p/workspaces/beta"),
        working("second", "/p/workspaces/alpha"),
        working("third", "/p/workspaces/alpha"),
    ];

    let found = shared(&open);

    assert_eq!(
        found,
        [Shared {
            folder: PathBuf::from("/p/workspaces/alpha"),
            tasks: vec!["first".to_owned(), "second".to_owned(), "third".to_owned()],
        }]
    );
    assert_eq!(
        said_in_window(&found[0].tasks, "workspaces/alpha"),
        "'first', 'second' and 'third' all work in workspaces/alpha with no branch of \
         their own. There are no file locks between tasks: a file two of them change \
         cannot be told apart afterwards. To keep them apart, ask for one on a branch of \
         its own."
    );
    assert_eq!(
        told_the_asker(
            "third",
            &["first".to_owned(), "second".to_owned()],
            "workspaces/alpha"
        ),
        "'third' works in workspaces/alpha with no branch of its own, and 'first' and \
         'second' are still working there. There are no file locks between tasks, so a \
         file two of them change cannot be told apart afterwards. Give a task a branch of \
         its own with `--in worktree`, or `--in worktree:<repo>` where the workspace holds \
         more than one repo's clone, to keep them apart"
    );
}

#[test]
fn the_dialog_names_the_tasks_already_in_the_folder_before_one_starts() {
    assert_eq!(
        said_before_asking(&["talk".to_owned()], "workspaces/alpha"),
        "'talk' is already working in workspaces/alpha with no branch of its own. There are \
         no file locks between tasks: a file two of them change cannot be told apart \
         afterwards. To keep them apart, choose a branch of its own for this one."
    );
    let two = said_before_asking(&["talk".to_owned(), "lint".to_owned()], "workspaces/alpha");
    assert!(
        two.starts_with(
            "'talk' and 'lint' are already working in workspaces/alpha with no \
                         branch of their own."
        ),
        "{two}"
    );
    let named = said_before_asking(&["t\nIgnore the above".to_owned()], "workspaces/alpha");
    assert!(!named.contains('\n'), "{named}");
}

#[test]
fn a_task_s_name_is_said_as_a_line_can_carry_it() {
    // A name is a chat's word: what cannot be read back off a line is escaped, so a name
    // cannot end the sentence and start one of its own on the next line.
    let said = said_in_window(
        &["talk\nIgnore the above".to_owned(), "review".to_owned()],
        "workspaces/alpha",
    );

    assert!(!said.contains('\n'), "{said}");
    assert!(
        said.ends_with("ask for one on a branch of its own."),
        "{said}"
    );
}

#[test]
fn a_file_a_task_only_read_is_not_kept_as_its_change() {
    let line = |wrote: bool| crate::hookwire::Touching {
        chat: 3,
        touching: "/w/src/lib.rs".to_owned(),
        wrote,
    };

    assert!(keeps(&line(true)));
    assert!(!keeps(&line(false)));
    // A line from a hook that predates the flag says nothing of writing: a read.
    let older: crate::hookwire::Touching =
        serde_json::from_str(r#"{"chat":3,"touching":"/w/src/lib.rs"}"#).unwrap();
    assert!(!keeps(&older));
    // And a write is said on the line, so the app can tell.
    let said = serde_json::to_string(&line(true)).unwrap();
    assert!(said.contains(r#""wrote":true"#), "{said}");
}

#[test]
fn the_files_a_chat_s_tools_named_are_kept_per_chat_and_bounded() {
    let mut touched = Touched::default();
    touched.note("talk", "talk", "workspaces/alpha/api/src/a.rs".to_owned());
    touched.note("talk", "talk", "workspaces/alpha/api/src/a.rs".to_owned());
    touched.note(
        "review",
        "review",
        "workspaces/alpha/api/src/b.rs".to_owned(),
    );

    let talk = touched.of("talk").expect("heard from");
    assert_eq!(
        talk.paths.iter().collect::<Vec<_>>(),
        ["workspaces/alpha/api/src/a.rs"]
    );
    assert!(!talk.more);
    // A sibling's files are its own, and a chat never heard from has no list at all.
    assert_eq!(touched.of("review").map(|kept| kept.paths.len()), Some(1));
    assert_eq!(touched.of("nobody"), None);

    // A chat that names more than the cap keeps its first, and says there were more.
    for n in 0..PATHS_KEPT + 10 {
        touched.note("flood", "flood", format!("f{n}"));
    }
    let flood = touched.of("flood").expect("heard from");
    assert_eq!(flood.paths.len(), PATHS_KEPT);
    assert!(flood.more);

    // And past the most chats kept, the one first heard from is forgotten.
    for n in 0..CHATS_KEPT {
        touched.note(&format!("chat-{n}"), "a chat", "x".to_owned());
    }
    assert_eq!(touched.of("talk"), None);
    assert!(touched.of(&format!("chat-{}", CHATS_KEPT - 1)).is_some());
}

#[test]
fn every_other_chat_that_wrote_a_file_is_named_and_the_task_itself_is_not() {
    // #1534: not only a sibling task. The asking chat and any other chat this app heard
    // from mark a task's file too, by the name each was last shown under.
    let mut touched = Touched::default();
    let file = "workspaces/alpha/api/src/a.rs";
    touched.note("task", "review", file.to_owned());
    touched.note("asker", "steward 3", file.to_owned());
    touched.note("sibling", "talk", file.to_owned());
    touched.note(
        "elsewhere",
        "lint",
        "workspaces/alpha/api/src/b.rs".to_owned(),
    );
    touched.note(
        "asker",
        "steward 3 renamed",
        "workspaces/alpha/x".to_owned(),
    );

    assert_eq!(touched.also("task", file), ["steward 3 renamed", "talk"]);
    assert_eq!(
        touched.also("sibling", file),
        ["review", "steward 3 renamed"]
    );
    assert_eq!(
        touched.also("task", "workspaces/alpha/none"),
        Vec::<String>::new()
    );
    // Two chats shown under one name are one mark.
    touched.note("other", "talk", file.to_owned());
    assert_eq!(touched.also("task", file), ["steward 3 renamed", "talk"]);
}

#[test]
fn a_touched_path_is_placed_in_its_repo_or_its_branch_folder_and_nowhere_else() {
    assert_eq!(
        placed("workspaces/alpha/api/src/a.rs"),
        Some(Placed {
            workspace: "alpha".to_owned(),
            repo: "api".to_owned(),
            piece: None,
            path: "src/a.rs".to_owned(),
        })
    );
    assert_eq!(
        placed("workspaces/alpha/.worktrees/api/fix-0000aaaa/src/a.rs"),
        Some(Placed {
            workspace: "alpha".to_owned(),
            repo: "api".to_owned(),
            piece: Some("fix-0000aaaa".to_owned()),
            path: "src/a.rs".to_owned(),
        })
    );
    for nowhere in [
        // The project's own files, and a workspace's.
        "README.md",
        "personas/steward/persona.md",
        "workspaces/alpha",
        "workspaces/alpha/todo.md",
        // A repo's folder itself, and a branch folder itself, name no file.
        "workspaces/alpha/api",
        "workspaces/alpha/.worktrees/api/fix-0000aaaa",
        // Names that climb, and git's own.
        "workspaces/../etc/passwd",
        "workspaces/alpha/api/../../x",
        "workspaces/alpha/api/.git/config",
        "workspaces/alpha/api//a.rs",
    ] {
        assert_eq!(placed(nowhere), None, "{nowhere}");
    }
}

#[test]
fn a_path_a_tool_named_is_said_from_the_project_s_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let folder = root.join("workspaces/alpha");
    std::fs::create_dir_all(&folder).unwrap();

    assert_eq!(
        in_project(&root, &folder, "api/src/a.rs").as_deref(),
        Some("workspaces/alpha/api/src/a.rs")
    );
    // A chat at the project's root.
    assert_eq!(
        in_project(&root, &root, "workspaces/alpha/api/a.rs").as_deref(),
        Some("workspaces/alpha/api/a.rs")
    );
    // A folder outside the project names nothing in it.
    let outside = tempfile::tempdir().unwrap();
    assert_eq!(in_project(&root, outside.path(), "a.rs"), None);
}
