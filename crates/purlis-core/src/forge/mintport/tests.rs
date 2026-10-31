// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner's way to the mint port (#4207): its proof against the worked example the operator also
//! holds, a stand-in port over real TLS held to a key, and following a roll by a word the key held
//! signed.

use super::*;
use purlis_session_protocol::hostlink::{LinkKey, Pinned};
use std::net::TcpListener;

fn fixture(dir: &str, name: &str) -> String {
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(dir)
        .join(name);
    std::fs::read_to_string(at).unwrap()
}

fn proof_fixture(name: &str) -> String {
    fixture("forge-proof", name)
}

fn roll_fixture(name: &str) -> String {
    fixture("runner-link-key", name).trim_end().to_owned()
}

fn bytes(hex: &str) -> [u8; 32] {
    crate::runnerlink::bytes_of(hex.trim()).unwrap()
}

#[test]
fn a_proof_is_the_worked_example_the_operator_holds_too() {
    let text = proof_fixture("text.txt");
    let (link, challenge) = (
        bytes(&proof_fixture("link-key.hex")),
        bytes(&proof_fixture("challenge.hex")),
    );
    assert_eq!(
        proof_for(&link, &challenge, &text).unwrap(),
        proof_fixture("proof.hex").trim()
    );
    assert_eq!(
        text,
        proof_text(
            &format!("ed25519:{}", "ab".repeat(32)),
            "team",
            "ada-1",
            "Iv23liExample",
            proof_fixture("challenge.hex").trim(),
        )
    );
}

#[test]
fn a_low_order_challenge_agrees_on_nothing_so_nothing_is_proved() {
    assert_eq!(proof_for(&[7u8; 32], &[0u8; 32], "text"), None);
}

/// The test-only signing key of the worked example, and its next.
fn signer(name: &str) -> Arc<rustls::sign::CertifiedKey> {
    let hex = fixture("runner-link-key", name);
    let hex = hex.trim();
    let der: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect();
    let private = rustls::pki_types::PrivatePkcs8KeyDer::from(der);
    let signing = rustls::crypto::ring::sign::any_eddsa_type(&private).unwrap();
    // A client held to a key never reads these bytes: its signature decides.
    Arc::new(rustls::sign::CertifiedKey::new(
        vec![CertificateDer::from(vec![0x30, 0x00])],
        signing,
    ))
}

#[derive(Debug)]
struct One(Arc<rustls::sign::CertifiedKey>);

impl rustls::server::ResolvesServerCert for One {
    fn resolve(
        &self,
        _hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        Some(self.0.clone())
    }
}

/// A stand-in for the port, signing with the worked example's `key`: each connection's request and
/// its answer by `answers`, on a thread, for as long as it is listened to.
fn a_port(
    key: &str,
    answers: impl Fn(&str, &[u8]) -> String + Send + Sync + 'static,
) -> (String, Arc<Mutex<Vec<String>>>) {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = Arc::new(
        rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(One(signer(key)))),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let heard = seen.clone();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { return };
            let connection = rustls::ServerConnection::new(config.clone()).unwrap();
            let mut stream = rustls::StreamOwned::new(connection, socket);
            let mut read = Vec::new();
            let mut buffer = [0u8; 2048];
            let (path, body) = loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break (String::new(), Vec::new()),
                    Ok(n) => read.extend_from_slice(&buffer[..n]),
                }
                let Some(ends) = read.windows(4).position(|end| end == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&read[..ends]).into_owned();
                let length: usize = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .and_then(|told| told.parse().ok())
                    .unwrap_or(0);
                if read.len() >= ends + 4 + length {
                    let path = head.split(' ').nth(1).unwrap_or_default().to_owned();
                    break (path, read[ends + 4..ends + 4 + length].to_vec());
                }
            };
            if path.is_empty() {
                continue;
            }
            heard.lock().unwrap().push(path.clone());
            let body = answers(&path, &body);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.conn.send_close_notify();
            let _ = stream.flush();
        }
    });
    (address, seen)
}

/// A home paired with a desktop, told of the port at `address`, held to `keys`: and its link key.
fn a_runner(address: &str, keys: &[String]) -> (tempfile::TempDir, LinkKey) {
    let home = tempfile::tempdir().unwrap();
    let config = home.path();
    let desktop = LinkKey::generate().unwrap();
    crate::runnerlink::pair(
        config,
        &Pinned {
            device: "01DESKTOP".to_owned(),
            key: desktop.public(),
        },
    )
    .unwrap();
    tell(
        config,
        Told {
            namespace: "team".into(),
            claim: "ada-1".into(),
            cluster: roll_fixture("cluster.txt"),
            address: address.to_owned(),
            keys: keys.to_vec(),
        },
    )
    .unwrap();
    let (ours, _) = crate::runnerlink::answering(config).unwrap().unwrap();
    (home, ours.key)
}

const CHALLENGE: [u8; 32] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
];

/// The port's public half for [`CHALLENGE`] as a private key.
fn challenge_public() -> [u8; 32] {
    curve25519_dalek::constants::X25519_BASEPOINT
        .mul_clamped(CHALLENGE)
        .to_bytes()
}

#[test]
fn a_mint_proves_the_link_key_to_a_port_holding_the_key_told_and_keeps_nothing_but_memory() {
    let operator = roll_fixture("operator-key.txt");
    let link: Arc<Mutex<Option<[u8; 32]>>> = Arc::default();
    let proofs_ok: Arc<Mutex<bool>> = Arc::default();
    let (held_link, held_ok, signed_by) = (link.clone(), proofs_ok.clone(), operator.clone());
    let (address, seen) = a_port(
        "test-only-signing-key.pkcs8.hex",
        move |path, body| match path {
            "/forge/challenge" => format!(
                "{{\"challenge\":\"{}\"}}",
                crate::runnerlink::hex_of(&challenge_public())
            ),
            "/forge/next-key" => "{}".to_owned(),
            "/forge/token" => {
                let asked: serde_json::Value = serde_json::from_slice(body).unwrap();
                let key = held_link
                    .lock()
                    .unwrap()
                    .expect("the link key's public half");
                let shared = curve25519_dalek::montgomery::MontgomeryPoint(key)
                    .mul_clamped(CHALLENGE)
                    .to_bytes();
                let text = proof_text(
                    &signed_by,
                    "team",
                    "ada-1",
                    "Iv23",
                    &crate::runnerlink::hex_of(&challenge_public()),
                );
                *held_ok.lock().unwrap() = proof_of_agreement(&shared, &challenge_public(), &text)
                    .as_deref()
                    == asked["proof"].as_str()
                    && asked["namespace"] == "team"
                    && asked["claim"] == "ada-1";
                "{\"token\":\"minted\",\"expires_at\":\"2099-01-01T00:00:00Z\",\"owner\":\"acme\",\"repos\":[\"app\"]}".to_owned()
            }
            _ => "{}".to_owned(),
        },
    );
    let (home, key) = a_runner(&address, std::slice::from_ref(&operator));
    *link.lock().unwrap() = Some(*key.public().as_bytes());
    let minted = mint(home.path(), "Iv23").unwrap().expect("a port was told");
    assert_eq!(*minted.token, "minted");
    assert_eq!(minted.repos, ["app"]);
    assert_eq!(minted.operator, operator);
    assert!(*proofs_ok.lock().unwrap(), "the port's proof check failed");
    assert_eq!(
        *seen.lock().unwrap(),
        ["/forge/next-key", "/forge/challenge", "/forge/token"]
    );
    let kept = std::fs::read_to_string(crate::machine::charterd(home.path()).join(FILE)).unwrap();
    assert!(!kept.contains("minted"), "{kept}");
}

#[test]
fn a_port_showing_another_key_is_asked_nothing() {
    let (address, seen) = a_port("test-only-next-signing-key.pkcs8.hex", |_, _| {
        "{}".to_owned()
    });
    let (home, _) = a_runner(&address, &[roll_fixture("operator-key.txt")]);
    let refused = mint(home.path(), "Iv23").err().expect("refused");
    assert!(refused.contains("not the operator"), "{refused}");
    assert!(
        seen.lock().unwrap().is_empty(),
        "the port was told something"
    );
}

#[test]
fn a_runner_told_of_no_port_asks_nothing() {
    let home = tempfile::tempdir().unwrap();
    assert!(mint(home.path(), "Iv23").unwrap().is_none());
    assert!(told(home.path()).unwrap().is_none());
    assert_eq!(follow(home.path(), 1), Ok(()));
}

#[test]
fn what_a_runner_is_told_is_read_whole_or_refused() {
    let good = Told {
        namespace: "team".into(),
        claim: "ada-1".into(),
        cluster: roll_fixture("cluster.txt"),
        address: "10.96.0.5:8443".into(),
        keys: vec![roll_fixture("operator-key.txt")],
    };
    assert!(good.clone().checked().is_ok());
    let bad = [
        Told {
            namespace: "Team/..".into(),
            ..good.clone()
        },
        Told {
            claim: "".into(),
            ..good.clone()
        },
        Told {
            cluster: "not-a-uid".into(),
            ..good.clone()
        },
        // A name would be resolved by the public name servers a runner is given.
        Told {
            address: "operator.example.com:8443".into(),
            ..good.clone()
        },
        Told {
            keys: vec![],
            ..good.clone()
        },
        Told {
            keys: vec!["sha256:abc".into()],
            ..good.clone()
        },
        Told {
            keys: vec![good.keys[0].clone(); 3],
            ..good.clone()
        },
    ];
    for told in bad {
        assert!(told.clone().checked().is_err(), "{told:?}");
    }
}

fn told_for_a_roll() -> Told {
    Told {
        namespace: "team".into(),
        claim: "ada-1".into(),
        cluster: roll_fixture("cluster.txt"),
        address: "10.96.0.5:8443".into(),
        keys: vec![roll_fixture("operator-key.txt")],
    }
}

fn the_word() -> NextAnswered {
    NextAnswered {
        next: Some(roll_fixture("next-operator-key.txt")),
        statement: Some(format!("{}\n", roll_fixture("key-roll.txt"))),
        signature: Some(roll_fixture("key-roll-signature.txt")),
    }
}

/// A moment the worked example's word holds at.
fn while_it_holds() -> u64 {
    crate::clusterpool::KeyRoll::from_text(&format!("{}\n", roll_fixture("key-roll.txt")))
        .unwrap()
        .until
        - 60
}

#[test]
fn a_runner_follows_a_word_the_key_it_holds_signed_and_keeps_both_till_the_port_shows_the_next() {
    let told = told_for_a_roll();
    let shown = told.keys[0].clone();
    let next = roll_fixture("next-operator-key.txt");
    let moved = following(&told, &shown, the_word(), while_it_holds())
        .unwrap()
        .expect("it moves");
    assert_eq!(moved.keys, [shown.clone(), next.clone()]);
    // Still on the old key: both stay. On the next: the old key goes.
    assert_eq!(settled_on(moved.clone(), &shown), None);
    assert_eq!(
        settled_on(moved, &next).expect("settled").keys,
        std::slice::from_ref(&next)
    );
}

#[test]
fn a_word_not_signed_by_the_key_held_or_past_its_time_moves_nothing() {
    let told = told_for_a_roll();
    let shown = told.keys[0].clone();
    let at = while_it_holds();
    // Nothing named: nothing moves.
    assert_eq!(
        following(&told, &shown, NextAnswered::default(), at),
        Ok(None)
    );
    let forged = NextAnswered {
        signature: Some("ab".repeat(64)),
        ..the_word()
    };
    assert!(following(&told, &shown, forged, at).is_err());
    let another = NextAnswered {
        next: Some(format!("ed25519:{}", "11".repeat(32))),
        ..the_word()
    };
    assert!(following(&told, &shown, another, at).is_err());
    let unsigned = NextAnswered {
        statement: None,
        signature: None,
        ..the_word()
    };
    assert!(following(&told, &shown, unsigned, at).is_err());
    assert!(following(&told, &shown, the_word(), at + 120).is_err());
    let elsewhere = Told {
        cluster: "11111111-1111-1111-1111-111111111111".into(),
        ..told
    };
    assert!(following(&elsewhere, &shown, the_word(), at).is_err());
}

#[test]
fn an_answer_is_read_exactly_as_long_as_it_says() {
    let (status, body) =
        answer_of(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\n\r\n{}").unwrap();
    assert_eq!((status, body.as_slice()), (403, b"{}".as_slice()));
    assert!(answer_of(b"HTTP/1.1 200 OK\r\ncontent-length: 9\r\n\r\n{}").is_err());
    assert!(answer_of(b"HTTP/1.1 200 OK\r\n\r\n{}").is_err());
    assert!(answer_of(b"nothing").is_err());
}

#[test]
fn a_refusal_is_said_in_the_operators_words_on_one_line() {
    assert_eq!(
        refused_in_words(403, b"{\"refused\":\"no\\nmore\"}"),
        "nomore"
    );
    assert_eq!(refused_in_words(500, b"oops"), "the mint port answered 500");
}
