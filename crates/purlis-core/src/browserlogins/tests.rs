// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Kept browser logins (#2021): sealed at rest, opened only where and as last sealed, one
//! writer, bound to the login as declared, swept and wiped with their profile. The keyring is a
//! file of the test's own, never this machine's.

use std::path::Path;

use super::{
    DIR, Declared, FILE, Keys, Store, hold, hold_since, identity_of, kept_with, lock_keys, mark,
    open_into, removed, seal, sweep, unseal, wipe_profile,
};
use crate::secrets::keyring::FileStore;

/// A cookie a signed-in browser holds: what must never be readable at rest.
const COOKIE: &str = "session=fixture-cookie-7d1f";

/// A browser's own data folder, signed in: its cookies, its storage, and what it caches.
fn a_signed_in_browser(at: &Path) {
    std::fs::create_dir_all(at.join("Default/Local Storage")).unwrap();
    std::fs::create_dir_all(at.join("Default/Cache")).unwrap();
    std::fs::create_dir_all(at.join("home")).unwrap();
    std::fs::write(at.join("Default/Cookies"), COOKIE).unwrap();
    std::fs::write(at.join("Default/Local Storage/leveldb"), "kept").unwrap();
    std::fs::write(at.join("Default/Cache/data_0"), "cached").unwrap();
    std::fs::write(at.join("engine.log"), "log").unwrap();
}

fn keys(store: &FileStore) -> Keys<'_> {
    Keys {
        store,
        device: "01JDEVICE0000000000000000".into(),
    }
}

/// `shop-login` as declared: vault `shop`, `user` and `pass`, on the shop's page.
const SHOP: Declared<'static> = Declared {
    vault: "shop",
    user_key: Some("user"),
    password_key: "pass",
    page: "https://shop.test/login",
};

/// `profile`'s `shop-login` of `persona`, declared as `declared`.
fn store_of(profile: &str, persona: &str, declared: &Declared<'_>) -> Store {
    let identity = identity_of(persona, "project-1", declared);
    Store::new(profile, persona, "shop-login", &identity).unwrap()
}

fn shop(profile: &str) -> Store {
    store_of(profile, "steward", &SHOP)
}

#[test]
fn a_kept_store_is_not_readable_as_plain_files_and_opens_where_it_was_kept() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let keyring = FileStore::at(base.path().join("keyring.json"));
    a_signed_in_browser(&browser);
    let store = shop("claude");
    let held = hold(&data, &store).unwrap().expect("held");
    held.keep(&browser, &keys(&keyring)).unwrap();
    assert!(store.exists(&data));
    let sealed = std::fs::read(store.dir(&data).join(FILE)).unwrap();
    assert!(
        !sealed.windows(COOKIE.len()).any(|w| w == COOKIE.as_bytes()),
        "the cookie is not in the store as written"
    );
    let opened = base.path().join("next");
    assert!(open_into(&data, &store, &opened, &keys(&keyring)).unwrap());
    assert_eq!(
        std::fs::read_to_string(opened.join("Default/Cookies")).unwrap(),
        COOKIE
    );
    assert!(opened.join("Default/Local Storage/leveldb").is_file());
    // What it caches, its home and its log are never kept.
    assert!(!opened.join("Default/Cache").exists());
    assert!(!opened.join("home").exists());
    assert!(!opened.join("engine.log").exists());
}

#[test]
fn a_store_moved_elsewhere_or_of_another_generation_or_key_does_not_open() {
    let base = tempfile::tempdir().unwrap();
    let browser = base.path().join("browser");
    a_signed_in_browser(&browser);
    let key = [7u8; 32];
    let place = shop("claude").place();
    let sealed = seal(&browser, &place, 2, &key).unwrap();
    let into = base.path().join("into");
    std::fs::create_dir_all(&into).unwrap();
    let elsewhere = unseal(&sealed, &shop("work").place(), 2, &key, &into).unwrap_err();
    assert!(elsewhere.contains("do not open"), "{elsewhere}");
    assert!(unseal(&sealed, &place, 1, &key, &into).is_err(), "older");
    assert!(unseal(&sealed, &place, 2, &[8u8; 32], &into).is_err());
    assert_eq!(
        std::fs::read_dir(&into).unwrap().count(),
        0,
        "nothing written"
    );
    unseal(&sealed, &place, 2, &key, &into).unwrap();
}

#[test]
fn an_older_copy_of_a_store_put_back_does_not_open() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let keyring = FileStore::at(base.path().join("keyring.json"));
    a_signed_in_browser(&browser);
    let store = shop("claude");
    let held = hold(&data, &store).unwrap().unwrap();
    held.keep(&browser, &keys(&keyring)).unwrap();
    let older = std::fs::read(store.dir(&data).join(FILE)).unwrap();
    held.keep(&browser, &keys(&keyring)).unwrap();
    std::fs::write(store.dir(&data).join(FILE), older).unwrap();
    let opened = base.path().join("next");
    assert!(open_into(&data, &store, &opened, &keys(&keyring)).is_err());
    assert!(!opened.exists(), "nothing opened");
}

#[test]
fn another_personas_or_another_declarations_login_of_the_same_name_has_a_store_of_its_own() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let keyring = FileStore::at(base.path().join("keyring.json"));
    a_signed_in_browser(&browser);
    let a = shop("claude");
    hold(&data, &a)
        .unwrap()
        .unwrap()
        .keep(&browser, &keys(&keyring))
        .unwrap();
    let persona_b = store_of("claude", "builder", &SHOP);
    let redeclared = store_of(
        "claude",
        "steward",
        &Declared {
            page: "https://elsewhere.test/login",
            ..SHOP
        },
    );
    let other_project = Store::new(
        "claude",
        "steward",
        "shop-login",
        &identity_of("steward", "project-2", &SHOP),
    )
    .unwrap();
    for other in [&persona_b, &redeclared, &other_project] {
        assert_ne!(other.dir(&data), a.dir(&data));
        assert!(!other.exists(&data));
        let into = base.path().join("other");
        assert!(!open_into(&data, other, &into, &keys(&keyring)).unwrap_or(false));
        assert!(
            !into.join("Default/Cookies").exists(),
            "{other:?} opened A's"
        );
    }
    // Even A's sealed file put in B's place does not open there.
    std::fs::create_dir_all(persona_b.dir(&data)).unwrap();
    std::fs::copy(a.dir(&data).join(FILE), persona_b.dir(&data).join(FILE)).unwrap();
    assert!(open_into(&data, &persona_b, &base.path().join("b"), &keys(&keyring)).is_err());
}

#[test]
fn one_browser_holds_a_store_and_another_gets_none() {
    let base = tempfile::tempdir().unwrap();
    let data = base.path().join("data");
    let store = shop("claude");
    let first = hold(&data, &store).unwrap();
    assert!(first.is_some());
    assert!(hold(&data, &store).unwrap().is_none(), "a copy");
    drop(first);
    assert!(hold(&data, &store).unwrap().is_some(), "free again");
}

#[test]
fn a_removed_profiles_stores_and_once_none_is_left_the_key_are_wiped() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let folders = base.path().join("browsers");
    let keyring = FileStore::at(base.path().join("keyring.json"));
    a_signed_in_browser(&browser);
    for profile in ["claude", "work"] {
        let held = hold(&data, &shop(profile)).unwrap().unwrap();
        held.keep(&browser, &keys(&keyring)).unwrap();
    }
    assert!(wipe_profile(&data, &folders, "claude", &keys(&keyring)).unwrap());
    assert!(!shop("claude").exists(&data));
    assert!(shop("work").exists(&data), "another profile's stays");
    assert!(keys(&keyring).read().unwrap().is_some(), "still in use");
    let opened = base.path().join("work-next");
    assert!(open_into(&data, &shop("work"), &opened, &keys(&keyring)).unwrap());
    assert!(wipe_profile(&data, &folders, "work", &keys(&keyring)).unwrap());
    assert!(
        keys(&keyring).read().unwrap().is_none(),
        "no store left, no key"
    );
    assert!(!wipe_profile(&data, &folders, "work", &keys(&keyring)).unwrap());
}

#[test]
fn a_removed_profiles_browser_folders_left_behind_are_removed_but_a_running_ones_stay() {
    let base = tempfile::tempdir().unwrap();
    let (data, folders) = (base.path().join("data"), base.path().join("browsers"));
    let keyring = FileStore::at(base.path().join("keyring.json"));
    let left = folders.join("prog-1");
    let running = folders.join("prog-2");
    let other = folders.join("prog-3");
    for (folder, profile) in [(&left, "claude"), (&running, "claude"), (&other, "work")] {
        a_signed_in_browser(folder);
        a_signed_in_browser(&folder.with_extension("opening"));
        drop(mark(folder, &shop(profile)).unwrap());
    }
    let _running = mark(&running, &shop("claude")).unwrap();
    // A folder being opened into that no browser holds, its mark never written.
    let stray = folders.join("prog-4.opening");
    a_signed_in_browser(&stray);
    wipe_profile(&data, &folders, "claude", &keys(&keyring)).unwrap();
    assert!(!left.exists() && !left.with_extension("opening").exists());
    assert!(!left.with_extension("kept").exists());
    assert!(!stray.exists());
    assert!(
        running.join("Default/Cookies").is_file(),
        "a running browser's stays"
    );
    assert!(
        other.join("Default/Cookies").is_file(),
        "another profile's stays"
    );
}

#[test]
fn a_sweep_never_takes_the_folder_about_to_be_used() {
    let base = tempfile::tempdir().unwrap();
    let folders = base.path().join("browsers");
    let own = folders.join("prog-1");
    a_signed_in_browser(&own);
    drop(mark(&own, &shop("claude")).unwrap());
    sweep(&folders, Some(&own), &|_, _| true);
    assert!(own.is_dir());
    sweep(&folders, None, &|_, _| true);
    assert!(!own.exists());
}

#[test]
fn a_name_that_is_no_plain_name_has_no_store() {
    let identity = identity_of("p", "x", &SHOP);
    for (profile, persona, login) in [
        ("..", "p", "x"),
        ("a/b", "p", "x"),
        ("claude", "p", "../x"),
        ("claude", "../p", "x"),
        ("", "p", "x"),
        (".keys.lock", "p", "x"),
    ] {
        assert!(
            Store::new(profile, persona, login, &identity).is_err(),
            "{profile} {persona} {login}"
        );
    }
    assert!(Store::new("claude", "p", "x", "../../etc").is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn the_kept_logins_folder_is_left_out_of_this_macs_backups() {
    let base = tempfile::tempdir().unwrap();
    let data = base.path().join("data");
    drop(hold(&data, &shop("claude")).unwrap());
    let set = xattr::get(data.join(super::DIR), super::BACKUP_EXCLUSION)
        .unwrap()
        .expect("excluded");
    assert_eq!(set, super::BACKUP_EXCLUDED);
}

#[test]
fn an_identity_is_of_the_declarations_fields_and_none_run_together() {
    let base = identity_of("steward", "project-1", &SHOP);
    assert_eq!(base, identity_of("steward", "project-1", &SHOP), "stable");
    for other in [
        Declared {
            vault: "shop2",
            ..SHOP
        },
        Declared {
            user_key: None,
            ..SHOP
        },
        Declared {
            user_key: Some(""),
            ..SHOP
        },
        Declared {
            password_key: "pass2",
            ..SHOP
        },
        Declared {
            page: "https://shop.test/login2",
            ..SHOP
        },
        // The same bytes moved from one field to the next are another declaration.
        Declared {
            vault: "shopuser",
            user_key: Some(""),
            ..SHOP
        },
    ] {
        assert_ne!(
            identity_of("steward", "project-1", &other),
            base,
            "{other:?}"
        );
    }
    assert_ne!(identity_of("stewar", "dproject-1", &SHOP), base);
}

#[test]
fn two_browsers_ending_at_once_both_keep_and_both_reopen() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let keyring = std::sync::Arc::new(FileStore::at(base.path().join("keyring.json")));
    a_signed_in_browser(&browser);
    let stores: Vec<Store> = (0..6)
        .map(|n| {
            Store::new(
                "claude",
                "steward",
                &format!("login-{n}"),
                &identity_of("steward", "project-1", &SHOP),
            )
            .unwrap()
        })
        .collect();
    let holds: Vec<_> = stores
        .iter()
        .map(|s| hold(&data, s).unwrap().unwrap())
        .collect();
    // Twice over, so a lost generation would show as a store that no longer opens.
    for _ in 0..2 {
        std::thread::scope(|scope| {
            for held in &holds {
                let (keyring, browser) = (&keyring, &browser);
                scope.spawn(move || held.keep(browser, &keys(keyring)).unwrap());
            }
        });
    }
    for (n, store) in stores.iter().enumerate() {
        let into = base.path().join(format!("next-{n}"));
        assert!(
            open_into(&data, store, &into, &keys(&keyring)).unwrap(),
            "{store:?}"
        );
        assert_eq!(
            std::fs::read_to_string(into.join("Default/Cookies")).unwrap(),
            COOKIE
        );
    }
}

#[test]
fn a_store_is_never_made_through_a_link() {
    let base = tempfile::tempdir().unwrap();
    let elsewhere = base.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    for linked in ["", "claude", "claude/steward"] {
        let data = base.path().join(format!("data-{}", linked.len()));
        let root = data.join(super::DIR);
        let at = if linked.is_empty() {
            root
        } else {
            root.join(linked)
        };
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &at).unwrap();
        assert!(hold(&data, &shop("claude")).is_err(), "{linked:?}");
        assert_eq!(
            std::fs::read_dir(&elsewhere).unwrap().count(),
            0,
            "{linked:?}"
        );
    }
}

// ---- Tightened after #2021 (#2693) ----

#[test]
fn opening_a_store_waits_for_a_write_back_under_way() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let keyring = FileStore::at(base.path().join("keyring.json"));
    a_signed_in_browser(&browser);
    let store = shop("claude");
    hold(&data, &store)
        .unwrap()
        .unwrap()
        .keep(&browser, &keys(&keyring))
        .unwrap();
    // A write-back holds the keyring's lock from its read of the generation to its write.
    let writing = lock_keys(&data).unwrap();
    let (opened, done) = std::sync::mpsc::channel();
    let (into, at) = (base.path().join("copy"), data.clone());
    let keyring_path = base.path().join("keyring.json");
    let opening = std::thread::spawn(move || {
        let keyring = FileStore::at(keyring_path);
        let _ = opened.send(open_into(&at, &shop("claude"), &into, &keys(&keyring)));
    });

    assert!(
        done.recv_timeout(std::time::Duration::from_millis(300))
            .is_err(),
        "it reads the store and its generation only once the write is done"
    );
    drop(writing);
    assert!(done.recv().unwrap().unwrap(), "it opens as last written");
    opening.join().unwrap();
}

#[test]
fn a_hold_taken_for_a_browser_that_began_before_its_profile_was_removed_makes_nothing() {
    let base = tempfile::tempdir().unwrap();
    let data = base.path().join("data");
    let began = std::time::SystemTime::now();
    // The profile never kept a login, and is removed while its browser runs.
    removed(&data, "claude").unwrap();

    let why = hold_since(&data, &shop("claude"), began).unwrap_err();

    assert!(why.contains("removed"), "{why}");
    assert!(
        !data.join(DIR).join("claude").exists(),
        "no folder made again"
    );
    // A browser that begins after it, as a new profile of the name's, keeps as any.
    assert!(hold(&data, &shop("claude")).unwrap().is_some());
}

#[test]
fn a_removed_profile_with_no_store_is_still_marked_removed() {
    let base = tempfile::tempdir().unwrap();
    let data = base.path().join("data");
    let keyring = FileStore::at(base.path().join("keyring.json"));
    let began = std::time::SystemTime::now();

    wipe_profile(
        &data,
        &base.path().join("browsers"),
        "claude",
        &keys(&keyring),
    )
    .unwrap();

    assert!(hold_since(&data, &shop("claude"), began).is_err());
    assert!(!data.join(DIR).join("claude").exists());
}

#[test]
fn no_login_is_kept_on_a_build_that_lists_nothing_it_leaves_out() {
    assert!(kept_with(&["Cache"]).is_ok());
    let why = kept_with(&[]).unwrap_err();
    assert!(why.contains("not kept on this build"), "{why}");
}

#[test]
fn a_hold_taken_just_before_its_profile_was_removed_writes_nothing_back() {
    let base = tempfile::tempdir().unwrap();
    let (data, browser) = (base.path().join("data"), base.path().join("browser"));
    let keyring = FileStore::at(base.path().join("keyring.json"));
    a_signed_in_browser(&browser);
    let store = shop("claude");
    let began = std::time::SystemTime::now();
    let held = hold_since(&data, &store, began).unwrap().expect("held");

    removed(&data, "claude").unwrap();

    let why = held.keep(&browser, &keys(&keyring)).unwrap_err();
    assert!(why.contains("removed"), "{why}");
    assert!(
        !store.dir(&data).join(FILE).exists(),
        "no state.sealed left"
    );
}

#[test]
fn a_removal_marked_again_replaces_its_mark_whole() {
    let base = tempfile::tempdir().unwrap();
    let data = base.path().join("data");
    removed(&data, "claude").unwrap();
    let began = std::time::SystemTime::now();
    removed(&data, "claude").unwrap();

    assert!(hold_since(&data, &shop("claude"), began).is_err());
    let left: Vec<_> = std::fs::read_dir(data.join(DIR))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".removed-"))
        .collect();
    assert_eq!(
        left,
        [".removed-claude"],
        "nothing written beside it is left"
    );
}
