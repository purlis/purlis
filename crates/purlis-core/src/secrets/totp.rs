// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A 2FA code worked out from a seed a vault holds** (#5118, spec #5100, ruling SF-4 and
//! decision SF-11 on #5096): TOTP as RFC 6238 defines it, on the vault's side of purlis, so only
//! the code ever reaches a browser.
//!
//! - **A seed is what an authenticator app is given**: an `otpauth://totp/…` address, with its
//!   own algorithm (SHA-1, SHA-256 or SHA-512), digits (6 to 10) and period, or the bare base32
//!   secret, read as six digits of SHA-1 every 30 seconds, the defaults every app takes.
//! - **A counter's seed (`otpauth://hotp/…`) is refused**: its code needs a count purlis doesn't
//!   keep, and a code used out of turn locks the account.
//! - **Words, never the seed**: a seed that can't be read is refused in fixed words, which name
//!   no part of what was read.

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use zeroize::Zeroizing;

/// The fewest bytes a seed is taken with: the 128 bits RFC 4226 asks for, at the least its 80.
const FEWEST_BYTES: usize = 10;

/// The HMAC of `message` under `key` with each hash a seed may name: the hash's bytes.
macro_rules! mac {
    ($hash:ty, $key:expr, $message:expr) => {
        match <Hmac<$hash> as KeyInit>::new_from_slice($key) {
            Ok(mut mac) => {
                mac.update($message);
                mac.finalize().into_bytes().to_vec()
            }
            // An HMAC takes a key of any length, so this is never reached.
            Err(_) => Vec::new(),
        }
    };
}

/// The hash a seed's codes are made with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}

/// **A TOTP seed, as read from a vault's value**: never printed, and wiped once dropped.
pub struct Seed {
    key: Zeroizing<Vec<u8>>,
    algorithm: Algorithm,
    digits: u32,
    period: u64,
}

impl std::fmt::Debug for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seed")
            .field("algorithm", &self.algorithm)
            .field("digits", &self.digits)
            .field("period", &self.period)
            .finish_non_exhaustive()
    }
}

/// What a value that is no TOTP seed is refused with.
const NOT_A_SEED: &str = "isn't a 2FA seed: give the key of an otpauth://totp/ address or of \
                          the base32 secret an authenticator app is given";

/// Whether `text` is an `otpauth://` address: a seed by its look, whatever else it holds.
pub fn an_address(text: &str) -> bool {
    text.trim_start()
        .get(..10)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("otpauth://"))
}

impl Seed {
    /// **Reads a seed from a vault's value**: an `otpauth://totp/` address or a bare base32
    /// secret. Refused, in words that hold nothing of `text`, for anything else.
    pub fn parse(text: &str) -> Result<Seed, &'static str> {
        if an_address(text) {
            Self::of_address(text.trim())
        } else {
            Ok(Seed {
                key: secret(text)?,
                algorithm: Algorithm::Sha1,
                digits: 6,
                period: 30,
            })
        }
    }

    fn of_address(text: &str) -> Result<Seed, &'static str> {
        let address = url::Url::parse(text).map_err(|_| NOT_A_SEED)?;
        if !address
            .host_str()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("totp"))
        {
            return Err(
                "is a 2FA seed for counted codes (hotp), which purlis doesn't make: only \
                        a time-based one (totp) is filled",
            );
        }
        let mut seed = Seed {
            key: Zeroizing::new(Vec::new()),
            algorithm: Algorithm::Sha1,
            digits: 6,
            period: 30,
        };
        let mut has_secret = false;
        for (name, value) in address.query_pairs() {
            match name.as_ref() {
                "secret" => {
                    seed.key = secret(&value)?;
                    has_secret = true;
                }
                "algorithm" => {
                    seed.algorithm = match value.to_ascii_uppercase().as_str() {
                        "SHA1" => Algorithm::Sha1,
                        "SHA256" => Algorithm::Sha256,
                        "SHA512" => Algorithm::Sha512,
                        _ => {
                            return Err("is a 2FA seed with an algorithm purlis doesn't know: \
                                        only SHA1, SHA256 and SHA512 are");
                        }
                    };
                }
                "digits" => {
                    seed.digits = value
                        .parse()
                        .ok()
                        .filter(|digits| (6..=10).contains(digits))
                        .ok_or("is a 2FA seed whose codes aren't 6 to 10 digits")?;
                }
                "period" => {
                    seed.period = value
                        .parse()
                        .ok()
                        .filter(|period| *period > 0)
                        .ok_or("is a 2FA seed with no period in whole seconds")?;
                }
                _ => {}
            }
        }
        if !has_secret {
            return Err(NOT_A_SEED);
        }
        Ok(seed)
    }

    /// **The code for the moment `unix`** (seconds since 1970, UTC): RFC 6238's TOTP over
    /// RFC 4226's HOTP, the step counted from 0.
    pub fn code_at(&self, unix: u64) -> String {
        let step = (unix / self.period).to_be_bytes();
        let hash: Zeroizing<Vec<u8>> = Zeroizing::new(match self.algorithm {
            Algorithm::Sha1 => mac!(sha1::Sha1, &self.key, &step),
            Algorithm::Sha256 => mac!(sha2::Sha256, &self.key, &step),
            Algorithm::Sha512 => mac!(sha2::Sha512, &self.key, &step),
        });
        if hash.len() < 20 {
            return String::new();
        }
        // RFC 4226 §5.3: four bytes from where the last byte's low nibble says, sign bit off.
        let at = usize::from(hash[hash.len() - 1] & 0x0f);
        let bits =
            u32::from_be_bytes([hash[at], hash[at + 1], hash[at + 2], hash[at + 3]]) & 0x7fff_ffff;
        let code = u64::from(bits) % 10u64.pow(self.digits);
        format!("{code:0width$}", width = self.digits as usize)
    }

    /// The code for now, by this machine's clock.
    pub fn code_now(&self) -> String {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        self.code_at(unix)
    }
}

/// A base32 secret as people copy one: in any case, in groups, padded or not.
fn secret(text: &str) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let plain: Zeroizing<String> = Zeroizing::new(
        text.chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
            .map(|c| c.to_ascii_uppercase())
            .collect(),
    );
    let key = Zeroizing::new(
        BASE32_NOPAD
            .decode(plain.as_bytes())
            .map_err(|_| NOT_A_SEED)?,
    );
    if key.len() < FEWEST_BYTES {
        return Err(NOT_A_SEED);
    }
    Ok(key)
}

#[cfg(test)]
mod tests;
