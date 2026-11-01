// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The authorization code flow with PKCE (ADR 0077 §1 and §3, RFC 7636, RFC 8252): the verifier
//! and its S256 challenge, the `state`, the address of the forge's authorization page, reading
//! the one redirect the browser comes back with, and [`sign_in`], which runs the flow from the
//! listener to the code's exchange. The listener itself is [`super::loopback`]'s.
//!
//! **Nothing here is stored.** A [`Verifier`] and a [`State`] are made for one sign-in, live in
//! memory for its length, and are wiped when dropped (ADR 0077 §3: *"A flow the person abandons
//! leaves nothing behind"*). Neither prints.
//!
//! **Why a code that leaks is useless.** The redirect comes back over the loopback, through the
//! person's browser, so the code in it must be assumed readable by other software on the
//! machine. The forge hands a token for that code only to whoever also sends the verifier whose
//! SHA-256 it was shown at the start, and the verifier never leaves this process except in that
//! one exchange, over HTTPS, to the forge. The application has no client secret, so nothing
//! else stands in for it.
//!
//! **Why a redirect is believed only with its `state`.** Anything on the machine can send the
//! loopback listener a request. Only the browser the person was sent to holds the `state` made
//! for this sign-in, so a redirect without it is refused ([`read_redirect`]), compared in
//! constant time, and never ends the sign-in.

use base64::Engine as _;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::loopback::{Clock, Listening, Stopped, take_the_redirect};
use super::oauth::{Endpoint, Endpoints, Granted, said_safely};
use super::registration::{Flow, REDIRECT_PATH, REDIRECT_PORTS, Registration, redirect_uri};
use super::route::HostScope;

/// The longest code or `state` a redirect may carry. A forge's are tens of characters.
const MOST_VALUE: usize = 512;

/// 32 random bytes as 43 characters of unpadded URL-safe base64: the alphabet and the length
/// RFC 7636 §4.1 allows a verifier, and what a `state` is made of too.
fn random_word() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| format!("purlis could not get random bytes for a sign-in: {e}"))?;
    let word = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    bytes.zeroize();
    Ok(word)
}

/// The PKCE code verifier of one sign-in. Sent once, to the forge's token address.
pub struct Verifier(SecretString);

impl Verifier {
    /// A new verifier, from the operating system's random source.
    pub fn random() -> Result<Verifier, String> {
        let mut word = random_word()?;
        let held = SecretString::from(word.as_str());
        word.zeroize();
        Ok(Verifier(held))
    }

    /// A verifier with a known value. Only a test build has it.
    #[cfg(any(test, feature = "fenced"))]
    pub fn for_a_test(value: &str) -> Verifier {
        Verifier(SecretString::from(value))
    }

    /// The S256 challenge the authorization page is shown: the verifier's SHA-256, as unpadded
    /// URL-safe base64 (RFC 7636 §4.2).
    pub fn challenge(&self) -> String {
        let digest = Sha256::digest(self.0.expose_secret().as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    }

    /// The verifier itself, for the one exchange that sends it.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for Verifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Verifier(***)")
    }
}

/// The `state` of one sign-in: what a redirect must carry to be this sign-in's.
pub struct State(SecretString);

impl State {
    /// A new `state`, from the operating system's random source.
    pub fn random() -> Result<State, String> {
        let mut word = random_word()?;
        let held = SecretString::from(word.as_str());
        word.zeroize();
        Ok(State(held))
    }

    /// A `state` with a known value. Only a test build has it.
    #[cfg(any(test, feature = "fenced"))]
    pub fn for_a_test(value: &str) -> State {
        State(SecretString::from(value))
    }

    /// Whether `offered` is this `state`, compared in constant time. The two are hashed first,
    /// so the comparison takes the same time whatever the length of what was offered.
    pub fn matches(&self, offered: &str) -> bool {
        let mine = Sha256::digest(self.0.expose_secret().as_bytes());
        let theirs = Sha256::digest(offered.as_bytes());
        bool::from(mine.as_slice().ct_eq(theirs.as_slice()))
    }

    pub(super) fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("State(***)")
    }
}

/// The authorization code a redirect carried. Sent once, to the forge's token address.
pub struct Code(SecretString);

impl Code {
    /// The code itself, for the one exchange that sends it.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Code(***)")
    }
}

/// A query value, percent-encoded: everything but RFC 3986's unreserved characters.
fn encoded(value: &str) -> String {
    const KEPT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    percent_encoding::utf8_percent_encode(value, KEPT).to_string()
}

/// The address of the forge's authorization page for one sign-in through `registration`: the
/// page the person's own browser is sent to (RFC 8252 §8.12, never a webview). It names the
/// client id, the loopback `redirect_uri` exactly as registered, the scopes, the `state` and the
/// S256 challenge. Always HTTPS, always the registration's own host.
pub fn authorize_url(
    registration: &Registration,
    redirect_uri: &str,
    state: &State,
    verifier: &Verifier,
) -> String {
    format!(
        "https://{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}\
         &scope={}&code_challenge={}&code_challenge_method=S256",
        registration.host,
        encoded(registration.client_id),
        encoded(redirect_uri),
        encoded(state.expose()),
        encoded(&registration.scopes().join(" ")),
        encoded(&verifier.challenge()),
    )
}

/// What one request to the loopback listener was.
#[derive(Debug)]
pub enum Redirected {
    /// This sign-in's redirect, with the code to exchange.
    Code(Code),
    /// This sign-in's redirect, saying the person or the forge said no. The `state` matched, so
    /// the sign-in ends here. Holds the forge's `error` word when it is one of OAuth's own.
    Declined(Declined),
    /// Not this sign-in's redirect: another path, no `state` or the wrong one, or a value no
    /// forge would send. Refused, and the sign-in goes on waiting.
    NotOurs,
}

/// Why a sign-in's own redirect carried no code (RFC 6749 §4.1.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declined {
    /// `access_denied`: the person pressed the forge's own "deny".
    ByThePerson,
    /// Any other answer: the forge would not authorize the application as asked.
    ByTheForge,
}

/// A percent-encoded query value, decoded; `None` when it is not UTF-8, is longer than a forge
/// would send, or holds a control character.
fn decoded(raw: &str) -> Option<String> {
    if raw.len() > MOST_VALUE * 3 {
        return None;
    }
    let spaced = raw.replace('+', " ");
    let value = percent_encoding::percent_decode_str(&spaced)
        .decode_utf8()
        .ok()?;
    (value.len() <= MOST_VALUE && !value.chars().any(char::is_control)).then(|| value.into_owned())
}

/// Reads `target`, the request target of one `GET` to the loopback listener (the path and the
/// query, as the request line has them), as this sign-in's redirect.
///
/// The path must be the registered one, exactly. The `state` is checked before anything else in
/// the query is believed: a request without this sign-in's `state` is [`Redirected::NotOurs`]
/// whatever else it says, so nothing on the machine can end a sign-in, or hand it a code, by
/// calling the listener. Nothing of the query is kept but the code.
pub fn read_redirect(target: &str, state: &State) -> Redirected {
    read_redirect_at(REDIRECT_PATH, target, state)
}

/// [`read_redirect`], for a redirect that comes back on `at`.
pub fn read_redirect_at(at: &str, target: &str, state: &State) -> Redirected {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != at {
        return Redirected::NotOurs;
    }
    let mut code = None;
    let mut offered = None;
    let mut error = None;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let slot = match name {
            "code" => &mut code,
            "state" => &mut offered,
            "error" => &mut error,
            _ => continue,
        };
        // A name given twice is no forge's redirect.
        if slot.is_some() {
            return Redirected::NotOurs;
        }
        let Some(value) = decoded(value) else {
            return Redirected::NotOurs;
        };
        *slot = Some(value);
    }
    if !offered.is_some_and(|offered| state.matches(&offered)) {
        return Redirected::NotOurs;
    }
    match (code, error) {
        (Some(mut code), None) if !code.is_empty() => {
            let held = SecretString::from(code.as_str());
            code.zeroize();
            Redirected::Code(Code(held))
        }
        (None, Some(error)) if error == "access_denied" => {
            Redirected::Declined(Declined::ByThePerson)
        }
        _ => Redirected::Declined(Declined::ByTheForge),
    }
}

// ---------------------------------------------------------------------------------------
// The flow.

/// How long a sign-in waits for the browser to come back. A forge's authorization code lives
/// ten minutes at most; a person who has not approved within five has gone elsewhere, and the
/// listener should not outlive their attention.
pub const ALLOWED: Duration = Duration::from_secs(5 * 60);

/// Posts one form, as names and values, to a token address and reads `G` from the answer.
pub type Exchange<'a, G> = dyn Fn(&[(&str, &str)]) -> Result<G, String> + 'a;

/// What a sign-in does outside this process, each replaced by a stand-in in a test.
pub struct Ways<'a, G> {
    /// Binds the loopback listener on the first unused of the ports given
    /// ([`super::loopback::bind`]).
    pub bind: &'a dyn Fn(&[u16]) -> Result<Listening, String>,
    /// Opens an address in the person's own browser.
    pub open: &'a dyn Fn(&str) -> Result<(), String>,
    /// Posts the form to the registration's token address and reads the grant
    /// ([`super::oauth`]). The form's values are secret: the code and the verifier.
    pub exchange: &'a Exchange<'a, G>,
    pub clock: Clock<'a>,
}

/// Why a listener that stopped without a code did, as the person is told.
fn stopped(why: Stopped, host: &str) -> String {
    const OR_A_TOKEN: &str = "Sign in again, or sign in with a token instead.";
    match why {
        Stopped::Cancelled => "The sign-in was cancelled.".to_string(),
        Stopped::TimedOut => format!(
            "purlis waited five minutes and {host}'s answer did not come back from your \
             browser. {OR_A_TOKEN}"
        ),
        Stopped::Declined(Declined::ByThePerson) => {
            format!("You declined on {host}, so nothing was signed in.")
        }
        Stopped::Declined(Declined::ByTheForge) => {
            format!("{host} would not authorize purlis as it asked. {OR_A_TOKEN}")
        }
        Stopped::Strangers => format!(
            "Other software on this machine kept calling the address purlis takes the \
             browser's answer on, so purlis stopped listening. {OR_A_TOKEN}"
        ),
        Stopped::Failed => {
            format!("purlis's listener for the browser's answer stopped working. {OR_A_TOKEN}")
        }
    }
}

/// Signs in through `registration` in the person's browser (ADR 0077 §3): binds the loopback
/// listener, opens the forge's authorization page, takes the one redirect, and exchanges its
/// code, with the verifier and no secret, for the grant `ways.exchange` reads.
///
/// **Only the host runs it**: it takes a [`HostScope`], which is refused inside a chat's process
/// tree. The listener is closed before this returns, whatever the outcome, and the verifier,
/// the `state` and the code are dropped with it: nothing of a flow is kept. `cancelled` stops it
/// at the listener's next look.
///
/// A refusal is a sentence for the person. It holds nothing the browser or the forge said.
pub fn sign_in<G>(
    _host: &HostScope,
    registration: &Registration,
    cancelled: &AtomicBool,
    ways: &Ways<'_, G>,
) -> Result<G, String> {
    if registration.flow != Flow::Pkce {
        return Err(format!("{} does not sign in this way.", registration.host));
    }
    let verifier = Verifier::random()?;
    let state = State::random()?;
    let listening = (ways.bind)(&REDIRECT_PORTS)?;
    let redirect = redirect_uri(listening.port());
    let page = authorize_url(registration, &redirect, &state, &verifier);
    if (ways.open)(&page).is_err() {
        // The listener is dropped here: nothing is left waiting for a page nobody saw.
        return Err(format!(
            "purlis could not open your browser to sign in to {}. Check that this machine has a \
             default browser and sign in again, or sign in with a token instead.",
            registration.host
        ));
    }
    drop(page);
    let code = take_the_redirect(listening, &state, ALLOWED, cancelled, &ways.clock)
        .map_err(|why| stopped(why, registration.host))?;
    if cancelled.load(Ordering::SeqCst) {
        return Err(stopped(Stopped::Cancelled, registration.host));
    }
    (ways.exchange)(&[
        ("grant_type", "authorization_code"),
        ("client_id", registration.client_id),
        ("code", code.expose()),
        ("code_verifier", verifier.expose()),
        ("redirect_uri", &redirect),
    ])
}

/// The exchange of a sign-in, for [`Ways::exchange`]: the form posted through `endpoint` to
/// the token address of `endpoints`, which are the registration's own
/// ([`Endpoints::of`]), and the grant read from the answer. The form carries the code and the verifier and no secret: the application has none.
///
/// A refusal is a sentence for the person. Of the forge's answer it repeats one thing, OAuth's
/// own `error` word, made safe to print; never a description, and never anything posted.
pub fn exchange_at<'a>(
    endpoint: &'a dyn Endpoint,
    endpoints: &'a Endpoints,
) -> impl Fn(&[(&str, &str)]) -> Result<Granted, String> + 'a {
    let host = endpoints.host.as_str();
    move |form| {
        let answer = endpoint.post(&endpoints.token, form).map_err(|why| {
            format!(
                "purlis could not reach {host} to finish the sign-in ({}). Check your \
                 connection and sign in again.",
                said_safely(&why)
            )
        })?;
        match answer.error().map(said_safely) {
            Some(word) if word == "invalid_grant" => Err(format!(
                "{host} would not take the sign-in's code: it had expired or was already used. \
                 Sign in again."
            )),
            Some(word) => Err(format!(
                "{host} refused to finish the sign-in ({word}). Sign in again, or sign in with \
                 a token instead."
            )),
            None => answer.granted().ok_or_else(|| {
                format!("{host} answered the sign-in without a token. Sign in again.")
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Kind;
    use super::super::registration;
    use super::*;

    #[test]
    fn the_challenge_is_rfc_7636s_own_example() {
        // RFC 7636, appendix B.
        let verifier = Verifier::for_a_test("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(
            verifier.challenge(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_verifier_and_a_state_are_43_url_safe_characters_and_new_each_time() {
        let (a, b) = (Verifier::random().unwrap(), Verifier::random().unwrap());
        let (s, t) = (State::random().unwrap(), State::random().unwrap());
        for word in [a.expose(), b.expose(), s.expose(), t.expose()] {
            assert_eq!(word.len(), 43);
            assert!(
                word.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            );
        }
        assert_ne!(a.expose(), b.expose());
        assert_ne!(s.expose(), t.expose());
        assert_ne!(a.expose(), s.expose());
    }

    #[test]
    fn neither_a_verifier_a_state_nor_a_code_prints() {
        let verifier = Verifier::for_a_test("the-verifier-of-this-sign-in");
        let state = State::for_a_test("the-state-of-this-sign-in");
        let Redirected::Code(code) = read_redirect(
            "/purlis/signin?code=the-code-it-carried&state=the-state-of-this-sign-in",
            &state,
        ) else {
            panic!("the redirect was not read");
        };
        let printed = format!("{verifier:?} {state:?} {code:?}");
        assert_eq!(printed, "Verifier(***) State(***) Code(***)");
    }

    #[test]
    fn the_authorization_page_is_the_registrations_own_over_https_with_the_s256_challenge() {
        let gitlab = registration::on(Kind::GitLab, "gitlab.com").unwrap();
        let url = authorize_url(
            gitlab,
            &registration::redirect_uri(47824),
            &State::for_a_test("st4te_-"),
            &Verifier::for_a_test("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        );
        assert_eq!(
            url,
            format!(
                "https://gitlab.com/oauth/authorize?client_id={}\
                 &redirect_uri=http%3A%2F%2F127.0.0.1%3A47824%2Fpurlis%2Fsignin\
                 &response_type=code&state=st4te_-&scope=api\
                 &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM\
                 &code_challenge_method=S256",
                gitlab.client_id
            )
        );
        // The verifier itself is never in the address the browser is sent to.
        assert!(!url.contains("dBjftJeZ4CVP"));
    }

    #[test]
    fn the_redirect_with_this_sign_ins_state_gives_its_code() {
        let state = State::for_a_test("s-1");
        for target in [
            "/purlis/signin?code=c0de&state=s-1",
            "/purlis/signin?state=s-1&code=c0de",
            "/purlis/signin?code=c0de&state=s%2D1&session_state=ignored",
        ] {
            match read_redirect(target, &state) {
                Redirected::Code(code) => assert_eq!(code.expose(), "c0de", "{target}"),
                other => panic!("{target}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_request_without_this_sign_ins_state_is_not_its_redirect_whatever_it_says() {
        let state = State::for_a_test("s-1");
        for target in [
            "/purlis/signin?code=c0de",
            "/purlis/signin?code=c0de&state=",
            "/purlis/signin?code=c0de&state=s-2",
            "/purlis/signin?code=c0de&state=s-1x",
            "/purlis/signin?code=c0de&state=S-1",
            "/purlis/signin?error=access_denied",
            "/purlis/signin?error=access_denied&state=other",
            "/purlis/signin",
            "/purlis/signin?",
            // Another path, even with the state.
            "/?code=c0de&state=s-1",
            "/purlis/signin/?code=c0de&state=s-1",
            "/purlis/signin/more?code=c0de&state=s-1",
            "/charter/signin?code=c0de&state=s-1",
            "/favicon.ico",
            // A name given twice.
            "/purlis/signin?code=c0de&state=s-2&state=s-1",
            "/purlis/signin?code=c0de&code=other&state=s-1",
            // Not UTF-8, and a control character.
            "/purlis/signin?code=%ff&state=s-1",
            "/purlis/signin?code=c0de%0d%0a&state=s-1",
        ] {
            assert!(
                matches!(read_redirect(target, &state), Redirected::NotOurs),
                "{target}"
            );
        }
        let long = format!(
            "/purlis/signin?code={}&state=s-1",
            "c".repeat(MOST_VALUE + 1)
        );
        assert!(matches!(read_redirect(&long, &state), Redirected::NotOurs));
    }

    #[test]
    fn this_sign_ins_redirect_without_a_code_ends_it_as_declined() {
        let state = State::for_a_test("s-1");
        for (target, why) in [
            (
                "/purlis/signin?error=access_denied&error_description=The+resource+owner+denied&state=s-1",
                Declined::ByThePerson,
            ),
            (
                "/purlis/signin?error=invalid_scope&state=s-1",
                Declined::ByTheForge,
            ),
            ("/purlis/signin?state=s-1", Declined::ByTheForge),
            ("/purlis/signin?code=&state=s-1", Declined::ByTheForge),
            (
                "/purlis/signin?code=c0de&error=access_denied&state=s-1",
                Declined::ByTheForge,
            ),
        ] {
            match read_redirect(target, &state) {
                Redirected::Declined(got) => assert_eq!(got, why, "{target}"),
                other => panic!("{target}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_state_matches_only_itself() {
        let state = State::for_a_test("exactly-this");
        assert!(state.matches("exactly-this"));
        for other in ["", "exactly-thi", "exactly-this ", "Exactly-this", "x"] {
            assert!(!state.matches(other), "{other:?}");
        }
    }

    // -----------------------------------------------------------------------------------
    // The flow, with a stand-in browser, listener and token address.

    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use super::super::loopback::standin::{Queue, get};
    use super::super::loopback::{self, Incoming};

    /// The value of `name` in `url`'s query, percent-decoded.
    fn query(url: &str, name: &str) -> String {
        let (_, query) = url.split_once('?').unwrap();
        let raw = query
            .split('&')
            .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
            .unwrap_or_else(|| panic!("{name} is not in {url}"));
        percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .unwrap()
            .into_owned()
    }

    /// A stand-in browser: handed the authorization page's address and the listener's queue.
    type Browser = dyn Fn(&str, &mut Queue) -> Result<(), String>;

    /// What a run of the flow did, as the stand-ins saw it.
    #[derive(Default)]
    struct Seen {
        opened: Vec<String>,
        posted: Vec<Vec<(String, String)>>,
        bound: usize,
    }

    /// Runs the flow on `port`. `browser` is handed the authorization page's address and the
    /// listener's queue, and comes back as a browser would, or not.
    fn run(port: u16, cancelled: &AtomicBool, browser: &Browser) -> (Result<String, String>, Seen) {
        let seen = Mutex::new(Seen::default());
        let queue = Arc::new(Mutex::new(Queue::default()));
        let start = Instant::now();
        let passed = Mutex::new(Duration::ZERO);
        let now = || start + *passed.lock().unwrap();
        let rest = |wait: Duration| *passed.lock().unwrap() += wait;
        let bind = |ports: &[u16]| {
            assert_eq!(ports, REDIRECT_PORTS);
            seen.lock().unwrap().bound += 1;
            Ok(Listening::over(
                Box::new(queue.clone()) as Box<dyn Incoming + Send>,
                port,
            ))
        };
        let open = |page: &str| {
            seen.lock().unwrap().opened.push(page.to_string());
            browser(page, &mut queue.lock().unwrap())
        };
        let exchange = |form: &[(&str, &str)]| {
            seen.lock().unwrap().posted.push(
                form.iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
            );
            Ok("the grant".to_string())
        };
        let got = sign_in(
            &HostScope::for_a_test(),
            registration::on(Kind::GitLab, "gitlab.com").unwrap(),
            cancelled,
            &Ways {
                bind: &bind,
                open: &open,
                exchange: &exchange,
                clock: loopback::Clock {
                    now: &now,
                    rest: &rest,
                },
            },
        );
        (got, seen.into_inner().unwrap())
    }

    /// A browser that approves: it comes back to the redirect address with a code and the state
    /// the page was opened with.
    fn approves(page: &str, queue: &mut Queue) -> Result<(), String> {
        let back = query(page, "redirect_uri");
        let target = back.strip_prefix("http://127.0.0.1:47825").unwrap();
        queue.visit(&get(
            &format!(
                "{target}?code=c0de-of-this-run&state={}",
                query(page, "state")
            ),
            47825,
        ));
        Ok(())
    }

    #[test]
    fn a_sign_in_opens_the_forges_page_takes_the_redirect_and_exchanges_the_code_with_the_verifier()
    {
        let (got, seen) = run(47825, &AtomicBool::new(false), &approves);
        assert_eq!(got.unwrap(), "the grant");
        assert_eq!(seen.bound, 1);
        let [page] = seen.opened.as_slice() else {
            panic!("{:?}", seen.opened)
        };
        assert!(page.starts_with("https://gitlab.com/oauth/authorize?"));
        // The redirect names the port that was bound, not the first of the three.
        assert_eq!(
            query(page, "redirect_uri"),
            "http://127.0.0.1:47825/purlis/signin"
        );
        assert_eq!(query(page, "scope"), "api");
        assert_eq!(query(page, "code_challenge_method"), "S256");
        let [form] = seen.posted.as_slice() else {
            panic!("{} exchanges", seen.posted.len())
        };
        let field = |name: &str| {
            let mut found = form.iter().filter(|(n, _)| n == name);
            let value = found.next().map(|(_, v)| v.clone());
            assert!(found.next().is_none(), "{name} is posted twice");
            value
        };
        assert_eq!(field("grant_type").as_deref(), Some("authorization_code"));
        assert_eq!(field("code").as_deref(), Some("c0de-of-this-run"));
        assert_eq!(
            field("redirect_uri").as_deref(),
            Some("http://127.0.0.1:47825/purlis/signin")
        );
        assert_eq!(
            field("client_id").as_deref(),
            Some(
                registration::on(Kind::GitLab, "gitlab.com")
                    .unwrap()
                    .client_id
            )
        );
        // The verifier posted is the one whose challenge the page was shown, and the page never
        // held it.
        let verifier = field("code_verifier").unwrap();
        assert_eq!(
            Verifier::for_a_test(&verifier).challenge(),
            query(page, "code_challenge")
        );
        assert!(!page.contains(&verifier));
        // No secret: the application has none.
        assert_eq!(field("client_secret"), None);
        assert_eq!(form.len(), 5);
    }

    #[test]
    fn each_sign_in_has_a_state_and_a_verifier_of_its_own() {
        let (_, first) = run(47825, &AtomicBool::new(false), &approves);
        let (_, second) = run(47825, &AtomicBool::new(false), &approves);
        for name in ["state", "code_challenge"] {
            assert_ne!(
                query(&first.opened[0], name),
                query(&second.opened[0], name),
                "{name}"
            );
        }
    }

    #[test]
    fn a_code_that_comes_back_with_another_state_is_never_exchanged() {
        let forged = |page: &str, queue: &mut Queue| {
            // Everything right but the state: a caller that saw none of this sign-in.
            queue.visit(&get("/purlis/signin?code=planted&state=guessed", 47825));
            let _ = page;
            Ok(())
        };
        let (got, seen) = run(47825, &AtomicBool::new(false), &forged);
        assert!(got.unwrap_err().contains("did not come back"));
        assert!(seen.posted.is_empty());
    }

    #[test]
    fn a_refusal_says_what_happened_and_exchanges_nothing() {
        let cases: [(&Browser, &str); 4] = [
            (
                &|page, queue| {
                    queue.visit(&get(
                        &format!(
                            "/purlis/signin?error=access_denied&state={}",
                            query(page, "state")
                        ),
                        47825,
                    ));
                    Ok(())
                },
                "You declined on gitlab.com, so nothing was signed in.",
            ),
            (
                &|page, queue| {
                    queue.visit(&get(
                        &format!(
                            "/purlis/signin?error=invalid_scope&error_description=said-by-the-forge&state={}",
                            query(page, "state")
                        ),
                        47825,
                    ));
                    Ok(())
                },
                "gitlab.com would not authorize purlis as it asked. Sign in again, or sign in \
                 with a token instead.",
            ),
            (
                &|_, _| Ok(()),
                "purlis waited five minutes and gitlab.com's answer did not come back from your \
                 browser. Sign in again, or sign in with a token instead.",
            ),
            (
                &|_, _| Err("no browser: said-by-the-opener".to_string()),
                "purlis could not open your browser to sign in to gitlab.com. Check that this \
                 machine has a default browser and sign in again, or sign in with a token \
                 instead.",
            ),
        ];
        for (browser, sentence) in cases {
            let (got, seen) = run(47825, &AtomicBool::new(false), browser);
            assert_eq!(got.unwrap_err(), sentence);
            assert!(seen.posted.is_empty(), "{sentence}");
        }
    }

    #[test]
    fn a_cancelled_sign_in_exchanges_nothing_even_with_the_redirect_waiting() {
        let (got, seen) = run(47825, &AtomicBool::new(true), &approves);
        assert_eq!(got.unwrap_err(), "The sign-in was cancelled.");
        assert!(seen.posted.is_empty());
    }

    #[test]
    fn with_no_port_to_listen_on_no_browser_is_opened() {
        let opened = Mutex::new(0);
        let bind = |_: &[u16]| Err("every port is taken".to_string());
        let open = |_: &str| {
            *opened.lock().unwrap() += 1;
            Ok(())
        };
        let exchange = |_: &[(&str, &str)]| Ok(());
        let got = sign_in(
            &HostScope::for_a_test(),
            registration::on(Kind::GitLab, "gitlab.com").unwrap(),
            &AtomicBool::new(false),
            &Ways {
                bind: &bind,
                open: &open,
                exchange: &exchange,
                clock: loopback::Clock::system(),
            },
        );
        assert_eq!(got.unwrap_err(), "every port is taken");
        assert_eq!(*opened.lock().unwrap(), 0);
    }

    #[test]
    fn a_registration_that_signs_in_by_another_flow_is_refused_before_anything_is_bound() {
        let bind = |_: &[u16]| -> Result<Listening, String> { panic!("bound") };
        let open = |_: &str| -> Result<(), String> { panic!("opened") };
        let exchange = |_: &[(&str, &str)]| -> Result<(), String> { panic!("exchanged") };
        let got = sign_in(
            &HostScope::for_a_test(),
            registration::on(Kind::GitHub, "github.com").unwrap(),
            &AtomicBool::new(false),
            &Ways {
                bind: &bind,
                open: &open,
                exchange: &exchange,
                clock: loopback::Clock::system(),
            },
        );
        assert_eq!(got.unwrap_err(), "github.com does not sign in this way.");
    }

    // -----------------------------------------------------------------------------------
    // The exchange, against a stand-in token address.

    use super::super::device::tests::StandIn;

    const FORM: [(&str, &str); 2] = [
        ("code", "c0de-posted"),
        ("code_verifier", "v3rifier-posted"),
    ];

    fn gitlab() -> &'static Registration {
        registration::on(Kind::GitLab, "gitlab.com").unwrap()
    }

    #[test]
    fn the_code_is_exchanged_at_the_registrations_own_token_address_for_a_grant() {
        let forge = StandIn::answering(&[Ok((
            200,
            r#"{"access_token": "granted-access-value", "token_type": "Bearer",
                "expires_in": 7200, "refresh_token": "granted-refresh-value",
                "scope": "api", "created_at": 1791878400}"#,
        ))]);
        let granted = exchange_at(&forge, &Endpoints::of(gitlab()))(&FORM).unwrap();
        assert_eq!(granted.access.expose_secret(), "granted-access-value");
        assert_eq!(
            granted.refresh.as_ref().map(|r| r.expose_secret()),
            Some("granted-refresh-value")
        );
        assert_eq!(granted.expires_in, Some(Duration::from_secs(7200)));
        assert_eq!(granted.scopes.as_deref(), Some(&["api".to_string()][..]));
        let posted = forge.posted.lock().unwrap();
        let [(url, form)] = posted.as_slice() else {
            panic!("{} posts", posted.len())
        };
        assert_eq!(url, "https://gitlab.com/oauth/token");
        assert_eq!(
            form.iter()
                .map(|(n, v)| (n.as_str(), v.as_str()))
                .collect::<Vec<_>>(),
            FORM
        );
    }

    #[test]
    fn a_refused_exchange_says_why_in_purlis_words_and_repeats_nothing_posted_or_described() {
        for (answer, sentence) in [
            (
                Ok((
                    400,
                    r#"{"error": "invalid_grant", "error_description": "described c0de-posted by the forge"}"#,
                )),
                "gitlab.com would not take the sign-in's code: it had expired or was already \
                 used. Sign in again.",
            ),
            (
                Ok((
                    401,
                    r#"{"error": "invalid_client", "error_description": "described by the forge"}"#,
                )),
                "gitlab.com refused to finish the sign-in (invalid_client). Sign in again, or \
                 sign in with a token instead.",
            ),
            (
                Ok((200, r#"{"token_type": "Bearer"}"#)),
                "gitlab.com answered the sign-in without a token. Sign in again.",
            ),
            (
                Ok((502, "<html>Bad gateway</html>")),
                "gitlab.com answered the sign-in without a token. Sign in again.",
            ),
            (
                Err("gitlab.com: timed out"),
                "purlis could not reach gitlab.com to finish the sign-in (gitlab.com: timed \
                 out). Check your connection and sign in again.",
            ),
        ] {
            let forge = StandIn::answering(&[answer]);
            let why = exchange_at(&forge, &Endpoints::of(gitlab()))(&FORM).unwrap_err();
            assert_eq!(why, sentence);
            for kept_out in ["c0de-posted", "v3rifier-posted", "described"] {
                assert!(!why.contains(kept_out), "{why}");
            }
        }
    }
}
