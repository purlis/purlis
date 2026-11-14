// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How the cluster operator key is confirmed before it is pinned (#4495).

use super::*;
use crate::clusterpool::{Confirmed, OperatorKey, PoolPin};

/// The cluster a test's Pools are in, by its `kube-system` namespace's uid.
const CLUSTER: &str = "5f0c1d2e-3a4b-4c5d-8e6f-7a8b9c0d1e2f";

fn a_key(byte: u8) -> OperatorKey {
    OperatorKey::of_public([byte; 32])
}

/// What a Pool in `cluster` naming `key` is pinned as.
fn a_pin(key: &OperatorKey, cluster: &str) -> PoolPin {
    PoolPin {
        key: key.clone(),
        cluster: cluster.to_owned(),
    }
}

fn made_on(config: &Path, context: &str, cluster: &str, key: &OperatorKey) {
    let _locked = runners::lock(config);
    let mut choices = read_choices(config).unwrap();
    choices.made = Some(MadeKey {
        context: context.to_owned(),
        cluster: cluster.to_owned(),
        key: key.spelled(),
    });
    write_choices(config, choices).unwrap();
}

/// **The paste is asked for on every path** but where this machine made the signing key in this
/// setup: a key pasted must be the Pool's, however it was copied, and with none pasted only the
/// key made here, on the same context, is pinned.
#[test]
fn the_key_is_confirmed_by_a_paste_unless_this_setup_made_it() {
    let config = tempfile::tempdir().unwrap();
    let key = a_key(0xab);
    let named = a_pin(&key, CLUSTER);
    let pasted = key.spelled().trim_start_matches("ed25519:").to_uppercase();
    let wrapped = format!("{}\n{}", &pasted[..32], &pasted[32..]);
    assert_eq!(
        confirm_key(config.path(), "team", &named, Some(&wrapped)).unwrap(),
        Confirmed::Pasted
    );
    let why = confirm_key(config.path(), "team", &named, Some(&a_key(1).spelled())).unwrap_err();
    assert!(why.to_string().contains("isn't the one"), "{why}");
    for nothing in [None, Some(""), Some("  \n")] {
        let why = confirm_key(config.path(), "team", &named, nothing).unwrap_err();
        assert!(why.to_string().contains("paste the key"), "{why}");
    }

    // A key made here confirms only itself, only on the context it was made on, and only in the
    // cluster it was made in: a context renamed or pointed at another cluster confirms nothing.
    made_on(config.path(), "team", CLUSTER, &a_key(2));
    assert!(confirm_key(config.path(), "team", &named, None).is_err());
    made_on(config.path(), "other", CLUSTER, &key);
    assert!(confirm_key(config.path(), "team", &named, None).is_err());
    made_on(config.path(), "team", "another-cluster-uid", &key);
    assert!(confirm_key(config.path(), "team", &named, None).is_err());
    made_on(config.path(), "team", "", &key);
    assert!(confirm_key(config.path(), "team", &named, None).is_err());
    made_on(config.path(), "team", CLUSTER, &key);
    assert_eq!(
        confirm_key(config.path(), "team", &named, None).unwrap(),
        Confirmed::MadeHere
    );
    // A paste still wins, and still must match.
    assert!(confirm_key(config.path(), "team", &named, Some(&a_key(1).spelled())).is_err());
}

/// **A key made in one setup confirms nothing in the next** (#4495): starting a new setup
/// forgets it, as it forgets the rest this setup chose.
#[test]
fn starting_over_forgets_the_key_this_setup_made() {
    let config = tempfile::tempdir().unwrap();
    choose_cluster(config.path(), "team").unwrap();
    made_on(config.path(), "team", CLUSTER, &a_key(3));
    assert!(made_here(config.path(), "team", &a_pin(&a_key(3), CLUSTER)));
    start_over(config.path()).unwrap();
    assert!(!made_here(
        config.path(),
        "team",
        &a_pin(&a_key(3), CLUSTER)
    ));
}

/// A stand-in kubectl where no signing key is there yet, so Install's Signing key makes one, in
/// the cluster whose `kube-system` namespace is [`CLUSTER`].
#[cfg(unix)]
fn a_cluster_without_a_key(dir: &Path) -> crate::clusterpool::Kubectl {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("kubectl");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ncat > /dev/null\ncase \"$*\" in *kube-system*) \
             printf '{{\"metadata\":{{\"uid\":\"{CLUSTER}\"}}}}' ;; esac\nexit 0\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    crate::clusterpool::Kubectl::at(script)
}

/// **Install's Signing key keeps the public half of the key it made** (#4495), on the context
/// chosen, and a key that was there keeps nothing.
#[cfg(unix)]
#[test]
fn making_the_signing_key_keeps_its_public_half_for_this_setup() {
    let (config, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    choose_cluster(config.path(), "team").unwrap();
    install_part(
        config.path(),
        &a_cluster_without_a_key(dir.path()),
        install::PartId::SigningKey,
    )
    .unwrap();
    let made = read_choices(config.path()).unwrap().made.expect("kept");
    assert_eq!(made.context, "team");
    assert_eq!(made.cluster, CLUSTER);
    let key = OperatorKey::from_spelled(&made.key).unwrap();
    assert!(made_here(config.path(), "team", &a_pin(&key, CLUSTER)));
    assert!(!made_here(
        config.path(),
        "team",
        &a_pin(&key, "another-cluster-uid")
    ));
    // Only the public half: nothing in the file is a private key.
    let text =
        std::fs::read_to_string(crate::machine::dir(config.path()).join(CHOICES_FILE)).unwrap();
    assert!(!text.contains("PRIVATE"), "{text}");
}
