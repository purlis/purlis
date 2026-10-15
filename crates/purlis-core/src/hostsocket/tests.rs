// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::*;

fn uid() -> u32 {
    purlis_same_user::Uid::effective().as_raw()
}

/// A folder of this user's with `mode`.
fn folder(mode: u32) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
    dir
}

fn in_tmp(uid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/purlis-{uid}/purlisd.sock"))
}

#[test]
fn a_base_that_is_this_users_alone_holds_the_socket_in_purlis_folder() {
    let base = folder(0o700);
    assert_eq!(
        socket_in(Some(base.path()), uid()),
        base.path().join("purlis").join("purlisd.sock"),
    );
}

#[test]
fn a_base_others_may_enter_is_passed_over_for_a_folder_of_this_users_own_in_tmp() {
    let base = folder(0o755);
    assert_eq!(socket_in(Some(base.path()), uid()), in_tmp(uid()));
}

#[test]
fn a_base_another_user_owns_is_passed_over() {
    let base = folder(0o700);
    let theirs = uid().wrapping_add(1);
    assert_eq!(socket_in(Some(base.path()), theirs), in_tmp(theirs));
}

#[test]
fn a_base_that_is_relative_missing_or_a_link_is_passed_over() {
    let real = folder(0o700);
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("runtime");
    std::os::unix::fs::symlink(real.path(), &link).unwrap();
    for base in [
        Path::new("relative/run"),
        Path::new("/no/such/runtime/directory/here"),
        link.as_path(),
    ] {
        assert_eq!(socket_in(Some(base), uid()), in_tmp(uid()), "{base:?}");
    }
    assert_eq!(socket_in(None, uid()), in_tmp(uid()));
}

#[cfg(target_os = "macos")]
#[test]
fn on_macos_the_socket_is_in_the_users_own_temporary_directory_whatever_the_environment_names() {
    // What the system names for this user, asked as a person would, with the environment gone:
    // a `$TMPDIR` this process inherited (a sandbox's, a shell's) plays no part.
    let asked = crate::forklock::output(
        std::process::Command::new("/usr/bin/getconf")
            .arg("DARWIN_USER_TEMP_DIR")
            .env_clear(),
    )
    .unwrap();
    let named = String::from_utf8(asked.stdout).unwrap();
    let users_own = PathBuf::from(named.trim_end().trim_end_matches('/'));
    let hostile = folder(0o700);

    for runtime in [None, Some(hostile.path().as_os_str())] {
        assert_eq!(
            socket_for(runtime),
            users_own.join("purlis").join("purlisd.sock")
        );
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn on_linux_a_runtime_directory_of_this_users_own_holds_the_socket_and_another_does_not() {
    let runtime = folder(0o700);
    assert_eq!(
        socket_for(Some(runtime.path().as_os_str())),
        runtime.path().join("purlis").join("purlisd.sock"),
    );
    let open = folder(0o755);
    assert_eq!(socket_for(Some(open.path().as_os_str())), in_tmp(uid()));
    assert_eq!(socket_for(None), in_tmp(uid()));
}

#[test]
fn the_window_makes_the_sockets_folder_this_users_alone() {
    let base = folder(0o700);
    let socket = socket_in(Some(base.path()), uid());
    make_the_folder(&socket).unwrap();
    let made = std::fs::metadata(socket.parent().unwrap()).unwrap();
    assert!(made.is_dir());
    assert_eq!(made.permissions().mode() & 0o777, 0o700);
}
