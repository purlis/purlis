// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What a plane that is not migrated writes: charter's markers (V93g).
const LIVE_BEGIN: &str = names::LIVE_BEGIN.reads[0];
const LIVE_END: &str = names::LIVE_END.reads[0];

/// A plane nobody migrated — no manifest at all to say otherwise.
fn unmigrated() -> &'static Path {
    Path::new("/nonexistent-plane")
}

fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

#[test]
fn the_block_lists_each_path_twice_and_re_ignores_the_landing_log() {
    let block = live_block(unmigrated(), ["beta"]);
    let lines: Vec<&str> = block.lines().collect();
    assert_eq!(lines[0], LIVE_BEGIN);
    assert_eq!(lines[lines.len() - 1], LIVE_END);
    assert!(block.contains("!/workspaces/beta/memory\n!/workspaces/beta/memory/**"));
    assert!(block.contains("!/workspaces/beta/todos\n!/workspaces/beta/todos/**"));
    assert!(
        block.contains("!/workspaces/beta/work\n!/workspaces/beta/work/**"),
        "a LIVE workspace's work link log is committed with it (ADR 0088 §3)"
    );
    assert!(
        block.contains("!/workspaces/beta/changes/**\n/workspaces/beta/changes/log/"),
        "the log is re-ignored, and only after its parent is re-included"
    );
}

#[test]
fn the_block_publishes_a_live_workspaces_session_records_with_it() {
    let block = live_block(unmigrated(), ["beta"]);
    assert!(
        block.contains("!/workspaces/beta/sessions\n!/workspaces/beta/sessions/**\n"),
        "{block}"
    );
}

#[test]
fn the_block_shares_a_live_workspaces_skills_and_never_its_local_ones() {
    // ADR 0099, #2767: `skills/` follows the workspace, `skills.local/` stays ignored under
    // `/workspaces/*/*`, which nothing in the block re-includes.
    let block = live_block(unmigrated(), ["beta"]);
    assert!(
        block.contains("!/workspaces/beta/skills\n!/workspaces/beta/skills/**\n"),
        "{block}"
    );
    assert!(!block.contains("skills.local"), "{block}");
}

#[test]
fn a_live_workspaces_skills_are_among_its_shareable_paths() {
    let dir = plane();
    let skill = dir.path().join("workspaces/beta/skills/review");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: review\ndescription: x\n---\n",
    )
    .unwrap();
    assert!(
        meta_paths(dir.path(), "beta").contains(&"workspaces/beta/skills".to_owned()),
        "{:?}",
        meta_paths(dir.path(), "beta")
    );
}

#[test]
fn a_live_workspaces_session_records_are_among_its_shareable_paths() {
    let dir = plane();
    let ws = dir.path().join("workspaces").join("beta");
    std::fs::create_dir_all(ws.join("sessions")).unwrap();
    std::fs::write(ws.join("sessions").join("index.md"), "").unwrap();
    assert_eq!(
        meta_paths(dir.path(), "beta"),
        vec!["workspaces/beta/sessions".to_string()]
    );
}

#[test]
fn the_block_is_written_where_charter_init_left_room_for_it() {
    let dir = plane();
    std::fs::write(
        dir.path().join(".gitignore"),
        "/workspaces/*/*\n!/workspaces/.gitkeep\nnode_modules/\n",
    )
    .unwrap();
    write_live_block(dir.path(), ["beta"]).unwrap();
    let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    let at_gitkeep = text.find("!/workspaces/.gitkeep").unwrap();
    let at_block = text.find(LIVE_BEGIN).unwrap();
    let at_modules = text.find("node_modules/").unwrap();
    assert!(at_gitkeep < at_block && at_block < at_modules, "{text}");
}

#[test]
fn rewriting_replaces_the_block_and_keeps_everything_around_it() {
    let dir = plane();
    write_live_block(dir.path(), ["beta"]).unwrap();
    std::fs::write(
        dir.path().join(".gitignore"),
        format!(
            "before\n{}\nafter\n",
            std::fs::read_to_string(dir.path().join(".gitignore"))
                .unwrap()
                .trim()
        ),
    )
    .unwrap();
    write_live_block(dir.path(), ["alpha"]).unwrap();
    let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(text.starts_with("before\n"), "{text}");
    assert!(text.ends_with("after\n"), "{text}");
    assert_eq!(text.matches(LIVE_BEGIN).count(), 1, "{text}");
    assert!(!text.contains("/beta/"), "{text}");
    assert!(text.contains("!/workspaces/alpha/workspace.json"), "{text}");
}

#[test]
fn liveness_round_trips_through_the_block_and_nothing_else() {
    let dir = plane();
    assert!(live_workspaces(dir.path()).is_empty());
    assert!(set_live(dir.path(), "beta", true).unwrap());
    assert!(
        !set_live(dir.path(), "beta", true).unwrap(),
        "already LIVE is not a change"
    );
    assert_eq!(
        live_workspaces(dir.path()),
        BTreeSet::from(["beta".to_string()])
    );
    assert!(set_live(dir.path(), "beta", false).unwrap());
    assert!(live_workspaces(dir.path()).is_empty());
    assert!(
        !set_live(dir.path(), "beta", false).unwrap(),
        "already LOCAL is not a change"
    );
}

/// The block exactly as charter wrote it before the rename, between somebody's lines.
const CHARTERS_BLOCK: &str = "head\n\
    # >>> charter live workspaces (managed by `charter workspace live`) >>>\n\
    !/workspaces/alpha/workspace.json\n\
    # <<< charter live workspaces <<<\n\
    tail\n";

#[test]
fn a_live_block_charter_wrote_is_read_and_rewritten_in_place_under_the_purlis_markers() {
    let dir = plane();
    // Migrated: the committed block takes purlis's markers on its next write.
    std::fs::write(dir.path().join("purlis.toml"), "").unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, CHARTERS_BLOCK).unwrap();
    assert_eq!(
        live_workspaces(dir.path()),
        BTreeSet::from(["alpha".to_string()]),
        "the old block is recognised"
    );

    assert!(set_live(dir.path(), "beta", true).unwrap());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("charter live workspaces"), "{text}");
    assert_eq!(
        text,
        format!(
            "head\n{}\ntail\n",
            live_block(dir.path(), ["alpha", "beta"])
        ),
        "rewritten where it stood, once"
    );
    assert!(
        text.contains("\n# >>> purlis live workspaces (managed by `purlis workspace live`) >>>\n"),
        "{text}"
    );
    assert_eq!(
        live_workspaces(dir.path()),
        BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
        "the purlis block is recognised"
    );
}

#[test]
fn a_plane_not_yet_migrated_keeps_charters_live_block_and_its_markers() {
    let dir = plane();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, CHARTERS_BLOCK).unwrap();
    // Already what the block would be: not a byte moves.
    assert!(!set_live(dir.path(), "alpha", true).unwrap());
    refresh_live_block(dir.path()).unwrap();
    let refreshed = std::fs::read_to_string(&path).unwrap();
    assert!(!refreshed.contains("purlis"), "{refreshed}");

    assert!(set_live(dir.path(), "beta", true).unwrap());
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        format!(
            "head\n{}\ntail\n",
            live_block(dir.path(), ["alpha", "beta"])
        )
    );
    assert!(text.contains(&format!("\n{LIVE_BEGIN}\n")), "{text}");
    assert!(!text.contains("purlis"), "{text}");
    assert_eq!(text.matches("live workspaces (managed by").count(), 1);
}

#[test]
fn a_block_opened_under_one_name_and_closed_under_the_other_is_one_block() {
    let dir = plane();
    let path = dir.path().join(".gitignore");
    std::fs::write(
        &path,
        CHARTERS_BLOCK.replace("# <<< charter live workspaces <<<", names::LIVE_END.write),
    )
    .unwrap();
    assert!(set_live(dir.path(), "alpha", false).unwrap());
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        text,
        format!("head\n{}\ntail\n", live_block(unmigrated(), [])),
        "{text}"
    );
}

#[test]
fn a_second_managed_block_is_rewritten_too_and_never_left_behind() {
    // The differential found this: Python's `re.sub` replaces EVERY span, and a first
    // port replaced one. A `live --off` then left the workspace un-ignored by the block
    // further down, so the next `charter save` would have committed its memory.
    let dir = plane();
    let block = live_block(unmigrated(), ["alpha"]);
    std::fs::write(
        dir.path().join(".gitignore"),
        format!("head\n{block}\nmiddle\n{block}\ntail\n"),
    )
    .unwrap();
    assert_eq!(
        live_workspaces(dir.path()),
        BTreeSet::from(["alpha".to_string()])
    );

    assert!(set_live(dir.path(), "alpha", false).unwrap());
    let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(!text.contains("/alpha/"), "{text}");
    assert_eq!(text.matches(LIVE_BEGIN).count(), 2, "{text}");
    assert!(text.starts_with("head\n"), "{text}");
    assert!(text.contains("\nmiddle\n"), "{text}");
    assert!(text.ends_with("\ntail\n"), "{text}");
}

#[test]
fn a_begin_with_no_end_after_it_is_left_exactly_as_it_is() {
    let dir = plane();
    let torn = format!("head\n{LIVE_BEGIN}\n!/workspaces/alpha/workspace.json\n");
    std::fs::write(dir.path().join(".gitignore"), &torn).unwrap();
    // The line is inside an unterminated block, so it still reads as LIVE…
    assert_eq!(
        live_workspaces(dir.path()),
        BTreeSet::from(["alpha".to_string()])
    );
    // …and the rewrite matches nothing, exactly as `re.sub` would.
    set_live(dir.path(), "alpha", false).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
        torn
    );
}

#[test]
fn a_line_outside_the_managed_block_does_not_make_a_workspace_live() {
    let dir = plane();
    std::fs::write(
        dir.path().join(".gitignore"),
        "!/workspaces/sneaky/workspace.json\n",
    )
    .unwrap();
    assert!(live_workspaces(dir.path()).is_empty());
}

#[test]
fn the_shareable_paths_are_only_the_ones_that_are_there() {
    let dir = plane();
    let ws = dir.path().join("workspaces").join("beta");
    std::fs::create_dir_all(ws.join("memory")).unwrap();
    std::fs::write(ws.join("workspace.md"), "").unwrap();
    assert_eq!(
        meta_paths(dir.path(), "beta"),
        vec!["workspaces/beta/workspace.md", "workspaces/beta/memory"],
        "a path git was never given is one `git rm --cached` fails the whole call on"
    );
}

#[test]
fn a_live_workspaces_work_link_log_is_staged_with_its_todos() {
    let dir = plane();
    let ws = dir.path().join("workspaces").join("beta");
    std::fs::create_dir_all(ws.join("todos")).unwrap();
    std::fs::create_dir_all(ws.join("work")).unwrap();
    assert_eq!(
        meta_paths(dir.path(), "beta"),
        vec!["workspaces/beta/todos", "workspaces/beta/work"]
    );
}

#[test]
fn a_changes_directory_with_no_record_in_it_is_not_a_shareable_path() {
    let dir = plane();
    let ws = dir.path().join("workspaces").join("beta");
    std::fs::create_dir_all(ws.join("changes").join("log")).unwrap();
    assert_eq!(meta_paths(dir.path(), "beta"), Vec::<String>::new());
    std::fs::write(ws.join("changes").join("r.json"), "{}").unwrap();
    assert_eq!(
        meta_paths(dir.path(), "beta"),
        vec!["workspaces/beta/changes".to_string()]
    );
}

#[test]
fn a_dangling_link_at_a_workspaces_name_is_not_nothing() {
    let dir = plane();
    std::fs::create_dir_all(dir.path().join("workspaces")).unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("nowhere"),
        dir.path().join("workspaces").join("beta"),
    )
    .unwrap();
    assert!(!dir.path().join("workspaces").join("beta").exists());
    assert!(
        workspace_dir_exists(dir.path(), "beta"),
        "exists() follows the link; a workspace charter would write through is not absent"
    );
}

/// #358: the LIVE block replaces `.gitignore` by rename. A crash between the write and
/// the rename leaves the file that keeps `/.charter/` out of git exactly as it was.
#[test]
fn the_live_block_replaces_gitignore_whole_or_not_at_all() {
    let dir = plane();
    let before = "/workspaces/*/*\n!/workspaces/.gitkeep\n/.charter/\n";
    std::fs::write(dir.path().join(".gitignore"), before).unwrap();
    let _hook = crate::rewrite::hook::set(move |target, temp| {
        assert_eq!(std::fs::read_to_string(target).unwrap(), before);
        let next = std::fs::read_to_string(temp).unwrap();
        assert!(
            next.contains("/.charter/") && next.contains(LIVE_END),
            "{next}"
        );
        Err(io::Error::other("killed before the rename"))
    });

    assert!(set_live(dir.path(), "beta", true).is_err());

    assert_eq!(
        std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
        before
    );
    let strays: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(strays.is_empty(), "{strays:?}");
}

/// #358: two workspaces made LIVE at once. The second `set_live` starts while the first
/// is between its read and its rename; it must see the first one's block, not the old one.
#[test]
fn two_workspaces_made_live_at_once_are_both_live() {
    let dir = plane();
    let root = dir.path().to_path_buf();
    let second: std::rc::Rc<std::cell::RefCell<Option<std::thread::JoinHandle<()>>>> =
        Default::default();
    let started = second.clone();
    let _hook = crate::rewrite::hook::set(move |_, _| {
        if started.borrow().is_none() {
            let root = root.clone();
            let (done, finished) = std::sync::mpsc::channel();
            *started.borrow_mut() = Some(std::thread::spawn(move || {
                set_live(&root, "gamma", true).unwrap();
                let _ = done.send(());
            }));
            let _ = finished.recv_timeout(std::time::Duration::from_millis(300));
        }
        Ok(())
    });

    set_live(dir.path(), "beta", true).unwrap();
    second
        .borrow_mut()
        .take()
        .expect("the hook ran")
        .join()
        .unwrap();

    assert_eq!(
        live_workspaces(dir.path()),
        BTreeSet::from(["beta".to_string(), "gamma".to_string()])
    );
}

/// A block as a charter before SK-4 wrote it for `names`: no `skills` pair.
fn older_block(names: &[&str]) -> String {
    let mut lines = vec![LIVE_BEGIN.to_owned()];
    for n in names {
        for path in ["workspace.json", "memory", "memory/**"] {
            lines.push(format!("!/workspaces/{n}/{path}"));
        }
        lines.push(format!("/workspaces/{n}/changes/log/"));
    }
    lines.push(LIVE_END.to_owned());
    lines.join("\n")
}

/// The local skills ignore lines a current baseline has.
fn local_ignores() -> String {
    crate::scaffold::skills_local_ignores().join("\n")
}

#[test]
fn an_older_block_lacks_each_live_workspaces_skills_pair_and_a_current_one_lacks_none() {
    let text = format!("{}\n{}\n", older_block(&["alpha", "beta"]), local_ignores());
    let missing = skills_lines_in(&text);
    assert_eq!(
        missing.unshared,
        BTreeMap::from([
            (
                "alpha".to_owned(),
                vec![
                    "!/workspaces/alpha/skills".to_owned(),
                    "!/workspaces/alpha/skills/**".to_owned()
                ]
            ),
            (
                "beta".to_owned(),
                vec![
                    "!/workspaces/beta/skills".to_owned(),
                    "!/workspaces/beta/skills/**".to_owned()
                ]
            ),
        ])
    );
    assert_eq!(missing.unignored, Vec::<String>::new());

    let current = format!(
        "{}\n{}\n",
        live_block(unmigrated(), ["alpha", "beta"]),
        local_ignores()
    );
    assert!(skills_lines_in(&current).is_empty());
}

#[test]
fn the_ignore_lines_are_every_levels_by_the_one_rule_a_personas_included() {
    let text = format!("{}\n/skills.local/\n", live_block(unmigrated(), ["alpha"]));
    let missing = skills_lines_in(&text);
    assert!(missing.unshared.is_empty());
    assert!(
        missing
            .unignored
            .contains(&"/personas/*/skills.local/".to_owned()),
        "{missing:?}"
    );
    assert!(!missing.unignored.contains(&"/skills.local/".to_owned()));
    // A pair outside the block shares nothing, so it does not count.
    let outside = format!(
        "{}\n!/workspaces/alpha/skills\n!/workspaces/alpha/skills/**\n",
        older_block(&["alpha"])
    );
    assert!(skills_lines_in(&outside).unshared.contains_key("alpha"));
}

#[test]
fn with_no_live_workspace_nothing_is_missing() {
    assert!(skills_lines_in("").is_empty());
    assert!(skills_lines_in("/workspaces/*/*\n!/workspaces/.gitkeep\n").is_empty());
    assert!(skills_lines_in(&format!("{}\n", older_block(&[]))).is_empty());
}

#[test]
fn adding_the_skills_lines_puts_each_pair_after_its_workspace_and_keeps_every_byte() {
    let dir = plane();
    let text = format!("head\r\n{}\r\ntail", older_block(&["alpha", "beta"]).replace('\n', "\r\n"));
    std::fs::write(dir.path().join(".gitignore"), &text).unwrap();

    let added = add_skills_lines(dir.path()).unwrap();

    assert_eq!(added.unshared.len(), 2, "{added:?}");
    let after = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    let block = after.split("tail").next().unwrap();
    assert!(
        block.contains(
            "/workspaces/alpha/changes/log/\r\n!/workspaces/alpha/skills\r\n\
             !/workspaces/alpha/skills/**\r\n!/workspaces/beta/workspace.json"
        ),
        "{after:?}"
    );
    assert!(
        block.contains("/workspaces/beta/changes/log/\r\n!/workspaces/beta/skills\r\n"),
        "{after:?}"
    );
    // The lines that were there, as they were and in their order.
    let mut kept = after.as_str();
    for line in text.split_inclusive('\n') {
        let at = kept.find(line).unwrap_or_else(|| panic!("{line:?} kept: {after:?}"));
        kept = &kept[at + line.len()..];
    }
    assert!(skills_lines_in(&after).is_empty(), "{after:?}");
    assert_eq!(live_in(&after), live_in(&text));

    // Twice is once.
    assert!(add_skills_lines(dir.path()).unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
        after
    );
}
