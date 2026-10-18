// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signing out of a browser sign-in ends its token at the forge, where the forge lets a
//! client with no secret do that** (ADR 0077 §3).
//!
//! - **GitLab** takes a revocation from a client that is not confidential (RFC 7009): one form
//!   posted to the registration's `/oauth/revoke`, with the public client id and the token, and
//!   no secret. The refresh token is the one sent where the item holds one: GitLab keeps a
//!   grant's two tokens as one record, so ending either ends both.
//! - **GitHub** cannot be asked. Its revocation of an app's grant is authenticated with the
//!   app's client secret, and purlis's registration has none. The window says the token lapses
//!   within eight hours and where on GitHub to end it now.
//! - **A pasted or imported token is never revoked.** It may be the person's for other uses.
//!
//! **The sign-out on this machine never waits on the forge's yes.** The keyring item is deleted
//! and the account taken off the list first; only then is the forge asked, once, and what it
//! said is answered beside the account ([`AtTheForge`]) so the window can say it. A forge that
//! could not be reached, or that refused, leaves the person signed out here all the same.
//!
//! Only the host signs out ([`HostScope`]). It waits for a refresh in flight, so a renewed
//! pair is never written back for an account that has just been signed out.

use std::path::Path;

use secrecy::ExposeSecret;

use super::accounts::{self, ForgeAccount, Method, State};
use super::oauth::{Endpoint, Endpoints};
use super::registration;
use super::route::HostScope;
use crate::secrets::keyring::Store;

/// What became of a signed-out account's token at its forge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtTheForge {
    /// The forge took the revocation: the token works nowhere now.
    Ended,
    /// The forge was asked and did not answer, or would not: the token is gone from this
    /// machine, and lasts at the forge until it lapses or the person ends it there.
    NotEnded,
    /// Nothing was asked: a token the person made, or a forge that takes no revocation without
    /// a secret.
    NotAsked,
}

/// Sign out of the account `id` ([`accounts::sign_out`]), then end its token at the forge
/// where purlis can: a browser sign-in on a host purlis has a registration on, whose forge
/// takes a revocation with no secret. `endpoint` posts the form; `None` asks no forge (a test
/// build). `at` is a stand-in's endpoints, or `None` for the registration's own.
///
/// The account as it was, and what became of its token at the forge. An `Err` is the sign-out
/// on this machine failing; the forge is not asked then.
pub fn sign_out(
    host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    endpoint: Option<&dyn Endpoint>,
    at: Option<&Endpoints>,
    id: &str,
) -> Result<(ForgeAccount, AtTheForge), String> {
    let _one = super::refresh::REFRESHING
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // Read before the item is deleted: what the forge is asked to end.
    let held = accounts::list(config_root, keyring)
        .unwrap_or_default()
        .into_iter()
        .find(|account| account.id == id)
        .filter(|account| {
            account.state == State::SignedIn
                && matches!(account.method, Method::Device | Method::Pkce)
        })
        .and_then(|account| {
            let registration = registration::on(account.kind, &account.host)?;
            let url = at
                .cloned()
                .unwrap_or_else(|| Endpoints::of(registration))
                .revoke?;
            // Afresh, past this process's memory: another process may have renewed the grant,
            // and the one revoked is the one that is current (#2656).
            let kept = accounts::kept_afresh(keyring, &account).ok().flatten()?;
            Some((registration, url, kept))
        });
    let account = accounts::sign_out(host, config_root, keyring, id)?;
    let (Some(endpoint), Some((registration, url, kept))) = (endpoint, held) else {
        return Ok((account, AtTheForge::NotAsked));
    };
    let token = kept.refresh.as_ref().unwrap_or(&kept.access);
    let hint = if kept.refresh.is_some() {
        "refresh_token"
    } else {
        "access_token"
    };
    let answered = endpoint.post(
        &url,
        &[
            ("client_id", registration.client_id),
            ("token", token.expose_secret()),
            ("token_type_hint", hint),
        ],
    );
    // RFC 7009 §2.2: 200 whether or not the forge still knew the token.
    let ended = matches!(&answered, Ok(answer) if answer.status == 200 && answer.error().is_none());
    Ok((
        account,
        if ended {
            AtTheForge::Ended
        } else {
            AtTheForge::NotEnded
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::Kind;
    use crate::forge::device::tests::StandIn;
    use crate::forge::refresh::tests::machine;

    fn signed_out(
        machine: &crate::forge::refresh::tests::Machine,
        forge: Option<&StandIn>,
    ) -> (ForgeAccount, AtTheForge) {
        let keyring = accounts::keyring_at(machine.home.path());
        sign_out(
            &HostScope::for_a_test(),
            machine.home.path(),
            keyring.as_ref(),
            forge.map(|forge| forge as &dyn Endpoint),
            None,
            &machine.account.id,
        )
        .unwrap()
    }

    fn gone(machine: &crate::forge::refresh::tests::Machine) -> bool {
        let keyring = accounts::keyring_at(machine.home.path());
        machine.kept().is_none()
            && accounts::list(machine.home.path(), keyring.as_ref())
                .unwrap()
                .is_empty()
    }

    #[test]
    fn a_gitlab_browser_sign_out_revokes_with_the_client_id_and_no_secret() {
        let machine = machine(Kind::GitLab, "gitlab.com", Method::Pkce);
        let forge = StandIn::answering(&[Ok((200, "{}"))]);
        let (account, at) = signed_out(&machine, Some(&forge));
        assert_eq!(account.id, machine.account.id);
        assert_eq!(at, AtTheForge::Ended);
        assert!(gone(&machine));
        let posted = forge.posted.lock().unwrap();
        assert_eq!(posted.len(), 1);
        assert_eq!(posted[0].0, "https://gitlab.com/oauth/revoke");
        let field = |name: &str| {
            posted[0]
                .1
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(
            field("client_id"),
            Some(
                registration::on(Kind::GitLab, "gitlab.com")
                    .unwrap()
                    .client_id
            )
        );
        assert_eq!(field("token"), Some("first-refresh-value"));
        assert_eq!(field("token_type_hint"), Some("refresh_token"));
        assert_eq!(field("client_secret"), None);
    }

    #[test]
    fn sign_out_always_revokes_the_grant_that_is_current() {
        use crate::secrets::keyring::Store as _;
        // This process remembers the first pair (#2656); another process of the app renewed
        // the grant since and wrote the second pair to the item.
        let machine = machine(Kind::GitLab, "gitlab.com", Method::Pkce);
        assert!(machine.kept().is_some(), "this process read the first pair");
        crate::secrets::keyring::FileStore::at(
            crate::machine::dir(machine.home.path()).join(crate::secrets::keyring::STUB_FILE),
        )
        .set(
            &accounts::service(&machine.account),
            "token",
            r#"{"access":"second-access-value","refresh":"second-refresh-value","expires_at":1}"#,
        )
        .unwrap();
        let forge = StandIn::answering(&[Ok((200, "{}"))]);
        let (_, at) = signed_out(&machine, Some(&forge));
        assert_eq!(at, AtTheForge::Ended);
        let posted = forge.posted.lock().unwrap();
        let token = posted[0]
            .1
            .iter()
            .find(|(key, _)| key == "token")
            .map(|(_, value)| value.as_str());
        assert_eq!(token, Some("second-refresh-value"));
    }

    #[test]
    fn a_token_removed_by_sign_out_is_never_handed_out_again_in_this_process() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        assert!(machine.kept().is_some(), "this process remembers the pair");
        signed_out(&machine, None);
        assert!(machine.kept().is_none());
        let keyring = accounts::keyring_at(machine.home.path());
        assert_eq!(
            keyring.exists(&accounts::service(&machine.account), "token"),
            crate::secrets::keyring::Presence::Absent
        );
    }

    #[test]
    fn a_revocation_that_fails_still_signs_out_here_and_says_so() {
        for answer in [
            Err("timed out"),
            Ok((403, r#"{"error": "unauthorized_client"}"#)),
            Ok((503, "<html>unavailable</html>")),
        ] {
            let machine = machine(Kind::GitLab, "gitlab.com", Method::Pkce);
            let forge = StandIn::answering(&[answer]);
            let (_, at) = signed_out(&machine, Some(&forge));
            assert_eq!(at, AtTheForge::NotEnded, "{answer:?}");
            assert!(gone(&machine), "{answer:?}");
            assert_eq!(forge.posted.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn github_is_never_asked_because_its_revocation_needs_a_secret() {
        let machine = machine(Kind::GitHub, "github.com", Method::Device);
        let forge = StandIn::answering(&[]);
        let (_, at) = signed_out(&machine, Some(&forge));
        assert_eq!(at, AtTheForge::NotAsked);
        assert!(gone(&machine));
        assert!(forge.posted.lock().unwrap().is_empty());
    }

    #[test]
    fn no_forge_is_asked_without_an_endpoint_or_on_a_host_with_no_registration() {
        // A test build hands no endpoint.
        let fenced = machine(Kind::GitLab, "gitlab.com", Method::Pkce);
        assert_eq!(signed_out(&fenced, None).1, AtTheForge::NotAsked);
        assert!(gone(&fenced));
        // A self-managed GitLab: purlis's client id is never sent to another host.
        let own = machine(Kind::GitLab, "git.example.com", Method::Pkce);
        let forge = StandIn::answering(&[]);
        assert_eq!(signed_out(&own, Some(&forge)).1, AtTheForge::NotAsked);
        assert!(forge.posted.lock().unwrap().is_empty());
    }

    #[test]
    fn an_account_that_is_not_there_is_refused_and_no_forge_is_asked() {
        let machine = machine(Kind::GitLab, "gitlab.com", Method::Pkce);
        let keyring = accounts::keyring_at(machine.home.path());
        let forge = StandIn::answering(&[]);
        let why = sign_out(
            &HostScope::for_a_test(),
            machine.home.path(),
            keyring.as_ref(),
            Some(&forge),
            None,
            "01J9NOTTHERE",
        )
        .unwrap_err();
        assert!(why.contains("not on this machine"), "{why}");
        assert!(forge.posted.lock().unwrap().is_empty());
    }
}
