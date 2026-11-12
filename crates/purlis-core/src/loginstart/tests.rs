// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::start_of;

const PAGE: &str = "https://app.example.com/login";
const FORM: Option<&str> = Some("https://auth.example.com");

/// An authorize address as an OpenID Connect client writes one: every value percent-encoded.
const AUTHORIZE: &str = "https://auth.example.com/oauth2/authorize?response_type=code\
                         &client_id=app-web&redirect_uri=https%3A%2F%2Fapp.example.com%2Fcallback\
                         &scope=openid%20profile&state=af0ifjsldkj\
                         &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM\
                         &code_challenge_method=S256#frag";

#[test]
fn an_authorize_address_on_the_sign_in_host_starts_with_its_query_and_fragment_as_written() {
    assert_eq!(start_of(AUTHORIZE, PAGE, FORM), Ok(AUTHORIZE.to_owned()));
}

#[test]
fn an_address_on_the_logins_own_page_site_starts_with_its_query() {
    let start = "https://app.example.com/login?next=%2Fdashboard";
    assert_eq!(start_of(start, PAGE, FORM), Ok(start.to_owned()));
    assert_eq!(start_of(start, PAGE, None), Ok(start.to_owned()));
}

#[test]
fn the_host_is_judged_as_a_browser_reads_it() {
    // The same origin however it is spelled.
    for start in [
        "https://AUTH.example.com/authorize",
        "https://auth.example.com:443/authorize",
        " https://auth.example.com/authorize ",
    ] {
        assert!(start_of(start, PAGE, FORM).is_ok(), "{start}");
    }
    // Another origin, however it hides.
    for start in [
        "https://evil.test/authorize",
        "https://auth.example.com.evil.test/authorize",
        "https://auth.example.com@evil.test/authorize",
        "https://evil.test%2f@auth.example.com/",
        "https://auth.example.com:8443/authorize",
        "http://auth.example.com/authorize",
        "https://example.com/authorize",
    ] {
        assert!(start_of(start, PAGE, FORM).is_err(), "{start}");
    }
}

#[test]
fn the_sign_in_host_is_refused_for_a_login_whose_form_is_on_its_own_page() {
    assert!(start_of(AUTHORIZE, PAGE, None).is_err());
}

#[test]
fn nothing_a_browser_reads_differently_starts() {
    for start in [
        "",
        "   ",
        "auth.example.com/authorize",
        "javascript:alert(1)",
        "https://auth.example.com\\@evil.test/",
        "https://auth.example.com/author\tize",
        "https://auth.example.com/author\nize",
        "https://auth.example.com/a b",
        "https://user:pass@auth.example.com/authorize",
    ] {
        assert!(start_of(start, PAGE, FORM).is_err(), "{start:?}");
    }
    let long = format!(
        "https://auth.example.com/?q={}",
        "a".repeat(super::MOST_START)
    );
    assert!(start_of(&long, PAGE, FORM).is_err());
}

#[test]
fn a_redirect_uri_off_the_logins_page_site_is_refused_decoded() {
    for start in [
        "https://auth.example.com/authorize?redirect_uri=https%3A%2F%2Fevil.test%2Fcb",
        "https://auth.example.com/authorize?redirect_uri=https://evil.test/cb",
        // The sign-in host's own pages are no way back to the app.
        "https://auth.example.com/authorize?redirect_uri=https%3A%2F%2Fauth.example.com%2Fdebug",
        // A second one an identity provider may read in place of the first.
        "https://auth.example.com/authorize?redirect_uri=https%3A%2F%2Fapp.example.com%2Fcb\
         &redirect_uri=https%3A%2F%2Fevil.test%2Fcb",
        // However its name is cased.
        "https://auth.example.com/authorize?Redirect_URI=https%3A%2F%2Fevil.test%2Fcb",
        // Its key encoded.
        "https://auth.example.com/authorize?redirect%5Furi=https%3A%2F%2Fevil.test%2Fcb",
        // One that isn't an address at all.
        "https://auth.example.com/authorize?redirect_uri=%2Fcb",
        "https://auth.example.com/authorize?redirect_uri=",
        // Encoded twice: what the provider decodes is still no address of the app.
        "https://auth.example.com/authorize?redirect_uri=https%253A%252F%252Fapp.example.com%252Fcb",
    ] {
        let why = start_of(start, PAGE, FORM).expect_err(start);
        assert!(why.contains("redirect_uri"), "{start}: {why}");
    }
}

#[test]
fn a_request_purlis_cant_read_the_redirect_of_is_refused() {
    for start in [
        "https://auth.example.com/authorize?client_id=app&request=eyJhbGciOiJub25lIn0.e30.",
        "https://auth.example.com/authorize?client_id=app&request_uri=urn%3Aietf%3Apar%3A1",
    ] {
        assert!(start_of(start, PAGE, FORM).is_err(), "{start}");
    }
}

#[test]
fn a_refusal_names_no_part_of_the_query() {
    let start = "https://auth.example.com/authorize?state=s3cr3t-state\
                 &redirect_uri=https%3A%2F%2Fevil.test%2Fcb%3Fcode%3Dc0de";
    let why = start_of(start, PAGE, FORM).unwrap_err();
    assert!(!why.contains("s3cr3t-state"), "{why}");
    assert!(!why.contains("c0de"), "{why}");
}
