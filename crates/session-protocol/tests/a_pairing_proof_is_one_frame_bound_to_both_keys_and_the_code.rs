//! The pairing proof (iOS design §4.2): one frame inside the encrypted channel, an HMAC under
//! the one-time code over both static keys and the host's device id, so a proof recorded on
//! one link proves nothing on another.

use purlis_session_protocol::noise::Identity;
use purlis_session_protocol::pairing::{self, Code, Proof, Verdict};
use tokio::io::duplex;

const HOST: &str = "01J9DEVCE00000000000000000";
const PHONE: &str = "01J9PHONE00000000000000000";

#[test]
fn a_proof_holds_only_for_its_code_keys_and_host() {
    let code = Code::mint();
    let host = Identity::generate();
    let phone = Identity::generate();
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: pairing::prove(&code, &phone.public(), &host.public(), HOST),
    };
    assert!(pairing::verify(
        &code,
        &proof,
        &phone.public(),
        &host.public(),
        HOST
    ));
    assert!(!pairing::verify(
        &Code::mint(),
        &proof,
        &phone.public(),
        &host.public(),
        HOST
    ));
    assert!(!pairing::verify(
        &code,
        &proof,
        &Identity::generate().public(),
        &host.public(),
        HOST
    ));
    assert!(!pairing::verify(
        &code,
        &proof,
        &phone.public(),
        &Identity::generate().public(),
        HOST
    ));
    assert!(!pairing::verify(
        &code,
        &proof,
        &phone.public(),
        &host.public(),
        "01J9OTHER0000000000000000"
    ));
}

#[tokio::test]
async fn the_proof_and_the_verdict_cross_as_one_frame_each() {
    let (mut a, mut b) = duplex(4096);
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: "ab".repeat(32),
    };
    pairing::send_proof(&mut a, &proof).await.unwrap();
    let (read, left) = pairing::read_proof(&mut b).await.unwrap();
    assert_eq!(read, proof);
    assert!(left.is_empty(), "nothing followed the proof");
    pairing::send_verdict(
        &mut b,
        &Verdict::Paired {
            device: HOST.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        pairing::read_verdict(&mut a).await.unwrap(),
        Verdict::Paired {
            device: HOST.into()
        }
    );
}

#[tokio::test]
async fn a_frame_that_is_not_a_proof_is_refused_not_read_as_one() {
    use tokio::io::AsyncWriteExt;
    let (mut a, mut b) = duplex(4096);
    a.write_all(b"\x89CSP\r\n\x1a\n\x00\x0f{\"call\":{\"id\":1}}")
        .await
        .unwrap();
    assert!(pairing::read_proof(&mut b).await.is_err());
}

#[test]
fn a_name_longer_than_the_bound_or_a_device_that_is_not_a_ulid_is_refused() {
    let long = Proof {
        device: PHONE.into(),
        name: "x".repeat(65),
        proof: String::new(),
    };
    assert!(long.check().is_err());
    let odd = Proof {
        device: "phone".into(),
        name: "ok".into(),
        proof: String::new(),
    };
    assert!(odd.check().is_err());
    let fine = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: String::new(),
    };
    assert!(fine.check().is_ok());
}

#[test]
fn a_code_prints_nothing_of_itself_and_round_trips_hex() {
    let code = Code::mint();
    assert_eq!(format!("{code:?}"), "Code(…)");
    assert_eq!(Code::from_hex(&code.to_hex()).unwrap(), code);
    assert!(Code::from_hex("zz").is_err());
}
