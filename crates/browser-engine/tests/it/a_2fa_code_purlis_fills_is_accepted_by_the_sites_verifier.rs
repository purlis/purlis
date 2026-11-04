// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A 2FA code purlis fills (#5118, spec #5100), through a real engine on the fixture site: the
//! code goes into the site's code field as any secret does, and the site's own verifier, which
//! holds the seed on its server, accepts it. Only the code reaches the engine: purlis works it
//! out from the seed on the vault's side (`purlis_core::secrets::totp`), and neither the engine's
//! step nor the page ever holds the seed. The engine knows nothing of purlis-core, so the code
//! here is the verifier's own arithmetic: what ties it to purlis-core's is that both are held to
//! RFC 6238's vectors, below and in `secrets::totp`'s tests.
//!
//! These need a real Chrome, which a sandboxed agent cannot start: they run where
//! `PURLIS_TEST_ENGINE` names one, and on CI. Anywhere else each says why it did not run and
//! passes.

#![cfg(unix)]

use crate::common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::drive::{call, find, open, ref_on, sees, snapshot};
use common::site::{TWO_FACTOR_SEED, started, two_factor_code_at, unix_now};
use purlis_browser_engine::drive::{Browser, Call, FillHold, Ref, SecretFill};

/// What holds the browser to the field's site while its page lives (#5117): here, nothing.
struct Holds;

impl FillHold for Holds {
    fn hold(&self) -> Result<(), String> {
        Ok(())
    }

    fn released(&self) {}
}

/// The reference of the element on a line holding `line`, waiting for it to show.
fn ref_of(browser: &Browser, line: &str) -> Ref {
    let label = line.split('"').nth(1).unwrap_or(line);
    let start = Instant::now();
    loop {
        let found = find(browser, label);
        if found.lines().any(|l| l.contains(line) && l.contains('[')) {
            return ref_on(&found, line);
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{label} never showed: {found}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn the_sites_verifier_makes_rfc_6238s_codes() {
    // RFC 6238's own: the codes purlis-core's TOTP is held to as well, so the two agree.
    assert_eq!(two_factor_code_at(59), "287082");
    assert_eq!(two_factor_code_at(1_111_111_109), "081804");
}

#[test]
fn a_code_filled_as_any_secret_is_accepted_by_the_sites_verifier() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "two-factor.html");
    let field = ref_of(&browser, "textbox \"Authentication code\"");
    let code = two_factor_code_at(unix_now());
    browser
        .fill_secret(
            browser.epoch(),
            &SecretFill {
                target: field,
                origin: "http://fixture.test".to_owned(),
                key: "dash-2fa".to_owned(),
                value: code.clone().into(),
                multi_line: false,
            },
            Arc::new(Holds),
        )
        .unwrap_or_else(|_| panic!("the code was filled"));
    let verify = ref_of(&browser, "button \"Verify\"");
    call(&browser, Call::Click { target: verify });

    assert!(sees(&browser, "Code accepted"), "{}", snapshot(&browser));
    let seed = String::from_utf8_lossy(TWO_FACTOR_SEED).into_owned();
    assert!(
        !snapshot(&browser).contains(&seed),
        "the seed is in no page"
    );
}
