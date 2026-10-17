// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Multi-call programs for a test of the name a program is started under (#1977): real ones,
//! found where the machine keeps its programs, each doing what the name it is started under
//! says. A test skips where the machine has none.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `name` by the path it really is, in the first folder that machines keep programs in.
fn found(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/opt/homebrew/bin", "/usr/local/bin"]
        .iter()
        .map(|dir| Path::new(dir).join(name))
        .find(|path| path.is_file())
        .and_then(|path| path.canonicalize().ok())
}

/// **`xz`**, which unpacks when it is started as `unxz`.
pub(crate) fn xz() -> Option<PathBuf> {
    found("xz")
}

/// **`git`**, which runs `git <command>` when it is started as `git-<command>`. Not macOS's
/// `/usr/bin/git`, which hands itself on to the developer tools' git by its own name.
pub(crate) fn git() -> Option<PathBuf> {
    found("git").filter(|git| !(cfg!(target_os = "macos") && git == Path::new("/usr/bin/git")))
}

/// `text`, packed by `xz` into `dir/packed.xz`.
pub(crate) fn packed(xz: &Path, dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("packed.xz");
    let mut child = crate::forklock::spawn(
        Command::new(xz)
            .arg("-c")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped()),
    )
    .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    std::fs::write(&path, out.stdout).unwrap();
    path
}
