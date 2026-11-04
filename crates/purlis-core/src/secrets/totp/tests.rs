// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use data_encoding::BASE32_NOPAD;

use super::*;

/// RFC 6238's test seeds, one per algorithm (Appendix B).
const SHA1_SEED: &[u8] = b"12345678901234567890";
const SHA256_SEED: &[u8] = b"12345678901234567890123456789012";
const SHA512_SEED: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

/// RFC 6238 Appendix B: the time, then the eight-digit code for SHA-1, SHA-256 and SHA-512.
const APPENDIX_B: [(u64, &str, &str, &str); 6] = [
    (59, "94287082", "46119246", "90693936"),
    (1_111_111_109, "07081804", "68084774", "25091201"),
    (1_111_111_111, "14050471", "67062674", "99943326"),
    (1_234_567_890, "89005924", "91819424", "93441116"),
    (2_000_000_000, "69279037", "90698825", "38618901"),
    (20_000_000_000, "65353130", "77737706", "47863826"),
];

fn address(seed: &[u8], algorithm: &str) -> String {
    format!(
        "otpauth://totp/Example:alice@example.test?secret={}&issuer=Example&algorithm={algorithm}&digits=8",
        BASE32_NOPAD.encode(seed)
    )
}

#[test]
fn a_seed_address_gives_rfc_6238s_codes_for_each_algorithm() {
    let sha1 = Seed::parse(&address(SHA1_SEED, "SHA1")).unwrap();
    let sha256 = Seed::parse(&address(SHA256_SEED, "SHA256")).unwrap();
    let sha512 = Seed::parse(&address(SHA512_SEED, "SHA512")).unwrap();
    for (at, one, two_five_six, five_one_two) in APPENDIX_B {
        assert_eq!(sha1.code_at(at), one, "SHA-1 at {at}");
        assert_eq!(sha256.code_at(at), two_five_six, "SHA-256 at {at}");
        assert_eq!(sha512.code_at(at), five_one_two, "SHA-512 at {at}");
    }
}

#[test]
fn a_bare_seed_is_six_digits_of_sha_1_every_30_seconds() {
    // As an authenticator app shows a seed to copy: in groups, in any case, padded or not.
    let shown = "gezd gnbv gy3t qojq gezd gnbv gy3t qojq";
    let seed = Seed::parse(shown).unwrap();
    assert_eq!(seed.code_at(59), "287082");
    assert_eq!(seed.code_at(1_111_111_109), "081804");
    assert_eq!(
        Seed::parse(&format!("{}====", BASE32_NOPAD.encode(SHA1_SEED)))
            .unwrap()
            .code_at(59),
        "287082"
    );
}

#[test]
fn a_seed_address_keeps_its_own_period_and_digits() {
    let seed = Seed::parse(&format!(
        "otpauth://totp/x?secret={}&period=60&digits=7",
        BASE32_NOPAD.encode(SHA1_SEED)
    ))
    .unwrap();
    // At 119 seconds a 60-second step is step 1, which a 30-second one reaches at 59.
    assert_eq!(seed.code_at(119), "4287082");
    assert_eq!(seed.code_at(119), seed.code_at(60));
    assert_ne!(seed.code_at(119), seed.code_at(120));
}

#[test]
fn what_is_no_totp_seed_is_refused_in_words_that_never_hold_it() {
    let secret = BASE32_NOPAD.encode(SHA1_SEED);
    let refused = [
        // A counter's code needs a count purlis doesn't keep.
        format!("otpauth://hotp/x?secret={secret}&counter=1"),
        format!("otpauth://totp/x?secret={secret}&algorithm=MD5"),
        format!("otpauth://totp/x?secret={secret}&digits=5"),
        format!("otpauth://totp/x?secret={secret}&digits=11"),
        format!("otpauth://totp/x?secret={secret}&period=0"),
        format!("otpauth://totp/x?secret={secret}&period=soon"),
        "otpauth://totp/x?issuer=Example".to_owned(),
        "otpauth://totp/x?secret=".to_owned(),
        // A password is no seed: not base32.
        "correct horse battery staple 1!".to_owned(),
        "1234-5678".to_owned(),
        String::new(),
    ];
    for text in refused {
        let why = Seed::parse(&text)
            .err()
            .unwrap_or_else(|| panic!("{text:?} is a seed"));
        assert!(!why.is_empty());
        assert!(!why.contains(&secret), "{why}");
        if !text.is_empty() {
            assert!(!why.contains(&text), "{why}");
        }
    }
}

#[test]
fn a_seed_too_short_to_be_one_is_refused() {
    // Under 80 bits, as RFC 4226 asks at the least: a word that happens to be base32 is no seed.
    assert!(Seed::parse("ABCDEFGH").is_err());
    assert!(Seed::parse(&BASE32_NOPAD.encode(&[7u8; 10])).is_ok());
}

#[test]
fn only_an_otpauth_address_is_known_for_a_seed_by_its_look() {
    assert!(an_address("otpauth://totp/x?secret=GEZDGNBV"));
    assert!(an_address("  OTPAUTH://hotp/x"));
    assert!(!an_address("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"));
    assert!(!an_address("https://example.test/otpauth://"));
}
