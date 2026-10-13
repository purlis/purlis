// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The OAuth device flow** (RFC 8628; ADR 0077 §3, as amended 2026-10-12): how a person signs
//! in to github.com through purlis's own registration, with no secret anywhere.
//!
//! Two acts.
//!
//! - [`start`] asks the forge for a device code with purlis's public client id and the scopes
//!   its registration asks for. The answer is a short **user code** and the forge's own page to
//!   type it into. The window shows both, and offers to open the page in the system browser.
//! - [`wait`] polls the forge's token endpoint, no faster than the forge said, until the person
//!   confirmed the code (a [`Granted`] token pair), refused it, or let it lapse.
//!
//! **Nothing in a flow is stored.** The device code is held in memory, as a secret, for the
//! flow's length, and a flow the person abandons leaves nothing behind. The user code is shown
//! in the window and nowhere else: this module never logs it.
//!
//! **The page is the forge's own.** A verification address that is not HTTPS on the
//! registration's own host is refused, so an answer that named another site could not send the
//! person there with a code to type.
//!
//! **The flow runs where the human scope is.** Only the window's process starts one, and what
//! it grants is signed in through [`super::accounts::sign_in_with_grant`], which takes the
//! host's scope. The `purlis` binary never names this module.

use std::path::Path;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

use super::accounts::{ForgeAccount, Method, Reach};
use super::oauth::{Answer, Endpoint, Endpoints, Granted, said_safely};
use super::registration::{Flow, Registration};
use super::route::HostScope;
use crate::secrets::keyring::Store;

/// The grant type a device flow polls with (RFC 8628 §3.4).
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// How often the forge is polled when it does not say (RFC 8628 §3.2).
const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// How much slower each `slow_down` makes the polling when the forge names no interval
/// (RFC 8628 §3.5).
const SLOW_DOWN_BY: Duration = Duration::from_secs(5);

/// The longest a code is waited on, whatever the forge says: GitHub's lasts fifteen minutes.
const LONGEST: Duration = Duration::from_secs(30 * 60);

/// How many polls in a row may go unanswered before the flow gives up: a laptop that changed
/// networks while the person was in the browser.
const MOST_UNANSWERED: u32 = 3;

/// A device flow the forge started: what the window shows, and what the polling needs.
pub struct Started {
    /// The code the person types on the forge's page.
    pub user_code: String,
    /// The forge's own page the code is typed into.
    pub verification_uri: String,
    /// How long the code lasts.
    pub expires_in: Duration,
    /// The least time between two polls.
    pub interval: Duration,
    device_code: SecretString,
}

impl std::fmt::Debug for Started {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Started")
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

/// How a flow ended without a grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// The code lapsed before it was confirmed.
    Expired,
    /// The person refused on the forge's page.
    Denied,
    /// The person cancelled in the window.
    Cancelled,
    /// The forge would not run the flow, or stopped answering: a sentence saying why.
    Failed(String),
}

impl Ended {
    /// What the window says, and what to do next.
    pub fn said(&self, forge: &str) -> String {
        match self {
            Ended::Expired => format!(
                "The code expired before it was confirmed on {forge}. Sign in again to get a new \
                 one."
            ),
            Ended::Denied => format!(
                "The sign-in was cancelled on {forge}, so purlis was not signed in. Sign in \
                 again when you want to."
            ),
            Ended::Cancelled => "The sign-in was cancelled.".to_string(),
            Ended::Failed(why) => why.clone(),
        }
    }
}

/// What a flow waits with between polls: the real clock, or a test's.
pub trait Pace {
    /// Waits `for_`, and says whether to go on: `false` when the person cancelled.
    fn wait(&self, for_: Duration) -> bool;
}

/// Whether `code` is a code a window may show: short, and letters, digits and dashes only.
fn showable(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 32
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Whether `address` is a page on `host` itself, over HTTPS; a test build also takes a
/// stand-in's page on this machine's loopback.
fn on_the_forge(address: &str, host: &str) -> bool {
    let Ok(uri) = address.parse::<http::Uri>() else {
        return false;
    };
    let Some(authority) = uri.authority() else {
        return false;
    };
    if authority.as_str().contains('@') || !authority.as_str().eq_ignore_ascii_case(host) {
        return false;
    }
    match uri.scheme_str() {
        Some("https") => true,
        Some("http") => {
            crate::fence::FENCED && matches!(authority.host(), "127.0.0.1" | "localhost")
        }
        _ => false,
    }
}

/// The sentence for a forge that answered a device flow's request with `error`.
fn refused(host: &str, error: &str, answer: &Answer) -> String {
    match error {
        "device_flow_disabled" => format!(
            "{host} has the device sign-in turned off for purlis. Sign in with a token instead."
        ),
        "incorrect_client_credentials" | "invalid_client" | "unauthorized_client" => format!(
            "{host} does not know this purlis. Update purlis, or sign in with a token instead."
        ),
        _ => {
            let said = answer.text("error_description").unwrap_or(error);
            format!(
                "{host} would not start the sign-in: {}. Try again, or sign in with a token \
                 instead.",
                said_safely(said)
            )
        }
    }
}

/// Starts a device flow for `registration` at `at`: the forge is asked for a code with the
/// registration's client id and scopes, and nothing else.
pub fn start(
    endpoint: &dyn Endpoint,
    at: &Endpoints,
    registration: &Registration,
) -> Result<Started, String> {
    let host = &at.host;
    let Some(url) = at
        .device_code
        .as_deref()
        .filter(|_| registration.flow == Flow::Device)
    else {
        return Err(format!(
            "purlis has no device sign-in on {host}. Sign in with a token instead."
        ));
    };
    let scope = registration.scopes().join(" ");
    let mut answer = endpoint
        .post(
            url,
            &[("client_id", registration.client_id), ("scope", &scope)],
        )
        .map_err(|why| {
            format!("purlis could not reach {host}: {why}. Check your connection and try again.")
        })?;
    if let Some(error) = answer.error() {
        return Err(refused(host, error, &answer));
    }
    let not_a_flow = || {
        format!(
            "{host} did not answer as a sign-in would (HTTP {}). Try again, or sign in with a \
             token instead.",
            answer.status
        )
    };
    let user_code = answer
        .text("user_code")
        .filter(|code| showable(code))
        .map(str::to_string)
        .ok_or_else(not_a_flow)?;
    let verification_uri = answer
        .text("verification_uri")
        .filter(|address| on_the_forge(address, host))
        .map(str::to_string)
        .ok_or_else(not_a_flow)?;
    let expires_in = answer
        .seconds("expires_in")
        .filter(|d| !d.is_zero())
        .unwrap_or(Duration::from_secs(900))
        .min(LONGEST);
    let interval = answer
        .seconds("interval")
        .filter(|d| !d.is_zero())
        .unwrap_or(DEFAULT_INTERVAL)
        .min(LONGEST);
    let missing = not_a_flow();
    let device_code = answer.secret("device_code").ok_or(missing)?;
    Ok(Started {
        user_code,
        verification_uri,
        expires_in,
        interval,
        device_code,
    })
}

/// Waits for the person to confirm `started`'s code: the token endpoint polled no faster than
/// the forge said, slower each time it says `slow_down`, until it grants a token, refuses, or
/// the code lapses. `pace` waits between polls and says when the person cancelled.
pub fn wait(
    endpoint: &dyn Endpoint,
    at: &Endpoints,
    registration: &Registration,
    started: &Started,
    pace: &dyn Pace,
) -> Result<Granted, Ended> {
    let host = &at.host;
    let mut interval = started.interval;
    let mut waited = Duration::ZERO;
    let mut unanswered = 0;
    loop {
        if waited + interval > started.expires_in {
            return Err(Ended::Expired);
        }
        if !pace.wait(interval) {
            return Err(Ended::Cancelled);
        }
        waited += interval;
        let answer = match endpoint.post(
            &at.token,
            &[
                ("client_id", registration.client_id),
                ("device_code", started.device_code.expose_secret()),
                ("grant_type", GRANT_TYPE),
            ],
        ) {
            Ok(answer) => answer,
            Err(why) => {
                unanswered += 1;
                if unanswered >= MOST_UNANSWERED {
                    return Err(Ended::Failed(format!(
                        "purlis could not reach {host}: {why}. Check your connection and sign \
                         in again."
                    )));
                }
                continue;
            }
        };
        unanswered = 0;
        match answer.error() {
            Some("authorization_pending") => {}
            Some("slow_down") => {
                // The forge's new interval when it names one, and never a faster one.
                interval = answer
                    .seconds("interval")
                    .filter(|named| *named > interval)
                    .unwrap_or(interval + SLOW_DOWN_BY)
                    .min(LONGEST);
            }
            Some("expired_token") => return Err(Ended::Expired),
            Some("access_denied") => return Err(Ended::Denied),
            Some(error) => return Err(Ended::Failed(refused(host, error, &answer))),
            None => {
                let status = answer.status;
                return answer.granted().ok_or_else(|| {
                    Ended::Failed(format!(
                        "{host} did not answer as a sign-in would (HTTP {status}). Sign in \
                         again, or sign in with a token instead."
                    ))
                });
            }
        }
    }
}

/// The whole second act: waits for `started`'s code to be confirmed ([`wait`]), then signs the
/// account in with what the forge granted ([`sign_in_with_grant`], [`Method::Device`]) on the
/// registration's own host. The account as listed, or how the flow ended.
///
/// [`sign_in_with_grant`]: super::accounts::sign_in_with_grant
#[allow(clippy::too_many_arguments)]
pub fn finish(
    host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    endpoint: &dyn Endpoint,
    at: &Endpoints,
    registration: &Registration,
    started: &Started,
    pace: &dyn Pace,
) -> Result<ForgeAccount, Ended> {
    let granted = wait(endpoint, at, registration, started, pace)?;
    super::accounts::sign_in_with_grant(
        host,
        config_root,
        keyring,
        reach,
        registration.kind,
        registration.host,
        granted,
        Method::Device,
    )
    .map_err(Ended::Failed)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::forge::Kind;
    use crate::forge::registration;
    use std::sync::Mutex;

    /// One posted form: the URL it went to, and its fields in order.
    pub(crate) type Posted = (String, Vec<(String, String)>);

    /// A stand-in forge: answers each posted form from a script, in order, and keeps what was
    /// posted.
    #[derive(Default)]
    pub(crate) struct StandIn {
        answers: Mutex<Vec<Result<(u16, String), String>>>,
        pub(crate) posted: Mutex<Vec<Posted>>,
    }

    impl StandIn {
        pub(crate) fn answering(answers: &[Result<(u16, &str), &str>]) -> StandIn {
            StandIn {
                answers: Mutex::new(
                    answers
                        .iter()
                        .rev()
                        .map(|a| match a {
                            Ok((status, body)) => Ok((*status, body.to_string())),
                            Err(why) => Err(why.to_string()),
                        })
                        .collect(),
                ),
                posted: Mutex::new(Vec::new()),
            }
        }

        pub(crate) fn unasked(&self) -> usize {
            self.answers.lock().unwrap().len()
        }
    }

    impl Endpoint for StandIn {
        fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<Answer, String> {
            self.posted.lock().unwrap().push((
                url.to_string(),
                form.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ));
            let next = self
                .answers
                .lock()
                .unwrap()
                .pop()
                .unwrap_or_else(|| Err("nothing scripted this request".to_string()));
            next.map(|(status, body)| Answer::of(status, &body))
        }
    }

    /// A clock that never sleeps: keeps each wait, and cancels after `cancel_after` of them.
    #[derive(Default)]
    struct Paced {
        waits: Mutex<Vec<Duration>>,
        cancel_after: Option<usize>,
    }

    impl Pace for Paced {
        fn wait(&self, for_: Duration) -> bool {
            let mut waits = self.waits.lock().unwrap();
            if self.cancel_after.is_some_and(|after| waits.len() >= after) {
                return false;
            }
            waits.push(for_);
            true
        }
    }

    fn github() -> (&'static Registration, Endpoints) {
        let registration = registration::on(Kind::GitHub, "github.com").unwrap();
        (registration, Endpoints::of(registration))
    }

    const CODES: &str = r#"{"device_code": "device-code-held-in-memory", "user_code": "WDJB-MJHT",
        "verification_uri": "https://github.com/login/device", "expires_in": 900, "interval": 5}"#;
    const PENDING: &str = r#"{"error": "authorization_pending"}"#;
    const GRANT: &str = r#"{"access_token": "device-access-value", "token_type": "bearer",
        "scope": "repo,read:org", "expires_in": 28800,
        "refresh_token": "device-refresh-value", "refresh_token_expires_in": 15897600}"#;

    fn started(forge: &StandIn) -> Started {
        let (registration, at) = github();
        start(forge, &at, registration).unwrap()
    }

    #[test]
    fn a_flow_starts_with_the_public_client_id_and_the_scopes_and_no_secret() {
        let forge = StandIn::answering(&[Ok((200, CODES))]);
        let flow = started(&forge);
        assert_eq!(flow.user_code, "WDJB-MJHT");
        assert_eq!(flow.verification_uri, "https://github.com/login/device");
        assert_eq!(flow.expires_in, Duration::from_secs(900));
        assert_eq!(flow.interval, Duration::from_secs(5));
        let posted = forge.posted.lock().unwrap().clone();
        assert_eq!(
            posted,
            [(
                "https://github.com/login/device/code".to_string(),
                vec![
                    ("client_id".to_string(), "Ov23litEn94I5HXrpdsX".to_string()),
                    ("scope".to_string(), "repo read:org".to_string()),
                ]
            )]
        );
        // The device code is held as a secret: never printed, and kept out of every log line.
        assert!(!format!("{flow:?}").contains("device-code-held"));
        assert!(crate::applog::holds_a_value_kept_out(
            "device-code-held-in-memory"
        ));
    }

    #[test]
    fn a_confirmed_code_is_granted_after_polling_at_the_forges_pace() {
        let forge = StandIn::answering(&[
            Ok((200, CODES)),
            Ok((200, PENDING)),
            Ok((200, PENDING)),
            Ok((200, GRANT)),
        ]);
        let flow = started(&forge);
        let (registration, at) = github();
        let pace = Paced::default();
        let granted = wait(&forge, &at, registration, &flow, &pace).unwrap();
        assert_eq!(granted.access.expose_secret(), "device-access-value");
        assert_eq!(
            granted.refresh.as_ref().map(|r| r.expose_secret()),
            Some("device-refresh-value")
        );
        assert_eq!(granted.expires_in, Some(Duration::from_secs(28800)));
        assert_eq!(*pace.waits.lock().unwrap(), [Duration::from_secs(5); 3]);
        let posted = forge.posted.lock().unwrap().clone();
        assert_eq!(posted.len(), 4);
        for (url, form) in &posted[1..] {
            assert_eq!(url, "https://github.com/login/oauth/access_token");
            assert_eq!(
                form,
                &[
                    ("client_id".to_string(), "Ov23litEn94I5HXrpdsX".to_string()),
                    (
                        "device_code".to_string(),
                        "device-code-held-in-memory".to_string()
                    ),
                    ("grant_type".to_string(), GRANT_TYPE.to_string()),
                ],
                "no client secret, and nothing else, is ever posted"
            );
        }
        assert_eq!(forge.unasked(), 0);
    }

    #[test]
    fn slow_down_is_honoured_with_the_forges_interval_or_five_seconds_more() {
        let forge = StandIn::answering(&[
            Ok((200, CODES)),
            Ok((200, r#"{"error": "slow_down", "interval": 12}"#)),
            Ok((200, r#"{"error": "slow_down"}"#)),
            // An interval shorter than the one in use never speeds the polling up.
            Ok((200, r#"{"error": "slow_down", "interval": 1}"#)),
            Ok((200, GRANT)),
        ]);
        let flow = started(&forge);
        let (registration, at) = github();
        let pace = Paced::default();
        wait(&forge, &at, registration, &flow, &pace).unwrap();
        assert_eq!(
            *pace.waits.lock().unwrap(),
            [5, 12, 17, 22].map(Duration::from_secs)
        );
    }

    #[test]
    fn an_interval_no_clock_could_hold_ends_the_flow_as_lapsed_and_never_overflows() {
        let (registration, at) = github();
        let codes = r#"{"device_code": "long-interval-device-code", "user_code": "AAAA-BBBB",
            "verification_uri": "https://github.com/login/device", "expires_in": 900,
            "interval": 18446744073709551615}"#;
        let forge = StandIn::answering(&[Ok((200, codes))]);
        let flow = started(&forge);
        assert_eq!(flow.interval, LONGEST);
        let pace = Paced::default();
        assert_eq!(
            wait(&forge, &at, registration, &flow, &pace).unwrap_err(),
            Ended::Expired
        );
        assert!(pace.waits.lock().unwrap().is_empty());

        let forge = StandIn::answering(&[
            Ok((200, CODES)),
            Ok((
                200,
                r#"{"error": "slow_down", "interval": 18446744073709551615}"#,
            )),
        ]);
        let flow = started(&forge);
        assert_eq!(
            wait(&forge, &at, registration, &flow, &Paced::default()).unwrap_err(),
            Ended::Expired
        );
    }

    #[test]
    fn a_lapsed_code_a_refusal_and_a_cancel_each_end_the_flow_in_their_own_words() {
        let (registration, at) = github();
        for (answer, ended, says) in [
            (
                r#"{"error": "expired_token"}"#,
                Ended::Expired,
                "The code expired",
            ),
            (
                r#"{"error": "access_denied"}"#,
                Ended::Denied,
                "cancelled on GitHub",
            ),
        ] {
            let forge =
                StandIn::answering(&[Ok((200, CODES)), Ok((200, PENDING)), Ok((200, answer))]);
            let flow = started(&forge);
            let why = wait(&forge, &at, registration, &flow, &Paced::default()).unwrap_err();
            assert_eq!(why, ended);
            assert!(why.said("GitHub").contains(says), "{}", why.said("GitHub"));
        }

        // Cancelled in the window: no further request is sent.
        let forge = StandIn::answering(&[Ok((200, CODES)), Ok((200, PENDING))]);
        let flow = started(&forge);
        let pace = Paced {
            cancel_after: Some(1),
            ..Paced::default()
        };
        assert_eq!(
            wait(&forge, &at, registration, &flow, &pace).unwrap_err(),
            Ended::Cancelled
        );
        assert_eq!(forge.posted.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_code_the_forge_never_ends_stops_being_polled_when_it_would_have_lapsed() {
        let short = r#"{"device_code": "short-lived-device-code", "user_code": "AAAA-BBBB",
            "verification_uri": "https://github.com/login/device", "expires_in": 12, "interval": 5}"#;
        let forge = StandIn::answering(&[
            Ok((200, short)),
            Ok((200, PENDING)),
            Ok((200, PENDING)),
            Ok((200, PENDING)),
        ]);
        let flow = started(&forge);
        let (registration, at) = github();
        let pace = Paced::default();
        assert_eq!(
            wait(&forge, &at, registration, &flow, &pace).unwrap_err(),
            Ended::Expired
        );
        assert_eq!(pace.waits.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_forge_that_stops_answering_is_tried_again_and_then_given_up_on() {
        let (registration, at) = github();
        // One missed poll is a changed network: the flow goes on.
        let forge = StandIn::answering(&[
            Ok((200, CODES)),
            Err("timed out"),
            Err("timed out"),
            Ok((200, GRANT)),
        ]);
        let flow = started(&forge);
        assert!(wait(&forge, &at, registration, &flow, &Paced::default()).is_ok());

        let forge = StandIn::answering(&[
            Ok((200, CODES)),
            Err("timed out"),
            Err("timed out"),
            Err("timed out"),
        ]);
        let flow = started(&forge);
        let why = wait(&forge, &at, registration, &flow, &Paced::default()).unwrap_err();
        let Ended::Failed(said) = why else {
            panic!("{why:?}");
        };
        assert!(said.contains("could not reach github.com"), "{said}");
    }

    #[test]
    fn a_forge_that_will_not_run_the_flow_says_so_and_offers_the_token() {
        let (registration, at) = github();
        for (answer, says) in [
            (
                Ok((200, r#"{"error": "device_flow_disabled"}"#)),
                "device sign-in turned off",
            ),
            (
                Ok((200, r#"{"error": "incorrect_client_credentials"}"#)),
                "does not know this purlis",
            ),
            (
                Ok((
                    400,
                    r#"{"error": "invalid_scope", "error_description": "no such scope"}"#,
                )),
                "no such scope",
            ),
            (Ok((502, "<html>bad gateway</html>")), "HTTP 502"),
            (Err("no route to host"), "could not reach github.com"),
        ] {
            let forge = StandIn::answering(&[answer]);
            let why = start(&forge, &at, registration).unwrap_err();
            assert!(why.contains(says), "{why}");
            assert!(why.contains("token") || why.contains("connection"), "{why}");
        }
    }

    #[test]
    fn a_verification_page_that_is_not_the_forges_own_is_refused() {
        let (registration, at) = github();
        for address in [
            "https://github.com.evil.example/login/device",
            "https://evil.example/login/device",
            "http://github.com/login/device",
            "javascript:alert(1)",
            "",
        ] {
            let codes = format!(
                r#"{{"device_code": "another-device-code-value", "user_code": "WDJB-MJHT",
                    "verification_uri": "{address}", "expires_in": 900, "interval": 5}}"#
            );
            let forge = StandIn::answering(&[Ok((200, codes.as_str()))]);
            let why = start(&forge, &at, registration).unwrap_err();
            assert!(
                why.contains("did not answer as a sign-in would"),
                "{address}: {why}"
            );
        }
    }

    #[test]
    fn a_user_code_that_is_not_a_short_plain_code_is_refused() {
        let (registration, at) = github();
        for code in ["", "<b>x</b>", "A B", &"A".repeat(40)] {
            let codes = format!(
                r#"{{"device_code": "another-device-code-value", "user_code": "{code}",
                    "verification_uri": "https://github.com/login/device"}}"#
            );
            let forge = StandIn::answering(&[Ok((200, codes.as_str()))]);
            assert!(start(&forge, &at, registration).is_err(), "{code}");
        }
    }

    #[test]
    fn gitlab_com_has_no_device_flow_here() {
        let registration = registration::on(Kind::GitLab, "gitlab.com").unwrap();
        let forge = StandIn::answering(&[]);
        let why = start(&forge, &Endpoints::of(registration), registration).unwrap_err();
        assert!(why.contains("no device sign-in on gitlab.com"), "{why}");
        assert!(forge.posted.lock().unwrap().is_empty());
    }

    #[test]
    fn a_forges_own_words_never_carry_the_device_code_into_a_sentence() {
        let (registration, at) = github();
        let forge = StandIn::answering(&[
            Ok((200, CODES)),
            Ok((
                200,
                r#"{"error": "bad_verification_code",
                    "error_description": "device-code-held-in-memory is unknown"}"#,
            )),
        ]);
        let flow = started(&forge);
        let why = wait(&forge, &at, registration, &flow, &Paced::default()).unwrap_err();
        assert!(
            !why.said("GitHub").contains("device-code-held-in-memory"),
            "{}",
            why.said("GitHub")
        );
    }
}
