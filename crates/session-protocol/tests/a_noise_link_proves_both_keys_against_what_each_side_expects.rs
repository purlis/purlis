//! Noise `XX` over any ordered byte stream (ADR 0078 §3): each side learns the other's static
//! key inside the handshake, the client aborts when the host's key is not the one it pinned,
//! the prologue binds the host's device id, and what crosses the wire is never the plaintext.

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::noise::{self, Error, Identity};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

const HOST: &str = "01J9DEVCE00000000000000000";

#[tokio::test]
async fn each_side_learns_the_others_static_key() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let served = served.unwrap();
    let connected = connected.unwrap();
    assert_eq!(served.remote_public(), phone.public());
    assert_eq!(connected.remote_public(), host.public());
}

#[tokio::test]
async fn the_client_refuses_a_host_whose_key_is_not_the_one_it_pinned() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let other = Identity::generate();
    let phone = Identity::generate();
    let (_served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(other.public()))
    );
    match connected {
        Err(Error::WrongHost { expected, offered }) => {
            assert_eq!(expected, other.public());
            assert_eq!(offered, host.public());
        }
        Err(other) => panic!("refused for another reason: {other}"),
        Ok(_) => panic!("a host with another key was accepted"),
    }
}

#[tokio::test]
async fn a_different_host_device_id_in_the_prologue_fails_the_handshake() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, "01J9OTHER0000000000000000", Some(host.public()))
    );
    assert!(
        served.is_err() || connected.is_err(),
        "the prologue did not bind the device"
    );
}

#[tokio::test]
async fn plaintext_written_on_one_end_is_read_on_the_other() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let mut host_plain = noise::pump(served.unwrap());
    let mut phone_plain = noise::pump(connected.unwrap());
    let secret = b"the quick brown fox jumps over the lazy dog, 70000 times over";
    let big: Vec<u8> = secret.iter().cycle().take(70_000).copied().collect();
    host_plain.write_all(&big).await.unwrap();
    host_plain.flush().await.unwrap();
    let mut got = vec![0u8; big.len()];
    phone_plain.read_exact(&mut got).await.unwrap();
    assert_eq!(got, big);
}

#[tokio::test]
async fn the_bytes_on_the_wire_are_not_the_plaintext() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    // The phone's side gives its wire back raw: what the host writes is read as ciphertext.
    let mut raw_end = connected.unwrap().into_io();
    let mut host_plain = noise::pump(served.unwrap());
    host_plain
        .write_all(b"PINEAPPLE PINEAPPLE PINEAPPLE")
        .await
        .unwrap();
    host_plain.flush().await.unwrap();
    let mut raw = vec![0u8; 2 + 29 + 16];
    raw_end.read_exact(&mut raw).await.unwrap();
    assert!(
        !raw.windows(9).any(|w| w == b"PINEAPPLE"),
        "the plaintext is on the wire"
    );
}

#[test]
fn an_identity_never_prints_its_private_half() {
    let id = Identity::generate();
    let shown = format!("{id:?}");
    assert!(!shown.contains(&id.private_hex()), "{shown}");
    assert!(shown.contains(&noise::fingerprint(&id.public())), "{shown}");
}

#[test]
fn a_fingerprint_is_sixteen_hex_digits_of_the_keys_hash() {
    let id = Identity::generate();
    let fp = noise::fingerprint(&id.public());
    assert_eq!(fp.len(), 16);
    assert!(fp.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(fp, noise::fingerprint(&Identity::generate().public()));
}

#[test]
fn an_identity_read_back_from_its_private_half_has_the_same_public_half() {
    let id = Identity::generate();
    let mut private = [0u8; 32];
    for (i, pair) in id.private_hex().as_bytes().chunks(2).enumerate() {
        private[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    assert_eq!(
        Identity::from_private(private).unwrap().public(),
        id.public()
    );
}

#[tokio::test]
async fn a_secured_stream_proven_as_a_scope_is_that_scope_with_that_key() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, _connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let (device, _stream) = served.unwrap().proven_as(Scope::RemoteDevice);
    assert_eq!(device.scope(), Scope::RemoteDevice);
    assert_eq!(device.key(), phone.public());
}
