// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const DESKTOP: &str = "01JA0DESKT0P00000000000000";

fn desktop() -> Pinned {
    Pinned {
        device: DESKTOP.into(),
        key: LinkKey::generate().unwrap().public(),
    }
}

#[test]
fn a_runners_pairing_makes_its_key_and_keeps_the_desktop_as_its_one_peer() {
    let folder = tempfile::tempdir().unwrap();
    let desktop = desktop();
    let runner = pair(folder.path(), &desktop).unwrap();
    let (ours, peer) = answering(folder.path()).unwrap().expect("paired");
    assert_eq!(ours.device, runner.device);
    assert_eq!(ours.key.public(), runner.key);
    assert_eq!(peer, desktop);
}

#[test]
fn a_runner_is_paired_once_whichever_desktop_asks_again() {
    let folder = tempfile::tempdir().unwrap();
    let first = desktop();
    let runner = pair(folder.path(), &first).unwrap();
    for again in [first.clone(), desktop()] {
        let refused = pair(folder.path(), &again).unwrap_err();
        assert!(refused.contains("already holds"), "{refused}");
    }
    let (ours, peer) = answering(folder.path()).unwrap().unwrap();
    assert_eq!((ours.key.public(), peer), (runner.key, first));
}

#[test]
fn a_home_that_already_holds_a_link_key_or_a_peers_file_is_not_paired() {
    type Folder = fn(&std::path::Path) -> std::path::PathBuf;
    for (folder_of, name) in [
        (crate::machine::charterd as Folder, KEY_FILE),
        (crate::machine::dir as Folder, PEERS_FILE),
    ] {
        let folder = tempfile::tempdir().unwrap();
        purlis_same_user::PrivateDirectory::open(&folder_of(folder.path()))
            .unwrap()
            .replace_file(name, b"brought by the image")
            .unwrap();
        let refused = pair(folder.path(), &desktop()).unwrap_err();
        assert!(refused.contains("already holds"), "{name}: {refused}");
    }
}

#[test]
fn a_runner_that_was_never_paired_answers_no_link() {
    let folder = tempfile::tempdir().unwrap();
    assert!(answering(folder.path()).unwrap().is_none());
}

#[test]
fn the_runners_key_and_peer_are_kept_for_its_user_alone() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), &desktop()).unwrap();
    for file in [
        crate::machine::charterd(folder.path()).join(KEY_FILE),
        crate::machine::dir(folder.path()).join(PEERS_FILE),
    ] {
        let name = file.display();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{name}");
    }
}

#[test]
fn a_kept_key_others_may_read_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), &desktop()).unwrap();
    std::fs::set_permissions(
        crate::machine::charterd(folder.path()).join(KEY_FILE),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(answering(folder.path()).is_err());
}

#[test]
fn a_kept_key_that_is_not_one_key_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), &desktop()).unwrap();
    purlis_same_user::PrivateDirectory::open(&crate::machine::charterd(folder.path()))
        .unwrap()
        .replace_file(KEY_FILE, b"not a key")
        .unwrap();
    assert!(answering(folder.path()).is_err());
}

#[test]
fn a_device_id_and_key_are_written_and_read_as_one_line() {
    let pinned = desktop();
    let line = to_line(&pinned);
    assert_eq!(from_line(&line).unwrap(), pinned);
    assert!(!line.contains('\n'));
    for refused in [
        "",
        "{}",
        r#"{"device":"01JA","key":"00"}"#,
        r#"{"device":"","key":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
        r#"{"device":"01JA","key":"0000000000000000000000000000000000000000000000000000000000000000","more":1}"#,
    ] {
        assert!(from_line(refused).is_err(), "{refused} was taken");
    }
}

/// A key is read in its one spelling (#2683): X25519 ignores a key's last bit, so a key with
/// it set is the same key spelled another way, and it is refused rather than taken for
/// another key.
#[test]
fn a_public_key_with_its_ignored_last_bit_set_is_refused() {
    let line = |last: &str| format!(r#"{{"device":"01JA","key":"{}{last}"}}"#, "00".repeat(31));
    assert!(from_line(&line("7f")).is_ok());
    for set in ["80", "ff"] {
        assert!(
            from_line(&line(set)).is_err(),
            "a key ending {set} was taken"
        );
    }
    let ours = desktop();
    assert_eq!(
        ours.key.as_bytes()[31] & 0x80,
        0,
        "a made key never sets it"
    );
}

fn write_peers(folder: &std::path::Path, text: &str) {
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(folder))
        .unwrap()
        .replace_file(PEERS_FILE, text.as_bytes())
        .unwrap();
}

#[test]
fn the_runners_peers_file_names_its_one_desktop_with_when_it_was_paired() {
    let folder = tempfile::tempdir().unwrap();
    let desktop = desktop();
    pair(folder.path(), &desktop).unwrap();
    let text =
        std::fs::read_to_string(crate::machine::dir(folder.path()).join(PEERS_FILE)).unwrap();
    let peers: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(peers["version"], 1);
    assert_eq!(peers["peers"].as_array().unwrap().len(), 1);
    assert_eq!(peers["peers"][0]["device"], DESKTOP);
    let hex: String = desktop
        .key
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(peers["peers"][0]["key"], hex);
    assert!(peers["peers"][0]["paired"].as_u64().unwrap() > 1_700_000_000);
}

#[test]
fn a_peers_file_with_more_than_one_desktop_or_of_another_shape_links_nobody() {
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), &desktop()).unwrap();
    let one = |device: &str| serde_json::json!({ "device": device, "key": "00".repeat(32), "paired": 1_800_000_000u64 });
    for text in [
        serde_json::json!({ "version": 1, "peers": [one(DESKTOP), one("01JC0OTHER")] }).to_string(),
        serde_json::json!({ "version": 2, "peers": [one(DESKTOP)] }).to_string(),
        serde_json::json!({ "version": 1, "peers": [] }).to_string(),
        serde_json::json!({ "version": 1, "peers": [{ "device": DESKTOP, "key": "00".repeat(32) }] }).to_string(),
        "not json".to_owned(),
    ] {
        write_peers(folder.path(), &text);
        assert!(answering(folder.path()).is_err(), "{text} was taken");
        assert!(pair(folder.path(), &desktop()).is_err(), "{text} let a pairing in");
    }
}

#[test]
fn of_two_pairings_at_once_one_is_made_and_the_other_refused() {
    for _ in 0..20 {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().to_path_buf();
        let (a, b) = (desktop(), desktop());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let racing = |desktop: Pinned| {
            let (root, barrier) = (root.clone(), std::sync::Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                pair(&root, &desktop).map(|runner| (runner, desktop))
            })
        };
        let (first, second) = (racing(a), racing(b));
        let results = [first.join().unwrap(), second.join().unwrap()];
        let made: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(made.len(), 1, "{results:?}");
        let (runner, desktop) = made[0];
        let (ours, peer) = answering(&root).unwrap().unwrap();
        assert_eq!((ours.key.public(), &peer), (runner.key, desktop));
    }
}

#[test]
fn a_runner_mints_its_device_id_at_its_pairing() {
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(crate::machine::known_device_id(folder.path()), None);
    let runner = pair(folder.path(), &desktop()).unwrap();
    assert!(
        ulid::Ulid::from_string(&runner.device).is_ok(),
        "{}",
        runner.device
    );
    assert_eq!(
        crate::machine::known_device_id(folder.path()).as_deref(),
        Some(runner.device.as_str())
    );
}

#[test]
fn a_home_that_came_with_a_device_id_is_not_paired_and_nothing_is_made() {
    let folder = tempfile::tempdir().unwrap();
    let brought = crate::machine::device_id(folder.path()).unwrap();
    let refused = pair(folder.path(), &desktop()).unwrap_err();
    assert!(
        refused.contains("came with a device id") && refused.contains("Remove the runner"),
        "{refused}"
    );
    assert!(
        !crate::machine::charterd(folder.path())
            .join(KEY_FILE)
            .exists()
    );
    assert!(!crate::machine::dir(folder.path()).join(PEERS_FILE).exists());
    assert!(answering(folder.path()).unwrap().is_none());
    assert_eq!(
        crate::machine::known_device_id(folder.path()),
        Some(brought),
        "the id it came with is left as it was"
    );
}

/// What Docker does with a new container's home volume: it starts as a copy of the image's.
fn copy_home(from: &std::path::Path, to: &std::path::Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            std::fs::set_permissions(&target, entry.metadata().unwrap().permissions()).unwrap();
            copy_home(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn two_runners_from_one_image_never_pair_with_one_device_id() {
    // An image whose build minted a device id: every home made from it holds that id.
    let image = tempfile::tempdir().unwrap();
    crate::machine::device_id(image.path()).unwrap();
    for _ in 0..2 {
        let home = tempfile::tempdir().unwrap();
        copy_home(image.path(), home.path());
        assert!(crate::machine::known_device_id(home.path()).is_some());
        assert!(pair(home.path(), &desktop()).is_err());
    }
    // An image whose home holds none: each runner mints an id of its own.
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, b) = (
        pair(a.path(), &desktop()).unwrap(),
        pair(b.path(), &desktop()).unwrap(),
    );
    assert_ne!(a.device, b.device);
}

#[test]
fn a_runner_asked_for_a_link_before_its_pairing_mints_no_device_id() {
    let folder = tempfile::tempdir().unwrap();
    assert!(answering(folder.path()).unwrap().is_none());
    assert_eq!(crate::machine::known_device_id(folder.path()), None);
    assert!(pair(folder.path(), &desktop()).is_ok());
}

#[test]
fn a_paired_runner_whose_device_id_is_gone_links_nobody() {
    let folder = tempfile::tempdir().unwrap();
    pair(folder.path(), &desktop()).unwrap();
    crate::machine::update(folder.path(), |store| store.device = None).unwrap();
    assert!(answering(folder.path()).is_err());
}

#[test]
fn the_paired_desktop_unpairing_leaves_a_runner_that_links_nobody_and_is_not_paired_again() {
    let folder = tempfile::tempdir().unwrap();
    let ours = desktop();
    pair(folder.path(), &ours).unwrap();
    unpair(folder.path(), &ours).unwrap();
    assert!(answering(folder.path()).unwrap().is_none());
    assert!(!crate::machine::dir(folder.path()).join(PEERS_FILE).exists());
    assert!(pair(folder.path(), &desktop()).is_err());
    // Again is nothing to do.
    unpair(folder.path(), &ours).unwrap();
}

#[test]
fn another_desktop_cannot_unpair_a_runner() {
    let folder = tempfile::tempdir().unwrap();
    let ours = desktop();
    pair(folder.path(), &ours).unwrap();
    assert!(unpair(folder.path(), &desktop()).is_err());
    let (_, peer) = answering(folder.path()).unwrap().unwrap();
    assert_eq!(peer, ours);
}

/// A pod's hand-off folder, as the operator's template mounts it: a folder of its own.
fn pod_made_a_key() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let pod = tempfile::tempdir().unwrap();
    let handoff = pod.path().join("link-key");
    let published = pod.path().join("termination-log");
    std::fs::write(&published, "").unwrap();
    make_at_start(&handoff, &published).unwrap();
    let public = std::fs::read_to_string(&published).unwrap();
    (pod, handoff, public)
}

#[test]
fn a_cluster_runner_pairs_with_the_key_its_pod_made_and_published_as_it_started() {
    let (_pod, handoff, public) = pod_made_a_key();
    assert!(is_lower_hex(&public, 64), "{public:?}");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&handoff).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let home = tempfile::tempdir().unwrap();
    let desktop = desktop();
    let runner = pair_with_key_made_at_start(home.path(), &desktop, &handoff).unwrap();
    assert_eq!(
        hex_of(runner.key.as_bytes()),
        public,
        "the key it published"
    );
    let (ours, peer) = answering(home.path()).unwrap().expect("paired");
    assert_eq!(
        (ours.key.public(), ours.device),
        (runner.key, runner.device)
    );
    assert_eq!(peer, desktop);
    assert!(!handoff.exists(), "the pod's copy of the key is taken away");
}

#[test]
fn a_cluster_runner_is_paired_once_since_its_pairing_takes_the_key() {
    let (_pod, handoff, _) = pod_made_a_key();
    let home = tempfile::tempdir().unwrap();
    pair_with_key_made_at_start(home.path(), &desktop(), &handoff).unwrap();
    let refused = pair_with_key_made_at_start(home.path(), &desktop(), &handoff).unwrap_err();
    assert!(refused.contains("paired once"), "{refused}");
}

#[test]
fn a_key_made_at_start_is_made_only_where_none_is() {
    let (_pod, handoff, public) = pod_made_a_key();
    let published = handoff.with_file_name("termination-log");
    let refused = make_at_start(&handoff, &published).unwrap_err();
    assert!(refused.contains("could not be made"), "{refused}");
    assert_eq!(std::fs::read_to_string(&published).unwrap(), public);
    // Nor through a link someone put there.
    let pod = tempfile::tempdir().unwrap();
    let elsewhere = pod.path().join("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, pod.path().join("link-key")).unwrap();
    assert!(make_at_start(&pod.path().join("link-key"), &published).is_err());
    assert!(!elsewhere.exists());
}

#[test]
fn a_key_made_at_start_that_others_may_read_is_not_paired_with() {
    use std::os::unix::fs::PermissionsExt;
    let (_pod, handoff, _) = pod_made_a_key();
    std::fs::set_permissions(&handoff, std::fs::Permissions::from_mode(0o644)).unwrap();
    let home = tempfile::tempdir().unwrap();
    let refused = pair_with_key_made_at_start(home.path(), &desktop(), &handoff).unwrap_err();
    assert!(refused.contains("could not be read safely"), "{refused}");
    assert!(answering(home.path()).unwrap().is_none());
}

/// The uid of the upstream claim a test's pod is for.
const CLAIM: &str = "11111111-1111-4111-8111-111111111111";

/// A cluster runner's pod made (again) on `volume`, its home there and the image's home at
/// `image`: its first step, run as [`make_at_start_in`] runs it, with a hand-off folder of its
/// own, in memory as the pod's is. Answers the folder, the hand-off path and what it published.
fn pod_started_on(
    volume: &std::path::Path,
    image: Option<&std::path::Path>,
) -> (tempfile::TempDir, std::path::PathBuf, String) {
    let pod = tempfile::tempdir().unwrap();
    let handoff = pod.path().join("link-key");
    let published = pod.path().join("termination-log");
    let home = volume.join("home");
    make_at_start_in(
        &handoff,
        &published,
        &home,
        image,
        &home.join(".config"),
        CLAIM,
    )
    .unwrap();
    let public = std::fs::read_to_string(&published).unwrap();
    (pod, handoff, public)
}

#[test]
fn a_kept_runner_made_again_keeps_its_pairing_and_publishes_the_key_it_was_paired_with() {
    let volume = tempfile::tempdir().unwrap();
    let config = volume.path().join("home").join(".config");
    let (_first, handoff, public) = pod_started_on(volume.path(), None);
    let desktop = desktop();
    let runner = pair_with_key_made_at_start(&config, &desktop, &handoff).unwrap();
    assert_eq!(hex_of(runner.key.as_bytes()), public);

    // The pod is lost and made again on the same volume: no new key, the same pairing.
    let (_again, handoff, published) = pod_started_on(volume.path(), None);
    assert_eq!(published, public, "the key it was paired with");
    assert!(!handoff.exists(), "nothing is handed over");
    let (ours, peer) = answering(&config).unwrap().expect("still paired");
    assert_eq!(
        (ours.device, ours.key.public()),
        (runner.device, runner.key)
    );
    assert_eq!(peer, desktop);
}

#[test]
fn a_fresh_runners_home_starts_as_a_copy_of_the_images_and_a_kept_one_is_left_as_it_is() {
    use std::os::unix::fs::PermissionsExt;
    let image = tempfile::tempdir().unwrap();
    std::fs::write(image.path().join(".gitconfig"), "[user]\n\tname = runner\n").unwrap();
    std::fs::create_dir(image.path().join(".ssh")).unwrap();
    std::fs::write(
        image.path().join(".ssh/known_hosts"),
        "github.com ssh-ed25519 AAAA\n",
    )
    .unwrap();
    std::os::unix::fs::symlink("/etc/hostname", image.path().join("hostname")).unwrap();
    let volume = tempfile::tempdir().unwrap();
    let home = volume.path().join("home");
    seed_home(&home, Some(image.path()), CLAIM).unwrap();
    assert_eq!(
        std::fs::read_to_string(home.join(".ssh/known_hosts")).unwrap(),
        "github.com ssh-ed25519 AAAA\n"
    );
    assert!(home.join(".gitconfig").is_file());
    assert_eq!(
        std::fs::read_link(home.join("hostname")).unwrap(),
        std::path::Path::new("/etc/hostname"),
        "a link is copied as a link, never followed"
    );
    assert_eq!(
        std::fs::metadata(&home).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // The runner's own work, and a later pod: nothing of the image's is copied over it.
    std::fs::write(home.join("conversation"), "kept").unwrap();
    std::fs::write(image.path().join(".gitconfig"), "changed in the image").unwrap();
    seed_home(&home, Some(image.path()), CLAIM).unwrap();
    assert_eq!(
        std::fs::read_to_string(home.join("conversation")).unwrap(),
        "kept"
    );
    assert_ne!(
        std::fs::read_to_string(home.join(".gitconfig")).unwrap(),
        "changed in the image"
    );
    // No image home: an empty one.
    let bare = volume.path().join("bare");
    seed_home(&bare, None, CLAIM).unwrap();
    assert_eq!(std::fs::read_dir(&bare).unwrap().count(), 0);
}

#[test]
fn a_home_left_half_made_is_made_again_and_one_that_is_not_a_folder_is_refused() {
    let image = tempfile::tempdir().unwrap();
    std::fs::write(image.path().join("dotfile"), "x").unwrap();
    let volume = tempfile::tempdir().unwrap();
    let home = volume.path().join("home");
    std::fs::create_dir(volume.path().join("home.making")).unwrap();
    std::fs::write(volume.path().join("home.making/half"), "stopped").unwrap();
    seed_home(&home, Some(image.path()), CLAIM).unwrap();
    assert!(home.join("dotfile").is_file());
    assert!(!home.join("half").exists());
    assert!(!volume.path().join("home.making").exists());

    let link = volume.path().join("linked");
    std::os::unix::fs::symlink(image.path(), &link).unwrap();
    assert!(seed_home(&link, None, CLAIM).is_err());
}

#[test]
fn a_fresh_runners_home_that_came_with_a_device_id_is_still_never_paired() {
    let image = tempfile::tempdir().unwrap();
    let minted = crate::machine::mint_device_id(&image.path().join(".config")).unwrap();
    assert!(minted.is_some());
    let volume = tempfile::tempdir().unwrap();
    let (_pod, handoff, _) = pod_started_on(volume.path(), Some(image.path()));
    let config = volume.path().join("home").join(".config");
    let refused = pair_with_key_made_at_start(&config, &desktop(), &handoff).unwrap_err();
    assert!(refused.contains("came with a device id"), "{refused}");
}

#[test]
fn a_home_on_a_volume_made_for_another_claim_or_for_none_is_refused() {
    let volume = tempfile::tempdir().unwrap();
    let home = volume.path().join("home");
    seed_home(&home, None, CLAIM).unwrap();
    assert_eq!(
        std::fs::read_to_string(volume.path().join(CLAIM_MARKER)).unwrap(),
        format!("{CLAIM}\n")
    );
    seed_home(&home, None, CLAIM).expect("its own claim's home is taken");
    let refused = seed_home(&home, None, "22222222-2222-4222-8222-222222222222").unwrap_err();
    assert!(refused.contains("not made for this claim"), "{refused}");
    // A volume filled before the claim was made, with a home and no mark.
    let filled = tempfile::tempdir().unwrap();
    std::fs::create_dir(filled.path().join("home")).unwrap();
    std::fs::write(filled.path().join("home/.bashrc"), "planted").unwrap();
    let refused = seed_home(&filled.path().join("home"), None, CLAIM).unwrap_err();
    assert!(refused.contains("not made for this claim"), "{refused}");
}

#[test]
fn an_image_home_that_holds_the_runners_home_is_refused_before_anything_is_copied() {
    let volume = tempfile::tempdir().unwrap();
    let home = volume.path().join("home");
    for image in [std::path::Path::new("/"), volume.path()] {
        let refused = seed_home(&home, Some(image), CLAIM).unwrap_err();
        assert!(refused.contains("runner image contract"), "{refused}");
        assert!(!home.exists());
        assert!(!volume.path().join("home.making").exists());
    }
}

#[test]
fn the_claim_variable_is_the_one_the_operators_template_sets() {
    // operator/src/upstream.rs CLAIM_VAR names the same variable.
    assert_eq!(CLAIM_VAR, "PURLIS_CLAIM_UID");
}

#[test]
fn a_home_made_warm_before_any_claim_is_made_again_for_the_claim_that_took_it() {
    let image = tempfile::tempdir().unwrap();
    std::fs::write(image.path().join(".profile"), "image").unwrap();
    let volume = tempfile::tempdir().unwrap();
    let home = volume.path().join("home");
    // Made warm: no claim yet.
    seed_home(&home, Some(image.path()), "").unwrap();
    std::fs::write(home.join("left"), "by the warm runner").unwrap();
    // Its pod's sandbox made again once a claim took it: the claim makes the home its own.
    seed_home(&home, Some(image.path()), CLAIM).unwrap();
    assert!(!home.join("left").exists());
    assert!(home.join(".profile").is_file());
    assert_eq!(
        std::fs::read_to_string(volume.path().join(CLAIM_MARKER)).unwrap(),
        format!("{CLAIM}\n")
    );
    seed_home(&home, Some(image.path()), CLAIM).expect("then it is its own");
}

#[test]
fn a_claims_mark_is_never_read_or_written_through_a_link() {
    let volume = tempfile::tempdir().unwrap();
    let elsewhere = volume.path().join("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, volume.path().join(CLAIM_MARKER)).unwrap();
    assert!(seed_home(&volume.path().join("home"), None, CLAIM).is_err());
    assert!(!elsewhere.exists(), "nothing written through the link");
    // A home whose mark is a link is refused, whatever the link names.
    std::fs::create_dir(volume.path().join("home")).unwrap();
    std::fs::write(&elsewhere, format!("{CLAIM}\n")).unwrap();
    assert!(seed_home(&volume.path().join("home"), None, CLAIM).is_err());
}

#[test]
fn an_image_home_that_reaches_the_runners_home_through_a_link_or_dots_is_refused() {
    let volume = tempfile::tempdir().unwrap();
    let home = volume.path().join("home");
    let images = tempfile::tempdir().unwrap();
    let linked = images.path().join("home");
    std::os::unix::fs::symlink(volume.path(), &linked).unwrap();
    std::fs::create_dir(volume.path().join("tools")).unwrap();
    for image in [linked, volume.path().join("tools").join("..")] {
        let refused = seed_home(&home, Some(&image), CLAIM).unwrap_err();
        assert!(
            refused.contains("runner image contract"),
            "{image:?}: {refused}"
        );
        assert!(!home.exists());
    }
}

#[test]
fn a_claims_mark_that_is_not_a_plain_file_is_refused_without_waiting_on_it() {
    let volume = tempfile::tempdir().unwrap();
    let marker = volume.path().join(CLAIM_MARKER);
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&marker)).unwrap();
    assert!(made.success());
    // Neither making a home nor taking one waits on a fifo put where the mark belongs.
    assert!(seed_home(&volume.path().join("home"), None, CLAIM).is_err());
    std::fs::create_dir(volume.path().join("home")).unwrap();
    assert!(seed_home(&volume.path().join("home"), None, CLAIM).is_err());
    // Nor is a folder a mark.
    let other = tempfile::tempdir().unwrap();
    std::fs::create_dir(other.path().join(CLAIM_MARKER)).unwrap();
    std::fs::create_dir(other.path().join("home")).unwrap();
    assert!(seed_home(&other.path().join("home"), None, CLAIM).is_err());
}

#[test]
fn a_key_an_earlier_start_of_the_pod_left_unpaired_is_made_again() {
    let volume = tempfile::tempdir().unwrap();
    let (pod, handoff, first) = pod_started_on(volume.path(), None);
    assert!(handoff.exists());
    // The runtime makes the pod's sandbox again; its folder in memory kept the key.
    let published = pod.path().join("termination-log");
    let home = volume.path().join("home");
    make_at_start_in(
        &handoff,
        &published,
        &home,
        None,
        &home.join(".config"),
        CLAIM,
    )
    .expect("the step starts again");
    let again = std::fs::read_to_string(&published).unwrap();
    assert!(is_lower_hex(&again, 64));
    assert_ne!(
        again, first,
        "a new key, handed over in the old one's place"
    );
    let runner = pair_with_key_made_at_start(&home.join(".config"), &desktop(), &handoff).unwrap();
    assert_eq!(hex_of(runner.key.as_bytes()), again);
}
