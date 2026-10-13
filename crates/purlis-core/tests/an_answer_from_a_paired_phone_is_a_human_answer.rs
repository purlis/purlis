//! A paired phone answers an ask as a human scope, `remote-device` (iOS design §4.3), so the
//! ask closes with "who answered" naming it, as the desktop's and `charter inbox`'s do.

use purlis_core::harness::asks::{Admitted, Answerer, HumanScope};

#[test]
fn remote_device_is_a_human_answerer_and_says_so() {
    let by = Answerer::admitted(Admitted::RemoteDevice).expect("a human scope");
    assert_eq!(by.scope(), HumanScope::RemoteDevice);
    assert_eq!(serde_json::to_value(by.scope()).unwrap(), "remote-device");
}

#[test]
fn the_other_scopes_are_as_they_were() {
    assert!(Answerer::admitted(Admitted::LocalUi).is_some());
    assert!(Answerer::admitted(Admitted::Approval).is_some());
    for not in [
        Admitted::Terminal,
        Admitted::FleetMcp,
        Admitted::Editor,
        Admitted::Chat,
    ] {
        assert!(Answerer::admitted(not).is_none(), "{not:?}");
    }
}
