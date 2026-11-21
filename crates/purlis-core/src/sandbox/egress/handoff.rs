// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The hand-off between a chat's egress proxy and its forge listener's `CONNECT` door** (ADR 0128
//! D3a): a per-run secret both ends hold, which each proves to the other.
//!
//! The door is a loopback port, and a port is not an identity: any process on this machine can
//! connect to it, and a port a run has given up can be bound by something else before the egress
//! proxy next dials it. So:
//!
//! - **The door requires proof.** The egress proxy's `CONNECT` carries [`HEADER`]: a fresh nonce
//!   and `HMAC-SHA256(secret, "request:" + nonce)`. A connection without a valid one is refused
//!   and carried nowhere.
//! - **The proxy requires proof back.** Every answer the door gives a proven connection carries
//!   [`REPLY_HEADER`]: `HMAC-SHA256(secret, "reply:" + nonce)`. The proxy reads the answer's head
//!   and checks it **before it splices**, so a port that is not the door never becomes a raw pipe
//!   to the chat.
//!
//! The secret is never sent: only proofs of it, each of one nonce, so what an impostor on a reused
//! port reads is worth nothing to the door. It is carried in the plan like the door's address,
//! and kept out of every log line ([`Secret`]'s `Debug`).

use hmac::{KeyInit as _, Mac as _};
use subtle::ConstantTimeEq as _;

/// The header the proxy's `CONNECT` carries: `<nonce>.<proof>`, both in hex.
pub const HEADER: &str = "Purlis-Handoff";

/// The header every answer of the door carries: the door's proof of the same nonce, in hex.
pub const REPLY_HEADER: &str = "Purlis-Door";

/// Bytes of the secret, and of a nonce.
const BYTES: usize = 32;

/// **A run's hand-off secret**, in hex on the wire.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

impl Secret {
    /// A new secret, from the system's random source.
    pub fn random() -> std::io::Result<Secret> {
        Ok(Secret(random_hex()?))
    }

    /// A secret as it was handed on (hex, [`Secret::expose`]): `None` for anything else.
    pub fn from_hex(hex: &str) -> Option<Secret> {
        (hex.len() == BYTES * 2 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| Secret(hex.to_ascii_lowercase()))
    }

    /// The secret as it travels in a plan.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// What a request's proof covers: the nonce and the target, so a proof captured on the wire
    /// opens that `CONNECT` and no other.
    fn bound(nonce: &str, host: &str, port: u16) -> String {
        format!(
            "{nonce}:{}:{port}",
            host.trim_end_matches('.').to_ascii_lowercase()
        )
    }

    fn proof(&self, role: &str, nonce: &str) -> String {
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(self.0.as_bytes())
            .expect("HMAC takes a key of any length");
        mac.update(role.as_bytes());
        mac.update(b":");
        mac.update(nonce.as_bytes());
        data_encoding::HEXLOWER.encode(&mac.finalize().into_bytes())
    }

    /// The value of [`HEADER`] for a new `CONNECT host:port`, and the nonce its answer must prove.
    /// The proof covers the target.
    pub fn ask(&self, host: &str, port: u16) -> std::io::Result<(String, String)> {
        let nonce = random_hex()?;
        let proof = self.proof("request", &Self::bound(&nonce, host, port));
        Ok((format!("{nonce}.{proof}"), nonce))
    }

    /// The nonce of a [`HEADER`] value whose proof is this secret's **for `CONNECT host:port`**, or
    /// `None`: a proof made for another target is not accepted.
    pub fn accept(&self, header: &str, host: &str, port: u16) -> Option<String> {
        let (nonce, proof) = header.trim().split_once('.')?;
        if nonce.len() != BYTES * 2 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let wanted = self.proof("request", &Self::bound(nonce, host, port));
        bool::from(
            wanted
                .as_bytes()
                .ct_eq(proof.to_ascii_lowercase().as_bytes()),
        )
        .then(|| nonce.to_owned())
    }

    /// The value of [`REPLY_HEADER`] that answers `nonce`.
    pub fn reply(&self, nonce: &str) -> String {
        self.proof("reply", nonce)
    }

    /// Whether `header` (a [`REPLY_HEADER`] value) is this secret's answer to `nonce`.
    pub fn answers(&self, nonce: &str, header: &str) -> bool {
        let wanted = self.reply(nonce);
        bool::from(
            wanted
                .as_bytes()
                .ct_eq(header.trim().to_ascii_lowercase().as_bytes()),
        )
    }
}

fn random_hex() -> std::io::Result<String> {
    let mut bytes = [0u8; BYTES];
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
    Ok(data_encoding::HEXLOWER.encode(&bytes))
}

/// The value of header `name` in an HTTP head (case-insensitive), the first one.
pub fn header_of<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> Secret {
        Secret::random().unwrap()
    }

    #[test]
    fn a_door_accepts_the_proof_of_its_own_secret_and_proves_the_same_nonce_back() {
        let secret = secret();
        let (header, nonce) = secret.ask("github.com", 443).unwrap();

        let accepted = secret
            .accept(&header, "github.com", 443)
            .expect("the door accepts its own proof");

        assert_eq!(accepted, nonce);
        assert!(secret.answers(&nonce, &secret.reply(&accepted)));
    }

    #[test]
    fn a_proof_of_another_secret_or_a_made_up_one_is_not_accepted() {
        let (header, _) = secret().ask("github.com", 443).unwrap();

        assert!(secret().accept(&header, "github.com", 443).is_none());
        assert!(secret().accept("", "github.com", 443).is_none());
        assert!(secret().accept("abc.def", "github.com", 443).is_none());
    }

    #[test]
    fn a_proof_captured_for_one_connect_opens_no_other() {
        let secret = secret();
        let (header, nonce) = secret.ask("github.com", 443).unwrap();

        assert_eq!(
            secret.accept(&header, "GitHub.com.", 443),
            Some(nonce),
            "the same target, spelled as a host is"
        );
        for (host, port) in [
            ("gitlab.com", 443),
            ("github.com", 8443),
            ("api.github.com", 443),
        ] {
            assert!(
                secret.accept(&header, host, port).is_none(),
                "{host}:{port} was opened by another target's proof"
            );
        }
    }

    #[test]
    fn an_answer_that_proves_another_nonce_or_nothing_is_not_the_doors() {
        let secret = secret();
        let (_, nonce) = secret.ask("github.com", 443).unwrap();
        let (_, other) = secret.ask("github.com", 443).unwrap();

        assert!(
            !secret.answers(&nonce, &secret.reply(&other)),
            "a replayed answer"
        );
        assert!(!secret.answers(&nonce, ""));
        assert!(!secret.answers(&nonce, "200"));
    }

    #[test]
    fn the_secret_is_in_no_debug_line() {
        let secret = secret();

        assert!(!format!("{secret:?}").contains(secret.expose()));
    }

    #[test]
    fn a_secret_read_back_from_a_plan_is_the_same_one() {
        let secret = secret();

        assert_eq!(Secret::from_hex(secret.expose()), Some(secret));
        assert!(Secret::from_hex("short").is_none());
    }
}
