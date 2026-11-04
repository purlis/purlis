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
fn an_address_is_found_anywhere_in_a_value_in_any_case() {
    assert!(holds_an_address("otpauth://totp/x?secret=GEZDGNBV"));
    assert!(holds_an_address("  OTPAUTH://hotp/x"));
    assert!(holds_an_address("otpauth-migration://offline?data=AAAA"));
    assert!(holds_an_address("my login\nOtpAuth://totp/x"));
    assert!(holds_an_address("é otpauth://totp/x"));
    assert!(!holds_an_address("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"));
    assert!(!holds_an_address("otpauth"));
}

#[test]
fn only_an_otpauth_address_is_known_for_a_seed_by_its_look() {
    assert!(an_address("otpauth://totp/x?secret=GEZDGNBV"));
    assert!(an_address("  OTPAUTH://hotp/x"));
    assert!(!an_address("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"));
    assert!(!an_address("https://example.test/otpauth://"));
}

#[test]
fn a_bare_secret_is_a_seed_only_at_the_lengths_apps_are_given() {
    for bytes in [10, 16, 20, 32, 64] {
        assert!(
            Seed::parse(&BASE32_NOPAD.encode(&vec![7u8; bytes])).is_ok(),
            "{bytes}"
        );
    }
    // A password that happens to be base32 at another length is no seed.
    for bytes in [11, 15, 24, 40] {
        assert!(
            Seed::parse(&BASE32_NOPAD.encode(&vec![7u8; bytes])).is_err(),
            "{bytes}"
        );
    }
    // In an address, a seed is a seed at any length from the fewest.
    let named = format!(
        "otpauth://totp/x?secret={}",
        BASE32_NOPAD.encode(&[7u8; 15])
    );
    assert!(Seed::parse(&named).is_ok());
}

#[test]
fn a_value_that_would_be_taken_as_a_seed_is_known_for_one() {
    assert!(looks_like_a_seed("gezd gnbv gy3t qojq gezd gnbv gy3t qojq"));
    assert!(looks_like_a_seed("otpauth://totp/x?secret=GEZDGNBV"));
    assert!(!looks_like_a_seed("fixture-key-5118"));
    assert!(!looks_like_a_seed("fixture-token-0189"));
}

#[test]
fn a_seed_grouped_cut_or_exported_another_way_still_looks_like_one() {
    // In groups of hyphens, at a length apps aren't given, and an app's export.
    assert!(looks_like_a_seed("GEZD-GNBV-GY3T-QOJQ-GEZD-GNBV"));
    assert!(looks_like_a_seed("GEZDGNBVGY3TQOJQGEZDGNBV"));
    assert!(looks_like_a_seed(&BASE32_NOPAD.encode(&[7u8; 25])));
    assert!(looks_like_a_seed("otpauth-migration://offline?data=abc"));
    // 26 characters whose last one's unused bits aren't zero, as a generator leaves them.
    let picked = "GEZDGNBVGY3TQOJQGEZDGNBVG7";
    assert!(looks_like_a_seed(picked));
    assert_eq!(Seed::parse(picked).unwrap().code_at(59).len(), 6);
    // Grouped in hyphens, it makes the code it makes whole.
    assert_eq!(
        Seed::parse("GEZD-GNBV-GY3T-QOJQ-GEZD-GNBV-GY3T-QOJQ")
            .unwrap()
            .code_at(59),
        Seed::parse("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")
            .unwrap()
            .code_at(59),
    );
}

#[test]
fn digits_alone_and_a_passphrase_are_no_seed() {
    // Every digit is in base32's alphabet, as a card's number may be.
    for value in [
        "2345234523452346",
        "7654 7654 7654 7654",
        "3333444455556666",
    ] {
        assert!(!looks_like_a_seed(value), "{value}");
        assert!(Seed::parse(value).is_err(), "{value}");
    }
    assert!(!looks_like_a_seed("correct-horse-battery-staple"));
}

#[test]
fn text_in_both_cases_is_no_seed_at_any_length() {
    // A password or an API key, base32's characters by chance: no app gives a seed so.
    for value in [
        "aB3dE5fG7hJ2kL4mN6pQ",
        "aB3dE5fG7hJ2kL4m",
        "aB3dE5fG7hJ2kL4mN6pQrS2tU3vW4xY5",
    ] {
        assert!(!looks_like_a_seed(value), "{value}");
        assert!(Seed::parse(value).is_err(), "{value}");
    }
    // Everything a bare seed is taken as still looks like one.
    let upper = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
    assert!(looks_like_a_seed(upper) && Seed::parse(upper).is_ok());
    let lower = upper.to_ascii_lowercase();
    assert!(looks_like_a_seed(&lower) && Seed::parse(&lower).is_ok());
}

#[test]
fn a_percent_encoded_secret_reads_as_its_plain_one() {
    let plain = Seed::parse(&format!(
        "otpauth://totp/x?secret={}",
        BASE32_NOPAD.encode(SHA1_SEED)
    ))
    .unwrap();
    let encoded = Seed::parse(&format!(
        "otpauth://totp/Dash%3Aalice?issuer=Dash&secret={}%3D%3D",
        BASE32_NOPAD.encode(SHA1_SEED)
    ))
    .unwrap();
    assert_eq!(plain.code_at(59), encoded.code_at(59));
}
