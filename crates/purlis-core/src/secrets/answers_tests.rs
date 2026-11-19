// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sandbox::local::OwnConfigHome;

/// A machine of the test's own: its config home, a keyring that knows nothing, a key, a window.
struct Machine {
    _home: OwnConfigHome,
    dir: tempfile::TempDir,
}

fn machine() -> Machine {
    let home = OwnConfigHome::new();
    seal::forget_kept_for_this_test();
    seal::set_for_this_test(Some(vec![7; 32]));
    seal::set_foreign_for_this_test(false);
    seal::set_locked_for_this_test(false);
    seal::set_no_keyring_for_this_test(false);
    set_window_for_this_test(true);
    Machine {
        _home: home,
        dir: tempfile::tempdir().expect("a folder for planes"),
    }
}

impl Machine {
    /// A plane called `name`, with a `.git`.
    fn plane(&self, name: &str) -> PathBuf {
        let root = self.dir.path().join(name);
        std::fs::create_dir_all(root.join(".git")).expect("a plane");
        std::fs::canonicalize(&root).expect("a real path")
    }
}

/// Registers `root` and adds `add`, as the person pressing in the window.
fn seal_in(root: &Path, add: &[(&str, &str)]) {
    register(root).expect("registered");
    commit(root, |draft| {
        for (kind, text) in add {
            draft.add(kind, text);
        }
    })
    .expect("sealed");
}

/// The plane's folder in the store.
fn store_of(root: &Path) -> PathBuf {
    plane_place(root).expect("a place").folder
}

fn entry_file(root: &Path, kind: &str, text: &str) -> PathBuf {
    store_of(root)
        .join(kind)
        .join(format!("{}.json", entry_digest(kind, text)))
}

/// A `.git` made again beside the old one, so its inode is another: a swapped clone.
fn swap_git(root: &Path) {
    std::fs::rename(root.join(".git"), root.join(".git-old")).expect("moved aside");
    std::fs::create_dir(root.join(".git")).expect("a new .git");
    std::fs::remove_dir_all(root.join(".git-old")).expect("the old one gone");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for item in std::fs::read_dir(from).expect("a folder").flatten() {
        let target = to.join(item.file_name());
        if item.path().is_dir() {
            copy_dir(&item.path(), &target);
        } else {
            std::fs::copy(item.path(), target).expect("copied");
        }
    }
}

fn holds(standing: &Standing) -> bool {
    standing.state == State::Holds
}

fn denied_by(standing: &Standing, cause: impl Fn(&Deny) -> bool) -> bool {
    standing.is_deny() && standing.causes().iter().any(cause)
}

#[test]
fn a_grant_edited_by_hand_counts_as_no_answer() {
    let m = machine();
    let root = m.plane("p");
    seal_in(
        &root,
        &[(GRANT, "run npm"), (ACCEPTED, "guards.git=enforce")],
    );
    let file = entry_file(&root, GRANT, "run npm");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("run npm", "run anything")).unwrap();
    let standing = standing(&root);
    assert!(holds(&standing), "{standing:?}");
    assert!(!standing.entries.iter().any(|e| e.kind == GRANT));
    assert_eq!(standing.no_answer.len(), 1);
    // The other entry still counts.
    assert!(
        standing
            .entries
            .contains(&Entry::new(ACCEPTED, "guards.git=enforce"))
    );
}

#[test]
fn a_grant_from_another_clone_or_of_another_kind_counts_as_no_answer() {
    let m = machine();
    let (a, b) = (m.plane("a"), m.plane("b"));
    seal_in(&a, &[(GRANT, "run npm")]);
    seal_in(&b, &[(GRANT, "run cargo")]);
    // Another clone's grant file, put where this manifest's digest names it.
    let from = std::fs::read_to_string(entry_file(&a, GRANT, "run npm")).unwrap();
    std::fs::write(entry_file(&b, GRANT, "run cargo"), from).unwrap();
    let standing_b = standing(&b);
    assert!(holds(&standing_b), "{standing_b:?}");
    assert!(standing_b.entries.is_empty());
    assert_eq!(standing_b.no_answer.len(), 1);
    // Of another kind: a never's file standing in a grant's place.
    seal_in(&a, &[(NEVER, "npm publish")]);
    let never = std::fs::read_to_string(entry_file(&a, NEVER, "npm publish")).unwrap();
    std::fs::write(entry_file(&a, GRANT, "run npm"), never).unwrap();
    let standing_a = standing(&a);
    assert!(holds(&standing_a), "{standing_a:?}");
    assert!(!standing_a.entries.iter().any(|e| e.kind == GRANT));
    assert_eq!(standing_a.no_answer.len(), 1);
}

#[test]
fn a_never_or_accepted_entry_that_fails_its_seal_is_the_deny_state() {
    for (kind, text) in [(NEVER, "rm -rf"), (ACCEPTED, "guards.git=enforce")] {
        let m = machine();
        let root = m.plane("p");
        seal_in(&root, &[(kind, text)]);
        let file = entry_file(&root, kind, text);
        let held = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, held.replace(text, "something else")).unwrap();
        let standing = standing(&root);
        assert!(
            denied_by(
                &standing,
                |c| matches!(c, Deny::EntryFailsSeal { kind: k, .. } if k == kind)
            ),
            "{kind}: {standing:?}"
        );
        assert!(
            standing.entries.is_empty(),
            "nothing counts in the deny state"
        );
    }
}

#[test]
fn a_deleted_nevers_file_with_its_marker_sealed_is_reported_not_read_as_empty() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    // The marker is sealed in the manifest beside the never.
    assert!(entry_file(&root, NEVER_MARKER, "present").exists());
    std::fs::remove_file(entry_file(&root, NEVER, "rm -rf")).unwrap();
    let deleted = standing(&root);
    assert!(
        denied_by(
            &deleted,
            |c| matches!(c, Deny::EntryMissing { kind, .. } if kind == NEVER)
        ),
        "{deleted:?}"
    );
    // The marker alone does not stand in for the list either.
    std::fs::remove_dir_all(store_of(&root).join(NEVER)).unwrap();
    assert!(standing(&root).is_deny());
}

#[test]
fn with_the_anchor_present_an_older_store_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "one")]);
    let before = m.dir.path().join("copy");
    copy_dir(&store_of(&root), &before);
    commit(&root, |d| d.add(NEVER, "two")).unwrap();
    commit(&root, |d| d.add(FLOOR, "v3")).unwrap();
    // The whole folder put back from before.
    std::fs::remove_dir_all(store_of(&root)).unwrap();
    copy_dir(&before, &store_of(&root));
    let standing = standing(&root);
    let lost = standing.causes().iter().find_map(|c| match c {
        Deny::ManifestOlder(lost) => Some(*lost),
        _ => None,
    });
    // The counts of what was lost: two writes, one never, one floor version.
    assert_eq!(
        lost,
        Some(Lost {
            writes: 2,
            nevers: 1,
            floors: 1
        }),
        "{standing:?}"
    );
}

#[test]
fn with_the_anchor_present_a_missing_manifest_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "guards.git=enforce"), (NEVER, "one")]);
    std::fs::remove_file(store_of(&root).join("manifest.json")).unwrap();
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(
            c,
            Deny::ManifestMissing(Lost { nevers: 1, .. })
        )),
        "{standing:?}"
    );
}

#[test]
fn a_swapped_git_at_an_anchored_path_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "guards.git=enforce")]);
    assert!(holds(&standing(&root)));
    swap_git(&root);
    assert_ne!(
        clone_key(&root),
        plane_place(&root).unwrap().bind.clone() + "x"
    );
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::CloneKeySwapped)),
        "{standing:?}"
    );
}

#[test]
fn a_moved_plane_whose_id_is_anchored_at_another_path_is_the_deny_state() {
    let m = machine();
    let (old, new) = (m.plane("old"), m.plane("new"));
    seal_in(&old, &[(ACCEPTED, "guards.git=enforce")]);
    // The plane moved with its store: the new place opens with the old one's manifest.
    register(&new).unwrap();
    copy_dir(&store_of(&old), &store_of(&new));
    let standing = standing(&new);
    assert!(
        denied_by(
            &standing,
            |c| matches!(c, Deny::PlaneMoved { anchored_at, .. } if *anchored_at == old)
        ),
        "{standing:?}"
    );
    assert!(standing.entries.is_empty());
}

#[cfg(unix)]
#[test]
fn a_moved_plane_with_its_old_path_linked_to_it_is_the_deny_state_never_a_fresh_install() {
    let m = machine();
    let (old, new) = (m.plane("old"), m.plane("new"));
    seal_in(&old, &[(ACCEPTED, "guards.git=enforce")]);
    // Moved away, a link left at the old path.
    std::fs::remove_dir_all(&old).unwrap();
    std::os::unix::fs::symlink(&new, &old).unwrap();
    let through = standing(&old);
    assert!(
        through.is_deny() && through.state != State::Fresh,
        "{through:?}"
    );
}

#[test]
fn a_sealed_store_with_no_anchor_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "guards.git=enforce")]);
    // The keyring forgets every anchor (a restored folder on a machine with a fresh keyring).
    let account = plane_place(&root).unwrap().account;
    seal::item_write(&account, "").then_some(()).unwrap();
    seal::forget_kept_for_this_test();
    register(&root).unwrap();
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::StoreWithoutAnchor)),
        "{standing:?}"
    );
}

#[test]
fn an_older_everywhere_entry_puts_every_plane_in_the_deny_state() {
    let m = machine();
    let (a, b) = (m.plane("a"), m.plane("b"));
    seal_in(&a, &[(ACCEPTED, "one")]);
    seal_in(&b, &[(ACCEPTED, "two")]);
    commit_everywhere(|d| d.add(NEVER, "curl | sh")).unwrap();
    let before = m.dir.path().join("everywhere");
    copy_dir(&everywhere_place().unwrap().folder, &before);
    commit_everywhere(|d| d.add(NEVER, "rm -rf /")).unwrap();
    assert!(holds(&standing(&a)) && holds(&standing(&b)));
    std::fs::remove_dir_all(everywhere_place().unwrap().folder).unwrap();
    copy_dir(&before, &everywhere_place().unwrap().folder);
    for root in [&a, &b] {
        let standing = standing(root);
        assert!(
            denied_by(&standing, |c| matches!(
                c,
                Deny::EverywhereOlder(Lost { nevers: 1, .. })
            )),
            "{standing:?}"
        );
    }
}

#[test]
fn a_sealed_file_the_manifest_does_not_list_is_ignored_with_a_notice() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "run npm")]);
    let stray = entry_file(&root, GRANT, "run cargo");
    // A real sealed file, made for another grant the manifest never listed.
    let mac = seal::sealed(&entry_kind(GRANT), &clone_key(&root), "run cargo").unwrap();
    std::fs::write(
        &stray,
        serde_json::to_string(&EntryFile {
            kind: GRANT.to_owned(),
            text: "run cargo".to_owned(),
            mac,
        })
        .unwrap(),
    )
    .unwrap();
    let standing = standing(&root);
    assert!(holds(&standing));
    assert_eq!(standing.entries, vec![Entry::new(GRANT, "run npm")]);
    assert_eq!(standing.notices.len(), 1, "{standing:?}");
    assert!(standing.notices[0].contains("does not list it"));
}

#[test]
fn a_used_once_grant_put_back_counts_for_nothing() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "once: git push")]);
    let file = entry_file(&root, GRANT, "once: git push");
    let kept = std::fs::read_to_string(&file).unwrap();
    // Its use: the grant is taken out of the store.
    commit(&root, |d| d.remove(GRANT, "once: git push")).unwrap();
    assert!(!file.exists());
    std::fs::write(&file, kept).unwrap();
    let standing = standing(&root);
    assert!(holds(&standing));
    assert!(standing.entries.is_empty(), "{standing:?}");
    assert_eq!(standing.notices.len(), 1);
}

#[test]
fn an_everywhere_never_survives_a_swapped_git() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "guards.git=enforce")]);
    commit_everywhere(|d| d.add(NEVER, "curl | sh")).unwrap();
    swap_git(&root);
    let standing = standing(&root);
    assert!(standing.is_deny(), "the plane is denied");
    assert_eq!(
        standing.everywhere,
        vec![
            Entry::new(NEVER, "curl | sh"),
            Entry::new(NEVER_MARKER, "present")
        ]
    );
}

#[test]
fn the_anchor_item_is_refused_by_the_one_item_writer() {
    for account in [
        REGISTRY_ACCOUNT.to_owned(),
        EVERYWHERE_ACCOUNT.to_owned(),
        plane_account("0123456789abcdef0123456789abcdef"),
    ] {
        let item = serde_json::json!({
            "service": seal::SERVICE, "account": account, "value": "{}",
        });
        assert!(
            super::super::keyhold::asked(item.to_string().as_bytes()).is_err(),
            "{account}"
        );
    }
    // Nor is the service one of purlis's own.
    assert!(
        super::super::keyring::OWN_PREFIXES
            .iter()
            .all(|prefix| !seal::SERVICE.starts_with(prefix))
    );
}

#[test]
fn the_counter_rises_on_a_grants_removal_and_a_once_grants_use() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "run npm"), (GRANT, "once: git push")]);
    let first = counter_of(&root);
    commit(&root, |d| d.remove(GRANT, "run npm")).unwrap();
    let removed = counter_of(&root);
    commit(&root, |d| d.remove(GRANT, "once: git push")).unwrap();
    let used = counter_of(&root);
    assert!(
        first < removed && removed < used,
        "{first} {removed} {used}"
    );
    // The keyring's counter keeps pace with the manifest's.
    assert_eq!(anchor_counter(&root), used);
}

fn counter_of(root: &Path) -> u64 {
    read_manifest(&plane_place(root).unwrap())
        .ok()
        .flatten()
        .expect("a manifest")
        .counter
}

fn anchor_counter(root: &Path) -> u64 {
    let place = plane_place(root).unwrap();
    match read_anchor(&place.account, &place.anchor_bind) {
        Read::Value(anchor) => anchor.counter,
        _ => panic!("an anchor"),
    }
}

#[test]
fn a_restored_state_folder_does_not_replay_past_the_anchored_counter() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "once: git push")]);
    let before = m.dir.path().join("before");
    copy_dir(&store_of(&root), &before);
    commit(&root, |d| d.remove(GRANT, "once: git push")).unwrap();
    std::fs::remove_dir_all(store_of(&root)).unwrap();
    copy_dir(&before, &store_of(&root));
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(
            c,
            Deny::ManifestOlder(Lost { writes: 1, .. })
        )),
        "{standing:?}"
    );
    assert!(
        standing.entries.is_empty(),
        "the used grant does not replay"
    );
}

#[test]
fn the_counter_is_written_to_the_keyring_item_before_the_manifest() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    // The manifest cannot be written (a folder is where its file goes): the write fails after
    // the anchor, which already counts it.
    let manifest = store_of(&root).join("manifest.json");
    std::fs::remove_file(&manifest).unwrap();
    std::fs::create_dir(&manifest).unwrap();
    let written = commit_over(&root);
    assert!(written.is_err());
    assert_eq!(anchor_counter(&root), 2, "the anchor was written first");
}

/// A commit of one more answer, on a plane whose manifest was already taken away.
fn commit_over(root: &Path) -> io::Result<u64> {
    let place = plane_place(root).unwrap();
    let id = match register(root)? {
        Registration::Registered(id) => id,
        Registration::NotKept => unreachable!(),
    };
    let mut draft = Draft::default();
    draft.add(ACCEPTED, "two");
    write(
        &place,
        Some((root, &id)),
        draft,
        Some(Anchor {
            counter: 1,
            nevers: 0,
            floors: 0,
        }),
        1,
    )
}

#[test]
fn with_no_anchor_and_no_store_the_shipped_default_applies_as_a_fresh_install() {
    let m = machine();
    let root = m.plane("p");
    assert_eq!(standing(&root).state, State::Fresh);
    // Registered at its first open, its counter still nought: nothing was sealed, so nothing
    // can be lost.
    register(&root).unwrap();
    let standing = standing(&root);
    assert_eq!(standing.state, State::Fresh);
    assert!(standing.entries.is_empty() && standing.notices.is_empty());
}

#[test]
fn deleting_the_accepted_entry_never_reads_as_the_shipped_default() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "guards.git=enforce")]);
    std::fs::remove_file(entry_file(&root, ACCEPTED, "guards.git=enforce")).unwrap();
    let standing = standing(&root);
    assert_ne!(standing.state, State::Fresh);
    assert!(
        denied_by(
            &standing,
            |c| matches!(c, Deny::EntryMissing { kind, .. } if kind == ACCEPTED)
        ),
        "{standing:?}"
    );
}

#[test]
fn without_a_keyring_the_shipped_default_holds_with_a_notice() {
    let m = machine();
    let root = m.plane("p");
    seal::set_no_keyring_for_this_test(true);
    let none = standing(&root);
    assert_eq!(none.state, State::NoKeyring);
    assert!(
        none.notices.iter().any(|n| n == NO_KEYRING_NOTICE),
        "{none:?}"
    );
    // A keyring that did not start where a store is kept is one that failed.
    seal::set_no_keyring_for_this_test(false);
    seal_in(&root, &[(ACCEPTED, "one")]);
    seal::set_no_keyring_for_this_test(true);
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::KeyringDidNotStart
    )));
}

#[test]
fn a_windowless_host_accepts_nothing_until_a_window_is_open() {
    let m = machine();
    let root = m.plane("p");
    register(&root).unwrap();
    set_window_for_this_test(false);
    let refused = commit(&root, |d| d.add(ACCEPTED, "one"));
    assert_eq!(refused.unwrap_err().to_string(), NO_WINDOW);
    assert!(confirm_device().is_err());
    assert_eq!(standing(&root).state, State::Fresh, "nothing was written");
    set_window_for_this_test(true);
    assert!(commit(&root, |d| d.add(ACCEPTED, "one")).is_ok());
}

#[test]
fn a_plane_is_registered_at_first_open_before_anything_is_sealed() {
    let m = machine();
    let root = m.plane("p");
    assert!(plane_containing(&root).is_none());
    let Registration::Registered(id) = register(&root).unwrap() else {
        panic!("registered");
    };
    let plane =
        plane_containing(&root.join("src/deep")).expect("a process inside is never plane-less");
    assert_eq!(plane.id, id);
    assert_eq!(plane.real, root);
    assert!(
        !plane.sealed && plane.clone.is_empty(),
        "nothing is sealed yet"
    );
    // Again, it is the same plane.
    assert_eq!(register(&root).unwrap(), Registration::Registered(id));
}

#[test]
fn context_machine_is_the_device_id_sealed_at_first_open() {
    let m = machine();
    let root = m.plane("p");
    register(&root).unwrap();
    let Device::Unconfirmed(id) = device() else {
        panic!("sealed, unconfirmed");
    };
    let home = crate::sandbox::local::config_home().unwrap();
    assert_eq!(id, crate::machine::device_id(&home).unwrap());
}

#[test]
fn an_unconfirmed_device_id_leaves_context_machine_unset() {
    let m = machine();
    register(&m.plane("p")).unwrap();
    assert_eq!(context_machine(), None);
}

#[test]
fn the_person_confirming_the_id_in_the_window_sets_context_machine() {
    let m = machine();
    register(&m.plane("p")).unwrap();
    confirm_device().unwrap();
    let Device::Confirmed(id) = device() else {
        panic!("confirmed");
    };
    assert_eq!(context_machine(), Some(id));
}

#[test]
fn editing_machine_jsons_device_id_does_not_change_context_machine() {
    let m = machine();
    register(&m.plane("p")).unwrap();
    confirm_device().unwrap();
    let before = context_machine().unwrap();
    let home = crate::sandbox::local::config_home().unwrap();
    let file = crate::machine::file(&home);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains(&before),
        "machine.json holds the id it minted"
    );
    std::fs::write(&file, text.replace(&before, "01ARZ3NDEKTSV4RRFFQ69G5FAV")).unwrap();
    assert_eq!(context_machine(), Some(before));
}

#[test]
fn without_a_keyring_context_machine_is_unset_and_doctor_declares_it() {
    let m = machine();
    seal::set_no_keyring_for_this_test(true);
    assert_eq!(register(&m.plane("p")).unwrap(), Registration::NotKept);
    assert_eq!(context_machine(), None);
    assert!(doctor_lines().iter().any(|line| line == DEVICE_UNSET));
}

#[test]
fn an_unreadable_registry_leaves_context_machine_unset() {
    let m = machine();
    register(&m.plane("p")).unwrap();
    confirm_device().unwrap();
    assert!(context_machine().is_some());
    // The registry edited by hand: its seal no longer holds.
    let Item::Value(text) = seal::item_read(REGISTRY_ACCOUNT) else {
        panic!("a registry");
    };
    assert!(text.contains("\"confirmed\":true"));
    assert!(seal::item_write(
        REGISTRY_ACCOUNT,
        &text.replace("\"confirmed\":true", "\"confirmed\":false")
    ));
    assert_eq!(context_machine(), None);
    assert!(!registry_reads());
    assert!(doctor_lines().iter().any(|line| line == DEVICE_UNSET));
}

#[test]
fn the_everywhere_anchor_counts_its_nevers() {
    let m = machine();
    let _ = m.plane("p");
    commit_everywhere(|d| {
        d.add(NEVER, "curl | sh");
        d.add(NEVER, "rm -rf /");
    })
    .unwrap();
    let place = everywhere_place().unwrap();
    let Read::Value(anchor) = read_anchor(&place.account, &place.anchor_bind) else {
        panic!("an anchor");
    };
    assert_eq!(anchor.nevers, 2);
    // A never taken from the store is a count the anchor names.
    commit_everywhere(|d| d.remove(NEVER, "rm -rf /")).unwrap();
    let Read::Value(after) = read_anchor(&place.account, &place.anchor_bind) else {
        panic!("an anchor");
    };
    assert_eq!(after.nevers, 1);
}

#[test]
fn a_locked_seal_key_on_an_anchored_plane_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    seal::set_locked_for_this_test(true);
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::SealKeyLocked)),
        "{standing:?}"
    );
}

#[test]
fn a_process_that_is_not_the_app_verifies_nothing() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    seal::set_for_this_test(None);
    let standing = standing(&root);
    assert_eq!(standing.state, State::Unverifiable);
    assert!(standing.entries.is_empty());
}

#[test]
fn a_commit_past_the_floor_cap_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(FLOOR, "v1")]);
    let standing = standing(&root).with_commit_past_the_floor_cap(true);
    assert!(denied_by(&standing, |c| matches!(c, Deny::PastTheFloorCap)));
    assert!(standing.entries.is_empty());
}

#[test]
fn a_registry_that_does_not_read_is_the_deny_state_on_a_plane_with_a_store() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    seal::item_write(REGISTRY_ACCOUNT, "not json");
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::RegistryUnreadable)),
        "{standing:?}"
    );
}
