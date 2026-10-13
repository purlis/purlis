//! `machine.json` keeps the gateway's switch, port and addresses and the keep-awake setting
//! (iOS design §4.1, §6.4): off by default, written through `update`, read back the same, and a
//! file from before them reads as the defaults.

use purlis_core::machine::{self, Gateway};

#[test]
fn the_defaults_are_off_7777_and_no_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = machine::read(dir.path());
    assert_eq!(loaded.store.gateway, Gateway::default());
    assert!(!loaded.store.gateway.on);
    assert_eq!(loaded.store.gateway.port, 7777);
    assert!(loaded.store.gateway.addresses.is_empty());
    assert!(!loaded.store.keep_awake);
}

#[test]
fn the_keys_round_trip_through_update() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |store| {
        store.gateway = Gateway {
            on: true,
            port: 7800,
            addresses: vec!["100.64.0.9".into()],
        };
        store.keep_awake = true;
    })
    .unwrap();
    let loaded = machine::read(dir.path());
    assert!(loaded.store.gateway.on);
    assert_eq!(loaded.store.gateway.port, 7800);
    assert_eq!(loaded.store.gateway.addresses, vec!["100.64.0.9"]);
    assert!(loaded.store.keep_awake);
    let text = std::fs::read_to_string(machine::file(dir.path())).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["gateway"]["on"], true);
    assert_eq!(json["gateway"]["port"], 7800);
    assert_eq!(json["keep_awake"], true);
}

#[test]
fn the_defaults_are_left_out_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |_| {}).unwrap();
    let text = std::fs::read_to_string(machine::file(dir.path())).unwrap();
    assert!(!text.contains("gateway"), "{text}");
    assert!(!text.contains("keep_awake"), "{text}");
}

#[test]
fn a_file_without_the_keys_reads_as_the_defaults_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |store| store.keep_awake = true).unwrap();
    let path = machine::file(dir.path());
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("gateway");
    json.as_object_mut().unwrap().remove("keep_awake");
    std::fs::write(&path, json.to_string()).unwrap();
    let loaded = machine::read(dir.path());
    assert_eq!(loaded.store.gateway, Gateway::default());
    assert!(!loaded.store.keep_awake);
    assert!(loaded.dropped.is_empty(), "{:?}", loaded.dropped);
}

#[test]
fn a_gateway_key_of_the_wrong_shape_is_dropped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |_| {}).unwrap();
    let path = machine::file(dir.path());
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json["gateway"] = serde_json::json!("yes please");
    std::fs::write(&path, json.to_string()).unwrap();
    let loaded = machine::read(dir.path());
    assert_eq!(loaded.store.gateway, Gateway::default());
    assert!(!loaded.dropped.is_empty(), "the bad key is reported");
}
