// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Two clones of one plane and a bare remote standing in for the forge, against real git: the
//! fixture of bringing a target in, of recovering a stop, and of the soak (#5824).

use std::path::{Path, PathBuf};

/// git for the test's own setup, which must go through.
pub(in crate::planegit) fn run(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?}: {done:?}");
    done.out.trim().to_owned()
}

pub(in crate::planegit) fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

pub(in crate::planegit) fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

pub(in crate::planegit) fn commit(dir: &Path, message: &str) {
    run(dir, &["add", "-A"]);
    run(dir, &["commit", "-q", "-m", message]);
}

/// This machine's plane and another's, on one commit, with `release` as the target. `fetch`
/// lands what the other made and updates this one's remote-tracking branch.
pub(in crate::planegit) struct Two {
    _dir: tempfile::TempDir,
    pub(in crate::planegit) bare: PathBuf,
    pub(in crate::planegit) mine: PathBuf,
    pub(in crate::planegit) theirs: PathBuf,
}

impl Two {
    pub(in crate::planegit) fn new(seed: &[(&str, &str)]) -> Two {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let (bare, mine, theirs) = (top.join("bare"), top.join("mine"), top.join("theirs"));
        std::fs::create_dir_all(&bare).unwrap();
        run(&bare, &["init", "-q", "--bare", "-b", "release", "."]);
        std::fs::create_dir_all(&mine).unwrap();
        run(&mine, &["init", "-q", "-b", "release", "."]);
        run(&mine, &["config", "user.name", "Mine"]);
        run(&mine, &["config", "user.email", "mine@example.invalid"]);
        run(&mine, &["remote", "add", "origin", bare.to_str().unwrap()]);
        write(&mine, "charter.toml", "[plane]\nname = \"fixture\"\n");
        for (file, text) in seed {
            write(&mine, file, text);
        }
        commit(&mine, "seed");
        run(&mine, &["push", "-q", "origin", "release"]);
        run(
            &top,
            &[
                "clone",
                "-q",
                bare.to_str().unwrap(),
                theirs.to_str().unwrap(),
            ],
        );
        run(&theirs, &["config", "user.name", "Theirs"]);
        run(&theirs, &["config", "user.email", "theirs@example.invalid"]);
        Two {
            _dir: dir,
            bare,
            mine,
            theirs,
        }
    }

    pub(in crate::planegit) fn land_theirs(&self, message: &str) {
        commit(&self.theirs, message);
        run(&self.theirs, &["push", "-q", "origin", "release"]);
        run(&self.mine, &["fetch", "-q", "origin"]);
    }

    pub(in crate::planegit) fn head(&self) -> String {
        run(&self.mine, &["rev-parse", "HEAD"])
    }
}
