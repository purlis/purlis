//! What a paired device does reaches the event log as metadata (iOS design §4.4): which device,
//! which chat, which ask and choice ids, how many bytes a prompt was. Never the text (D9).

use purlis_core::eventlog::{self, Recorder};
use purlis_core::remote::audit::{self, Act};

const DEVICE: &str = "01J9PHONE00000000000000000";
const CHAT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";

fn recorder(data: &std::path::Path, config: &std::path::Path) -> Recorder {
    Recorder::open_in(config, data).unwrap()
}

#[test]
fn each_act_is_its_kind_with_the_devices_id_and_fingerprint() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut rec = recorder(data.path(), config.path());
    let key = [5u8; 32];
    let fp = purlis_core::remote::fingerprint(&key);
    let paired = audit::record(
        &mut rec,
        Act::Paired {
            device: DEVICE.into(),
            key,
        },
    )
    .unwrap();
    assert_eq!(paired.kind, "remote.paired");
    assert_eq!(paired.body["device"], DEVICE);
    assert_eq!(paired.body["fingerprint"], fp);
    assert_eq!(paired.body["scope"], "remote-device");
    assert_eq!(paired.chat, None);
    let answered = audit::record(
        &mut rec,
        Act::Answered {
            device: DEVICE.into(),
            key,
            chat: CHAT.into(),
            ask: "ask-1".into(),
            choice: "allow".into(),
        },
    )
    .unwrap();
    assert_eq!(answered.kind, "remote.answered");
    assert_eq!(answered.chat.as_deref(), Some(CHAT));
    assert_eq!(answered.body["ask"], "ask-1");
    assert_eq!(answered.body["choice"], "allow");
    let prompted = audit::record(
        &mut rec,
        Act::Prompted {
            device: DEVICE.into(),
            key,
            chat: CHAT.into(),
            bytes: 27,
        },
    )
    .unwrap();
    assert_eq!(prompted.kind, "remote.prompted");
    assert_eq!(prompted.body["bytes"], 27);
    let refused = audit::record(&mut rec, Act::Refused { key }).unwrap();
    assert_eq!(refused.kind, "remote.refused");
    assert_eq!(refused.body["fingerprint"], fp);
    assert!(refused.body["device"].is_null());
    let linked = audit::record(
        &mut rec,
        Act::Linked {
            device: Some(DEVICE.into()),
            key,
        },
    )
    .unwrap();
    assert_eq!(linked.kind, "remote.linked");
    let unpaired = audit::record(
        &mut rec,
        Act::Unpaired {
            device: DEVICE.into(),
            key,
        },
    )
    .unwrap();
    assert_eq!(unpaired.kind, "remote.unpaired");
}

#[test]
fn no_act_can_carry_text_and_the_log_holds_none() {
    // The enum has no field for a prompt's, message's or answer's text: `Prompted` takes a
    // length. This test pins what reaches the disk.
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut rec = recorder(data.path(), config.path());
    let key = [6u8; 32];
    let text = "also add a test for expiry";
    audit::record(
        &mut rec,
        Act::Prompted {
            device: DEVICE.into(),
            key,
            chat: CHAT.into(),
            bytes: text.len(),
        },
    )
    .unwrap();
    let written = eventlog::read(rec.dir()).unwrap();
    let last = serde_json::to_string(&written.last().unwrap().body).unwrap();
    assert!(!last.contains("expiry"), "{last}");
    assert_eq!(written.last().unwrap().body["bytes"], text.len());
    for kind in audit::KINDS {
        assert!(kind.starts_with("remote."), "{kind}");
    }
}
