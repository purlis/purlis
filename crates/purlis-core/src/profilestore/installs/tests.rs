// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;

use crate::profilestore::pin::Pinned;
use crate::profilestore::{Adopted, Folders};

/// A vendor's releases, as a test serves them: what each address answers, and every address
/// asked.
#[derive(Default)]
struct Served {
    answers: HashMap<String, Vec<u8>>,
    asked: RefCell<Vec<String>>,
}

impl Served {
    fn with(mut self, url: &str, body: impl Into<Vec<u8>>) -> Self {
        self.answers.insert(url.to_owned(), body.into());
        self
    }
}

impl Fetch for Served {
    fn fetch(&self, url: &str, most: u64, into: &mut dyn Write) -> Result<(), String> {
        self.asked.borrow_mut().push(url.to_owned());
        let body = self.answers.get(url).ok_or("404 Not Found")?;
        if body.len() as u64 > most {
            return Err("too large".to_owned());
        }
        into.write_all(body).map_err(|e| e.to_string())
    }
}

fn sha(bytes: &[u8]) -> String {
    crate::extension::hex(&Sha256::digest(bytes))
}

const CLAUDE: &[u8] = b"#!/bin/sh\necho '2.1.300 (Claude Code)'\n";

fn manifest_url(version: &str) -> String {
    format!("{CLAUDE_RELEASES}/{version}/manifest.json")
}

/// Claude Code's release of 2.1.300, as its bucket serves it, its manifest naming `checksum`.
fn claude_released(checksum: &str) -> Served {
    Served::default()
        .with(
            &manifest_url("2.1.300"),
            serde_json::json!({
                "version": "2.1.300",
                "platforms": {
                    "linux-x64": {"binary": "claude", "checksum": checksum, "size": CLAUDE.len()},
                },
            })
            .to_string(),
        )
        .with(
            &format!("{CLAUDE_RELEASES}/2.1.300/linux-x64/claude"),
            CLAUDE,
        )
}

/// A runner's record of the desktop's profile `work`, kept for it and pinned to `pinned`.
fn kept_pinned(pinned: Option<&str>) -> MachineProfile {
    MachineProfile {
        name: "work".to_owned(),
        harness: "claude".to_owned(),
        title: "work".to_owned(),
        folders: Folders {
            config: PathBuf::from("/runner/data/profiles/work/config"),
            data: None,
        },
        adopted: None,
        imported: None,
        sign_in: None,
        pinned: pinned.map(|version| Pinned {
            version: version.to_owned(),
            at: 1,
        }),
        sandbox_only: false,
        kept_for: Some("01JA0DESKT0P00000000000000".to_owned()),
        rest: Default::default(),
    }
}

/// Every entry left under `dir`, by name.
fn left_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// **A runner chat on a profile pinned to a version runs exactly that version, installed on the
/// runner** (#3446): downloaded from its vendor's release, checked against the digest the
/// vendor publishes, and kept in the data home, never under `/tmp`.
#[test]
fn a_pinned_profile_runs_exactly_its_version_installed_from_its_vendors_release() {
    let data = tempfile::tempdir().unwrap();
    let served = claude_released(&sha(CLAUDE));

    let program = program_for(
        &kept_pinned(Some("2.1.300")),
        data.path(),
        Some(Platform::X64),
        &served,
        &|_| {},
    )
    .expect("installed")
    .expect("a program of its own");

    assert_eq!(program, data.path().join("harnesses/claude/2.1.300/claude"));
    assert_eq!(fs::read(&program).unwrap(), CLAUDE);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&program).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert_eq!(
        left_in(&data.path().join("harnesses/claude")),
        [".lock", "2.1.300"]
    );
}

/// Installed once, a version is run again with nothing downloaded.
#[test]
fn a_version_installed_once_is_not_downloaded_again() {
    let data = tempfile::tempdir().unwrap();
    let served = claude_released(&sha(CLAUDE));
    let record = kept_pinned(Some("2.1.300"));
    program_for(&record, data.path(), Some(Platform::X64), &served, &|_| {}).unwrap();
    served.asked.borrow_mut().clear();

    let again = program_for(&record, data.path(), Some(Platform::X64), &served, &|_| {}).unwrap();

    assert!(again.is_some());
    assert!(served.asked.borrow().is_empty(), "{:?}", served.asked);
}

/// **A version the runner cannot verify is not installed, and the chat says why**: a download
/// that does not match its published digest leaves nothing behind.
#[test]
fn a_download_that_does_not_match_its_published_digest_is_not_installed() {
    let data = tempfile::tempdir().unwrap();
    let served = claude_released(&sha(b"another build"));

    let refused = program_for(
        &kept_pinned(Some("2.1.300")),
        data.path(),
        Some(Platform::X64),
        &served,
        &|_| {},
    )
    .unwrap_err();

    assert!(
        refused.contains("Claude Code 2.1.300 was not installed"),
        "{refused}"
    );
    assert!(refused.contains("does not match the SHA-256"), "{refused}");
    assert_eq!(left_in(&data.path().join("harnesses/claude")), [".lock"]);
}

/// A version whose vendor publishes no release of it, or no digest for this platform, is not
/// installed either, and nothing is downloaded.
#[test]
fn a_version_with_no_published_digest_is_not_installed() {
    let data = tempfile::tempdir().unwrap();
    let record = kept_pinned(Some("2.1.300"));

    let unreleased = program_for(
        &record,
        data.path(),
        Some(Platform::X64),
        &Served::default(),
        &|_| {},
    )
    .unwrap_err();
    assert!(unreleased.contains("was not read"), "{unreleased}");

    let no_digest = claude_released(&sha(CLAUDE));
    let refused = program_for(
        &record,
        data.path(),
        Some(Platform::Arm64),
        &no_digest,
        &|_| {},
    )
    .unwrap_err();
    assert!(refused.contains("no digest for linux-arm64"), "{refused}");
    assert!(
        !no_digest
            .asked
            .borrow()
            .iter()
            .any(|u| u.ends_with("/claude")),
        "{:?}",
        no_digest.asked
    );
}

/// A gzipped tar holding `entries`: each a name, and a file's bytes or a link's target.
fn tarball(entries: &[(&str, Result<&[u8], &str>)]) -> Vec<u8> {
    let mut built = tar::Builder::new(Vec::new());
    for (name, what) in entries {
        let mut header = tar::Header::new_gnu();
        match what {
            Ok(bytes) => {
                header.set_size(bytes.len() as u64);
                header.set_mode(0o755);
                header.set_entry_type(tar::EntryType::Regular);
                built.append_data(&mut header, name, *bytes).unwrap();
            }
            Err(target) => {
                header.set_size(0);
                header.set_entry_type(tar::EntryType::Symlink);
                built.append_link(&mut header, name, target).unwrap();
            }
        }
    }
    let tar = built.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar).unwrap();
    gz.finish().unwrap()
}

/// Codex's GitHub release `tag`, with `asset` and the digest GitHub publishes for it.
fn codex_released(asset: &[u8], digest: Option<String>) -> Served {
    let name = "codex-x86_64-unknown-linux-musl.tar.gz";
    let url = format!("https://github.com/openai/codex/releases/download/rust-v0.165.0/{name}");
    Served::default()
        .with(
            "https://api.github.com/repos/openai/codex/releases/tags/rust-v0.165.0",
            serde_json::json!({
                "tag_name": "rust-v0.165.0",
                "assets": [{"name": name, "browser_download_url": url, "digest": digest}],
            })
            .to_string(),
        )
        .with(&url, asset.to_vec())
}

fn codex_pinned() -> MachineProfile {
    MachineProfile {
        harness: "codex".to_owned(),
        ..kept_pinned(Some("0.165.0"))
    }
}

/// A harness released as an archive is installed as the one program in it its release names,
/// and nothing else of the archive is written.
#[test]
fn an_archived_release_installs_only_its_program() {
    let data = tempfile::tempdir().unwrap();
    let archive = tarball(&[
        ("README.md", Ok(b"readme")),
        ("codex-x86_64-unknown-linux-musl", Ok(b"the codex program")),
    ]);
    let served = codex_released(&archive, Some(format!("sha256:{}", sha(&archive))));

    let program = program_for(
        &codex_pinned(),
        data.path(),
        Some(Platform::X64),
        &served,
        &|_| {},
    )
    .unwrap()
    .unwrap();

    assert_eq!(program, data.path().join("harnesses/codex/0.165.0/codex"));
    assert_eq!(fs::read(&program).unwrap(), b"the codex program");
    assert_eq!(left_in(program.parent().unwrap()), ["codex"]);
}

/// A release asset GitHub publishes no digest for is not installed, and one whose program is
/// a link in the archive is not either.
#[test]
fn an_archive_with_no_digest_or_a_link_for_its_program_is_not_installed() {
    let data = tempfile::tempdir().unwrap();
    let archive = tarball(&[("codex-x86_64-unknown-linux-musl", Err("/bin/sh"))]);

    let undigested = codex_released(&archive, None);
    let refused = program_for(
        &codex_pinned(),
        data.path(),
        Some(Platform::X64),
        &undigested,
        &|_| {},
    )
    .unwrap_err();
    assert!(refused.contains("publishes no SHA-256"), "{refused}");

    let linked = codex_released(&archive, Some(format!("sha256:{}", sha(&archive))));
    let refused = program_for(
        &codex_pinned(),
        data.path(),
        Some(Platform::X64),
        &linked,
        &|_| {},
    )
    .unwrap_err();
    assert!(refused.contains("is not a file"), "{refused}");
    assert_eq!(left_in(&data.path().join("harnesses/codex")), [".lock"]);
}

/// **Nothing is downloaded for a profile no runner chat starts on that it installs**: one not
/// kept for a desktop, one with no pin, and the desktop's own records run the harness on `PATH`.
#[test]
fn nothing_is_fetched_for_a_profile_not_kept_for_a_desktop_with_a_pin() {
    let data = tempfile::tempdir().unwrap();
    let served = claude_released(&sha(CLAUDE));
    let unkept = MachineProfile {
        kept_for: None,
        ..kept_pinned(Some("2.1.300"))
    };
    let adopted_here = MachineProfile {
        adopted: Some(Adopted {
            at: 1,
            variables: Vec::new(),
        }),
        ..unkept.clone()
    };

    for record in [unkept, adopted_here, kept_pinned(None)] {
        assert_eq!(
            program_for(&record, data.path(), Some(Platform::X64), &served, &|_| {}),
            Ok(None)
        );
    }
    assert!(served.asked.borrow().is_empty());
    assert!(!data.path().join("harnesses").exists());
}

/// A pin that is not a version names no folder, and nothing is fetched for it.
#[test]
fn a_pin_that_is_not_a_version_is_refused_with_nothing_fetched() {
    let data = tempfile::tempdir().unwrap();
    let served = claude_released(&sha(CLAUDE));

    let refused = program_for(
        &kept_pinned(Some("../../bin")),
        data.path(),
        Some(Platform::X64),
        &served,
        &|_| {},
    )
    .unwrap_err();

    assert!(refused.contains("not a version"), "{refused}");
    assert!(served.asked.borrow().is_empty());
}

/// Only an HTTPS address is fetched.
#[test]
fn only_an_https_address_is_fetched() {
    let refused = OverHttps
        .fetch("http://example.com/claude", 10, &mut Vec::new())
        .unwrap_err();
    assert!(refused.contains("not an HTTPS address"), "{refused}");
}

/// **A fetch says what it fetches, in a line the sign-in's shell shows** (#4007): the harness,
/// its version and that it is checked against its vendor's SHA-256. A version installed already
/// is run with nothing said.
#[test]
fn a_fetch_says_what_it_fetches_and_nothing_once_it_is_installed() {
    let data = tempfile::tempdir().unwrap();
    let served = claude_released(&sha(CLAUDE));
    let record = kept_pinned(Some("2.1.300"));
    let said = RefCell::new(Vec::<String>::new());
    let saying = |line: &str| said.borrow_mut().push(line.to_owned());

    program_for(&record, data.path(), Some(Platform::X64), &served, &saying).unwrap();
    let first = said.borrow().clone();
    program_for(&record, data.path(), Some(Platform::X64), &served, &saying).unwrap();

    assert_eq!(first.len(), 1, "{first:?}");
    assert!(first[0].contains("Claude Code 2.1.300"), "{first:?}");
    assert!(first[0].contains("SHA-256"), "{first:?}");
    assert_eq!(*said.borrow(), first, "nothing more is said once installed");
}

/// A vendor's releases a look's fetch reads from another thread: what each address answers, and
/// how many downloads of a program were asked.
struct ServedFromAnywhere {
    answers: HashMap<String, Vec<u8>>,
    downloads: std::sync::atomic::AtomicUsize,
}

impl Fetch for ServedFromAnywhere {
    fn fetch(&self, url: &str, most: u64, into: &mut dyn Write) -> Result<(), String> {
        if !url.ends_with(".json") {
            self.downloads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        let body = self.answers.get(url).ok_or("404 Not Found")?;
        if body.len() as u64 > most {
            return Err("too large".to_owned());
        }
        into.write_all(body).map_err(|e| e.to_string())
    }
}

fn served_from_anywhere(served: Served) -> &'static ServedFromAnywhere {
    Box::leak(Box::new(ServedFromAnywhere {
        answers: served.answers,
        downloads: std::sync::atomic::AtomicUsize::new(0),
    }))
}

/// Asks [`for_a_look_at`] until it answers a program, for at most ten seconds.
fn looked_until_installed(
    record: &MachineProfile,
    data: &Path,
    fetch: &'static ServedFromAnywhere,
) -> Option<PathBuf> {
    let until = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(program) = for_a_look_at(record, data, Some(Platform::X64), fetch) {
            return Some(program);
        }
        if std::time::Instant::now() > until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// **Whether a runner's profile is signed in is asked of its pinned version** (#4007): a look
/// at one whose version is not installed yet fetches it, through the same install a chat's
/// start runs, without holding the look up, and a later look finds it installed.
#[test]
fn a_look_fetches_the_pinned_version_once_and_finds_it_installed() {
    let data = tempfile::tempdir().unwrap();
    let fetch = served_from_anywhere(claude_released(&sha(CLAUDE)));
    let record = kept_pinned(Some("2.1.300"));

    let program = looked_until_installed(&record, data.path(), fetch).expect("installed");

    assert_eq!(program, data.path().join("harnesses/claude/2.1.300/claude"));
    assert_eq!(fs::read(&program).unwrap(), CLAUDE);
}

/// A look whose fetch is refused (a download that does not match its published digest)
/// installs nothing and answers no program, so the harness on `PATH` is asked, as before; the
/// looks after it, as often as the window asks, do not download it again for a while.
#[test]
fn a_look_whose_fetch_is_refused_installs_nothing() {
    let data = tempfile::tempdir().unwrap();
    let fetch = served_from_anywhere(claude_released(&sha(b"another build")));
    let record = kept_pinned(Some("2.1.300"));

    for _ in 0..50 {
        assert_eq!(
            for_a_look_at(&record, data.path(), Some(Platform::X64), fetch),
            None
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!data.path().join("harnesses/claude/2.1.300").exists());
    assert_eq!(
        fetch.downloads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "downloaded once"
    );
}

/// A look at a profile no runner installs a version of fetches nothing.
#[test]
fn a_look_at_a_profile_not_kept_with_a_pin_fetches_nothing() {
    let data = tempfile::tempdir().unwrap();
    let fetch = served_from_anywhere(claude_released(&sha(CLAUDE)));
    let unkept = MachineProfile {
        kept_for: None,
        ..kept_pinned(Some("2.1.300"))
    };

    for record in [unkept, kept_pinned(None)] {
        assert_eq!(
            for_a_look_at(&record, data.path(), Some(Platform::X64), fetch),
            None
        );
    }
    std::thread::sleep(Duration::from_millis(100));
    assert!(!data.path().join("harnesses").exists());
}
