//! This device's link key (ADR 0078 §3, iOS design §4.1): a static X25519 key pair whose
//! private half lives in the Keyring (`docs/plane-format.md`, "keyring item for the link key"),
//! minted the first time the gateway or a pairing asks, read back the same every time, and
//! rotated only on purpose ([`forget`]), after which a new pairing is needed.

use crate::secrets::Ctx;
use crate::secrets::keyring;

/// The Keyring service the link key is under; the account is this device's id. Under purlis's
/// own prefix ([`keyring::SERVICE_PREFIX`]), which a test holds.
pub const SERVICE: &str = "purlis/link-key";

/// The key pair. The private half is here so the handshake can use it; it never leaves through
/// `Debug`.
#[derive(Clone)]
pub struct LinkKey {
    pub private: [u8; 32],
    pub public: [u8; 32],
}

impl LinkKey {
    /// The pair whose private half is `private`: one base-point multiplication, clamped as
    /// X25519 clamps.
    pub fn from_private(private: [u8; 32]) -> LinkKey {
        let public = curve25519_dalek::MontgomeryPoint::mul_base_clamped(private).to_bytes();
        LinkKey { private, public }
    }

    fn mint() -> LinkKey {
        let mut private = [0u8; 32];
        getrandom::fill(&mut private).expect("the OS generator works");
        LinkKey::from_private(private)
    }

    pub fn public_hex(&self) -> String {
        super::hex(&self.public)
    }

    pub fn fingerprint(&self) -> String {
        super::fingerprint(&self.public)
    }
}

impl std::fmt::Debug for LinkKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LinkKey({})", self.fingerprint())
    }
}

/// The link key for `device`, minting and storing one when the Keyring has none. An item that
/// is there but does not hold a key is refused, never overwritten: somebody put it there.
pub fn load_or_mint(ctx: &Ctx, device: &str) -> Result<LinkKey, String> {
    let store = keyring::store(ctx);
    match store.get(SERVICE, device).map_err(|e| e.to_string())? {
        Some(secret) => {
            let private = super::unhex32(&secret.into_inner()).map_err(|_| {
                format!(
                    "the Keyring item under '{SERVICE}' does not hold a link key (64 hex digits)"
                )
            })?;
            Ok(LinkKey::from_private(private))
        }
        None => {
            let key = LinkKey::mint();
            store
                .set(SERVICE, device, &super::hex(&key.private))
                .map_err(|e| e.to_string())?;
            Ok(key)
        }
    }
}

/// Rotate: delete the key. `false` when there was none. Every pairing made with it is void
/// until the devices pair again (ADR 0078 §3).
pub fn forget(ctx: &Ctx, device: &str) -> Result<bool, String> {
    keyring::store(ctx)
        .delete(SERVICE, device)
        .map_err(|e| e.to_string())
}
