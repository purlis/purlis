// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::ffi::OsStr;
use std::path::PathBuf;

use super::*;

#[test]
fn on_linux_the_socket_is_in_purlis_folder_of_the_runtime_directory() {
    let runtime = tempfile::tempdir().expect("a runtime directory");
    assert_eq!(
        socket_in(Some(runtime.path().as_os_str()), None, 1000),
        runtime.path().join("purlis").join("purlisd.sock"),
    );
}

#[test]
fn without_a_runtime_directory_the_socket_is_in_a_folder_of_this_users_own_in_the_temporary_one() {
    assert_eq!(
        socket_in(None, Some(OsStr::new("/var/folders/ab/xyz/T")), 501),
        PathBuf::from("/var/folders/ab/xyz/T/purlis-501/purlisd.sock"),
    );
}

#[test]
fn a_runtime_directory_that_is_relative_or_not_there_is_passed_over() {
    for runtime in ["relative/run", "/no/such/runtime/directory/here"] {
        assert_eq!(
            socket_in(Some(OsStr::new(runtime)), None, 1000),
            PathBuf::from("/tmp/purlis-1000/purlisd.sock"),
            "{runtime}"
        );
    }
}

#[test]
fn a_temporary_directory_that_is_relative_is_passed_over_for_tmp() {
    assert_eq!(
        socket_in(None, Some(OsStr::new("relative/tmp")), 42),
        PathBuf::from("/tmp/purlis-42/purlisd.sock"),
    );
}
