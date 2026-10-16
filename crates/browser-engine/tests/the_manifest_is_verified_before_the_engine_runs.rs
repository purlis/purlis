// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The engine is downloaded on first use, and runs only once what was downloaded is what a
//! trusted manifest pins: the manifest this build ships, or a newer one whose signature
//! verifies. What verifies an engine manifest's signature in a build is Sigstore's
//! (`an_engine_update_is_taken_only_as_the_manifest_workflow_signed_it.rs`); here a minisign key
//! made for each test stands in for it, so these hold the mechanics around any signer. Nothing here reaches the network: the download is served from memory.

#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;

use common::fixture::{self, Served, VERSION, archive, at, manifest, manifest_text, sha256, url};
use purlis_browser_engine::install::{
    EngineChoice, EngineSettings, Engines, ManifestSignature, NoSigner, Platform, Refused,
};

const ISSUED: &str = "2026-10-14T11:19:22Z";
const NOW: &str = "2026-10-15T09:00:00Z";

fn engines(root: &std::path::Path, digest: &str, signer: Box<dyn ManifestSignature>) -> Engines {
    Engines::new(
        root,
        Platform::Linux64,
        manifest(VERSION, ISSUED, digest),
        signer,
    )
}

fn leftovers(root: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .map(|dir| {
            dir.map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn the_first_use_downloads_the_pinned_engine_checks_its_digest_and_runs_it_from_where_it_was_unpacked()
 {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes), Box::new(NoSigner));
    let ready = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert_eq!(ready.version(), Some(VERSION));
    assert!(ready.program().starts_with(root.path()));
    assert!(ready.program().ends_with("chrome-linux64/chrome"));
    let script = std::fs::read_to_string(ready.program()).unwrap();
    assert!(script.contains("pinned"));
    let mode = std::fs::metadata(ready.program())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111, "the engine is executable");
    assert_eq!(*served.asked.borrow(), vec![url(VERSION)]);
}

#[test]
fn an_installed_engine_is_not_downloaded_again() {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes), Box::new(NoSigner));
    let first = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    let second = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert_eq!(first.program(), second.program());
    assert_eq!(served.asked.borrow().len(), 1);
}

#[test]
fn an_archive_that_is_not_the_pinned_one_is_refused_and_nothing_of_it_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let served = Served::with(url(VERSION), archive("someone else's"));
    let engines = engines(root.path(), &sha256(&archive("pinned")), Box::new(NoSigner));
    let refused = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::Digest { .. }), "{refused:?}");
    assert!(
        refused.to_string().contains("not the one this build pins"),
        "{refused}"
    );
    assert_eq!(leftovers(root.path()), Vec::<String>::new());
}

#[test]
fn a_platform_the_manifest_does_not_pin_is_refused_in_words() {
    let root = tempfile::tempdir().unwrap();
    let engines = Engines::new(
        root.path(),
        Platform::MacArm64,
        manifest(VERSION, ISSUED, &sha256(b"x")),
        Box::new(NoSigner),
    );
    let refused = engines
        .prepare(&EngineSettings::default(), &Served::default(), at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::NotPinned { .. }), "{refused:?}");
    assert!(refused.to_string().contains("mac-arm64"), "{refused}");
}

#[test]
fn a_manifest_that_names_a_plain_http_download_or_a_malformed_digest_does_not_parse() {
    let plain = manifest_text(VERSION, ISSUED, &sha256(b"x")).replace("https://", "http://");
    assert!(purlis_browser_engine::install::Manifest::parse(plain.as_bytes()).is_err());
    let short = manifest_text(VERSION, ISSUED, "abc123");
    assert!(purlis_browser_engine::install::Manifest::parse(short.as_bytes()).is_err());
    let unpinned = manifest_text(VERSION, ISSUED, "");
    assert!(purlis_browser_engine::install::Manifest::parse(unpinned.as_bytes()).is_err());
}

#[test]
fn the_manifest_this_build_ships_parses_and_names_a_version() {
    let shipped = purlis_browser_engine::install::Manifest::shipped();
    assert!(!shipped.version.is_empty());
}

struct Keys(minisign::KeyPair);

impl Keys {
    fn new() -> Self {
        Self(minisign::KeyPair::generate_unencrypted_keypair().unwrap())
    }
    fn signer(&self) -> Box<dyn ManifestSignature> {
        Box::new(Trusts(self.0.pk.clone()))
    }
    fn sign(&self, text: &str) -> String {
        minisign::sign(
            None,
            &self.0.sk,
            std::io::Cursor::new(text.as_bytes()),
            Some("engine manifest"),
            None,
        )
        .unwrap()
        .into_string()
    }
}

/// A signer that trusts one minisign key.
struct Trusts(minisign::PublicKey);

impl ManifestSignature for Trusts {
    fn verify(&self, manifest: &[u8], signature: &str) -> Result<(), String> {
        let signature =
            minisign::SignatureBox::from_string(signature).map_err(|why| why.to_string())?;
        minisign::verify(
            &self.0,
            &signature,
            std::io::Cursor::new(manifest),
            true,
            false,
            false,
        )
        .map_err(|why| why.to_string())
    }
}

const NEWER: &str = "156.0.8100.10";
const NEWER_ISSUED: &str = "2026-10-20T08:00:00Z";

#[test]
fn a_newer_manifest_signed_by_the_trusted_key_is_installed_and_runs_from_then_on() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let engines = engines(root.path(), &sha256(&old), keys.signer());
    engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();

    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    let updated = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at("2026-10-20T09:00:00Z"),
        )
        .unwrap();
    assert_eq!(updated.version(), Some(NEWER));
    let ready = engines
        .prepare(
            &EngineSettings::default(),
            &served,
            at("2026-10-20T09:00:00Z"),
        )
        .unwrap();
    assert_eq!(ready.version(), Some(NEWER));
    assert!(
        std::fs::read_to_string(ready.program())
            .unwrap()
            .contains("newer")
    );
}

#[test]
fn a_manifest_signed_by_another_key_altered_after_signing_or_unsigned_is_refused_and_the_engine_stays()
 {
    let root = tempfile::tempdir().unwrap();
    let (trusted, stranger) = (Keys::new(), Keys::new());
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let engines = engines(root.path(), &sha256(&old), trusted.signer());
    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    let altered = text.replace(&sha256(&new), &sha256(b"something else"));
    for (manifest, signature) in [
        (text.clone(), stranger.sign(&text)),
        (altered, trusted.sign(&text)),
        (text.clone(), String::new()),
        (text.clone(), "untrusted comment: nothing\nRWQ\n".to_owned()),
    ] {
        let refused = engines
            .update(
                manifest.as_bytes(),
                &signature,
                &EngineSettings::default(),
                &served,
                at(NOW),
            )
            .unwrap_err();
        assert!(matches!(refused, Refused::Signature(_)), "{refused:?}");
    }
    assert!(
        !served.asked.borrow().contains(&url(NEWER)),
        "nothing unverified was downloaded"
    );
    let ready = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert_eq!(ready.version(), Some(VERSION));
}

#[test]
fn a_build_that_trusts_no_update_key_refuses_every_update_and_keeps_its_pinned_engine() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let engines = engines(root.path(), &sha256(&old), Box::new(NoSigner));
    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    let refused = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at(NOW),
        )
        .unwrap_err();
    assert!(matches!(refused, Refused::Signature(_)), "{refused:?}");
    let ready = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert_eq!(ready.version(), Some(VERSION));
}

#[test]
fn a_signed_manifest_older_than_the_one_in_use_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let old = archive("pinned");
    let served = Served::with(url(VERSION), old.clone());
    let engines = engines(root.path(), &sha256(&old), keys.signer());
    let text = manifest_text("150.0.1.1", "2026-09-01T00:00:00Z", &sha256(&old));
    let refused = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at(NOW),
        )
        .unwrap_err();
    assert!(matches!(refused, Refused::Older { .. }), "{refused:?}");
}

#[test]
fn a_stored_update_that_no_longer_verifies_is_ignored_for_the_pinned_engine() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    engines(root.path(), &sha256(&old), keys.signer())
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at(NOW),
        )
        .unwrap();
    // The same folder, read by a build that trusts another key.
    let other = engines(root.path(), &sha256(&old), Keys::new().signer());
    let ready = other
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert_eq!(ready.version(), Some(VERSION));
}

#[test]
fn the_machines_own_chrome_runs_as_chosen_with_nothing_downloaded() {
    let root = tempfile::tempdir().unwrap();
    let chrome = root.path().join("my-chrome");
    std::fs::write(&chrome, "#!/bin/sh\n").unwrap();
    let served = Served::default();
    let engines = engines(root.path(), &sha256(b"x"), Box::new(NoSigner));
    let settings = EngineSettings {
        engine: EngineChoice::Machine(chrome.clone()),
        ..EngineSettings::default()
    };
    let ready = engines.prepare(&settings, &served, at(NOW)).unwrap();
    assert_eq!(ready.program(), chrome);
    assert_eq!(ready.version(), None);
    assert!(served.asked.borrow().is_empty());

    let missing = EngineSettings {
        engine: EngineChoice::Machine(root.path().join("no-such-chrome")),
        ..EngineSettings::default()
    };
    let refused = engines.prepare(&missing, &served, at(NOW)).unwrap_err();
    assert!(matches!(refused, Refused::MachineChrome(_)), "{refused:?}");
    let _ = fixture::VERSION;
}

#[test]
fn a_signed_manifest_issued_later_but_naming_an_older_version_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let old = archive("pinned");
    let served = Served::with(url(VERSION), old.clone());
    let engines = engines(root.path(), &sha256(&old), keys.signer());
    let text = manifest_text("150.0.1.1", NEWER_ISSUED, &sha256(&old));
    let refused = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at(NOW),
        )
        .unwrap_err();
    assert!(matches!(refused, Refused::Older { .. }), "{refused:?}");
}

#[test]
fn an_update_reports_its_age_by_the_persons_settings() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let engines = engines(root.path(), &sha256(&old), keys.signer());
    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    let settings = EngineSettings {
        warn_after: std::time::Duration::from_secs(3600),
        ..EngineSettings::default()
    };
    let ready = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &settings,
            &served,
            at("2026-10-21T09:00:00Z"),
        )
        .unwrap();
    assert_eq!(
        ready.age(),
        purlis_browser_engine::install::Age::Behind { days: 1 }
    );
}

#[test]
fn settings_that_refuse_no_later_than_they_warn_are_refused_naming_them() {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes), Box::new(NoSigner));
    let settings = EngineSettings {
        warn_after: std::time::Duration::from_secs(10 * 86_400),
        refuse_after: std::time::Duration::from_secs(10 * 86_400),
        ..EngineSettings::default()
    };
    let refused = engines.prepare(&settings, &served, at(NOW)).unwrap_err();
    assert!(matches!(refused, Refused::Settings(_)), "{refused:?}");
    assert!(refused.to_string().contains("Settings"), "{refused}");
    assert!(served.asked.borrow().is_empty());
}

#[test]
fn an_archive_with_a_link_out_of_its_folder_is_refused_and_nothing_of_it_is_kept() {
    for target in [
        "/etc/passwd",
        "../../outside",
        "../chrome-linux64/../../outside",
    ] {
        let root = tempfile::tempdir().unwrap();
        let bytes = fixture::archive_with_link("chrome-linux64/lib", target);
        let served = Served::with(url(VERSION), bytes.clone());
        let engines = engines(root.path(), &sha256(&bytes), Box::new(NoSigner));
        let refused = engines
            .prepare(&EngineSettings::default(), &served, at(NOW))
            .unwrap_err();
        assert!(
            matches!(refused, Refused::Archive(_)),
            "{target}: {refused:?}"
        );
        assert_eq!(leftovers(root.path()), Vec::<String>::new(), "{target}");
    }
}

#[test]
fn an_archive_with_a_link_inside_its_folder_is_unpacked_with_the_link() {
    // macOS's engine is an app bundle, whose frameworks are linked within it.
    let root = tempfile::tempdir().unwrap();
    let bytes = fixture::archive_with_link("chrome-linux64/current", "resources.pak");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes), Box::new(NoSigner));
    let ready = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    let link = ready.program().parent().unwrap().join("current");
    assert_eq!(std::fs::read(link).unwrap(), b"resources");
}

#[test]
fn an_archive_that_names_the_install_record_itself_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let bytes = fixture::archive_with_link("installed.json", "chrome-linux64/resources.pak");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes), Box::new(NoSigner));
    let refused = engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::Archive(_)), "{refused:?}");
    assert_eq!(leftovers(root.path()), Vec::<String>::new());
}

/// The manifest workflow signs a manifest every day, so one naming the version in use, issued
/// later, says that version is still current: it is taken, and the engine ages from it.
#[test]
fn a_signed_manifest_of_the_same_version_issued_later_is_taken_and_the_engine_ages_from_it() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let old = archive("pinned");
    let served = Served::with(url(VERSION), old.clone());
    let engines = engines(root.path(), &sha256(&old), keys.signer());
    let late = "2026-11-10T09:00:00Z";
    let refused = engines
        .prepare(&EngineSettings::default(), &served, at(late))
        .unwrap_err();
    assert!(matches!(refused, Refused::Outdated { .. }), "{refused:?}");

    let text = manifest_text(VERSION, "2026-11-09T05:00:00Z", &sha256(&old));
    let ready = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at(late),
        )
        .unwrap();
    assert_eq!(ready.version(), Some(VERSION));
    let ready = engines
        .prepare(&EngineSettings::default(), &served, at(late))
        .unwrap();
    assert_eq!(ready.age(), purlis_browser_engine::install::Age::Current);
    assert_eq!(
        served.asked.borrow().len(),
        1,
        "the engine already installed is not downloaded again"
    );

    // The same manifest again is nothing new.
    let again = engines
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at(late),
        )
        .unwrap_err();
    assert!(matches!(again, Refused::Older { .. }), "{again:?}");
}

/// A build whose own pin is newer than an update kept from before never steps back to it.
#[test]
fn a_kept_update_naming_an_older_version_than_the_builds_pin_is_not_used_even_issued_later() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let text = manifest_text(VERSION, "2026-10-25T00:00:00Z", &sha256(&old));
    engines(root.path(), &sha256(&old), keys.signer())
        .update(
            text.as_bytes(),
            &keys.sign(&text),
            &EngineSettings::default(),
            &served,
            at("2026-10-25T01:00:00Z"),
        )
        .unwrap();
    let newer_build = Engines::new(
        root.path(),
        Platform::Linux64,
        manifest(NEWER, NEWER_ISSUED, &sha256(&new)),
        keys.signer(),
    );
    let ready = newer_build
        .prepare(
            &EngineSettings::default(),
            &served,
            at("2026-10-25T02:00:00Z"),
        )
        .unwrap();
    assert_eq!(ready.version(), Some(NEWER));
}

#[test]
fn an_update_is_kept_beside_its_sigstore_bundle() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    let signature = keys.sign(&text);
    engines(root.path(), &sha256(&old), keys.signer())
        .update(
            text.as_bytes(),
            &signature,
            &EngineSettings::default(),
            &served,
            at(NOW),
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("update.json")).unwrap(),
        text
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("update.json.sigstore.json")).unwrap(),
        signature
    );
}

/// The background check (#2073): the published manifest and its bundle are read, and taken only
/// as [`Engines::update`] takes one.
#[test]
fn the_published_manifest_is_read_with_its_bundle_and_taken_when_it_verifies() {
    use purlis_browser_engine::install::{PUBLISHED_BUNDLE, PUBLISHED_MANIFEST};
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (old, new) = (archive("pinned"), archive("newer"));
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(url(NEWER), new.clone());
    let engines = engines(root.path(), &sha256(&old), keys.signer());
    assert!(!engines.in_use(), "nothing is installed yet");
    engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert!(engines.in_use());

    // Nothing published yet: refused as a download, and the engine stays.
    let refused = engines
        .update_from_published(&EngineSettings::default(), &served, at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::Download(_)), "{refused:?}");

    let text = manifest_text(NEWER, NEWER_ISSUED, &sha256(&new));
    served
        .files
        .insert(PUBLISHED_MANIFEST.to_owned(), text.clone().into_bytes());
    served.files.insert(
        PUBLISHED_BUNDLE.to_owned(),
        Keys::new().sign(&text).into_bytes(),
    );
    let refused = engines
        .update_from_published(&EngineSettings::default(), &served, at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::Signature(_)), "{refused:?}");

    served
        .files
        .insert(PUBLISHED_BUNDLE.to_owned(), keys.sign(&text).into_bytes());
    let ready = engines
        .update_from_published(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    assert_eq!(ready.version(), Some(NEWER));
}

#[test]
fn a_published_manifest_larger_than_any_manifest_is_refused_unread() {
    use purlis_browser_engine::install::{
        MAX_MANIFEST_BYTES, PUBLISHED_BUNDLE, PUBLISHED_MANIFEST,
    };
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let old = archive("pinned");
    let mut served = Served::with(url(VERSION), old.clone());
    served.files.insert(
        PUBLISHED_MANIFEST.to_owned(),
        vec![b' '; MAX_MANIFEST_BYTES as usize + 1],
    );
    served
        .files
        .insert(PUBLISHED_BUNDLE.to_owned(), b"{}".to_vec());
    let refused = engines(root.path(), &sha256(&old), keys.signer())
        .update_from_published(&EngineSettings::default(), &served, at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::Download(_)), "{refused:?}");
}

/// With no network at all after the first download, and no update ever received, the engine
/// the build pins keeps running with the verifier a build uses.
#[test]
fn the_pinned_engine_runs_offline_with_no_update_ever_received() {
    use purlis_browser_engine::sigstore::Sigstore;
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(
        root.path(),
        &sha256(&bytes),
        Box::new(Sigstore::engine_manifests()),
    );
    engines
        .prepare(&EngineSettings::default(), &served, at(NOW))
        .unwrap();
    let offline = Served::default();
    let refused = engines
        .update_from_published(&EngineSettings::default(), &offline, at(NOW))
        .unwrap_err();
    assert!(matches!(refused, Refused::Download(_)), "{refused:?}");
    let ready = engines
        .prepare(&EngineSettings::default(), &offline, at(NOW))
        .unwrap();
    assert_eq!(ready.version(), Some(VERSION));
}
