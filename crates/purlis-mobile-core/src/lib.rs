//! The phone's side of purlis (iOS design §3): what it keeps (its link key, its name, the
//! desktops it paired with, where each subscription stands), how it pairs ([`pair`]), the
//! link to a desktop ([`link`]) and a desktop across reconnects ([`session`]). Plain Rust: the
//! Tauri commands are the phone app's, one thin layer over this.
//!
//! Nothing here depends on `purlis-core`: the phone carries no desktop code. What the two
//! share — the handshake, the proof, the protocol — is `purlis-session-protocol`'s.

pub mod hosts;
pub mod identity;
pub mod link;
pub mod pair;
pub mod payload;
pub mod session;
pub mod store;

/// Why something here failed, in words the app shows.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A record could not be read or written.
    #[error("{0}")]
    Store(String),
    /// The scanned or pasted text is not a pairing code.
    #[error("not a purlis pairing code: {0}")]
    Payload(String),
    /// The desktop answered the proof with a refusal, in its words.
    #[error("the desktop refused the pairing: {0}")]
    PairingRefused(String),
    /// The desktop's key is not the pinned one: it rotated its key, or this is not it.
    #[error("the desktop's key is not the one you paired with")]
    WrongKey,
    /// Nothing is sent later (iOS design §6.3): a call with no link fails now.
    #[error("no link to the desktop")]
    NotConnected,
    /// The desktop answered the handshake and then nothing: it no longer has this phone's key.
    #[error("this desktop no longer knows this phone; pair it again")]
    NotPaired,
    /// The desktop took too long: a connection that is up but silent (iOS design §6.3).
    #[error("the desktop did not answer in time")]
    TimedOut,
    /// No address answered.
    #[error("the desktop is unreachable: {0}")]
    Unreachable(String),
    /// The link failed in a way the protocol words.
    #[error("{0}")]
    Link(String),
    /// The desktop refused a command.
    #[error(transparent)]
    Refused(#[from] purlis_session_protocol::session::Refusal),
}

impl From<purlis_session_protocol::session::CallError> for Error {
    fn from(e: purlis_session_protocol::session::CallError) -> Self {
        use purlis_session_protocol::session::CallError;
        match e {
            CallError::Refused(refusal) => Error::Refused(refusal),
            CallError::Closed(_) => Error::NotConnected,
            CallError::Unexpected(why) => Error::Link(why),
        }
    }
}

/// 32 bytes as 64 lowercase hex digits.
pub(crate) fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 64 hex digits as 32 bytes.
pub(crate) fn unhex32(text: &str) -> Result<[u8; 32], String> {
    let text = text.trim();
    if text.len() != 64 {
        return Err("not 64 hex digits".into());
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| "not 64 hex digits".to_owned())?;
    }
    Ok(out)
}
