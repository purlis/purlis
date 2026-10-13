// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use secrecy::ExposeSecret;

#[test]
fn github_com_has_a_device_endpoint_and_gitlab_com_only_a_token_one() {
    let github = Endpoints::of(crate::forge::registration::on(Kind::GitHub, "github.com").unwrap());
    assert_eq!(
        github.device_code.as_deref(),
        Some("https://github.com/login/device/code")
    );
    assert_eq!(github.token, "https://github.com/login/oauth/access_token");
    assert_eq!(github.host, "github.com");
    let gitlab = Endpoints::of(crate::forge::registration::on(Kind::GitLab, "gitlab.com").unwrap());
    assert_eq!(gitlab.device_code, None);
    assert_eq!(gitlab.token, "https://gitlab.com/oauth/token");
}

#[test]
fn a_grant_is_read_with_its_refresh_token_lifetime_and_scopes() {
    let answer = Answer::of(
        200,
        r#"{"access_token": "granted-access-value", "token_type": "bearer",
                "scope": "repo,read:org", "expires_in": 28800,
                "refresh_token": "granted-refresh-value", "refresh_token_expires_in": 15897600}"#,
    );
    let granted = answer.granted().unwrap();
    assert_eq!(granted.access.expose_secret(), "granted-access-value");
    assert_eq!(
        granted.refresh.as_ref().map(|r| r.expose_secret()),
        Some("granted-refresh-value")
    );
    assert_eq!(granted.expires_in, Some(Duration::from_secs(28800)));
    assert_eq!(
        granted.scopes.as_deref(),
        Some(&["repo".to_string(), "read:org".to_string()][..])
    );
    // Both are kept out of every log line and error from the moment they are read.
    assert!(crate::applog::holds_a_value_kept_out(
        "said granted-access-value"
    ));
    assert!(crate::applog::holds_a_value_kept_out(
        "said granted-refresh-value"
    ));
    let shown = format!("{granted:?}");
    assert!(!shown.contains("granted-"), "{shown}");
}

#[test]
fn gitlabs_space_separated_scopes_are_read_too() {
    let granted = Answer::of(
        200,
        r#"{"access_token": "gitlab-access-value", "scope": "api read_user"}"#,
    )
    .granted()
    .unwrap();
    assert_eq!(
        granted.scopes.as_deref(),
        Some(&["api".to_string(), "read_user".to_string()][..])
    );
    assert!(granted.refresh.is_none());
    assert_eq!(granted.expires_in, None);
}

#[test]
fn a_refusal_grants_nothing_and_names_its_error() {
    let answer = Answer::of(
        200,
        r#"{"error": "authorization_pending", "error_description": "not yet"}"#,
    );
    assert_eq!(answer.error(), Some("authorization_pending"));
    assert!(answer.granted().is_none());
    // Not JSON at all: an answer that says nothing.
    let page = Answer::of(502, "<html>bad gateway</html>");
    assert_eq!(page.error(), None);
    assert!(page.granted().is_none());
}

#[test]
fn an_answer_never_prints_what_it_holds() {
    let answer = Answer::of(200, r#"{"access_token": "never-printed-value"}"#);
    assert_eq!(format!("{answer:?}"), "Answer { status: 200, .. }");
}

#[test]
fn a_form_is_percent_encoded() {
    let body = encoded(&[
        ("scope", "repo read:org"),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ]);
    assert_eq!(
        body.as_str(),
        "scope=repo%20read%3Aorg&grant%5Ftype=urn%3Aietf%3Aparams%3Aoauth%3Agrant%2Dtype%3Adevice%5Fcode"
    );
}

#[test]
fn a_form_is_posted_over_https_only() {
    let https = Https::new();
    for url in ["http://github.com/login/device/code", "ftp://github.com/x"] {
        assert!(https.post(url, &[("client_id", "x")]).is_err(), "{url}");
    }
}

#[test]
fn a_forges_words_lose_kept_out_values_and_control_characters() {
    let held = SecretString::from("kept-out-of-a-sentence");
    crate::applog::keep_out(&held);
    let said = said_safely("bad\u{1b}[31m kept-out-of-a-sentence\nhere");
    assert!(!said.contains("kept-out-of-a-sentence"), "{said}");
    assert!(!said.contains('\u{1b}') && !said.contains('\n'), "{said}");
    assert_eq!(said_safely(&"x".repeat(500)).len(), MOST_SAID);
}
