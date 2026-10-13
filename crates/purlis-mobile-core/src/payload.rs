//! What the desktop's pairing code carries (iOS design §4.2): `purlis-pair:` then compact
//! JSON — the desktop's device id and public key, where to reach it, and the one-time code.
//! The same shape the desktop writes (`purlis_core::remote::pairing::Payload`), kept here so
//! the phone carries no desktop code.

use purlis_session_protocol::pairing::Code;

use crate::{Error, unhex32};

const QR_PREFIX: &str = "purlis-pair:";

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Payload {
    pub version: u32,
    pub host_device: String,
    /// The desktop's public link key, hex.
    pub host_key: String,
    /// Where to reach it, `host:port` each, LAN and Tailscale.
    pub addresses: Vec<String>,
    /// The one-time code, hex.
    pub code: String,
}

impl std::fmt::Debug for Payload {
    /// The code is a secret for five minutes: never in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Payload")
            .field("version", &self.version)
            .field("host_device", &self.host_device)
            .field("host_key", &self.host_key)
            .field("addresses", &self.addresses)
            .field("code", &"…")
            .finish()
    }
}

impl Payload {
    /// The scanned, or pasted, text. Whitespace around it is a paste's, not the code's.
    pub fn from_qr_text(text: &str) -> Result<Payload, Error> {
        let json = text
            .trim()
            .strip_prefix(QR_PREFIX)
            .ok_or_else(|| Error::Payload("it does not begin with purlis-pair:".into()))?;
        let payload: Payload =
            serde_json::from_str(json).map_err(|err| Error::Payload(err.to_string()))?;
        if payload.version != 1 {
            return Err(Error::Payload(format!(
                "version {} is newer than this phone reads",
                payload.version
            )));
        }
        Ok(payload)
    }

    pub fn to_qr_text(&self) -> String {
        format!(
            "{QR_PREFIX}{}",
            serde_json::to_string(self).expect("plain data")
        )
    }

    pub fn host_key(&self) -> Result<[u8; 32], Error> {
        unhex32(&self.host_key).map_err(|_| Error::Payload("the desktop's key is not a key".into()))
    }

    pub fn code(&self) -> Result<Code, Error> {
        Code::from_hex(&self.code).map_err(Error::Payload)
    }
}
