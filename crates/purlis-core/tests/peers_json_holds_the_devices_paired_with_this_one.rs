//! `<config>/peers.json` (ADR 0078 §8, iOS design §4.1): the devices paired with this one,
//! each with its pinned key, written whole and privately, read back exactly, and never taken
//! as empty when it cannot be read.

use purlis_core::remote::peers::{self, Peer};

const PHONE: &str = "01J9PHONE00000000000000000";
const OTHER: &str = "01J9OTHER00000000000000000";

fn peer(device: &str, key: u8) -> Peer {
    Peer {
        device: device.into(),
        name: format!("phone {key}"),
        key: [key; 32],
        paired: 1_760_000_000,
    }
}

#[test]
fn no_file_is_no_peers() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
}

#[test]
fn a_peer_added_is_read_back_and_its_key_is_pinned() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer(PHONE, 1)).unwrap();
    let read = peers::read(dir.path()).unwrap();
    assert_eq!(read, vec![peer(PHONE, 1)]);
    assert_eq!(
        peers::scope_of(dir.path(), &[1u8; 32]),
        Some("remote-device")
    );
    assert_eq!(peers::scope_of(dir.path(), &[2u8; 32]), None);
}

#[test]
fn the_file_is_private_and_versioned_json() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer(PHONE, 1)).unwrap();
    let path = peers::path(dir.path());
    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["peers"][0]["key"], "01".repeat(32));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn a_device_paired_again_replaces_its_record_and_a_key_under_another_device_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer(PHONE, 1)).unwrap();
    let mut renamed = peer(PHONE, 3);
    renamed.name = "the same phone, new key".into();
    peers::add(dir.path(), renamed.clone()).unwrap();
    assert_eq!(peers::read(dir.path()).unwrap(), vec![renamed]);
    let refused = peers::add(dir.path(), peer(OTHER, 3)).unwrap_err();
    assert!(refused.contains("already pinned"), "{refused}");
}

#[test]
fn removing_a_peer_unpins_its_key() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer(PHONE, 1)).unwrap();
    assert_eq!(
        peers::remove(dir.path(), PHONE).unwrap(),
        Some(peer(PHONE, 1))
    );
    assert_eq!(peers::remove(dir.path(), PHONE).unwrap(), None);
    assert_eq!(peers::scope_of(dir.path(), &[1u8; 32]), None);
}

#[test]
fn an_unreadable_file_is_an_error_never_an_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer(PHONE, 1)).unwrap();
    std::fs::write(peers::path(dir.path()), "{ torn").unwrap();
    assert!(peers::read(dir.path()).is_err());
    assert_eq!(
        peers::scope_of(dir.path(), &[1u8; 32]),
        None,
        "a torn file pins nothing"
    );
}
