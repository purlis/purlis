// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A cluster runner's way to the operator's mint port** (#4207, the rulings on #3807).
//!
//! A runner on a Pool that opts in to minting is told once, by its desktop as it pairs it, where
//! the port is and the key to hold it to ([`Told`], kept in the session host's own folder, which no
//! chat reads). The desktop tells only the key it pinned, whatever the Claim's status says
//! ([`crate::clusterpool::told_of_the_port`]).
//!
//! **A mint** ([`mint`]) answers the operator's one-use X25519 challenge with a MAC only the holder
//! of this runner's link key can make, and takes back a token limited by name to the Pool's and the
//! Claim's repos. The operator decides from those alone. Nothing here decides, and nothing is
//! written: the token is kept in memory ([`crate::forge::apptoken::token_in_cluster`]).
//!
//! **The port is held to a key, not a name or an authority** ([`HeldToKeys`]): the TLS handshake's
//! signature must be one of the held keys' over the transcript, so the certificate, which that
//! signature covers, is not read at all.
//!
//! **A runner follows a roll by itself** ([`follow`]): the port serves the current key alone and
//! answers the current key's word for the next one, the same word every Pool's status holds. A
//! word the key held does not sign moves nothing. The runner then holds both, and lets the old one
//! go when the port shows the next. One that missed a whole roll (its pod stopped, say) finds its
//! port refused and asks its desktop, with the person's Ask, as a pool not opted in does.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::clusterpool::{Following, OperatorKey, PoolPin, PoolSeen, is_label, is_subdomain};

/// What a runner keeps of the port, in the session host's folder.
pub const FILE: &str = "mint-port.json";

/// The most keys held at once: the one the port shows and the next.
const MOST_KEYS: usize = 2;

/// The most of [`FILE`] and of the port's answer read.
const MOST_READ: u64 = 64 * 1024;

/// How long the port's TCP connection is waited for.
const CONNECTED_WITHIN: Duration = Duration::from_secs(5);

/// How long the port's answer is waited for: past the operator's own minute on a connection.
const ANSWERED_WITHIN: Duration = Duration::from_secs(70);

/// How long one mint is waited for in all.
const MINTED_WITHIN: Duration = Duration::from_secs(120);

/// How often a host looks for a roll of the operator's key.
const FOLLOWED_EVERY: Duration = Duration::from_secs(3600);

const CHALLENGE_PATH: &str = "/forge/challenge";
const TOKEN_PATH: &str = "/forge/token";
const NEXT_KEY_PATH: &str = "/forge/next-key";

/// **What a runner is told of the port**, as its desktop tells it, and as [`FILE`] keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Told {
    /// The Claim's namespace.
    pub namespace: String,
    /// The Claim's name.
    pub claim: String,
    /// The uid of the Claim's cluster's `kube-system` namespace, which a roll's word names.
    pub cluster: String,
    /// The port, `<ip>:<port>`.
    pub address: String,
    /// The operator's keys the port is held to, each `ed25519:<64 hex>`: one, or two during a roll.
    pub keys: Vec<String>,
}

impl Told {
    /// Whether each value is one purlis reads, so nothing else is ever connected to or sent.
    fn checked(self) -> Result<Told, String> {
        let keys_read = !self.keys.is_empty()
            && self.keys.len() <= MOST_KEYS
            && self
                .keys
                .iter()
                .all(|key| OperatorKey::from_spelled(key).is_ok());
        if !keys_read
            || !is_label(&self.namespace)
            || !is_subdomain(&self.claim)
            || !crate::clusterpool::is_uid(&self.cluster)
            || self.address.parse::<SocketAddr>().is_err()
        {
            return Err(
                "what this runner was told of the mint port is not one purlis reads".into(),
            );
        }
        Ok(self)
    }
}

/// **Keeps what this runner's desktop told it of the port**, replacing what it was told before.
pub fn tell(config_root: &Path, told: Told) -> Result<(), String> {
    let told = told.checked()?;
    let held = purlis_same_user::PrivateDirectory::open(&crate::machine::charterd(config_root))
        .map_err(|why| format!("the session host's folder could not be opened: {why}"))?;
    let text = serde_json::to_vec(&told).map_err(|why| why.to_string())?;
    held.replace_file(FILE, &text)
        .map_err(|why| format!("what this runner was told of the mint port was not kept: {why}"))
}

/// What this runner was told of the port. `None` where nothing was, as on a pool not opted in.
pub fn told(config_root: &Path) -> Result<Option<Told>, String> {
    let at = crate::machine::charterd(config_root).join(FILE);
    match purlis_same_user::read_private_file(&at) {
        Ok(text) => serde_json::from_str::<Told>(&text)
            .map_err(|_| "what this runner keeps of the mint port is not one purlis reads".into())
            .and_then(Told::checked)
            .map(Some),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(format!(
            "what this runner keeps of the mint port could not be read safely: {why}"
        )),
    }
}

/// **The text a proof is a MAC of**, each value one line: the same as the operator's.
pub fn proof_text(
    operator: &str,
    namespace: &str,
    claim: &str,
    app: &str,
    challenge: &str,
) -> String {
    format!(
        "purlis forge token request v1\noperator: {operator}\nnamespace: {namespace}\nclaim: \
         {claim}\napp: {app}\nchallenge: {challenge}\n"
    )
}

/// **The proof of holding a link key**: HMAC-SHA-256 of `text`, keyed by HKDF-SHA-256 of the X25519
/// agreement of `private` and `challenge`, with the challenge as HKDF's `info`. As 64 lower-case
/// hex. `None` where the challenge is a low-order point, which agrees on nothing.
pub fn proof_for(private: &[u8; 32], challenge: &[u8; 32], text: &str) -> Option<String> {
    let shared = Zeroizing::new(
        curve25519_dalek::montgomery::MontgomeryPoint(*challenge)
            .mul_clamped(*private)
            .to_bytes(),
    );
    if shared.iter().all(|byte| *byte == 0) {
        return None;
    }
    proof_of_agreement(&shared, challenge, text)
}

/// [`proof_for`], from the agreement itself.
fn proof_of_agreement(shared: &[u8; 32], challenge: &[u8; 32], text: &str) -> Option<String> {
    let salt = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, b"purlis forge token request v1");
    let info: [&[u8]; 1] = [challenge];
    let key = ring::hmac::Key::from(
        salt.extract(shared)
            .expand(&info, ring::hmac::HMAC_SHA256)
            .ok()?,
    );
    Some(crate::runnerlink::hex_of(
        ring::hmac::sign(&key, text.as_bytes()).as_ref(),
    ))
}

/// **Holds TLS to the held keys, and to nothing else** (module docs): `shown` is the one whose
/// signature the handshake carried.
#[derive(Debug)]
struct HeldToKeys {
    keys: Vec<([u8; 32], String)>,
    shown: Arc<Mutex<Option<String>>>,
}

impl ServerCertVerifier for HeldToKeys {
    /// Says nothing yet: the handshake's signature, over the certificate too, decides.
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    /// Never taken: the connection is TLS 1.3 alone.
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General(
            "the mint port speaks TLS 1.3 alone".into(),
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        _cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let refused = || rustls::Error::General("not a key this runner holds the port to".into());
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(refused());
        }
        let shown = self.keys.iter().find(|(key, _)| {
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
                .verify(message, dss.signature())
                .is_ok()
        });
        let Some((_, spelled)) = shown else {
            return Err(refused());
        };
        *self.shown.lock().unwrap_or_else(PoisonError::into_inner) = Some(spelled.clone());
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

/// One connection to the port, its handshake done.
struct Connected {
    stream: rustls::StreamOwned<rustls::ClientConnection, TcpStream>,
    /// The held key the port showed.
    shown: String,
}

fn connected(told: &Told) -> Result<Connected, String> {
    let keys = told
        .keys
        .iter()
        .map(|spelled| {
            let hex = spelled.strip_prefix("ed25519:").unwrap_or_default();
            crate::runnerlink::bytes_of(hex)
                .map(|key| (key, spelled.clone()))
                .ok_or_else(|| "a key this runner holds is not one purlis reads".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shown = Arc::new(Mutex::new(None));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|why| why.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(HeldToKeys {
            keys,
            shown: shown.clone(),
        }))
        .with_no_client_auth();
    let address: SocketAddr = told
        .address
        .parse()
        .map_err(|_| "the mint port's address is not one purlis reads".to_owned())?;
    let name = ServerName::try_from("purlis-cluster-operator").map_err(|why| why.to_string())?;
    let connection =
        rustls::ClientConnection::new(Arc::new(config), name).map_err(|why| why.to_string())?;
    let socket = TcpStream::connect_timeout(&address, CONNECTED_WITHIN)
        .map_err(|why| format!("the mint port did not answer: {why}"))?;
    socket
        .set_read_timeout(Some(ANSWERED_WITHIN))
        .and_then(|()| socket.set_write_timeout(Some(ANSWERED_WITHIN)))
        .map_err(|why| why.to_string())?;
    let mut stream = rustls::StreamOwned::new(connection, socket);
    stream
        .conn
        .complete_io(&mut stream.sock)
        .map_err(|why| format!("the mint port is not the operator this runner holds: {why}"))?;
    let shown = shown
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or("the mint port showed no key")?;
    Ok(Connected { stream, shown })
}

/// `POST path` with `body`: the answer's status and what it says, wiped as it is dropped.
fn posted(
    connected: &mut Connected,
    path: &str,
    body: &[u8],
) -> Result<(u16, Zeroizing<Vec<u8>>), String> {
    let head = format!(
        "POST {path} HTTP/1.1\r\nhost: purlis-cluster-operator\r\ncontent-type: \
         application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let not_asked = |why: std::io::Error| format!("the mint port could not be asked: {why}");
    connected
        .stream
        .write_all(head.as_bytes())
        .and_then(|()| connected.stream.write_all(body))
        .and_then(|()| connected.stream.flush())
        .map_err(not_asked)?;
    let mut read = Zeroizing::new(Vec::new());
    let mut limited = (&mut connected.stream).take(MOST_READ);
    match limited.read_to_end(&mut read) {
        Ok(_) => {}
        // A port that closes without its goodbye has still said what it read in full, or the
        // parse below refuses it.
        Err(why) if why.kind() == std::io::ErrorKind::UnexpectedEof => {}
        Err(why) => return Err(not_asked(why)),
    }
    answer_of(&read)
}

/// The status and the body of an answer, exactly as long as it says.
fn answer_of(read: &[u8]) -> Result<(u16, Zeroizing<Vec<u8>>), String> {
    let bad = || "the mint port's answer is not one purlis reads".to_owned();
    let ends = read
        .windows(4)
        .position(|end| end == b"\r\n\r\n")
        .ok_or_else(bad)?;
    let head = std::str::from_utf8(&read[..ends]).map_err(|_| bad())?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(bad)?;
    let length: usize = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .ok_or_else(bad)?;
    let body = read
        .get(ends + 4..ends + 4 + length)
        .ok_or_else(bad)?
        .to_vec();
    Ok((status, Zeroizing::new(body)))
}

/// **A token the operator minted** for this runner.
pub struct Minted {
    pub token: Zeroizing<String>,
    pub expires_at: Option<String>,
    pub owner: String,
    pub repos: Vec<String>,
    /// The operator's key the port showed.
    pub operator: String,
}

/// What the port answers for a mint.
#[derive(Deserialize)]
struct TokenAnswered {
    token: String,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    repos: Vec<String>,
}

/// What the port answers for a challenge.
#[derive(Deserialize)]
struct Challenge {
    challenge: String,
}

#[derive(Deserialize)]
struct Refusal {
    refused: String,
}

/// What `body` says with `status`, as a refusal in the operator's words where it is not a 200: the
/// words plain, so nothing the port says can move what a chat is shown.
fn refused_in_words(status: u16, body: &[u8]) -> String {
    let words = serde_json::from_slice::<Refusal>(body)
        .map(|refusal| refusal.refused)
        .unwrap_or_else(|_| format!("the mint port answered {status}"));
    words
        .chars()
        .filter(|c| !c.is_control())
        .take(500)
        .collect()
}

/// [`mint`], for at most [`MINTED_WITHIN`] in all, however slowly the port answers: past it, a run
/// goes to its desktop.
pub fn mint_in_time(config_root: &Path, app: &str) -> Result<Option<Minted>, String> {
    let (config_root, app) = (config_root.to_path_buf(), app.to_owned());
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("purlisd-mint".into())
        .spawn(move || {
            // One minted after its asker gave up has nobody to hold it, so it goes at once.
            if let Err(std::sync::mpsc::SendError(Ok(Some(late)))) =
                done.send(mint(&config_root, &app))
            {
                let _ = crate::forge::apptoken::revoke_now(&late.token);
            }
        })
        .map_err(|why| why.to_string())?;
    finished
        .recv_timeout(MINTED_WITHIN)
        .map_err(|_| "the mint port did not answer in time".to_owned())?
}

/// **Asks the port for a token of the GitHub App `app`** (module docs), following a roll first
/// where one is under way. `Ok(None)` where this runner was told of no port.
pub fn mint(config_root: &Path, app: &str) -> Result<Option<Minted>, String> {
    if told(config_root)?.is_none() {
        return Ok(None);
    }
    if let Err(why) = follow(config_root, crate::runners::now()) {
        tracing::info!("purlis: the mint port's next key was not looked for: {why}");
    }
    let Some(told) = told(config_root)? else {
        return Ok(None);
    };
    let (ours, _desktop) = crate::runnerlink::answering(config_root)?
        .ok_or("this runner is not paired, so it holds no link key to prove")?;
    let private = ours
        .key
        .private_half()
        .ok_or("this runner's link key holds no private half")?;

    let mut asking = connected(&told)?;
    let (status, body) = posted(&mut asking, CHALLENGE_PATH, b"{}")?;
    if status != 200 {
        return Err(refused_in_words(status, &body));
    }
    let spelled = serde_json::from_slice::<Challenge>(&body)
        .map_err(|_| "the mint port gave no challenge".to_owned())?
        .challenge;
    let challenge = crate::runnerlink::bytes_of(&spelled)
        .ok_or_else(|| "the mint port gave no challenge".to_owned())?;

    let mut asking = connected(&told)?;
    let text = proof_text(&asking.shown, &told.namespace, &told.claim, app, &spelled);
    let proof = proof_for(&private, &challenge, &text)
        .ok_or_else(|| "the mint port gave a challenge no key agrees with".to_owned())?;
    let request = serde_json::json!({
        "namespace": told.namespace,
        "claim": told.claim,
        "app": app,
        "challenge": spelled,
        "proof": proof,
    })
    .to_string();
    let (status, body) = posted(&mut asking, TOKEN_PATH, request.as_bytes())?;
    if status != 200 {
        return Err(refused_in_words(status, &body));
    }
    let answered: TokenAnswered = serde_json::from_slice(&body)
        .map_err(|_| "the mint port's answer holds no token purlis reads".to_owned())?;
    if answered.token.is_empty() {
        return Err("the mint port's answer holds no token".to_owned());
    }
    let operator = asking.shown;
    let moved = settled_on(told, &operator);
    if let Some(moved) = moved
        && let Err(why) = tell(config_root, moved)
    {
        // The token is live, and the next look at the port settles this again.
        tracing::warn!("purlis: the mint port's old key is still held: {why}");
    }
    Ok(Some(Minted {
        token: Zeroizing::new(answered.token),
        expires_at: answered.expires_at,
        owner: answered.owner,
        repos: answered.repos,
        operator,
    }))
}

/// Where the port showed the later of two held keys, the earlier one has gone: `told` holding
/// `shown` alone, so a port showing the old key again is refused.
fn settled_on(told: Told, shown: &str) -> Option<Told> {
    (told.keys.len() > 1 && told.keys.last().is_some_and(|last| last == shown)).then(|| Told {
        keys: vec![shown.to_owned()],
        ..told
    })
}

/// What the port answers of the next key.
#[derive(Deserialize, Default)]
struct NextAnswered {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    statement: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

/// `told` holding the next key as well, where the key the port showed vouches for it at `now`
/// ([`PoolPin::followed`], the rule a desktop's pin moves by). `Err` with why not, where a next key
/// was named and its word does not hold.
fn following(
    told: &Told,
    shown: &str,
    answered: NextAnswered,
    now: u64,
) -> Result<Option<Told>, String> {
    let Some(next) = answered.next else {
        return Ok(None);
    };
    if told.keys.contains(&next) {
        return Ok(None);
    }
    let pin = PoolPin {
        key: OperatorKey::from_spelled(shown).map_err(|why| why.to_string())?,
        cluster: told.cluster.clone(),
    };
    let seen = PoolSeen {
        signing_key: Some(shown.to_owned()),
        cluster: Some(told.cluster.clone()),
        next_signing_key: Some(next),
        next_signing_key_vouch: answered.statement.zip(answered.signature),
        mints_in_cluster: true,
    };
    match pin.followed(&seen, now).map_err(|why| why.to_string())? {
        Following::Moves(moved) => Ok(Some(Told {
            keys: vec![shown.to_owned(), moved.key.spelled()],
            ..told.clone()
        })),
        Following::Stays => Ok(None),
        Following::NotFollowed(why) => Err(why),
    }
}

/// **Looks for a roll of the operator's key at `now`, and follows one the held key vouches for**
/// (module docs). Nothing where this runner was told of no port.
pub fn follow(config_root: &Path, now: u64) -> Result<(), String> {
    let Some(told) = told(config_root)? else {
        return Ok(());
    };
    let mut asking = connected(&told)?;
    let shown = asking.shown.clone();
    let (status, body) = posted(&mut asking, NEXT_KEY_PATH, b"{}")?;
    if status != 200 {
        return Err(refused_in_words(status, &body));
    }
    let answered = serde_json::from_slice(&body)
        .map_err(|_| "the mint port's word for its next key is not one purlis reads".to_owned())?;
    let moved = following(&told, &shown, answered, now)?.or_else(|| settled_on(told, &shown));
    match moved {
        Some(moved) => tell(config_root, moved),
        None => Ok(()),
    }
}

/// **Keeps looking for a roll** while this host runs, on a thread of its own, every
/// [`FOLLOWED_EVERY`]. A runner told of no port does nothing but read that it was not.
pub fn keep_following(config_root: std::path::PathBuf) {
    let spawned = std::thread::Builder::new()
        .name("purlisd-mint-key".into())
        .spawn(move || {
            loop {
                if let Err(why) = follow(&config_root, crate::runners::now()) {
                    tracing::info!("purlis: the mint port's next key was not looked for: {why}");
                }
                std::thread::sleep(FOLLOWED_EVERY);
            }
        });
    if let Err(why) = spawned {
        tracing::warn!("purlis: this runner will not follow a roll of the mint port's key: {why}");
    }
}

#[cfg(test)]
mod tests;
