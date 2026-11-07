// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A 2FA code worked out from a seed a vault holds** (#5118, spec #5100, ruling SF-4 and
//! decision SF-11 on #5096): TOTP as RFC 6238 defines it, on the vault's side of purlis, so only
//! the code ever reaches a browser.
//!
//! - **A seed is what an authenticator app is given**: an `otpauth://totp/…` address, with its
//!   own algorithm (SHA-1, SHA-256 or SHA-512), digits (6 to 10) and period, or the bare base32
//!   secret, read as six digits of SHA-1 every 30 seconds, the defaults every app takes. A bare
//!   secret is taken only at the lengths apps are given (10, 16, 20, 32 or 64 bytes), in one case,
//!   and is never digits alone. A value that looks like a seed never goes into a page as itself, a
//!   wider net than what a code is made from ([`looks_like_a_seed`]).
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

/// **Whether `text` holds an authenticator's address anywhere** (#5237): `otpauth:` or its
/// export's `otpauth-migration:`, whether its `:` is written plainly or escaped as in a link, in
/// any case and wherever in the text it stands. Such a value is refused as a plain fill whatever
/// the person's mark says, so this looks inside the text where [`looks_like_a_seed`] looks at its
/// start: a label before the address changes nothing.
pub fn holds_an_address(text: &str) -> bool {
    [
        "otpauth:",
        "otpauth%3a",
        "otpauth-migration:",
        "otpauth-migration%3a",
    ]
    .iter()
    .any(|scheme| {
        text.as_bytes()
            .windows(scheme.len())
            .any(|at| at.eq_ignore_ascii_case(scheme.as_bytes()))
    })
}

/// The fewest base32 characters a value is taken for a seed's by its look: 80 bits' worth.
const FEWEST_CHARACTERS: usize = 16;

/// How many base32 characters a bare secret of each of [`BARE_BYTES`] is.
const BARE_CHARACTERS: [usize; 5] = [16, 26, 32, 52, 103];

/// **Whether `text` looks like a 2FA seed** (#5118): such a value never goes into a page as
/// itself, so this reads wider than [`Seed::parse`] and accepts refusing some real values, which
/// the person's mark lets through ([`super::no_seed`]).
pub fn looks_like_a_seed(text: &str) -> bool {
    let trimmed = text.trim_start();
    if an_address(trimmed)
        || trimmed
            .get(..MIGRATION.len())
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case(MIGRATION))
    {
        return true;
    }
    let whole = !ends_in_hex(text)
        && (base32_alone(text)
            || trimmed
                .split_once('\n')
                .is_some_and(|(_, rest)| base32_alone(rest)));
    whole || text.lines().any(|line| !ends_in_hex(line) && seed_on(line))
}

/// Whether the words that end `text` are hex digits, as many as the fewest a seed has.
fn ends_in_hex(text: &str) -> bool {
    let hex: usize = words_of(text)
        .iter()
        .rev()
        .map(|(_, word)| *word)
        .take_while(|word| word.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::len)
        .sum();
    hex >= FEWEST_CHARACTERS
}

/// Whether `text`, once its spaces, `=` and `-` are set aside, is a seed by its characters.
fn base32_alone(text: &str) -> bool {
    let mut count = Base32Count::default();
    for c in text.chars() {
        if c.is_whitespace() || c == '=' || c == '-' {
            continue;
        }
        if !count.add(c) {
            return false;
        }
    }
    count.seed()
}

/// Whether `line` holds a seed. Walks back from its end, as what isn't one stays none however far
/// back it goes, so a long line costs one pass.
fn seed_on(line: &str) -> bool {
    let after = line.rsplit(char::is_alphanumeric).next().unwrap_or("");
    if !after
        .chars()
        .all(|c| CLOSERS.contains(&c) || c.is_whitespace())
    {
        return false;
    }
    let words = words_of(line);
    let mut count = Base32Count::default();
    let mut length = 0;
    for (at, (gap, word)) in words.iter().enumerate().rev() {
        let last = at + 1 == words.len();
        let even = last
            || if at + 2 == words.len() {
                word.len() >= length
            } else {
                word.len() == length && same_gap(words[at + 1].0, words[at + 2].0)
            };
        if !even || !word.chars().all(|c| count.add(c)) {
            return false;
        }
        length = word.len();
        let starts = if at == 0 {
            gap.chars()
                .all(|c| OPENERS.contains(&c) || c.is_whitespace())
        } else {
            gap.contains(|c: char| {
                NAME_ENDS.contains(&c) || JOINERS.contains(&c) || c.is_whitespace()
            })
        };
        let group = last || at + 3 <= words.len() || is_white(words[at + 1].0);
        if starts && group && count.seed() {
            return true;
        }
    }
    false
}

/// What may stand before a value on its line.
const OPENERS: [char; 5] = ['"', '\'', '(', '[', '{'];

/// What may stand after a value on its line.
const CLOSERS: [char; 8] = ['"', '\'', ')', ']', '}', ',', ';', '='];

/// What stands between a name and the value after it, besides white space.
const NAME_ENDS: [char; 7] = [':', '=', '"', '\'', ',', ';', '|'];

/// **What joins a name to the value after it as one word** (the person's ruling of 2026-11-06):
/// `secret_GEZD…` is a seed with its name before it.
const JOINERS: [char; 2] = ['_', '-'];

fn is_white(gap: &str) -> bool {
    gap.chars().all(char::is_whitespace)
}

/// Whether two separators are one: the same text, or white space of any kind and length.
fn same_gap(one: &str, other: &str) -> bool {
    one == other || (is_white(one) && is_white(other))
}

/// The words of `text`, its runs of letters and digits, each with the text between it and the last.
/// Slices of `text`, so no copy of a value is made here.
fn words_of(text: &str) -> Vec<(&str, &str)> {
    let mut words = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(char::is_alphanumeric) {
        let (gap, from) = rest.split_at(start);
        let end = from
            .find(|c: char| !c.is_alphanumeric())
            .unwrap_or(from.len());
        let (word, after) = from.split_at(end);
        words.push((gap, word));
        rest = after;
    }
    words
}

/// How many characters of each kind a stretch of base32 holds.
#[derive(Default)]
struct Base32Count {
    letters: usize,
    digits: usize,
}

impl Base32Count {
    /// Counts `c`, or answers false where base32 has no such character.
    fn add(&mut self, c: char) -> bool {
        match c {
            'a'..='z' | 'A'..='Z' => self.letters += 1,
            '2'..='7' => self.digits += 1,
            _ => return false,
        }
        true
    }

    fn all(&self) -> usize {
        self.letters + self.digits
    }

    /// **With letters, and digits from 80 bits' worth, or letters at a length apps give**,
    /// whatever its case mix (the person's ruling of 2026-11-06): a seed with one letter
    /// capitalised, or in random case, is still a seed.
    fn seed(&self) -> bool {
        self.letters > 0
            && ((self.digits > 0 && self.all() >= FEWEST_CHARACTERS)
                || BARE_CHARACTERS.contains(&self.all()))
    }
}

/// The address an authenticator app exports its seeds in.
const MIGRATION: &str = "otpauth-migration://";

impl Seed {
    /// **Reads a seed from a vault's value**: an `otpauth://totp/` address or a bare base32
    /// secret. Refused, in words that hold nothing of `text`, for anything else.
    pub fn parse(text: &str) -> Result<Seed, &'static str> {
        if an_address(text) {
            Self::of_address(text.trim())
        } else {
            Ok(Seed {
                key: secret(text, Shape::Bare)?,
                algorithm: Algorithm::Sha1,
                digits: 6,
                period: 30,
            })
        }
    }

    /// An `otpauth://<kind>/<label>?<parameters>` address, read by hand inside wiped buffers:
    /// no copy of the secret outlives the read unwiped.
    fn of_address(text: &str) -> Result<Seed, &'static str> {
        let rest = text.get("otpauth://".len()..).ok_or(NOT_A_SEED)?;
        let (kind, rest) = rest.split_once('/').ok_or(NOT_A_SEED)?;
        if kind.eq_ignore_ascii_case("hotp") {
            return Err(
                "is a 2FA seed for counted codes (hotp), which purlis doesn't make: only a \
                 time-based one (totp) is filled",
            );
        }
        if !kind.eq_ignore_ascii_case("totp") {
            return Err(NOT_A_SEED);
        }
        let query = rest.split_once('?').map_or("", |(_, query)| query);
        let query = query.split('#').next().unwrap_or_default();
        let mut seed = Seed {
            key: Zeroizing::new(Vec::new()),
            algorithm: Algorithm::Sha1,
            digits: 6,
            period: 30,
        };
        let mut has_secret = false;
        for pair in query.split('&') {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            // Sized first, so it never grows and leaves an unwiped copy behind.
            let mut decoded = Zeroizing::new(Vec::with_capacity(value.len()));
            decoded.extend(percent_encoding::percent_decode_str(value));
            let value = std::str::from_utf8(&decoded).map_err(|_| NOT_A_SEED)?;
            match name {
                "secret" => {
                    seed.key = secret(value, Shape::Named)?;
                    has_secret = true;
                }
                "algorithm" => {
                    seed.algorithm = if value.eq_ignore_ascii_case("SHA1") {
                        Algorithm::Sha1
                    } else if value.eq_ignore_ascii_case("SHA256") {
                        Algorithm::Sha256
                    } else if value.eq_ignore_ascii_case("SHA512") {
                        Algorithm::Sha512
                    } else {
                        return Err("is a 2FA seed with an algorithm purlis doesn't know: only \
                                    SHA1, SHA256 and SHA512 are");
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

/// How a base32 secret was given.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// As an address's `secret`: a seed by where it stands, of any length from the fewest bytes.
    Named,
    /// As a value alone: a seed only by its shape, the lengths authenticator apps are given
    /// (80, 128, 160, 256 or 512 bits), so a password is seldom taken for one.
    Bare,
}

/// The lengths in bytes a bare secret is taken at.
const BARE_BYTES: [usize; 5] = [10, 16, 20, 32, 64];

/// A base32 secret as people copy one: in either case, in groups of spaces or hyphens, padded or
/// not, and nothing else (as a value alone, in one case throughout), with whatever its last character's unused bits hold, as a generator
/// that picks characters leaves them. Every copy is wiped.
fn secret(text: &str, shape: Shape) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let mut plain = Zeroizing::new(String::with_capacity(text.len()));
    for c in text.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '2'..='7' => plain.push(c.to_ascii_uppercase()),
            '=' | '-' => {}
            c if c.is_whitespace() => {}
            _ => return Err(NOT_A_SEED),
        }
    }
    // Digits alone are a card's number sooner than a seed, and both cases a password's.
    if shape == Shape::Bare
        && (plain.chars().all(|c| c.is_ascii_digit())
            || (text.chars().any(|c| c.is_ascii_lowercase())
                && text.chars().any(|c| c.is_ascii_uppercase())))
    {
        return Err(NOT_A_SEED);
    }
    let mut base32 = BASE32_NOPAD.specification();
    base32.check_trailing_bits = false;
    let key = Zeroizing::new(
        base32
            .encoding()
            .map_err(|_| NOT_A_SEED)?
            .decode(plain.as_bytes())
            .map_err(|_| NOT_A_SEED)?,
    );
    let fits = match shape {
        Shape::Named => key.len() >= FEWEST_BYTES,
        Shape::Bare => BARE_BYTES.contains(&key.len()),
    };
    if !fits {
        return Err(NOT_A_SEED);
    }
    Ok(key)
}

#[cfg(test)]
mod tests;
