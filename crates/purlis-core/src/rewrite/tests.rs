use super::*;

#[test]
fn a_replace_that_dies_before_its_rename_leaves_the_old_file_whole_and_no_temp() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("charter.toml");
    std::fs::write(&target, "old\n").unwrap();
    let _hook = hook::set(|target, temp| {
        assert_eq!(std::fs::read_to_string(target).unwrap(), "old\n");
        assert_eq!(std::fs::read_to_string(temp).unwrap(), "new\n");
        Err(io::Error::other("killed"))
    });

    let err = replace(dir.path(), &target, b"new\n", Mode::Kept).unwrap_err();

    assert_eq!(err.to_string(), "killed");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old\n");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(left.len(), 1, "the temp file was cleaned up");
}

#[cfg(unix)]
#[test]
fn a_replace_keeps_the_mode_the_file_had_and_gives_a_new_one_the_mode_asked_for() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let kept = dir.path().join("kept");
    std::fs::write(&kept, "x").unwrap();
    std::fs::set_permissions(&kept, std::fs::Permissions::from_mode(0o640)).unwrap();
    replace(dir.path(), &kept, b"y", Mode::KeptOrPrivate).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&kept), 0o640);

    let fresh = dir.path().join("fresh");
    replace(dir.path(), &fresh, b"z", Mode::KeptOrPrivate).unwrap();
    assert_eq!(mode(&fresh), 0o600);
}

#[cfg(unix)]
#[test]
fn a_replace_refuses_a_target_that_is_a_link_and_leaves_what_it_points_at() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("bashrc");
    std::fs::write(&victim, "mine").unwrap();
    let target = dir.path().join(".gitignore");
    std::os::unix::fs::symlink(&victim, &target).unwrap();

    assert!(replace(dir.path(), &target, b"x", Mode::Kept).is_err());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "mine");
}

#[cfg(unix)]
#[test]
fn a_secret_with_a_second_name_is_refused_and_both_names_keep_their_bytes() {
    use std::os::unix::fs::MetadataExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("app.json");
    std::fs::write(&target, "old\n").unwrap();
    let second = dir.path().join("second.json");
    std::fs::hard_link(&target, &second).unwrap();
    let inode = std::fs::metadata(&target).unwrap().ino();

    let err = replace(dir.path(), &target, b"new\n", Mode::Secret).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string()
            .starts_with(&format!("{} has a second name", target.display())),
        "{err}"
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"old\n");
    assert_eq!(std::fs::read(&second).unwrap(), b"old\n");
    assert_eq!(std::fs::metadata(&target).unwrap().ino(), inode);
    assert_eq!(std::fs::metadata(&target).unwrap().nlink(), 2);
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        2,
        "no temp left"
    );
}

#[cfg(unix)]
#[test]
fn a_secret_refused_for_its_second_name_says_what_to_do_next() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("app.json");
    std::fs::write(&target, "old\n").unwrap();
    std::fs::hard_link(&target, dir.path().join("second.json")).unwrap();

    let err = replace(dir.path(), &target, b"new\n", Mode::Secret).unwrap_err();

    let t = target.display();
    assert_eq!(
        err.to_string(),
        format!(
            "{t} has a second name (a hard link), so purlis does not rewrite it in place. Change \
             the value where the file comes from, or put a plain copy of the file at {t}, then \
             try again"
        )
    );
}

#[cfg(unix)]
#[test]
fn a_second_name_inside_the_project_is_named_in_the_refusal() {
    let project = tempfile::tempdir().unwrap();
    let held = project.path().join("held");
    std::fs::create_dir(&held).unwrap();
    let target = held.join("app.json");
    std::fs::write(&target, "old\n").unwrap();
    let source = project.path().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("other.json"), "not it\n").unwrap();
    let other = source.join("app.json");
    std::fs::hard_link(&target, &other).unwrap();

    let err = replace(&held, &target, b"new\n", Mode::Secret).unwrap_err();
    let err = named_in(err, project.path());

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    let (t, o) = (target.display(), other.display());
    assert_eq!(
        err.to_string(),
        format!(
            "{t} has a second name (a hard link), {o}, so purlis does not rewrite it in place. \
             Change the value at {o}, or put a plain copy of the file at {t}, then try again"
        )
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"old\n");
}

#[cfg(unix)]
#[test]
fn a_second_name_the_search_does_not_enter_is_not_named() {
    let project = tempfile::tempdir().unwrap();
    let target = project.path().join("app.json");
    std::fs::write(&target, "old\n").unwrap();
    let git = project.path().join(".git");
    std::fs::create_dir(&git).unwrap();
    std::fs::hard_link(&target, git.join("app.json")).unwrap();
    let big = project.path().join("big");
    std::fs::create_dir(&big).unwrap();
    for n in 0..=super::SEARCH_FOLDER_ENTRIES {
        std::fs::write(big.join(format!("{n:05}")), "").unwrap();
    }
    std::fs::hard_link(&target, big.join("app.json")).unwrap();

    let err = replace(project.path(), &target, b"new\n", Mode::Secret).unwrap_err();
    let err = named_in(err, project.path());

    assert!(
        err.to_string()
            .contains("Change the value where the file comes from"),
        "{err}"
    );
}

#[test]
fn naming_the_other_name_leaves_any_other_error_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let e = named_in(io::Error::other("disk full"), dir.path());
    assert_eq!(e.to_string(), "disk full");
}

#[cfg(unix)]
#[test]
fn a_secret_with_one_name_is_rewritten_as_before() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("app.json");
    std::fs::write(&target, "old\n").unwrap();

    replace(dir.path(), &target, b"new\n", Mode::Secret).unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"new\n");
    let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn a_plane_file_with_a_second_name_is_rewritten_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("charter.toml");
    std::fs::write(&target, "old\n").unwrap();
    let second = dir.path().join("second.toml");
    std::fs::hard_link(&target, &second).unwrap();

    replace(dir.path(), &target, b"new\n", Mode::Kept).unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"new\n");
    assert_eq!(std::fs::read(&second).unwrap(), b"old\n");
}

#[cfg(unix)]
#[test]
fn a_read_only_file_is_refused_as_an_in_place_write_would_refuse_it() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("charter.toml");
    std::fs::write(&target, "old\n").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o444)).unwrap();

    let err = replace(dir.path(), &target, b"new\n", Mode::Kept).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old\n");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

/// EPERM, which is what a sandbox answers a write it refuses — not EACCES, a file's mode.
fn eperm() -> io::Error {
    io::Error::from_raw_os_error(1)
}

#[test]
fn a_write_a_sandbox_refuses_names_the_file_it_was_writing() {
    // #1345: `purlis persona remember` from a chat printed only "Operation not permitted
    // (os error 1)", with no path and no next step.
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("memory.md");
    let _hook = hook::set(|_, _| Err(eperm()));

    let err = replace(dir.path(), &target, b"new\n", Mode::Kept).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string().contains(&target.display().to_string()),
        "{err}"
    );
}

#[test]
fn only_a_chat_given_a_sandbox_is_told_its_sandbox_refused_the_write() {
    let path = Path::new("/plane/personas/steward/memory/fact.md");
    let said = refused_write(eperm(), path, true).to_string();
    assert!(said.contains("this chat's sandbox refused"), "{said}");
    assert!(
        said.contains("/plane/personas/steward/memory/fact.md"),
        "{said}"
    );
    assert!(said.contains("operator"), "the next step: {said}");

    // EPERM is also privacy controls, system integrity protection and an immutable flag.
    let unsandboxed = refused_write(eperm(), path, false).to_string();
    assert_eq!(
        unsandboxed,
        "could not write /plane/personas/steward/memory/fact.md: Operation not permitted"
    );
}

#[test]
fn a_sandbox_s_refusal_says_what_it_refused_and_the_one_way_on() {
    // #1421: "a sandboxed chat writes only inside its own folder" stopped being true when
    // the cache and host grants came (#1337, #1341); and a write of purlis's own is never
    // granted, so no Allow is promised.
    let path = Path::new("/plane/personas/_dispatch/2026-10.host.jsonl");
    let said = refused_write(eperm(), path, true).to_string();
    assert_eq!(
        said,
        "this chat's sandbox refused writing /plane/personas/_dispatch/2026-10.host.jsonl \
         (Operation not permitted). purlis does not grant its own writes to a chat; ask the \
         operator to run this command outside the chat."
    );
    assert!(!said.contains("only inside its own folder"), "{said}");
}

#[test]
fn the_os_s_words_keep_the_sandbox_s_clause_in_a_sandboxed_chat() {
    // #1421: `init`, `browser install` and the handoff's warning name the path themselves,
    // and said only "(Operation not permitted)" in a sandboxed chat.
    let path = Path::new("/plane/.gitignore");
    let sandboxed = refused_write(eperm(), path, true);
    assert_eq!(
        os_words(&sandboxed),
        "Operation not permitted: this chat's sandbox refused it"
    );
    let not = refused_write(eperm(), path, false);
    assert_eq!(os_words(&not), "Operation not permitted");
    // A refusal no site reworded is judged as this process stands.
    assert_eq!(
        words_of(&eperm(), true),
        "Operation not permitted: this chat's sandbox refused it"
    );
    assert_eq!(words_of(&eperm(), false), "Operation not permitted");
    // Anything else is the OS's words alone, sandboxed or not.
    assert_eq!(
        words_of(&io::Error::from_raw_os_error(28), true),
        "No space left on device"
    );
    assert!(
        !os_words(&sandboxed).contains("/plane"),
        "named once, by the caller"
    );
}

#[test]
fn a_reworded_refusal_still_carries_its_errno() {
    let refused = refused_write(eperm(), Path::new("/plane/x.md"), true);
    assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(os_cause(&refused).raw_os_error(), Some(1));
    let plain = io::Error::from_raw_os_error(13);
    assert_eq!(os_cause(&plain).raw_os_error(), Some(13));
}

#[test]
fn a_write_site_maps_a_refusal_to_one_naming_its_path() {
    // #1359: what every append and every folder made ahead of a write maps its error
    // through. Run on every platform: the sites' own tests need macOS's immutable flag.
    let path = Path::new("/plane/workspaces/alpha/work/device.jsonl");
    let refused = refused_at(path)(eperm());
    let said = refused.to_string();
    assert!(
        said.contains("/plane/workspaces/alpha/work/device.jsonl"),
        "{said}"
    );
    assert!(!said.contains("os error"), "{said}");
    assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(os_cause(&refused).raw_os_error(), Some(1));

    let full = refused_at(path)(io::Error::from_raw_os_error(28));
    assert_eq!(
        full.raw_os_error(),
        Some(28),
        "a full disk stays a full disk"
    );
}

#[test]
fn a_folder_made_ahead_of_a_write_is_made_and_a_failure_that_is_not_a_refusal_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let deep = dir.path().join("a/b/c");
    create_dir_all(&deep).unwrap();
    assert!(deep.is_dir());

    // A file where a folder must go: not EPERM, so the OS's own error comes back.
    let file = dir.path().join("file");
    std::fs::write(&file, "").unwrap();
    let blocked = create_dir_all(&file.join("below")).unwrap_err();
    assert!(blocked.raw_os_error().is_some(), "{blocked}");
}

#[test]
fn a_failure_that_is_not_a_refusal_is_passed_on_as_it_was() {
    let path = Path::new("/plane/x.md");
    let full = refused_write(io::Error::from_raw_os_error(28), path, true);
    assert_eq!(
        full.raw_os_error(),
        Some(28),
        "a full disk stays a full disk"
    );
}

#[test]
fn a_file_charter_cannot_read_as_text_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(".gitignore");
    std::fs::write(&target, b"\xff\xfe").unwrap();
    let err = update(dir.path(), &target, |_| Ok(Some("new".into()))).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(std::fs::read(&target).unwrap(), b"\xff\xfe");
}

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
fn chmod(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
#[test]
fn a_kept_file_that_is_new_gets_the_umask_s_mode_and_not_a_private_one() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain");
    std::fs::write(&plain, "x").unwrap();
    let fresh = dir.path().join("workspace.json");
    replace(dir.path(), &fresh, b"{}", Mode::Kept).unwrap();
    assert_eq!(mode(&fresh), mode(&plain));
}

#[cfg(unix)]
#[test]
fn a_private_file_ends_0600_over_a_loose_or_read_only_one() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("launched.json");
    std::fs::write(&state, "old").unwrap();
    chmod(&state, 0o644);
    replace(dir.path(), &state, b"new", Mode::Private).unwrap();
    assert_eq!(mode(&state), 0o600);

    // Charter's own state: its mode is charter's, so read-only is not a refusal here.
    chmod(&state, 0o400);
    replace(dir.path(), &state, b"newer", Mode::Private).unwrap();
    assert_eq!(std::fs::read_to_string(&state).unwrap(), "newer");
    assert_eq!(mode(&state), 0o600);

    let fresh = dir.path().join("fresh");
    replace(dir.path(), &fresh, b"z", Mode::Private).unwrap();
    assert_eq!(mode(&fresh), 0o600);
}

#[cfg(unix)]
#[test]
fn a_secret_ends_0600_and_a_read_only_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("app.json");
    std::fs::write(&vault, "{}").unwrap();
    chmod(&vault, 0o644);
    replace(dir.path(), &vault, b"{\"K\": 1}", Mode::Secret).unwrap();
    assert_eq!(mode(&vault), 0o600);

    chmod(&vault, 0o400);
    let err = replace(dir.path(), &vault, b"{}", Mode::Secret).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(std::fs::read_to_string(&vault).unwrap(), "{\"K\": 1}");
}

/// A temp that will end private is CREATED 0600: a descriptor opened on it in the instant
/// before a chmod would keep reading whatever is written after.
#[cfg(unix)]
#[test]
fn a_private_temp_is_0600_from_the_instant_it_exists() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let loose = dir.path().join("loose");
    std::fs::write(&loose, "old").unwrap();
    chmod(&loose, 0o644);
    let seen: std::rc::Rc<std::cell::RefCell<Vec<(u32, u64)>>> = Default::default();
    let log = seen.clone();
    let _watch = hook::watch_created(move |file| {
        let meta = file.metadata().unwrap();
        log.borrow_mut()
            .push((meta.permissions().mode() & 0o777, meta.len()));
    });

    replace(dir.path(), &dir.path().join("new"), b"s", Mode::Secret).unwrap();
    replace(dir.path(), &loose, b"s", Mode::Private).unwrap();
    replace(
        dir.path(),
        &dir.path().join("tab"),
        b"s",
        Mode::KeptOrPrivate,
    )
    .unwrap();
    let kept = dir.path().join("kept");
    std::fs::write(&kept, "old").unwrap();
    chmod(&kept, 0o600);
    replace(dir.path(), &kept, b"s", Mode::Kept).unwrap();

    assert_eq!(*seen.borrow(), [(0o600, 0); 4]);
}

#[test]
fn the_temp_prefix_is_the_layer_s_marker_and_a_dot() {
    assert_eq!(TEMP_PREFIX, format!("{}.", crate::layer::MARKER));
}

#[test]
fn every_temp_carries_the_one_prefix_charter_s_temps_are_known_by() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("settings.json");
    let _hook = hook::set(|_, temp| {
        let name = temp.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(".purlis-generated.settings.json."),
            "{name}"
        );
        assert!(name.ends_with(".tmp"), "{name}");
        Ok(())
    });
    replace(dir.path(), &target, b"{}", Mode::Kept).unwrap();
}

/// The link is planted at the TEMP path, which is the path the write actually opens.
/// Planting it at the target proves nothing about this: the target's gate answers first.
#[cfg(unix)]
#[test]
fn a_link_at_the_temp_path_is_refused_and_what_it_points_at_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let captured = dir.path().join("captured.json");
    let target = dir.path().join("launched.json");
    let temp = dir.path().join(".launched.json.writing");
    std::os::unix::fs::symlink(&captured, &temp).unwrap();

    let refused = replace_through(dir.path(), &target, &temp, b"{}", Mode::Private);

    assert!(refused.is_err(), "the temp path was written through");
    assert!(!captured.exists(), "the bytes went through a link");
    assert!(!target.exists());
}

/// The walk, not `O_NOFOLLOW`, answers about a directory ABOVE the file: the flag speaks
/// only for the last component.
#[cfg(unix)]
#[test]
fn a_directory_swapped_for_a_link_is_refused_at_the_moment_of_the_write() {
    let plane = tempfile::tempdir().unwrap();
    let elsewhere = plane.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, plane.path().join(".charter")).unwrap();

    let target = plane.path().join(".charter/launched.json");
    assert!(replace(plane.path(), &target, b"{}", Mode::Private).is_err());
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}

/// Debris from a killed process at the temp's name is written over, not a refusal of
/// every later write.
#[test]
fn a_temp_left_by_a_killed_process_does_not_stop_the_next_write() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("launched.json");
    let temp = dir.path().join(".launched.json.writing");
    std::fs::write(&temp, "half a record from a process that died").unwrap();

    replace_through(dir.path(), &target, &temp, b"{}", Mode::Private).unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"{}");
    assert!(!temp.exists());
}
