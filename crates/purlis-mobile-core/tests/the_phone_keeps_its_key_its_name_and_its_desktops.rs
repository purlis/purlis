//! What the phone keeps (iOS design §4.1): its link key, minted once and read back the same;
//! its device id and name; the desktops it paired with, keyed by device id; and a file store
//! whose failed write leaves the old record.

use purlis_mobile_core::hosts::{self, Host};
use purlis_mobile_core::identity::{self, DEFAULT_NAME};
use purlis_mobile_core::store::{FileStore, MemoryStore, Store};

const MAC: &str = "01J9DEVCE00000000000000000";
const LAPTOP: &str = "01J9LAPTOP0000000000000000";

fn a_host(device: &str, name: &str) -> Host {
    Host {
        device: device.into(),
        name: name.into(),
        key: [7u8; 32],
        addresses: vec!["192.168.9.155:7777".into()],
        paired: 1,
    }
}

#[test]
fn the_link_key_is_minted_once_and_read_back_the_same_until_forgotten() {
    let store = MemoryStore::default();
    let first = identity::link_key(&store).unwrap();
    let again = identity::link_key(&store).unwrap();
    assert_eq!(first.public(), again.public(), "one key, read back");
    identity::forget_link_key(&store).unwrap();
    let fresh = identity::link_key(&store).unwrap();
    assert_ne!(fresh.public(), first.public(), "forgotten, so minted anew");
}

#[test]
fn a_stored_record_that_is_not_a_key_is_refused_not_written_over() {
    let store = MemoryStore::default();
    store.set(identity::LINK_KEY, "not a key").unwrap();
    assert!(identity::link_key(&store).is_err());
    assert_eq!(
        store.get(identity::LINK_KEY).unwrap().as_deref(),
        Some("not a key")
    );
}

#[test]
fn the_device_has_a_ulid_and_keeps_its_name_until_renamed() {
    let store = MemoryStore::default();
    let device = identity::device(&store, None).unwrap();
    assert_eq!(device.id.len(), 26);
    assert_eq!(device.name, DEFAULT_NAME);
    let same = identity::device(&store, None).unwrap();
    assert_eq!(same, device, "minted once");
    let renamed = identity::rename(&store, " Noro's iPhone ").unwrap();
    assert_eq!(renamed.id, device.id);
    assert_eq!(renamed.name, "Noro's iPhone", "trimmed");
    assert_eq!(identity::device(&store, None).unwrap(), renamed);
    // A name the desktop would refuse after spending the code is refused here first.
    assert!(identity::rename(&store, "  ").is_err());
    assert!(identity::rename(&store, &"x".repeat(65)).is_err());
    assert_eq!(identity::device(&store, None).unwrap(), renamed, "kept");
}

#[test]
fn desktops_are_kept_by_device_id_replaced_renamed_readdressed_and_unpaired() {
    let store = MemoryStore::default();
    assert!(hosts::read(&store).unwrap().is_empty());
    hosts::add(&store, a_host(MAC, "Mac")).unwrap();
    hosts::add(&store, a_host(LAPTOP, "Laptop")).unwrap();
    // Paired again: the record is replaced, not doubled.
    hosts::add(
        &store,
        Host {
            key: [9u8; 32],
            ..a_host(MAC, "Mac")
        },
    )
    .unwrap();
    let kept = hosts::read(&store).unwrap();
    assert_eq!(kept.len(), 2);
    assert_eq!(hosts::find(&store, MAC).unwrap().unwrap().key, [9u8; 32]);

    hosts::rename(&store, MAC, "Desk Mac").unwrap();
    hosts::set_addresses(&store, MAC, vec!["100.64.0.9:7777".into()]).unwrap();
    let mac = hosts::find(&store, MAC).unwrap().unwrap();
    assert_eq!(mac.name, "Desk Mac");
    assert_eq!(mac.addresses, vec!["100.64.0.9:7777".to_owned()]);
    assert!(hosts::rename(&store, "01J9NOBODY0000000000000000", "x").is_err());

    assert_eq!(
        hosts::remove(&store, LAPTOP).unwrap().map(|h| h.name),
        Some("Laptop".into())
    );
    assert!(hosts::remove(&store, LAPTOP).unwrap().is_none());
    assert_eq!(hosts::read(&store).unwrap().len(), 1);
    assert!(hosts::find(&store, LAPTOP).unwrap().is_none());
}

#[test]
fn the_file_store_keeps_each_record_private_and_round_trips_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::new(dir.path().join("purlis"));
    assert_eq!(store.get("hosts").unwrap(), None);
    store.set("hosts", "[]").unwrap();
    assert_eq!(store.get("hosts").unwrap().as_deref(), Some("[]"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("purlis/hosts.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "private to the app");
    }
    store.delete("hosts").unwrap();
    assert_eq!(store.get("hosts").unwrap(), None);
    store.delete("hosts").unwrap();
    assert!(
        store.set("Not A Name", "x").is_err(),
        "a record's name is a short word"
    );
}

#[cfg(unix)]
#[test]
fn a_write_that_fails_leaves_the_old_record() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("purlis");
    let store = FileStore::new(&home);
    store.set("device", "{\"id\":\"one\"}").unwrap();
    // The directory can no longer be written: the temporary file cannot be made.
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o500)).unwrap();
    let refused = store.set("device", "{\"id\":\"two\"}");
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(refused.is_err());
    assert_eq!(
        store.get("device").unwrap().as_deref(),
        Some("{\"id\":\"one\"}")
    );
}
