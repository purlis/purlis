// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Model policy in the one policy file (T1a-35): the tolerant reader, `min-version` read after
//! the trust check and before the strict parse, and the start's refusals (R-428, R-429, R-430,
//! R-128, R-714, R-699, R-617). Every file is a policy fixture, read by the shipped reader.

use std::path::{Path, PathBuf};

use super::{
    Fixture, Locks, MACHINE_FILE, Of, Seen, TEST_READ, min_version_above, set_for_this_test,
    use_fixture_for_this_test,
};
use crate::harness::Harness;
use crate::harness_declaration::{self as declaration, Declaration};
use crate::keyroute::{self, Level, Route, Way};
use crate::model_provider::Format;
use crate::window_copy::{fill, line};

fn claude() -> &'static Declaration {
    declaration::of(Harness::ClaudeCode)
}

const PROXIED: Route = Route::Proxied {
    format: Format::AnthropicMessages,
};

/// A policy fixture holding `bytes`, in a folder of its own, and the locks this machine's policy
/// reads from it.
struct Policy {
    _dir: tempfile::TempDir,
    file: PathBuf,
}

impl Policy {
    fn holding(bytes: impl AsRef<[u8]>) -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let file = dir.path().join("policy.json");
        std::fs::write(&file, bytes).expect("written");
        Self { _dir: dir, file }
    }

    fn fixture(&self) -> Fixture {
        Fixture::at(&self.file)
    }

    /// The locks this machine reads with this fixture standing in for its file.
    fn locks(&self) -> Locks {
        read(self.fixture())
    }

    fn shown(&self) -> String {
        self.file.display().to_string()
    }
}

fn read(fixture: Fixture) -> Locks {
    use_fixture_for_this_test(fixture);
    let locks = Locks::of_this_machine();
    set_for_this_test(Locks::none());
    locks
}

/// How a subscription chat and a key chat on Claude Code start under `locks`.
fn starts(locks: &Locks) -> (Result<Route, String>, Result<Route, String>) {
    (
        keyroute::start(claude(), Way::Subscription, Level::One, locks),
        keyroute::start(claude(), Way::YourKey, Level::One, locks),
    )
}

/// This build's release: its version without a pre-release.
fn this_release() -> String {
    let version = semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("a version");
    format!("{}.{}.{}", version.major, version.minor, version.patch)
}

fn w64(file: &str, error: &str) -> String {
    fill(line::W64, &[("file", file), ("error", error)])
}

// R-428 ------------------------------------------------------------------------------------

#[test]
fn model_policy_keys_parse_from_the_shared_policy_file() {
    let policy = Policy::holding(format!(
        r#"{{
            "owner": "IT",
            "min-version": "{}",
            "models": {{}},
            "sandbox": {{ "hosts": ["api.example.com"], "write-grants": false }},
            "dispatch": {{ "allow": false }}
        }}"#,
        this_release()
    ));
    let locks = policy.locks();

    assert_eq!(locks.refused_because(), None, "{locks:?}");
    assert_eq!(locks.owner(), "IT");
    assert_eq!(locks.file(), Some(policy.file.as_path()));
    assert_eq!(
        locks.allowed_hosts().map(<[_]>::len),
        Some(1),
        "the sandbox keys beside the model keys are read"
    );
    assert!(locks.forbids_write_grants());
    assert!(locks.forbids_dispatch());
    assert!(!locks.forbids_opt_out(), "only the locks the file states");
    assert_eq!(starts(&locks), (Ok(Route::OwnSignIn), Ok(PROXIED)));
}

#[test]
fn a_policy_file_over_64_kib_is_refused_naming_the_cap() {
    let mut text = String::from("{\"owner\": \"");
    text.push_str(&"a".repeat(64 * 1024));
    text.push_str("\"}");
    let policy = Policy::holding(text);
    let locks = policy.locks();

    let why = locks.refused_because().expect("refused");
    assert!(why.contains("64 KiB"), "{why}");
    assert!(locks.forbids_opt_out());
}

#[test]
fn a_policy_file_of_exactly_64_kib_is_read() {
    let mut text = String::from("{\"owner\": \"");
    text.push_str(&"a".repeat(64 * 1024 - "{\"owner\": \"\"}".len()));
    text.push_str("\"}");
    assert_eq!(text.len(), 64 * 1024, "the cap itself");
    let policy = Policy::holding(text);
    let locks = policy.locks();

    assert_eq!(locks.refused_because(), None, "{locks:?}");
    assert!(locks.any());
    assert!(!locks.forbids_opt_out());
}

// R-128 ------------------------------------------------------------------------------------

/// How one row of the table makes its file refused.
enum Make {
    /// Write these bytes as the policy file.
    Bytes(&'static [u8]),
    /// Do this to the folder and file after writing a good file.
    Then(fn(&Path)),
    /// Read a good file through this fixture.
    Through(fn(Fixture) -> Fixture),
}

fn other_owner(seen: Seen, of: Of, wanted: Of) -> Seen {
    if of == wanted {
        Seen { uid: 501, ..seen }
    } else {
        seen
    }
}

#[cfg(unix)]
#[test]
fn each_policy_refusal_cause_refuses_a_proxied_start_naming_it() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        // Root reads a file whatever its bits say, so the unreadable row can't be made.
        return;
    }
    let rows: [(&str, Make, &str); 14] = [
        (
            "the file a link",
            Make::Then(|file| {
                let real = file.with_file_name("real.json");
                std::fs::rename(file, &real).unwrap();
                std::os::unix::fs::symlink(&real, file).unwrap();
            }),
            "it is a link",
        ),
        (
            "the file not a regular file",
            Make::Then(|file| {
                std::fs::remove_file(file).unwrap();
                std::fs::create_dir(file).unwrap();
            }),
            "it is not a regular file",
        ),
        (
            "the file not owned by root",
            Make::Through(|fixture| fixture.seen_as(|seen, of| other_owner(seen, of, Of::File))),
            "it is not owned by the system's administrator",
        ),
        (
            "the file writable by others",
            Make::Then(|file| {
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o666)).unwrap();
            }),
            "it can be written by others than the administrator",
        ),
        (
            "the folder a link",
            Make::Through(|fixture| {
                let file = fixture.file().to_path_buf();
                let folder = file.parent().unwrap();
                let real = folder.with_extension("real");
                std::fs::rename(folder, &real).unwrap();
                std::os::unix::fs::symlink(&real, folder).unwrap();
                fixture
            }),
            "its folder is a link",
        ),
        (
            "the folder not a folder",
            Make::Through(|fixture| {
                // The file's folder is a file.
                Fixture::at(fixture.file().join("policy.json"))
            }),
            "its folder is not a folder",
        ),
        (
            "the folder not owned by root",
            Make::Through(|fixture| fixture.seen_as(|seen, of| other_owner(seen, of, Of::Folder))),
            "its folder is not owned by the system's administrator",
        ),
        (
            "the folder writable by others",
            Make::Then(|file| {
                std::fs::set_permissions(
                    file.parent().unwrap(),
                    std::fs::Permissions::from_mode(0o777),
                )
                .unwrap();
            }),
            "its folder can be written by others than the administrator",
        ),
        (
            "the file changed while read",
            Make::Through(|fixture| {
                fixture.meanwhile(|file| {
                    let new = file.with_file_name("new.json");
                    std::fs::write(&new, "{}").unwrap();
                    std::fs::rename(&new, file).unwrap();
                })
            }),
            "it changed while it was read",
        ),
        (
            "the file unreadable",
            Make::Then(|file| {
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o000)).unwrap();
            }),
            "it cannot be opened",
        ),
        (
            "the file over 64 KiB",
            Make::Bytes(&[b' '; 64 * 1024 + 1]),
            "it is larger than 64 KiB",
        ),
        (
            "the file not UTF-8",
            Make::Bytes(b"{\"owner\": \"\xff\"}"),
            "it is not UTF-8 text",
        ),
        (
            "the file not JSON",
            Make::Bytes(b"{\"owner\": "),
            "it is not JSON",
        ),
        (
            "a duplicate key",
            Make::Bytes(br#"{"owner": "IT", "owner": "someone else"}"#),
            "it says \"owner\" twice",
        ),
    ];
    for (row, make, cause) in rows {
        let policy = Policy::holding(match make {
            Make::Bytes(bytes) => bytes,
            Make::Then(_) | Make::Through(_) => br#"{"sandbox": {"opt-out": true}}"#,
        });
        let fixture = match make {
            Make::Then(then) => {
                then(&policy.file);
                policy.fixture()
            }
            Make::Through(through) => through(policy.fixture()),
            Make::Bytes(_) => policy.fixture(),
        };
        let file = fixture.file().display().to_string();
        let locks = read(fixture);
        // Put the folder back so tempfile can remove it.
        if let Some(folder) = policy.file.parent() {
            if folder.is_symlink() {
                std::fs::remove_file(folder).unwrap();
                std::fs::rename(folder.with_extension("real"), folder).unwrap();
            }
            let _ = std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700));
        }
        let _ = std::fs::set_permissions(&policy.file, std::fs::Permissions::from_mode(0o600));

        let why = locks
            .refused_because()
            .unwrap_or_else(|| panic!("{row}: not refused"));
        assert!(why.starts_with(cause), "{row}: {why}");
        let (subscription, key) = starts(&locks);
        assert_eq!(
            subscription,
            Ok(Route::OwnSignIn),
            "{row}: a subscription chat starts"
        );
        assert_eq!(key, Err(w64(&file, why)), "{row}");
        assert!(locks.forbids_opt_out(), "{row}: every lock but the presets");
    }
}

// R-429 ------------------------------------------------------------------------------------

#[test]
fn an_unknown_key_inside_models_is_tolerated() {
    let policy = Policy::holding(
        r#"{"models": {"a-key-from-a-later-version": {"x": 1}}, "sandbox": {"opt-out": false}}"#,
    );
    let locks = policy.locks();

    assert_eq!(locks.refused_because(), None, "{locks:?}");
    assert!(locks.forbids_opt_out(), "the file's own lock");
    assert!(!locks.forbids_write_grants(), "and only its own");
}

#[test]
fn an_unknown_top_level_key_still_sets_every_lock_but_the_presets() {
    let policy = Policy::holding(r#"{"admins": [], "sandbox": {"presets": ["forge"]}}"#);
    let locks = policy.locks();

    let why = locks.refused_because().expect("refused");
    assert!(why.contains("\"admins\""), "{why}");
    assert!(locks.forbids_opt_out());
    assert!(locks.forbids_write_grants());
    assert!(locks.forbids_vault_grants());
    assert!(locks.forbids_personal_hosts());
    assert!(locks.forbids_persona_hosts());
    assert!(locks.forbids_socket_grants());
    assert!(locks.forbids_dispatch());
    assert_eq!(locks.allowed_hosts(), Some(&[][..]));
    assert!(!locks.fixes_presets(), "the presets stay the project's");
}

// R-430 ------------------------------------------------------------------------------------

#[test]
fn a_models_key_refuses_every_chat_that_reaches_a_model() {
    let policy = Policy::holding(
        r#"{"models": {"allowed": ["claude-sonnet-5-5"]},
            "sandbox": {"hosts": ["api.example.com"], "socket-grants": false}}"#,
    );
    let locks = policy.locks();

    let w66 = fill(line::W66, &[("file", &policy.shown()), ("key", "allowed")]);
    assert_eq!(starts(&locks), (Err(w66.clone()), Err(w66)));
    // The sandbox locks are those the file states, not all of them.
    assert_eq!(locks.refused_because(), None);
    assert_eq!(locks.allowed_hosts().map(<[_]>::len), Some(1));
    assert!(locks.forbids_socket_grants());
    assert!(!locks.forbids_opt_out());
    assert!(!locks.forbids_write_grants());
    assert!(!locks.forbids_dispatch());
}

// R-714 ------------------------------------------------------------------------------------

#[test]
fn min_version_above_this_build_refuses_every_model_chat() {
    let policy = Policy::holding(r#"{"min-version": "999.0.0", "sandbox": {"opt-out": false}}"#);
    let locks = policy.locks();

    let w36 = fill(
        line::W36,
        &[("file", &policy.shown()), ("version", "999.0.0")],
    );
    assert_eq!(starts(&locks), (Err(w36.clone()), Err(w36)));
    assert_eq!(locks.refused_because(), None);
    assert!(locks.forbids_opt_out());
}

#[test]
fn min_version_is_read_before_the_strict_parse() {
    let policy = Policy::holding(r#"{"min-version": "999.0.0", "admins": []}"#);
    let locks = policy.locks();

    let w36 = fill(
        line::W36,
        &[("file", &policy.shown()), ("version", "999.0.0")],
    );
    let (subscription, key) = starts(&locks);
    assert_eq!(subscription, Err(w36.clone()));
    assert_eq!(key, Err(w36));
    // The strict parse still refuses the file for its unknown key.
    assert!(
        locks
            .refused_because()
            .is_some_and(|why| why.contains("\"admins\""))
    );
    assert!(locks.forbids_opt_out());
}

#[test]
fn min_version_in_an_untrusted_file_is_never_honoured() {
    let policy = Policy::holding(r#"{"min-version": "999.0.0"}"#);
    let locks = read(
        policy
            .fixture()
            .seen_as(|seen, of| other_owner(seen, of, Of::File)),
    );

    let (subscription, key) = starts(&locks);
    assert_eq!(
        subscription,
        Ok(Route::OwnSignIn),
        "no min-version was read"
    );
    let why = locks.refused_because().expect("refused");
    assert_eq!(key, Err(w64(&policy.shown(), why)));
}

#[test]
fn a_malformed_min_version_refuses_the_file() {
    let policy = Policy::holding(r#"{"min-version": "v999.0.0"}"#);
    let locks = policy.locks();

    let why = locks.refused_because().expect("refused");
    assert!(why.contains("\"min-version\""), "{why}");
    assert!(why.contains("MAJOR.MINOR.PATCH"), "{why}");
    let (subscription, key) = starts(&locks);
    assert_eq!(subscription, Ok(Route::OwnSignIn));
    assert_eq!(key, Err(w64(&policy.shown(), why)));
    assert!(locks.forbids_opt_out());
}

#[test]
fn a_duplicate_key_refuses_the_file() {
    let policy = Policy::holding(r#"{"min-version": "0.0.1", "min-version": "999.0.0"}"#);
    let locks = policy.locks();

    assert_eq!(
        locks.refused_because(),
        Some("it says \"min-version\" twice")
    );
    let (subscription, key) = starts(&locks);
    assert_eq!(subscription, Ok(Route::OwnSignIn), "neither value is read");
    assert!(key.is_err());
}

#[test]
fn min_version_equal_to_this_build_passes() {
    let policy = Policy::holding(format!(r#"{{"min-version": "{}"}}"#, this_release()));
    let locks = policy.locks();

    assert_eq!(locks.refused_because(), None, "{locks:?}");
    assert_eq!(starts(&locks), (Ok(Route::OwnSignIn), Ok(PROXIED)));
}

#[test]
fn a_pre_release_compares_below_its_release() {
    assert_eq!(min_version_above("0.6.0-rc.1", "0.6.0"), Ok(false));
    assert_eq!(min_version_above("0.6.1-rc.1", "0.6.0"), Ok(true));
    assert_eq!(min_version_above("0.6.0", "0.6.0"), Ok(false));
    // In a file: this build's own release, as a pre-release, starts.
    let policy = Policy::holding(format!(r#"{{"min-version": "{}-rc.1"}}"#, this_release()));
    let locks = policy.locks();
    assert_eq!(locks.refused_because(), None, "{locks:?}");
    assert_eq!(starts(&locks), (Ok(Route::OwnSignIn), Ok(PROXIED)));
}

#[test]
fn a_dev_build_compares_its_base_version() {
    assert_eq!(min_version_above("0.6.0", "0.6.0-dev.3"), Ok(false));
    assert_eq!(min_version_above("0.6.0-rc.1", "0.6.0-dev.3"), Ok(false));
    assert_eq!(min_version_above("0.6.1", "0.6.0-dev.3"), Ok(true));
}

#[test]
fn min_version_accepts_and_refuses_the_listed_forms() {
    for accepted in ["0.6.0", "0.6.0-rc.1"] {
        assert!(
            min_version_above(accepted, "0.6.0").is_ok(),
            "{accepted} is accepted"
        );
    }
    for refused in ["2", "0.6", "v0.6.0", "1!0.6.0", "0.6.0.post1"] {
        let why = min_version_above(refused, "0.6.0").expect_err(refused);
        assert!(why.contains("MAJOR.MINOR.PATCH"), "{refused}: {why}");
        // And in a file, the file is refused.
        let policy = Policy::holding(format!(r#"{{"min-version": "{refused}"}}"#));
        assert!(policy.locks().refused_because().is_some(), "{refused}");
    }
}

#[test]
fn a_duplicate_key_at_any_depth_refuses_the_file() {
    for (text, key) in [
        (
            r#"{"sandbox": {"hosts": [], "hosts": ["a.example"]}}"#,
            "hosts",
        ),
        (
            r#"{"dispatch": {"locked": [{"from": "qa", "from": "steward", "to": "devops"}]}}"#,
            "from",
        ),
        (r#"{"models": {"allowed": [], "allowed": []}}"#, "allowed"),
    ] {
        let locks = Policy::holding(text).locks();
        assert_eq!(
            locks.refused_because(),
            Some(format!("it says \"{key}\" twice").as_str()),
            "{text}"
        );
        assert!(locks.forbids_opt_out(), "{text}");
    }
}

#[test]
fn a_non_object_models_refuses_the_file() {
    for text in [
        r#"{"models": []}"#,
        r#"{"models": "allowed"}"#,
        r#"{"models": null}"#,
    ] {
        let locks = Policy::holding(text).locks();
        assert_eq!(
            locks.refused_because(),
            Some("its \"models\" is not an object"),
            "{text}"
        );
    }
}

// R-699 ------------------------------------------------------------------------------------

#[test]
fn policy_tests_read_the_fixture_path_never_etc() {
    // A test build reads the fixture it is handed, or nothing: never this machine's file.
    TEST_READ.with(|read| read.borrow_mut().clear());
    set_for_this_test(Locks::none());
    let _ = Locks::of_this_machine();
    assert_eq!(
        TEST_READ.with(|read| read.borrow().clone()),
        Vec::<PathBuf>::new()
    );
    let policy = Policy::holding("{}");
    let _ = policy.locks();
    assert_eq!(
        TEST_READ.with(|read| read.borrow().clone()),
        vec![policy.file.clone()]
    );
    // And no test of this crate hands the reader this machine's file.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    for top in ["src", "tests"] {
        rust_files(&crate_dir.join(top), &mut sources);
    }
    let offenders: Vec<String> = sources
        .iter()
        .flat_map(|file| {
            let text = std::fs::read_to_string(file).unwrap_or_default();
            text.lines()
                .filter(|line| {
                    (line.contains("Locks::read(") || line.contains("Fixture::at("))
                        && (line.contains("MACHINE_FILE") || line.contains("/etc/"))
                })
                .map(|line| format!("{}: {}", file.display(), line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(offenders.is_empty(), "{offenders:#?}");
    assert_eq!(MACHINE_FILE, "/etc/purlis/policy.json");
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

// R-617 ------------------------------------------------------------------------------------

#[test]
fn no_policy_key_is_read_from_a_plane_file() {
    let root = tempfile::tempdir().expect("a project");
    let said =
        "[models]\nallowed = [\"claude-sonnet-5-5\"]\n\n[policy]\nmin-version = \"999.0.0\"\n";
    std::fs::write(root.path().join("charter.toml"), said).expect("written");
    std::fs::write(root.path().join("charter.local.toml"), said).expect("written");
    std::fs::create_dir_all(root.path().join(".purlis")).expect("made");
    std::fs::write(
        root.path().join(".purlis/policy.json"),
        r#"{"models": {"allowed": []}, "min-version": "999.0.0"}"#,
    )
    .expect("written");
    set_for_this_test(Locks::none());

    let locks = Locks::of(root.path());

    assert!(!locks.any(), "{locks:?}");
    assert_eq!(locks.refuses_model_start(Route::OwnSignIn), None);
    assert_eq!(locks.refuses_model_start(PROXIED), None);
}
