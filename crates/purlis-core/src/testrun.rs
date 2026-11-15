// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! This test binary, re-run as a child with an environment of the test's choosing — for a
//! test whose subject IS the environment: the variables a charter chat exports
//! (charter-app#243), or the `HOME` the product's git reads its global config from
//! (charter-app#242).
//!
//! **Why a child.** Setting a variable in this process needs `unsafe` (the workspace forbids
//! it) and would leak into every test running beside this one. A child gets exactly the
//! environment it is handed.
//!
//! **What the child never inherits.** Every `PURLIS_*` and `CHARTER_*` variable of the process running the
//! suite is removed first — the chat the suite may be running in is not the environment under
//! test — except [`crate::fence::VAR`], the fence that keeps a test off planes it did not make.
//!
//! **And the integration tests** (charter-app#267). `cfg(test)` hides a steering variable
//! from this crate's unit tests only ([`crate::steer`]); the tests under `tests/` link the
//! library as the product does, and a `CHARTER_WORKTREES` or `CHARTER_HOME` in the shell that
//! ran them reached every one — failing ~45, and able to write into the real state
//! directory. So each of them opens with [`unsteered!`](crate::unsteered), which re-runs that
//! one test here, without the shell's variables, whenever one of them is set. This module is
//! compiled into the library so those crates can reach it; the product never calls it.

use std::ffi::OsStr;
use std::process::Command;

/// Run `tests` (full paths, matched exactly) in a child of this test binary with `env` set,
/// and answer what it printed. Fails the calling test unless every one of them ran and passed:
/// a filter that matches nothing passes too, so the count is the evidence they ran.
pub fn rerun(tests: &[&str], env: &[(&str, &OsStr)]) -> String {
    rerun_with(tests, env, false, false)
}

/// [`rerun`], for a test that is `#[ignore]`d everywhere but the job that runs it by name: the
/// child is told to run it too, or the count of what passed would stop the caller.
pub fn rerun_ignored(tests: &[&str], env: &[(&str, &OsStr)]) -> String {
    rerun_with(tests, env, false, true)
}

/// **The profile directory this test binary was built under**, `<build-dir>[/<triple>]/<profile>`:
/// what its name says (`debug`, `dev-release`), and where Cargo filed it.
///
/// A test binary is filed in its `deps/`, or in `build/<crate>/<hash>/out/` from Cargo 1.100's
/// build directory layout. `None` for a binary in neither, which is not one Cargo built for a
/// test, whatever folder above it is named `build` or `deps`.
///
/// **Not where the workspace's binaries are** when `build.build-dir` is set apart from the
/// target directory: Cargo copies those into the target directory, which [`bin_dir`] answers.
pub fn profile_dir() -> Option<std::path::PathBuf> {
    profile_dir_of(&std::env::current_exe().ok()?).map(std::path::Path::to_path_buf)
}

/// [`profile_dir`] for the binary at `exe`.
pub fn profile_dir_of(exe: &std::path::Path) -> Option<&std::path::Path> {
    let named = |dir: &std::path::Path, name: &str| dir.file_name().is_some_and(|it| it == name);
    let parent = exe.parent()?;
    if named(parent, "deps") {
        return parent.parent();
    }
    // `<profile>/build/<crate>/<hash>/out/<exe>`.
    let build = parent.ancestors().nth(3)?;
    (named(parent, "out") && named(build, "build"))
        .then(|| build.parent())
        .flatten()
}

/// **The folder the workspace's binaries are in** for this test binary's build,
/// `<target-dir>[/<triple>]/<profile>`: `purlis`, `fake-harness`, `sandbox-probe`, the app.
///
/// Cargo builds under its build directory and copies each binary into the target directory,
/// and the two are apart when `build.build-dir` says so (in `~/.cargo/config.toml`, say), so
/// where they are is asked of Cargo (`cargo metadata`, once a process). Where Cargo cannot say,
/// the profile directory itself. `None` where this binary was not built for a test
/// ([`profile_dir`]).
pub fn bin_dir() -> Option<std::path::PathBuf> {
    Some(folder_of_binaries(
        &resolved(profile_dir()?),
        cargo_dirs().as_ref(),
    ))
}

/// [`bin_dir`] for the profile directory `profile` of a build under `build_dir`, whose binaries
/// Cargo copies into `target_dir`: the same place under the one as under the other.
pub fn bin_dir_of(
    profile: &std::path::Path,
    build_dir: &std::path::Path,
    target_dir: &std::path::Path,
) -> Option<std::path::PathBuf> {
    Some(target_dir.join(profile.strip_prefix(build_dir).ok()?))
}

/// [`bin_dir_of`] for the profile directory `profile`, with what `cargo metadata` said, or the
/// profile directory itself where Cargo said nothing or names a build directory it is not in (a
/// `--target-dir` given on the command line, a test binary run from an archive): there the
/// build directory is the target directory, as Cargo has it when nothing sets it apart.
fn folder_of_binaries(
    profile: &std::path::Path,
    dirs: Option<&(std::path::PathBuf, std::path::PathBuf)>,
) -> std::path::PathBuf {
    dirs.and_then(|(build_dir, target_dir)| bin_dir_of(profile, build_dir, target_dir))
        .unwrap_or_else(|| profile.to_path_buf())
}

/// The build directory and the target directory, as `cargo metadata` says them for the
/// workspace this test was built in, each with its links resolved as [`std::env::current_exe`]
/// resolves them. The build directory is the target directory for a Cargo that names none.
fn cargo_dirs() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    static SAID: std::sync::OnceLock<Option<(std::path::PathBuf, std::path::PathBuf)>> =
        std::sync::OnceLock::new();
    SAID.get_or_init(asked_of_cargo).clone()
}

/// [`cargo_dirs`], asked of Cargo.
fn asked_of_cargo() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let mut cargo = cargo();
    cargo.args(["metadata", "--format-version", "1", "--no-deps"]);
    let out = crate::forklock::output(&mut cargo).ok()?;
    if !out.status.success() {
        return None;
    }
    let said: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let dir = |key: &str| Some(resolved(said.get(key)?.as_str()?.into()));
    let target_dir = dir("target_directory")?;
    Some((
        dir("build_directory").unwrap_or_else(|| target_dir.clone()),
        target_dir,
    ))
}

/// `path` with its links resolved, or as it is where it cannot be (it is not there yet).
fn resolved(path: std::path::PathBuf) -> std::path::PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// The Cargo that runs this test, in the package it was built from.
fn cargo() -> Command {
    let mut cargo = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    if let Some(package) = std::env::var_os("CARGO_MANIFEST_DIR") {
        cargo.current_dir(package);
    }
    cargo
}

/// **The binary `name` of package `package`, as this test's build has it**, in [`bin_dir`]: built
/// first when it is not there yet, with the profile and target this test was built with.
///
/// What Cargo built is taken from what Cargo says it built (`--message-format=json`, the
/// artifact's `executable`), and the test fails at once when that is not the binary looked for,
/// or is not there. **Never built over one that is**: a binary already there may be one another
/// package's tests built with their features (`purlis` with `purlis-core/fenced`, which those
/// tests reach as `CARGO_BIN_EXE_purlis`), and a plain build would put one without them in its
/// place.
///
/// # Panics
///
/// Where this binary was not built for a test, the build fails (with what Cargo said), or what
/// it built is not where this build's binaries are.
pub fn built(name: &str, package: &str) -> std::path::PathBuf {
    let bins = bin_dir().expect("this test binary is in a folder of Cargo's build");
    let beside = bins.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    if beside.is_file() {
        return beside;
    }
    let mut build = cargo();
    build.args([
        "build",
        "--message-format=json-render-diagnostics",
        "-p",
        package,
        "--bin",
        name,
    ]);
    // `[<triple>/]<profile>` under the target directory, as Cargo names a build of the profile
    // for that target. Where Cargo did not say where that is, the build is taken to be for no
    // other target, and the check below says so if it was.
    let filed: Vec<_> = cargo_dirs()
        .and_then(|(_, target_dir)| {
            bins.strip_prefix(target_dir)
                .ok()
                .map(std::path::Path::to_path_buf)
        })
        .map(|filed| filed.iter().map(ToOwned::to_owned).collect())
        .unwrap_or_else(|| {
            bins.file_name()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        });
    if let [triple, _] = filed.as_slice() {
        build.arg("--target").arg(triple);
    }
    match filed.last().and_then(|it| it.to_str()) {
        Some("debug") => {}
        Some(other) => {
            build.args(["--profile", other]);
        }
        None => panic!("no profile in {}", bins.display()),
    }
    let out = crate::forklock::output(&mut build).expect("cargo runs");
    assert!(
        out.status.success(),
        "{name} did not build ({}):\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let made = executable_of(&String::from_utf8_lossy(&out.stdout), name)
        .unwrap_or_else(|| panic!("cargo built {name} and named no executable for it"));
    assert!(
        made.is_file() && resolved(made.clone()) == resolved(beside.clone()),
        "cargo built {name} at {}, where this test's build has its binaries in {}: no test \
         runs a binary of another build",
        made.display(),
        bins.display()
    );
    made
}

/// The `executable` Cargo's JSON `messages` name for the binary `name`, which it built.
fn executable_of(messages: &str, name: &str) -> Option<std::path::PathBuf> {
    messages
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| message["target"]["name"] == name)
        .find_map(|message| message["executable"].as_str().map(std::path::PathBuf::from))
}

/// Set on a child [`rerun_steered`] started: its steering variables were chosen by the test,
/// so [`rerun_if_steered`] leaves them in place.
const STEERED_ON_PURPOSE: &str = "TESTRUN_STEERED_ON_PURPOSE";

/// [`rerun`], for a test in `tests/` whose subject IS a steering variable that is set.
///
/// Every test there opens with [`unsteered!`](crate::unsteered), which re-runs a test with a
/// steering variable set WITHOUT it — right for the shell's variables, and exactly wrong for
/// the ones a test hands its own child. The child this starts is marked, so the guard lets the
/// variables in `env` through; any other `CHARTER_*` of the shell is still removed first.
pub fn rerun_steered(tests: &[&str], env: &[(&str, &OsStr)]) -> String {
    rerun_with(tests, env, true, false)
}

fn rerun_with(tests: &[&str], env: &[(&str, &OsStr)], steered: bool, ignored: bool) -> String {
    let mut child = Command::new(std::env::current_exe().expect("the test binary"));
    child.args(["--exact", "--test-threads=1", "--nocapture"]);
    if ignored {
        child.arg("--include-ignored");
    }
    child.args(tests);
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if crate::envvar::rest(&name).is_some() && name != crate::fence::VAR {
            child.env_remove(&*name);
        }
    }
    // Never inherited: a steered child's own re-runs are ordinary ones.
    child.env_remove(STEERED_ON_PURPOSE);
    if steered {
        child.env(STEERED_ON_PURPOSE, "1");
    }
    for (name, value) in env {
        child.env(name, value);
    }
    let out = crate::forklock::output(&mut child).expect("the test binary re-runs");
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{said}");
    assert!(
        said.contains(&format!("test result: ok. {} passed", tests.len())),
        "{said}"
    );
    said
}

/// Whether this process has a steering variable set — one of [`crate::steer::STEERING`] — and
/// so has re-run the calling test without it, in a child ([`rerun`]). The caller returns when
/// it has: the child ran the test, and failed this one if the test failed there. A child
/// [`rerun_steered`] started answers `false`: its variables are the test's, not the shell's.
///
/// The test is named by its thread, which is how libtest names one. Read straight from the
/// environment, not through [`crate::steer::var_os`], which hides these under `cfg(test)`.
pub fn rerun_if_steered() -> bool {
    if std::env::var_os(STEERED_ON_PURPOSE).is_some() {
        return false;
    }
    if !crate::steer::STEERING
        .iter()
        .flat_map(|name| crate::envvar::spellings(name))
        .any(|name| std::env::var_os(name).is_some())
    {
        return false;
    }
    let me = std::thread::current();
    let name = me
        .name()
        .filter(|name| *name != "main")
        .expect("libtest names the thread a test runs on after the test");
    rerun(&[name], &[]);
    true
}

/// The first line of every test in `crates/purlis-core/tests/`: when the shell that ran the
/// suite set a steering variable, run this test again without it and stop here
/// ([`testrun::rerun_if_steered`](crate::testrun::rerun_if_steered)).
///
/// Not for a `#[should_panic]` test: the child's panic fails the re-run, and the parent then
/// panics for the wrong reason. The directory's own check refuses one.
#[macro_export]
macro_rules! unsteered {
    () => {
        if $crate::testrun::rerun_if_steered() {
            return;
        }
    };
}

#[cfg(test)]
mod tests {
    /// Set on the child: where the test's body writes what it saw.
    const SAW: &str = "TESTRUN_GUARD_SAW";

    use std::path::{Path, PathBuf};

    #[test]
    fn a_test_binary_in_deps_is_filed_under_its_profile() {
        assert_eq!(
            super::profile_dir_of(Path::new("/w/target/debug/deps/it-0123abcd")),
            Some(Path::new("/w/target/debug"))
        );
    }

    #[test]
    fn a_test_binary_in_the_build_directory_layout_is_filed_under_its_profile() {
        assert_eq!(
            super::profile_dir_of(Path::new(
                "/w/target/aarch64-apple-darwin/debug/build/purlis-core/0123abcd/out/it-0123abcd"
            )),
            Some(Path::new("/w/target/aarch64-apple-darwin/debug"))
        );
    }

    #[test]
    fn a_binary_cargo_did_not_file_for_a_test_has_no_profile_even_under_a_folder_named_build_or_deps()
     {
        for exe in [
            "/home/me/build/target/debug/purlis",
            "/home/me/build/src/tool",
            "/srv/deps/project/target/debug/purlis",
            "/w/target/debug/build/purlis-core/0123abcd/it-0123abcd",
            "/w/target/debug/purlis",
        ] {
            assert_eq!(super::profile_dir_of(Path::new(exe)), None, "{exe}");
        }
    }

    #[test]
    fn the_binaries_of_a_build_directory_set_apart_are_in_the_target_directory() {
        assert_eq!(
            super::bin_dir_of(
                Path::new("/cache/build/w-0123/aarch64-apple-darwin/debug"),
                Path::new("/cache/build/w-0123"),
                Path::new("/w/target"),
            ),
            Some(PathBuf::from("/w/target/aarch64-apple-darwin/debug"))
        );
    }

    #[test]
    fn the_binaries_of_a_build_directory_that_is_the_target_directory_are_beside_its_profile() {
        assert_eq!(
            super::bin_dir_of(
                Path::new("/w/target/debug"),
                Path::new("/w/target"),
                Path::new("/w/target"),
            ),
            Some(PathBuf::from("/w/target/debug"))
        );
    }

    #[test]
    fn a_profile_outside_the_build_directory_has_no_folder_of_binaries() {
        assert_eq!(
            super::bin_dir_of(
                Path::new("/elsewhere/debug"),
                Path::new("/cache/build/w-0123"),
                Path::new("/w/target"),
            ),
            None
        );
    }

    #[test]
    fn this_builds_binaries_are_in_the_target_directory_cargo_names_under_this_tests_profile() {
        let (_, target_dir) = super::cargo_dirs().expect("cargo metadata answers");
        let profile = super::profile_dir().expect("a test binary of Cargo's");
        let bins = super::bin_dir().expect("a folder of binaries");
        assert!(bins.starts_with(&target_dir), "{}", bins.display());
        assert_eq!(bins.file_name(), profile.file_name());
    }

    #[test]
    fn the_folder_of_binaries_is_the_profiles_own_where_cargo_cannot_place_it() {
        let profile = Path::new("/x/debug");
        let dirs = (PathBuf::from("/w/target"), PathBuf::from("/w/target"));
        assert_eq!(super::folder_of_binaries(profile, Some(&dirs)), profile);
        assert_eq!(super::folder_of_binaries(profile, None), profile);
        let apart = (PathBuf::from("/cache/w"), PathBuf::from("/w/target"));
        assert_eq!(
            super::folder_of_binaries(Path::new("/cache/w/debug"), Some(&apart)),
            Path::new("/w/target/debug")
        );
    }

    #[test]
    #[should_panic(expected = "did not match any packages")]
    fn a_binary_that_does_not_build_says_what_cargo_said() {
        super::built("no-such-binary", "no-such-package");
    }

    #[test]
    fn what_cargo_built_is_the_executable_its_messages_name_for_that_binary() {
        let messages = [
            r#"{"reason":"compiler-artifact","target":{"name":"purlis_core","kind":["lib"]},"executable":null}"#,
            r#"{"reason":"compiler-artifact","target":{"name":"purlis","kind":["bin"]},"executable":"/w/target/debug/purlis"}"#,
            r#"{"reason":"build-finished","success":true}"#,
        ]
        .join("\n");
        assert_eq!(
            super::executable_of(&messages, "purlis"),
            Some(PathBuf::from("/w/target/debug/purlis"))
        );
        assert_eq!(super::executable_of(&messages, "fake-harness"), None);
    }

    #[test]
    fn a_test_started_with_a_steering_variable_set_runs_again_without_it() {
        if let Some(saw) = std::env::var_os(SAW) {
            if super::rerun_if_steered() {
                return;
            }
            let seen: Vec<&str> = ["CHARTER_WORKTREES", "CHARTER_HOME"]
                .into_iter()
                .filter(|name| std::env::var_os(name).is_some())
                .collect();
            std::fs::write(saw, format!("still set: {seen:?}")).unwrap();
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let saw = dir.path().join("saw");

        super::rerun(
            &["testrun::tests::a_test_started_with_a_steering_variable_set_runs_again_without_it"],
            &[
                ("CHARTER_WORKTREES", "/nowhere/worktrees".as_ref()),
                ("CHARTER_HOME", "/nowhere/state".as_ref()),
                (SAW, saw.as_os_str()),
            ],
        );

        assert_eq!(
            std::fs::read_to_string(&saw).ok().as_deref(),
            Some("still set: []"),
            "the body ran, and without either variable"
        );
    }
}
