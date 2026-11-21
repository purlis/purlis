// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sandbox::local::OwnConfigHome;

/// A machine of the test's own: its config home, a keyring that knows nothing, a key.
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

/// The person's press in the window.
#[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
fn press() -> Press {
    Press::of_the_persons_press()
}

/// Registers `root` and adds `add`, as the person pressing in the window.
fn seal_in(root: &Path, add: &[(&str, &str)]) {
    register(root).expect("registered");
    commit(root, |draft| {
        for (kind, text) in add {
            draft.add(&press(), kind, text);
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

/// The person confirming the id the window shows.
fn confirm_shown_device() {
    let Device::Unconfirmed(id) = device() else {
        panic!("an unconfirmed id to show");
    };
    confirm_device(&press(), &id).unwrap();
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
    commit(&root, |d| d.add(&press(), NEVER, "two")).unwrap();
    commit(&root, |d| d.add(&press(), FLOOR, "v3")).unwrap();
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
    let (clone_before, anchor_before) = (clone_key(&root), anchor_key(&root));
    swap_git(&root);
    // The swap moves the clone key and never the anchor's: the anchor is what still speaks.
    assert_ne!(clone_key(&root), clone_before);
    assert_eq!(anchor_key(&root), anchor_before);
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
    // The old path's anchor is still asked for, and the clone key behind the link is another
    // one with no folder: the swap's cause, never a fresh install.
    let through = standing(&old);
    assert!(
        denied_by(&through, |c| matches!(c, Deny::CloneKeySwapped)),
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
    // A registry is not made over a store: the person's press replaces it.
    assert!(register(&root).is_err());
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
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
    let before = m.dir.path().join("everywhere");
    copy_dir(&everywhere_place().unwrap().folder, &before);
    commit_everywhere(|d| d.add(&press(), NEVER, "rm -rf /")).unwrap();
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
    commit(&root, |d| d.remove_narrowing(GRANT, "once: git push")).unwrap();
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
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
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
    commit(&root, |d| d.remove_narrowing(GRANT, "run npm")).unwrap();
    let removed = counter_of(&root);
    commit(&root, |d| d.remove_narrowing(GRANT, "once: git push")).unwrap();
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
    commit(&root, |d| d.remove_narrowing(GRANT, "once: git push")).unwrap();
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
    draft.add(&press(), ACCEPTED, "two");
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
fn a_windowless_host_has_no_press_so_it_can_only_narrow() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "once: git push"), (NEVER, "rm -rf")]);
    let before = counter_of(&root);
    // The one thing a host with no press can do: take out what only narrows. Taking out a
    // never would widen, and without a press it is refused with nothing changed.
    let widening = commit(&root, |d| d.remove_narrowing(NEVER, "rm -rf"));
    assert_eq!(widening.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert_eq!(counter_of(&root), before, "nothing was written");
    assert!(
        standing(&root)
            .entries
            .contains(&Entry::new(NEVER, "rm -rf"))
    );
}

#[test]
fn a_once_grants_use_in_a_windowless_host_is_recorded_and_the_grant_is_gone() {
    let m = machine();
    let root = m.plane("p");
    seal_in(
        &root,
        &[
            (GRANT, "once: git push"),
            (PASS, "TOKEN"),
            (ACCEPTANCE, "p"),
        ],
    );
    let before = counter_of(&root);
    // No press anywhere in this commit: the use, a pass's removal and a withdrawal narrow.
    commit(&root, |d| d.remove_narrowing(GRANT, "once: git push")).expect("the use is recorded");
    commit(&root, |d| d.remove_narrowing(PASS, "TOKEN")).expect("a pass's removal");
    commit(&root, |d| d.remove_narrowing(ACCEPTANCE, "p")).expect("a withdrawal");
    assert_eq!(counter_of(&root), before + 3);
    let after = standing(&root);
    assert!(holds(&after), "{after:?}");
    assert!(after.entries.is_empty(), "the grant is gone: {after:?}");
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
    confirm_shown_device();
    let Device::Confirmed(id) = device() else {
        panic!("confirmed");
    };
    assert_eq!(context_machine(), Some(id));
}

#[test]
fn editing_machine_jsons_device_id_does_not_change_context_machine() {
    let m = machine();
    register(&m.plane("p")).unwrap();
    confirm_shown_device();
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
    confirm_shown_device();
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
        d.add(&press(), NEVER, "curl | sh");
        d.add(&press(), NEVER, "rm -rf /");
    })
    .unwrap();
    let place = everywhere_place().unwrap();
    let Read::Value(anchor) = read_anchor(&place.account, &place.anchor_bind) else {
        panic!("an anchor");
    };
    assert_eq!(anchor.nevers, 2);
    // A never taken from the store is a count the anchor names.
    commit_everywhere(|d| d.remove(&press(), NEVER, "rm -rf /")).unwrap();
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

// ---- round 2 (#7234): fail closed everywhere ---------------------------------------------------

/// The plane's keyring account and anchor, as the keyring holds them.
fn anchor_of(root: &Path) -> Anchor {
    let place = plane_place(root).unwrap();
    match read_anchor(&place.account, &place.anchor_bind) {
        Read::Value(anchor) => anchor,
        _ => panic!("an anchor"),
    }
}

#[test]
fn a_swapped_git_where_the_keyring_did_not_start_is_the_deny_state_never_the_shipped_default() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf"), (FLOOR, "v2")]);
    // Linux: the secret service did not answer as purlis started, and `.git` was swapped, so
    // the new clone key has no folder at all.
    swap_git(&root);
    seal::set_no_keyring_for_this_test(true);
    assert!(!store_exists(&plane_place(&root).unwrap()), "no folder");
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::KeyringDidNotStart)),
        "{standing:?}"
    );
}

#[test]
fn a_store_folder_taken_away_where_the_keyring_did_not_start_is_still_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    std::fs::remove_dir_all(store_of(&root)).unwrap();
    seal::set_no_keyring_for_this_test(true);
    // Only the path's marker says a store was kept here.
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::KeyringDidNotStart
    )));
    // A plane nothing was ever kept for is still the shipped default.
    assert_eq!(standing(&m.plane("other")).state, State::NoKeyring);
}

#[test]
fn a_commit_started_inside_a_commit_is_refused_and_never_deletes_its_entries() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    // The second commit starts after the first has read the store and before it writes: with no
    // lock it reads the same counter, writes one past it, and the first's write then takes its
    // entry out as a file its manifest does not list.
    let mut second = None;
    commit(&root, |draft| {
        draft.add(&press(), NEVER, "first");
        second = Some(commit(&root, |inner| inner.add(&press(), NEVER, "second")));
    })
    .expect("the first commit");
    let second = second.expect("the second ran");
    assert!(second.is_err(), "the second commit took turns: {second:?}");
    let standing = standing(&root);
    assert!(holds(&standing), "{standing:?}");
    assert!(standing.entries.contains(&Entry::new(NEVER, "first")));
    assert!(!standing.entries.contains(&Entry::new(NEVER, "second")));
    assert_eq!(counter_of(&root), anchor_counter(&root));
    // And it goes through once the first is done.
    commit(&root, |d| d.add(&press(), NEVER, "second")).expect("its turn");
    let after = standing_entries(&root);
    assert!(
        after.contains(&Entry::new(NEVER, "first")) && after.contains(&Entry::new(NEVER, "second"))
    );
}

fn standing_entries(root: &Path) -> Vec<Entry> {
    standing(root).entries
}

#[cfg(unix)]
#[test]
fn a_commit_waits_for_another_process_holding_the_stores_file_lock() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    let before = counter_of(&root);
    // Another process (or window) is committing: its flock on the lock file.
    let other = std::fs::File::open(lock_file().unwrap()).unwrap();
    rustix::fs::flock(&other, rustix::fs::FlockOperation::LockExclusive).unwrap();
    assert!(commit(&root, |d| d.add(&press(), NEVER, "two")).is_err());
    assert!(recommit(&root, &press(), &[]).is_err());
    assert!(commit_everywhere(|d| d.add(&press(), NEVER, "three")).is_err());
    assert_eq!(counter_of(&root), before, "nothing was changed");
    drop(other);
    assert!(commit(&root, |d| d.add(&press(), NEVER, "two")).is_ok());
}

#[test]
fn recommit_mends_a_deny_and_starts_past_every_counter_seen() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "one")]);
    let backup = m.dir.path().join("backup");
    copy_dir(&store_of(&root), &backup);
    commit(&root, |d| d.add(&press(), NEVER, "two")).unwrap();
    commit(&root, |d| d.add(&press(), FLOOR, "v3")).unwrap();
    let kept = standing(&root).entries;
    // The anchor does not hold its seal (edited, or made by another program): its counter and
    // the manifest's are both still on disk.
    let account = plane_place(&root).unwrap().account;
    assert!(seal::item_write(&account, "not json"));
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::AnchorGone
    )));
    let kept: Vec<Entry> = kept
        .into_iter()
        .filter(|e| e.kind != NEVER_MARKER)
        .collect();
    let counter = recommit(&root, &press(), &kept).expect("the person's press mends it");
    assert_eq!(counter, 4, "past the manifest's 3, never back to 1");
    let mended = standing(&root);
    assert!(holds(&mended), "{mended:?}");
    assert_eq!(anchor_counter(&root), 4);
    // The earliest backup put back is older than the anchor, as it was before the break.
    std::fs::remove_dir_all(store_of(&root)).unwrap();
    copy_dir(&backup, &store_of(&root));
    assert!(
        denied_by(&standing(&root), |c| matches!(c, Deny::ManifestOlder(_))),
        "{:?}",
        standing(&root)
    );
}

#[test]
fn recommit_reads_the_anchors_counter_where_its_seal_fails_and_the_manifest_is_gone() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    commit(&root, |d| d.add(&press(), ACCEPTED, "two")).unwrap();
    commit(&root, |d| d.add(&press(), ACCEPTED, "three")).unwrap();
    // The anchor's seal edited (its counter still reads 3) and the manifest taken away.
    let account = plane_place(&root).unwrap().account;
    let Item::Value(text) = seal::item_read(&account) else {
        panic!("an anchor");
    };
    let mut item: serde_json::Value = serde_json::from_str(&text).unwrap();
    item["mac"] = "00".into();
    assert!(seal::item_write(&account, &item.to_string()));
    std::fs::remove_file(store_of(&root).join("manifest.json")).unwrap();
    assert!(standing(&root).is_deny());
    assert_eq!(recommit(&root, &press(), &[]).unwrap(), 4);
}

#[test]
fn a_commit_cut_short_after_the_anchor_is_mended_by_recommit() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    // The write cut short: the anchor says 2, and the manifest never got written.
    let manifest = store_of(&root).join("manifest.json");
    std::fs::remove_file(&manifest).unwrap();
    std::fs::create_dir(&manifest).unwrap();
    assert!(commit_over(&root).is_err());
    std::fs::remove_dir(&manifest).unwrap();
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::ManifestMissing(_)
    )));
    assert!(commit(&root, |d| d.add(&press(), ACCEPTED, "three")).is_err());
    let counter = recommit(&root, &press(), &[Entry::new(ACCEPTED, "one")]).unwrap();
    assert_eq!(counter, 3);
    let mended = standing(&root);
    assert!(holds(&mended), "{mended:?}");
    assert_eq!(mended.entries, vec![Entry::new(ACCEPTED, "one")]);
}

#[test]
fn an_anchor_older_than_the_manifest_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    commit(&root, |d| d.add(&press(), ACCEPTED, "two")).unwrap();
    // The anchor taken away and made again at an older counter (sealed, so it reads).
    let place = plane_place(&root).unwrap();
    write_anchor(
        &place.account,
        &place.anchor_bind,
        Anchor {
            counter: 1,
            nevers: 0,
            floors: 0,
        },
    )
    .unwrap();
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::AnchorOlder)),
        "{standing:?}"
    );
}

#[test]
fn counts_the_anchor_disagrees_with_are_the_deny_state() {
    for (nevers, floors) in [(5, 1), (1, 7)] {
        let m = machine();
        let root = m.plane("p");
        seal_in(&root, &[(NEVER, "one"), (FLOOR, "v1")]);
        let place = plane_place(&root).unwrap();
        let counter = anchor_of(&root).counter;
        write_anchor(
            &place.account,
            &place.anchor_bind,
            Anchor {
                counter,
                nevers,
                floors,
            },
        )
        .unwrap();
        let standing = standing(&root);
        assert!(
            denied_by(&standing, |c| matches!(c, Deny::CountsDisagree)),
            "{nevers}/{floors}: {standing:?}"
        );
    }
}

#[test]
fn an_anchor_that_does_not_hold_its_seal_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    let account = plane_place(&root).unwrap().account;
    let Item::Value(text) = seal::item_read(&account) else {
        panic!("an anchor");
    };
    // Its counter edited down by hand: the seal no longer holds.
    assert!(seal::item_write(
        &account,
        &text.replace("\"counter\":1", "\"counter\":0")
    ));
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::AnchorGone)),
        "{standing:?}"
    );
}

#[test]
fn a_missing_registry_with_an_anchor_or_a_store_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    seal::item_forget_for_this_test(REGISTRY_ACCOUNT);
    assert!(matches!(read_registry(), Read::Missing));
    let standing = standing(&root);
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::RegistryUnreadable)),
        "{standing:?}"
    );
    // Nothing kept anywhere is still a fresh install.
    assert_eq!(super::standing(&m.plane("fresh")).state, State::Fresh);
}

#[test]
fn a_missing_registry_with_only_the_everywhere_entry_kept_is_the_deny_state() {
    let m = machine();
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
    seal::item_forget_for_this_test(REGISTRY_ACCOUNT);
    let standing = standing(&m.plane("p"));
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::RegistryUnreadable)),
        "{standing:?}"
    );
}

#[test]
fn an_everywhere_entry_whose_anchor_and_folder_are_both_gone_is_the_deny_state() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
    let place = everywhere_place().unwrap();
    std::fs::remove_dir_all(&place.folder).unwrap();
    seal::item_forget_for_this_test(&place.account);
    let everywhere = everywhere_standing();
    assert!(everywhere.is_deny(), "{everywhere:?}");
    let plane = standing(&root);
    assert!(
        denied_by(&plane, |c| matches!(c, Deny::AnchorGone)),
        "{plane:?}"
    );
    assert!(commit_everywhere(|d| d.add(&press(), NEVER, "again")).is_err());
}

#[test]
fn a_plane_moved_with_no_link_left_is_the_deny_state_never_a_fresh_install() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let new = m.dir.path().join("new");
    std::fs::rename(&old, &new).unwrap();
    let new = std::fs::canonicalize(&new).unwrap();
    // The app opens the new path: it is registered, and nothing is kept at it.
    register(&new).unwrap();
    let standing = standing(&new);
    assert!(
        denied_by(
            &standing,
            |c| matches!(c, Deny::PlaneMoved { anchored_at, .. } if *anchored_at == old)
        ),
        "{standing:?}"
    );
    // A plane nothing was sealed for, or another folder, is not taken for it.
    assert_eq!(super::standing(&m.plane("other")).state, State::Fresh);
}

#[test]
fn a_kind_that_names_a_file_of_the_stores_folder_is_refused() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    let before = counter_of(&root);
    for kind in ["manifest", "../x", "grant/../../x", ""] {
        let refused = commit(&root, |d| d.add(&press(), kind, "text"));
        assert_eq!(
            refused.unwrap_err().kind(),
            io::ErrorKind::InvalidInput,
            "{kind}"
        );
        assert!(
            recommit(&root, &press(), &[Entry::new(kind, "text")]).is_err(),
            "{kind}"
        );
        assert!(
            commit_everywhere(|d| d.add(&press(), kind, "text")).is_err(),
            "{kind}"
        );
    }
    assert_eq!(counter_of(&root), before, "nothing was changed");
    assert!(!m.dir.path().join("x").exists());
    assert!(!store_of(&root).join("x").exists());
    assert!(holds(&standing(&root)));
}

#[test]
fn a_keyring_that_fails_the_first_commits_anchor_changes_nothing() {
    let m = machine();
    let root = m.plane("p");
    register(&root).unwrap();
    seal::set_item_write_fails_for_this_test(true);
    let refused = commit(&root, |d| d.add(&press(), NEVER, "one"));
    seal::set_item_write_fails_for_this_test(false);
    assert!(refused.is_err());
    assert!(
        !store_exists(&plane_place(&root).unwrap()),
        "no file was written before the anchor"
    );
    // Not the deny state: nothing was kept, so the next press simply works.
    assert_eq!(standing(&root).state, State::Fresh);
    assert!(commit(&root, |d| d.add(&press(), NEVER, "one")).is_ok());
}

#[test]
fn the_person_confirms_the_id_the_window_showed_and_no_other() {
    let m = machine();
    register(&m.plane("p")).unwrap();
    let Device::Unconfirmed(shown) = device() else {
        panic!("an unconfirmed id");
    };
    assert!(confirm_device(&press(), "01ARZ3NDEKTSV4RRFFQ69G5FAV").is_err());
    assert_eq!(context_machine(), None);
    confirm_device(&press(), &shown).unwrap();
    assert_eq!(context_machine(), Some(shown));
}

#[test]
fn the_everywhere_entry_is_mended_by_the_persons_press_and_never_by_a_commit() {
    let m = machine();
    let _ = m.plane("p");
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
    commit_everywhere(|d| d.add(&press(), NEVER, "rm -rf /")).unwrap();
    let place = everywhere_place().unwrap();
    std::fs::remove_file(place.folder.join("manifest.json")).unwrap();
    assert!(everywhere_standing().is_deny());
    // A chat's commit changes nothing under the deny.
    assert!(commit_everywhere(|d| d.remove(&press(), NEVER, "curl | sh")).is_err());
    assert!(everywhere_standing().is_deny());
    // The person's press, past every counter seen.
    let counter = recommit_everywhere(&press(), &[Entry::new(NEVER, "curl | sh")]).unwrap();
    assert_eq!(counter, 3);
    let mended = everywhere_standing();
    assert!(holds(&mended), "{mended:?}");
    assert!(mended.entries.contains(&Entry::new(NEVER, "curl | sh")));
    assert!(!mended.entries.contains(&Entry::new(NEVER, "rm -rf /")));
}

#[test]
fn a_registry_that_does_not_hold_its_seal_is_replaced_only_at_the_persons_press() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
    seal::item_write(REGISTRY_ACCOUNT, "not json");
    assert!(commit(&root, |d| d.add(&press(), ACCEPTED, "two")).is_err());
    assert!(commit_everywhere(|d| d.add(&press(), NEVER, "x")).is_err());
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::RegistryUnreadable
    )));
    // Nothing but a press replaces it: a commit, a register and a read leave it as it is.
    assert!(matches!(register(&root).unwrap(), Registration::NotKept));
    assert!(matches!(read_registry(), Read::Broken));
    recommit(&root, &press(), &[Entry::new(ACCEPTED, "one")]).expect("the press mends it");
    assert!(matches!(read_registry(), Read::Value(_)));
    let mended = standing(&root);
    assert!(holds(&mended), "{mended:?}");
    // The device id is sealed again, unconfirmed: the person confirms it again.
    assert!(matches!(device(), Device::Unconfirmed(_)));
    // The Everywhere entry is still there, and its flag kept.
    assert!(holds(&everywhere_standing()));
    let Read::Value(registry) = read_registry() else {
        panic!("a registry");
    };
    assert!(registry.everywhere_sealed);
}

// ---- round 3 (#7234): the counter's ceiling, Once grants, the press, the registry ----------------

/// `manifest.json`'s counter edited by hand to `counter`, its seal left as it was.
fn edit_manifest_counter(root: &Path, counter: u64) {
    let file = store_of(root).join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    manifest["counter"] = counter.into();
    std::fs::write(&file, manifest.to_string()).unwrap();
}

#[test]
fn a_manifest_counter_edited_to_the_top_does_not_freeze_the_counter_where_the_anchor_reads() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    commit(&root, |d| d.add(&press(), NEVER, "two")).unwrap();
    let anchored = anchor_counter(&root);
    edit_manifest_counter(&root, u64::MAX);
    assert!(standing(&root).is_deny(), "the edit fails its seal");
    let kept = vec![Entry::new(NEVER, "rm -rf")];
    let counter = recommit(&root, &press(), &kept).expect("the person's press mends it");
    assert_eq!(counter, anchored + 1, "past the anchor's, never at the top");
    // And it goes on rising: an older copy put back after a later commit has a lower counter.
    let copy = m.dir.path().join("copy");
    copy_dir(&store_of(&root), &copy);
    let next = commit(&root, |d| d.add(&press(), NEVER, "three")).unwrap();
    assert_eq!(next, counter + 1);
    std::fs::remove_dir_all(store_of(&root)).unwrap();
    copy_dir(&copy, &store_of(&root));
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::ManifestOlder(_)
    )));
}

#[test]
fn an_unsealed_counter_beyond_reach_is_refused_where_the_anchor_does_not_hold_its_seal() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    let account = plane_place(&root).unwrap().account;
    assert!(seal::item_write(&account, "not json"));
    edit_manifest_counter(&root, u64::MAX);
    let manifest = std::fs::read_to_string(store_of(&root).join("manifest.json")).unwrap();
    let refused = recommit(&root, &press(), &[Entry::new(NEVER, "rm -rf")]);
    assert!(
        refused.unwrap_err().to_string().contains("never reached"),
        "it says why"
    );
    assert_eq!(
        std::fs::read_to_string(store_of(&root).join("manifest.json")).unwrap(),
        manifest,
        "nothing was changed"
    );
    // A claim within reach is still read where the anchor is broken.
    edit_manifest_counter(&root, 5);
    assert_eq!(recommit(&root, &press(), &[]).unwrap(), 6);
}

#[test]
fn an_anchor_that_cannot_be_read_makes_recommit_refuse_and_never_counts_as_nought() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    let place = plane_place(&root).unwrap();
    assert_eq!(highest_counter(&place).unwrap(), 1);
    // The keyring did not answer: what the anchor held is unknown, never nought.
    seal::set_locked_for_this_test(true);
    assert!(highest_counter(&place).is_err());
    assert!(recommit(&root, &press(), &[]).is_err());
    seal::set_locked_for_this_test(false);
    // An anchor that was never written is no failure: a fresh plane starts from nought.
    assert_eq!(
        highest_counter(&plane_place(&m.plane("fresh")).unwrap()).unwrap(),
        0
    );
}

#[test]
fn a_counter_that_cannot_rise_is_refused_and_writes_nothing() {
    let m = machine();
    let root = m.plane("p");
    let place = plane_place(&root).unwrap();
    let Registration::Registered(id) = register(&root).unwrap() else {
        panic!("registered");
    };
    let mut draft = Draft::default();
    draft.add(&press(), NEVER, "one");
    let refused = write(&place, Some((&root, &id)), draft, None, u64::MAX);
    assert!(refused.is_err(), "the counter would stay at the top");
    assert!(!store_exists(&place), "nothing was written");
    assert_eq!(standing(&root).state, State::Fresh);
}

#[test]
fn the_everywhere_entry_with_a_store_and_no_keyring_is_the_deny_state() {
    let m = machine();
    let _ = m.plane("p");
    assert_eq!(everywhere_standing().state, State::Fresh);
    commit_everywhere(|d| d.add(&press(), NEVER, "curl | sh")).unwrap();
    seal::set_no_keyring_for_this_test(true);
    let standing = everywhere_standing();
    assert!(
        denied_by(&standing, |c| matches!(c, Deny::KeyringDidNotStart)),
        "{standing:?}"
    );
}

#[test]
fn the_everywhere_entry_with_nothing_kept_and_no_keyring_is_no_keyring() {
    let m = machine();
    let _ = m.plane("p");
    seal::set_no_keyring_for_this_test(true);
    assert_eq!(everywhere_standing().state, State::NoKeyring);
}

#[test]
fn register_makes_no_registry_where_a_store_a_marker_or_an_anchor_is_kept() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    let place = plane_place(&root).unwrap();
    // Each of the three alone, the registry gone.
    for gone in ["marker", "store", "anchor"] {
        let m2 = machine();
        let root = m2.plane("p");
        seal_in(&root, &[(ACCEPTED, "one")]);
        let place = plane_place(&root).unwrap();
        match gone {
            "marker" => {
                std::fs::remove_dir_all(store_of(&root)).unwrap();
                seal::item_forget_for_this_test(&place.account);
            }
            "store" => {
                std::fs::remove_dir_all(answers_folder().unwrap().join("sealed-here")).unwrap();
                seal::item_forget_for_this_test(&place.account);
            }
            _ => {
                std::fs::remove_dir_all(store_of(&root)).unwrap();
                std::fs::remove_dir_all(answers_folder().unwrap().join("sealed-here")).unwrap();
            }
        }
        seal::item_forget_for_this_test(REGISTRY_ACCOUNT);
        let refused = register(&root);
        assert!(refused.is_err(), "{gone}: {refused:?}");
        assert!(matches!(read_registry(), Read::Missing), "{gone}");
    }
    // Another plane's store counts as well: the registry that should list it is gone.
    seal::item_forget_for_this_test(REGISTRY_ACCOUNT);
    assert!(register(&m.plane("other")).is_err());
    assert!(matches!(read_registry(), Read::Missing));
    // The person's press replaces it, and the plane reads again.
    recommit(&root, &press(), &[Entry::new(ACCEPTED, "one")]).expect("the press mends it");
    assert!(matches!(read_registry(), Read::Value(_)));
    assert!(holds(&standing(&root)), "{:?}", standing(&root));
    let _ = place;
}

#[cfg(unix)]
#[test]
fn register_and_confirm_device_wait_for_the_stores_file_lock() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    let other = std::fs::File::open(lock_file().unwrap()).unwrap();
    rustix::fs::flock(&other, rustix::fs::FlockOperation::LockExclusive).unwrap();
    assert!(register(&m.plane("second")).is_err());
    let Device::Unconfirmed(id) = device() else {
        panic!("an unconfirmed id");
    };
    assert!(confirm_device(&press(), &id).is_err());
    assert_eq!(context_machine(), None, "nothing was confirmed");
    drop(other);
    assert!(register(&m.plane("second")).is_ok());
    confirm_device(&press(), &id).unwrap();
}

// ---- part 2: the import of the old nevers, and a plane the app moves ---------------------------

/// The nevers file an earlier build left in the project at `root`, written by hand.
fn old_nevers(root: &Path, pairs: &[(&str, &str)]) {
    let path = crate::dispatchnever::path(root);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let list: Vec<serde_json::Value> = pairs
        .iter()
        .map(|(a, t)| serde_json::json!({"asking": a, "target": t}))
        .collect();
    std::fs::write(path, serde_json::json!({ "never": list }).to_string()).unwrap();
}

fn never_texts(root: &Path) -> Vec<String> {
    let mut texts: Vec<String> = standing(root)
        .entries
        .into_iter()
        .filter(|e| e.kind == NEVER)
        .map(|e| e.text)
        .collect();
    texts.sort();
    texts
}

fn imported(import: Import) -> Imported {
    match import {
        Import::Imported(done) => done,
        other => panic!("an import, got {other:?}"),
    }
}

#[test]
fn existing_nevers_are_imported_once_and_sealed() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops"), ("qa", "prod")]);
    let done = imported(import_nevers(&root).unwrap());
    assert_eq!(done.nevers, ["steward -> devops", "qa -> prod"]);
    let here = standing(&root);
    assert!(holds(&here), "{here:?}");
    assert_eq!(never_texts(&root), ["qa -> prod", "steward -> devops"]);
    // The presence marker is sealed with them, so the list taken away is never "no nevers".
    assert!(here.entries.iter().any(|e| e.kind == NEVER_MARKER));
    // Sealed as an entry of the store: a file by hand in its place fails its seal.
    let file = entry_file(&root, NEVER, "qa -> prod");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("qa -> prod", "qa -> nothing")).unwrap();
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::EntryFailsSeal { .. }
    )));
}

#[test]
fn imported_nevers_are_listed_in_a_notice_and_marked_in_the_manifest() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    assert_eq!(imported_nevers(&root), Some(false));
    let done = imported(import_nevers(&root).unwrap());
    assert!(done.notice.contains("steward -> devops"), "{}", done.notice);
    assert!(done.notice.contains("safe to delete"), "{}", done.notice);
    assert_eq!(imported_nevers(&root), Some(true));
}

#[test]
fn an_import_that_has_run_changes_nothing_when_it_runs_again() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    imported(import_nevers(&root).unwrap());
    let counter = counter_of(&root);
    assert_eq!(import_nevers(&root).unwrap(), Import::Already);
    assert_eq!(counter_of(&root), counter, "the second call wrote nothing");
}

#[test]
fn a_never_the_old_file_gained_after_the_import_is_the_deny_state_until_it_is_sealed() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    imported(import_nevers(&root).unwrap());
    old_nevers(&root, &[("steward", "devops"), ("late", "one")]);
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::NeversNotImported
    )));
    let done = imported(import_nevers(&root).unwrap());
    assert_eq!(done.nevers, ["late -> one"]);
    assert!(holds(&standing(&root)));
    assert_eq!(never_texts(&root), ["late -> one", "steward -> devops"]);
    assert_eq!(import_nevers(&root).unwrap(), Import::Already);
}

#[test]
fn a_never_pair_a_chat_appends_keeps_the_verified_answers_for_a_start() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    imported(import_nevers(&root).unwrap());
    let before = standing(&root);
    assert!(holds(&before), "{before:?}");
    old_nevers(&root, &[("steward", "devops"), ("forged", "pair")]);
    let after = standing(&root);
    assert!(denied_by(&after, |c| matches!(c, Deny::NeversNotImported)));
    assert!(after.store_verified, "{after:?}");
    assert_eq!(after.kept, before.entries, "the verified answers are kept");
    assert!(!after.is_unverified_by_a_reachable_cause());
}

#[test]
fn existing_grants_are_not_imported() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    // A grant an earlier build kept on this machine, in the file beside the clone.
    let grants = crate::sandbox::local::path(&root);
    std::fs::create_dir_all(grants.parent().unwrap()).unwrap();
    std::fs::write(
        &grants,
        r#"{"dispatch_mine":[{"asking":"steward","target":"qa"}],"writes_mine":["/tmp/x"]}"#,
    )
    .unwrap();
    imported(import_nevers(&root).unwrap());
    let here = standing(&root);
    assert!(holds(&here), "{here:?}");
    assert!(
        here.entries
            .iter()
            .all(|e| e.kind == NEVER || e.kind == NEVER_MARKER),
        "{:?}",
        here.entries
    );
}

#[test]
fn nevers_still_in_their_old_file_are_the_deny_state_until_they_are_imported() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    let before = standing(&root);
    assert!(
        denied_by(&before, |c| matches!(c, Deny::NeversNotImported)),
        "{before:?}"
    );
    assert!(before.entries.is_empty());
    imported(import_nevers(&root).unwrap());
    assert!(holds(&standing(&root)));
    // A plane with no old nevers is untouched by all this: a fresh install.
    assert_eq!(standing(&m.plane("clean")).state, State::Fresh);
}

#[test]
fn an_old_file_that_does_not_read_is_not_imported_and_the_plane_stays_denied() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    let path = crate::dispatchnever::path(&root);
    std::fs::write(&path, r#"{"never":[{"asking":"steward"}]}"#).unwrap();
    assert!(import_nevers(&root).is_err());
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::NeversNotImported
    )));
    assert!(
        !store_exists(&plane_place(&root).unwrap()),
        "nothing sealed"
    );
    // Mended by putting the file back: the import then goes through.
    old_nevers(&root, &[("steward", "devops")]);
    imported(import_nevers(&root).unwrap());
    assert!(holds(&standing(&root)));
}

#[test]
fn a_sealed_never_taken_from_the_old_file_is_not_imported_and_stays_denied() {
    let m = machine();
    let root = m.plane("p");
    // The nevers the earlier build sealed as a set, then the file taken away.
    crate::dispatchnever::add(&root, "steward", "devops").unwrap();
    std::fs::remove_file(crate::dispatchnever::path(&root)).unwrap();
    assert!(import_nevers(&root).is_err());
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::NeversNotImported
    )));
    assert!(!store_exists(&plane_place(&root).unwrap()));
}

#[test]
fn where_the_old_files_hold_no_never_nothing_is_imported_or_written() {
    let m = machine();
    let root = m.plane("p");
    assert_eq!(import_nevers(&root).unwrap(), Import::Nothing);
    old_nevers(&root, &[]);
    assert_eq!(import_nevers(&root).unwrap(), Import::Nothing);
    assert!(!store_exists(&plane_place(&root).unwrap()));
    assert_eq!(standing(&root).state, State::Fresh);
}

#[test]
fn nevers_an_earlier_build_kept_in_the_sandbox_file_are_imported_too() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    let grants = crate::sandbox::local::path(&root);
    std::fs::create_dir_all(grants.parent().unwrap()).unwrap();
    std::fs::write(
        &grants,
        r#"{"dispatch_never":[{"asking":"qa","target":"prod"}]}"#,
    )
    .unwrap();
    let done = imported(import_nevers(&root).unwrap());
    assert!(done.nevers.contains(&"qa -> prod".to_owned()), "{done:?}");
    assert_eq!(never_texts(&root), ["qa -> prod", "steward -> devops"]);
}

#[test]
fn a_keyring_that_cannot_keep_the_anchor_leaves_the_old_file_authoritative() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    seal::set_item_write_fails_for_this_test(true);
    let refused = import_nevers(&root);
    seal::set_item_write_fails_for_this_test(false);
    assert!(refused.is_err());
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::NeversNotImported
    )));
    assert!(!store_exists(&plane_place(&root).unwrap()));
    imported(import_nevers(&root).unwrap());
    assert!(holds(&standing(&root)));
}

#[test]
fn where_this_process_cannot_seal_the_old_file_stays_authoritative() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    seal::set_no_keyring_for_this_test(true);
    assert_eq!(import_nevers(&root).unwrap(), Import::NotKept);
    seal::set_no_keyring_for_this_test(false);
    // Not the app: no key held at all.
    seal::set_for_this_test(None);
    assert_eq!(import_nevers(&root).unwrap(), Import::NotKept);
    seal::set_for_this_test(Some(vec![7; 32]));
    seal::set_locked_for_this_test(true);
    assert!(import_nevers(&root).is_err());
    seal::set_locked_for_this_test(false);
    assert!(!store_exists(&plane_place(&root).unwrap()));
}

#[test]
fn a_plane_in_the_deny_state_for_another_cause_imports_nothing() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(ACCEPTED, "one")]);
    old_nevers(&root, &[("steward", "devops")]);
    // The manifest taken away: an anchored path with no manifest.
    std::fs::remove_file(store_of(&root).join("manifest.json")).unwrap();
    assert!(import_nevers(&root).is_err());
    assert!(
        !denied_by(&standing(&root), |c| matches!(c, Deny::NeversNotImported))
            || standing(&root).is_deny()
    );
    assert_eq!(imported_nevers(&root), Some(false));
}

#[test]
fn the_imported_nevers_hold_after_the_old_files_are_deleted() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    imported(import_nevers(&root).unwrap());
    std::fs::remove_file(crate::dispatchnever::path(&root)).unwrap();
    let here = standing(&root);
    assert!(holds(&here), "{here:?}");
    assert_eq!(never_texts(&root), ["steward -> devops"]);
}

/// `root` renamed to `name` beside it, as a move on one filesystem.
fn moved_to(m: &Machine, root: &Path, name: &str) -> PathBuf {
    let new = m.dir.path().join(name);
    std::fs::rename(root, &new).unwrap();
    std::fs::canonicalize(&new).unwrap()
}

#[test]
fn a_plane_the_app_moves_keeps_its_answers() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf"), (ACCEPTED, "guards.git=enforce")]);
    confirm_shown_device();
    let counter = counter_of(&old);
    let old_folder = store_of(&old);
    assert!(old_folder.exists(), "the old place kept a store");
    let new = moved_to(&m, &old, "new");
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::Carried);
    let here = standing(&new);
    assert!(holds(&here), "{here:?}");
    assert!(here.entries.contains(&Entry::new(NEVER, "rm -rf")));
    assert!(
        here.entries
            .contains(&Entry::new(ACCEPTED, "guards.git=enforce"))
    );
    assert!(counter_of(&new) > counter, "the counter rose");
    assert_eq!(
        plane_containing(&new).map(|plane| plane.real),
        Some(new.clone())
    );
    assert!(
        context_machine().is_some(),
        "the device id stayed confirmed"
    );
    // And the store it left is not read again: its folder is gone.
    assert!(!old_folder.exists(), "the old place's store was taken away");
}

#[test]
fn a_moved_plane_whose_answers_could_not_follow_stays_the_deny_state() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let new = moved_to(&m, &old, "new");
    // Not the app: nothing is sealed, so nothing is carried, and the new place reads as moved.
    seal::set_locked_for_this_test(true);
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::NotKept);
    seal::set_locked_for_this_test(false);
    register(&new).unwrap();
    assert!(denied_by(&standing(&new), |c| matches!(
        c,
        Deny::PlaneMoved { .. }
    )));
}

#[test]
fn a_keyring_that_fails_the_move_changes_nothing_and_the_move_goes_through_later() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let new = moved_to(&m, &old, "new");
    seal::set_item_write_fails_for_this_test(true);
    assert!(follow_move(&old, &new).is_err());
    seal::set_item_write_fails_for_this_test(false);
    assert!(!matches!(standing(&new).state, State::Holds));
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::Carried);
    assert_eq!(never_texts(&new), ["rm -rf"]);
}

#[test]
fn an_old_store_that_is_not_whole_is_not_carried_to_the_new_place() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let file = entry_file(&old, NEVER, "rm -rf");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("rm -rf", "ls")).unwrap();
    let new = moved_to(&m, &old, "new");
    assert!(follow_move(&old, &new).is_err());
    assert!(!matches!(standing(&new).state, State::Holds));
}

#[test]
fn a_move_never_carries_answers_over_what_the_new_place_already_keeps() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let old_clone = clone_key(&old);
    // Answers kept for the path the plane is moved to, by a project that was there before.
    let before = m.plane("new");
    seal_in(&before, &[(ACCEPTED, "mine")]);
    std::fs::remove_dir_all(&before).unwrap();
    let new = moved_to(&m, &old, "new");
    assert!(follow_move(&old, &new).is_err());
    assert!(
        store_exists(&Place {
            folder: store_of_clone(&old_clone),
            ..plane_place(&new).unwrap()
        }),
        "the old place's store is as it was"
    );
    assert!(
        !holds(&standing(&new)),
        "nothing was carried over what is kept"
    );
}

#[test]
fn a_copy_is_not_a_move_and_the_old_place_keeps_its_answers() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let copy = m.plane("copy");
    assert_eq!(follow_move(&old, &copy).unwrap(), Moved::NothingKept);
    assert!(holds(&standing(&old)));
    assert_eq!(standing(&copy).state, State::Fresh);
}

/// The folder a clone key's store is in.
fn store_of_clone(clone: &str) -> PathBuf {
    answers_folder().unwrap().join(clone)
}

// ---- nevers kept in step after the import, recommit, and the move's edge cases ---------------

fn imported_plane(m: &Machine, name: &str, pairs: &[(&str, &str)]) -> PathBuf {
    let root = m.plane(name);
    old_nevers(&root, pairs);
    imported(import_nevers(&root).unwrap());
    root
}

#[test]
fn a_never_said_after_the_import_is_in_the_store_and_its_lift_takes_it_out() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops")]);
    crate::dispatchnever::add(&root, "qa", "prod").unwrap();
    assert!(holds(&standing(&root)), "{:?}", standing(&root));
    // The store holds it by itself: the file deleted changes nothing.
    std::fs::remove_file(crate::dispatchnever::path(&root)).unwrap();
    assert!(holds(&standing(&root)));
    assert_eq!(never_texts(&root), ["qa -> prod", "steward -> devops"]);
}

#[test]
fn a_never_lifted_after_the_import_is_taken_out_of_the_store_too() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops"), ("qa", "prod")]);
    assert!(crate::dispatchnever::lift(&root, "qa", "prod").unwrap());
    assert!(holds(&standing(&root)), "{:?}", standing(&root));
    assert_eq!(never_texts(&root), ["steward -> devops"]);
    assert!(!crate::dispatchnever::lift(&root, "qa", "prod").unwrap());
}

#[test]
fn a_never_in_the_file_that_the_store_lacks_is_the_deny_state_until_it_is_sealed() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops")]);
    crate::dispatchnever::add(&root, "qa", "prod").unwrap();
    let place = plane_place(&root).unwrap();
    let before = std::fs::read_to_string(place.folder.join("manifest.json")).unwrap();
    // Said where only the old files hold it, by something that could not seal it in the store
    // (an earlier build, or a process that is not the app).
    let grants = crate::sandbox::local::path(&root);
    std::fs::create_dir_all(grants.parent().unwrap()).unwrap();
    std::fs::write(
        &grants,
        r#"{"dispatch_never":[{"asking":"late","target":"one"}]}"#,
    )
    .unwrap();
    let here = standing(&root);
    assert!(
        denied_by(&here, |c| matches!(c, Deny::NeversNotImported)),
        "{here:?}"
    );
    assert_eq!(
        before,
        std::fs::read_to_string(place.folder.join("manifest.json")).unwrap()
    );
    // Mended by the import, which seals what the store lacks and nothing it has.
    let done = imported(import_nevers(&root).unwrap());
    assert_eq!(done.nevers, ["late -> one"]);
    assert!(holds(&standing(&root)));
}

#[test]
fn a_lift_is_tried_again_where_the_store_still_holds_a_never_the_file_lacks() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops"), ("qa", "prod")]);
    // A lift whose store write failed: the file already lacks the never the store holds.
    old_nevers(&root, &[("steward", "devops")]);
    assert_eq!(never_texts(&root), ["qa -> prod", "steward -> devops"]);
    assert!(crate::dispatchnever::lift(&root, "qa", "prod").unwrap());
    assert_eq!(never_texts(&root), ["steward -> devops"]);
}

#[test]
fn recommit_keeps_the_nevers_the_old_file_holds_and_the_imported_mark() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops")]);
    std::fs::remove_file(store_of(&root).join("manifest.json")).unwrap();
    assert!(standing(&root).is_deny());
    recommit(&root, &press(), &[Entry::new(ACCEPTED, "one")]).unwrap();
    let here = standing(&root);
    assert!(holds(&here), "{here:?}");
    assert_eq!(never_texts(&root), ["steward -> devops"]);
    assert_eq!(imported_nevers(&root), Some(true));
    assert_eq!(import_nevers(&root).unwrap(), Import::Already);
}

#[test]
fn recommit_keeps_the_sealed_nevers_where_the_old_file_is_gone() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops")]);
    commit(&root, |draft| draft.add(&press(), ACCEPTED, "one")).unwrap();
    std::fs::remove_file(crate::dispatchnever::path(&root)).unwrap();
    // An answer that fails its seal: a deny, with the nevers still readable.
    let file = entry_file(&root, ACCEPTED, "one");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, text.replace("one", "two")).unwrap();
    assert!(standing(&root).is_deny());
    recommit(&root, &press(), &[]).unwrap();
    let here = standing(&root);
    assert!(holds(&here), "{here:?}");
    assert_eq!(never_texts(&root), ["steward -> devops"]);
    assert_eq!(import_nevers(&root).unwrap(), Import::Already);
}

#[test]
fn recommit_never_dead_ends_in_never_imported_where_the_old_file_was_deleted() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops")]);
    // Doctor said the old file was safe to delete, then the manifest was lost as well.
    std::fs::remove_file(crate::dispatchnever::path(&root)).unwrap();
    std::fs::remove_file(store_of(&root).join("manifest.json")).unwrap();
    recommit(&root, &press(), &[]).unwrap();
    let here = standing(&root);
    assert!(
        !denied_by(&here, |c| matches!(c, Deny::NeversNotImported)),
        "{here:?}"
    );
    assert!(holds(&here), "{here:?}");
    assert_eq!(imported_nevers(&root), Some(true));
}

#[test]
fn an_import_with_a_locked_key_changes_nothing_and_goes_through_when_it_opens() {
    let m = machine();
    let root = m.plane("p");
    old_nevers(&root, &[("steward", "devops")]);
    seal::set_locked_for_this_test(true);
    assert!(import_nevers(&root).is_err());
    seal::set_locked_for_this_test(false);
    assert!(
        !store_exists(&plane_place(&root).unwrap()),
        "nothing sealed"
    );
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::NeversNotImported
    )));
    imported(import_nevers(&root).unwrap());
    assert!(holds(&standing(&root)));
}

#[test]
fn a_legacy_key_that_does_not_read_is_not_imported_and_stays_denied() {
    let m = machine();
    let root = m.plane("p");
    let grants = crate::sandbox::local::path(&root);
    std::fs::create_dir_all(grants.parent().unwrap()).unwrap();
    std::fs::write(&grants, r#"{"dispatch_never":5}"#).unwrap();
    assert!(import_nevers(&root).is_err());
    assert!(denied_by(&standing(&root), |c| matches!(
        c,
        Deny::NeversNotImported
    )));
    assert!(!store_exists(&plane_place(&root).unwrap()));
    // Mended by taking the key away: nothing is left to import.
    std::fs::write(&grants, "{}").unwrap();
    assert_eq!(import_nevers(&root).unwrap(), Import::Nothing);
}

#[test]
fn an_import_cut_short_before_its_manifest_is_mended_by_the_persons_press() {
    let m = machine();
    let root = imported_plane(&m, "p", &[("steward", "devops")]);
    // The anchor and the entries were written, the manifest not: as a crash between them leaves it.
    let manifest = store_of(&root).join("manifest.json");
    std::fs::remove_file(&manifest).unwrap();
    let here = standing(&root);
    assert!(here.is_deny(), "{here:?}");
    assert!(
        import_nevers(&root).is_err(),
        "a plane in the deny state imports nothing"
    );
    recommit(&root, &press(), &[]).unwrap();
    assert!(holds(&standing(&root)));
    assert_eq!(never_texts(&root), ["steward -> devops"]);
    assert_eq!(import_nevers(&root).unwrap(), Import::Already);
}

#[cfg(unix)]
#[test]
fn a_plane_moved_with_its_link_left_reads_moved_never_fresh_and_follows_through_the_link() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf"), (ACCEPTED, "one")]);
    let new = moved_to(&m, &old, "new");
    std::os::unix::fs::symlink(&new, &old).unwrap();
    let before = standing(&new);
    assert!(
        denied_by(&before, |c| matches!(c, Deny::PlaneMoved { .. })),
        "{before:?}"
    );
    // The link makes the old path a project again (its `.git` is the new place's), and the
    // move is still one.
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::Carried);
    let here = standing(&new);
    assert!(holds(&here), "{here:?}");
    assert!(here.entries.contains(&Entry::new(NEVER, "rm -rf")));
}

#[test]
fn a_move_prefers_the_sealed_plane_of_the_old_place_over_a_placeholder_at_the_new_one() {
    let m = machine();
    // A placeholder registered at the new path first, as an open of a project there leaves it.
    let before = m.plane("new");
    register(&before).unwrap();
    std::fs::remove_dir_all(&before).unwrap();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let new = moved_to(&m, &old, "new");
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::Carried);
    assert_eq!(never_texts(&new), ["rm -rf"]);
}

#[test]
fn a_move_already_followed_is_carried_and_one_cut_short_is_the_deny_state() {
    let m = machine();
    let old = m.plane("old");
    seal_in(&old, &[(NEVER, "rm -rf")]);
    let new = moved_to(&m, &old, "new");
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::Carried);
    // Again, as the next launch finishes what a journal says: nothing changes.
    let counter = counter_of(&new);
    assert_eq!(follow_move(&old, &new).unwrap(), Moved::Carried);
    assert_eq!(counter_of(&new), counter);
    // The registry followed and the store did not: the deny state, said.
    std::fs::remove_file(store_of(&new).join("manifest.json")).unwrap();
    assert!(follow_move(&old, &new).is_err());
}

#[test]
fn a_move_under_any_deny_cause_carries_nothing_and_leaves_the_old_store() {
    type Break = fn(&Path);
    let breaks: [(&str, Break); 4] = [
        ("an entry gone", |root| {
            std::fs::remove_file(entry_file(root, NEVER, "rm -rf")).unwrap();
        }),
        ("the manifest gone", |root| {
            std::fs::remove_file(store_of(root).join("manifest.json")).unwrap();
        }),
        ("a manifest that fails its seal", |root| {
            let file = store_of(root).join("manifest.json");
            let text = std::fs::read_to_string(&file).unwrap();
            std::fs::write(&file, text.replace("\"counter\"", "\"counter \"")).unwrap();
        }),
        ("the anchor gone", |root| {
            let place = plane_place(root).unwrap();
            seal::item_forget_for_this_test(&place.account);
        }),
    ];
    for (name, break_it) in breaks {
        let m = machine();
        let old = m.plane("old");
        seal_in(&old, &[(NEVER, "rm -rf")]);
        let old_clone = clone_key(&old);
        break_it(&old);
        let new = moved_to(&m, &old, "new");
        assert!(follow_move(&old, &new).is_err(), "{name}");
        assert!(!holds(&standing(&new)), "{name}");
        assert!(
            store_of_clone(&old_clone).exists() || name == "the anchor gone",
            "{name}: the old store is as it was"
        );
    }
}

// ---- M75-07b: the 07a acceptance items carried to it (#7016, the final check of #7234) --------

/// A Once grant is spent by a commit that records its use first: the grant is gone and the counter
/// has risen when the call answers, and the caller honours the grant only on `Ok`.
#[test]
fn a_once_grant_is_consumed_before_it_is_honoured() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "once: git push")]);
    let before = counter_of(&root);
    let after = use_once_grant(&root, "once: git push").expect("the use is recorded");
    assert_eq!(after, before + 1, "the use raised the counter");
    assert!(
        !entry_file(&root, GRANT, "once: git push").exists(),
        "the grant is gone from the store"
    );
    // Spent: a second use is refused, never honoured again.
    assert!(
        use_once_grant(&root, "once: git push")
            .unwrap_err()
            .to_string()
            .contains("not honoured")
    );
}

/// Where the use cannot be recorded, the grant is not honoured: a plane in the deny state, a key
/// that stays locked (so a host that cannot seal) and a store with no such grant each refuse.
#[test]
fn a_once_grant_whose_use_cannot_be_recorded_is_refused() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(GRANT, "once: git push"), (NEVER, "rm -rf")]);
    // A host that cannot seal.
    seal::set_locked_for_this_test(true);
    assert!(use_once_grant(&root, "once: git push").is_err());
    seal::set_locked_for_this_test(false);
    assert!(
        entry_file(&root, GRANT, "once: git push").exists(),
        "a refused use spent nothing"
    );
    // The deny state: an older copy of the store put back.
    let copy = m.dir.path().join("copy");
    copy_dir(&store_of(&root), &copy);
    commit(&root, |d| d.add(&press(), NEVER, "two")).unwrap();
    std::fs::remove_dir_all(store_of(&root)).unwrap();
    copy_dir(&copy, &store_of(&root));
    assert!(standing(&root).is_deny());
    assert!(use_once_grant(&root, "once: git push").is_err());
}

/// Only what narrows is taken out without a press: an acceptance only ever widens, so its
/// withdrawal narrows. Pinned, so a kind that widens when it goes is never added to the list.
#[test]
fn only_a_removal_that_narrows_needs_no_press() {
    for kind in [GRANT, PASS, ACCEPTANCE] {
        let mut draft = Draft::default();
        draft.add(&press(), kind, "x");
        draft.remove_narrowing(kind, "x");
        assert!(draft.refused.is_none(), "{kind} narrows when it goes");
        assert!(draft.entries().is_empty());
    }
    for kind in [NEVER, NEVER_MARKER, FLOOR, ACCEPTED] {
        let mut draft = Draft::default();
        draft.add(&press(), kind, "x");
        draft.remove_narrowing(kind, "x");
        assert!(draft.refused.is_some(), "{kind} widens when it goes");
    }
}

/// A sealed manifest is no unsealed claim, so the counter cap never refuses it where the anchor is
/// missing or broken (#7234 final check, item 4).
#[test]
fn a_sealed_manifest_is_never_refused_by_the_counter_cap() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    let place = plane_place(&root).unwrap();
    // A sealed manifest past the cap, written by the store's own writer.
    {
        let _turn = store_lock().unwrap();
        let Registration::Registered(id) = register_unlocked(&root).unwrap() else {
            panic!("registered");
        };
        let mut draft = Draft::default();
        draft.add(&press(), NEVER, "rm -rf");
        draft.mark_nevers_imported();
        write(&place, Some((&root, &id)), draft, None, PLAUSIBLE_COUNTER).unwrap();
    }
    assert!(counter_of(&root) > PLAUSIBLE_COUNTER);
    // The anchor is gone: the sealed manifest's own counter is the floor, and not refused.
    assert!(seal::item_write(&place.account, "not json"));
    assert_eq!(
        highest_counter(&place).expect("a sealed manifest is no claim to refuse"),
        counter_of(&root)
    );
}

/// `recommit` that refuses on the counter changes nothing: the registry is not mended first.
#[test]
fn a_recommit_refused_on_the_counter_leaves_the_registry_as_it_was() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    let account = plane_place(&root).unwrap().account;
    assert!(seal::item_write(&account, "not json"));
    edit_manifest_counter(&root, u64::MAX);
    assert!(seal::item_write(REGISTRY_ACCOUNT, "not json"));
    assert!(recommit(&root, &press(), &[]).is_err());
    assert!(
        matches!(seal::item_read(REGISTRY_ACCOUNT), Item::Value(text) if text == "not json"),
        "nothing was changed: the registry was not mended before the refusal"
    );
}

/// Old nevers files that do not read may hold what purlis cannot see, so a press does not mark them
/// imported: the plane stays denied until they read or are gone (#7251, item 3).
#[test]
fn a_recommit_does_not_mark_nevers_imported_while_the_old_files_do_not_read() {
    let m = machine();
    let root = m.plane("p");
    seal_in(&root, &[(NEVER, "rm -rf")]);
    let old = crate::dispatchnever::path(&root);
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "not json").unwrap();
    assert!(
        denied_by(&standing(&root), |c| matches!(c, Deny::NeversNotImported)),
        "unread old files and no mark: the plane is denied"
    );
    recommit(&root, &press(), &[]).expect("the press goes through");
    assert!(
        denied_by(&standing(&root), |c| matches!(c, Deny::NeversNotImported)),
        "the press did not mark what it could not read"
    );
    // Once the file is gone there is nothing hidden, and the press settles it.
    std::fs::remove_file(&old).unwrap();
    recommit(&root, &press(), &[]).expect("the press goes through");
    assert!(holds(&standing(&root)), "{:?}", standing(&root));
}

/// A plane nested under a moved project resolves through the link left at the project's old path;
/// its own last component is no link, and it is still gone from there (#7251, item 4).
#[cfg(unix)]
#[test]
fn a_nested_path_through_a_link_is_not_still_a_folder_of_its_own() {
    let m = machine();
    let real = std::fs::canonicalize(m.dir.path()).unwrap();
    std::fs::create_dir_all(real.join("new/child")).unwrap();
    std::os::unix::fs::symlink(real.join("new"), real.join("old")).unwrap();
    assert!(still_a_folder(&real.join("new/child")));
    assert!(
        !still_a_folder(&real.join("old/child")),
        "the old project path is a link, so a plane under it is not a folder there"
    );
    assert!(!still_a_folder(&real.join("gone")));
}
