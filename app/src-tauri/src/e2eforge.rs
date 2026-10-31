// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The recorded forge the scenario runs sign in to and list from** (#1818, spec #1810's first
//! seam). Only the `e2e` build has this module.
//!
//! A scenario spec cannot reach github.com, and must not. So in the `e2e` build, when the run's
//! environment names a recording file ([`RECORDED_FORGE`], written by `e2e/recordedForge.ts`),
//! the window's native forge calls and its token sign-in's check are answered from that file
//! instead of over HTTPS, in the `forge_contract` recordings' own format
//! ([`purlis_core::forge::recorded`]). The file is read afresh for every request, so a spec can
//! write it after the app started, and each exchange answers as often as it is asked: the page
//! lists again each time it is drawn.
//!
//! Everything else is the shipped path: the token goes to the account store and its keyring item
//! (the fenced build's stub), the sign-ins are handed over through the host's scope, and every
//! call is routed and metered by the resolver. A call nobody recorded is refused, naming it.

use std::path::PathBuf;
use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::oauth::Endpoints;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::registration::{Flow, Registration};
use purlis_core::forge::transport::{Call, NoAnswer, Reply, Transport};
use purlis_core::forge::{Forge, ForgeError};

/// The variable a scenario run sets to the file its recorded forge answers from.
/// `e2e/recordedForge.test.ts` holds the spec's spelling to this one.
pub(crate) const RECORDED_FORGE: &str = "PURLIS_E2E_RECORDED_FORGE";

/// The variable a scenario run sets to the file naming its fake forge
/// (`THE_RUNS_FAKE_FORGE` in `e2e/harness.ts`, written by `e2e/fakeGitLab.ts`).
pub(crate) const FAKE_FORGE: &str = "PURLIS_E2E_FAKE_FORGE";

/// **A real forge's stand-in on this machine, which a spec names while it runs.** It serves git
/// over smart HTTP and the OAuth side of a browser sign-in. With it named:
///
/// - **Sign in with GitLab** signs in to its host, with the shipped flow, listener and exchange.
///   A shipped build has a registration on gitlab.com alone, and a self-managed GitLab takes a
///   token (ADR 0077): this registration exists only here.
/// - The browser is the spec: the page the flow would open is written to `page` for it to answer.
/// - The window's git names purlis's own credential helper, as a shipped build's does
///   ([`crate::forgeaccounts::window_git`]).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BrowserForge {
    /// The forge's server: its host and port, as an account and a clone URL name it.
    host: String,
    /// The root of its OAuth side, on this machine's loopback.
    oauth: String,
    /// The file the page to open is written to.
    page: PathBuf,
}

/// The fake forge a spec names now, read for each ask: `None` before it starts and after it ends.
pub(crate) fn browser_forge() -> Option<BrowserForge> {
    browser_forge_from(purlis_core::envvar::var)
}

fn browser_forge_from(var: impl FnOnce(&str) -> Option<String>) -> Option<BrowserForge> {
    let file = var(FAKE_FORGE).filter(|v| !v.is_empty())?;
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

impl BrowserForge {
    /// Its registration and endpoints. Leaked: a spec signs in a handful of times in a run, and
    /// a [`Registration`] is the static a shipped one is.
    pub(crate) fn registration(&self) -> (&'static Registration, Endpoints) {
        let registration = Box::leak(Box::new(Registration {
            kind: Kind::GitLab,
            host: Box::leak(self.host.clone().into_boxed_str()),
            client_id: "purlise2eclient",
            flow: Flow::Pkce,
        }));
        (registration, Endpoints::at(Kind::GitLab, &self.oauth))
    }

    /// Has the spec play the browser on `page`.
    pub(crate) fn open(&self, page: &str) -> Result<(), String> {
        std::fs::write(&self.page, page).map_err(|err| err.to_string())
    }
}

/// The recorded forge this run names, or `None` when its environment names none.
pub(crate) fn stand_in() -> Option<Arc<dyn Transport>> {
    stand_in_from(purlis_core::envvar::var)
}

fn stand_in_from(var: impl FnOnce(&str) -> Option<String>) -> Option<Arc<dyn Transport>> {
    let file = var(RECORDED_FORGE).filter(|v| !v.is_empty())?;
    Some(Arc::new(Replayed(PathBuf::from(file))))
}

/// A forge answering from the recording in its file, read for each request.
struct Replayed(PathBuf);

impl Transport for Replayed {
    fn send(&self, forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        let text = std::fs::read_to_string(&self.0).map_err(|err| {
            NoAnswer::Missing(format!(
                "the run's recorded forge {} could not be read: {err}",
                self.0.display()
            ))
        })?;
        Recorded::parse(&text)
            .map_err(NoAnswer::Missing)?
            .send(forge, call)
    }

    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORDING: &str = r#"{"source": "a scenario's forge", "exchanges": [
        {"call": {"endpoint": {"rest": {"method": null, "path": "user"}}, "fields": []},
         "reply": {"code": 0, "out": "{\"login\": \"octocat\"}", "err": "", "status": 200}}]}"#;

    #[test]
    fn a_fake_forge_is_named_only_while_its_file_says_so() {
        let dir = tempfile::tempdir().expect("a directory");
        let file = dir.path().join("fake.json");
        let named =
            || browser_forge_from(|name| (name == FAKE_FORGE).then(|| file.display().to_string()));
        assert!(named().is_none());
        std::fs::write(
            &file,
            r#"{"host": "git.example.test:8443", "oauth": "http://127.0.0.1:9", "page": "/p"}"#,
        )
        .expect("written");
        let (registration, endpoints) = named().expect("named").registration();
        assert_eq!(registration.host, "git.example.test:8443");
        assert_eq!(endpoints.token, "http://127.0.0.1:9/oauth/token");
        assert!(browser_forge_from(|_| None).is_none());
    }

    #[test]
    fn a_recorded_exchange_answers_every_time_it_is_asked_and_nothing_else_does() {
        let dir = tempfile::tempdir().expect("a directory");
        let file = dir.path().join("forge.json");
        let forge =
            stand_in_from(|name| (name == RECORDED_FORGE).then(|| file.display().to_string()))
                .expect("named");
        let github = Forge::default_of(Kind::GitHub);
        let user = Call::get("user", std::time::Duration::from_secs(5));
        // Written after the app started: read when asked, not when made.
        assert!(forge.send(&github, &user).is_err());
        std::fs::write(&file, RECORDING).expect("written");
        for _ in 0..2 {
            let reply = forge.send(&github, &user).expect("answered");
            assert!(reply.out.contains("octocat"));
        }
        let other = Call::get("user/repos", std::time::Duration::from_secs(5));
        assert!(forge.send(&github, &other).is_err());
    }

    #[test]
    fn a_run_that_names_no_recording_has_no_stand_in() {
        assert!(stand_in_from(|_| None).is_none());
        assert!(stand_in_from(|_| Some(String::new())).is_none());
    }
}
