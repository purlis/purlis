// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge that says whose token it is **by the token**, for the tests of one login (ADR 0128).
//!
//! `GET user` answers the login and the scopes (`X-OAuth-Scopes`) the table holds for the token
//! it was sent, and 401 for a token it does not know. The table can change between calls, as a
//! forge's answer changes when a grant is widened or folded.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use purlis_core::forge::http::TokenSource;
use purlis_core::forge::oauth::Answer;
use purlis_core::forge::oauth::Granted;
use purlis_core::forge::transport::{Call, NoAnswer, Reply, Transport};
use purlis_core::forge::{Forge, ForgeError, Kind};
use secrecy::ExposeSecret;

/// What the forge says of one token.
#[derive(Clone)]
pub struct Whose {
    pub login: &'static str,
    /// Sent as `X-OAuth-Scopes` on GitHub; GitLab's grant says its own. `None` sends no header,
    /// as a forge that does not say what a token may do.
    pub scopes: Option<&'static str>,
}

#[derive(Clone, Default)]
pub struct Tokens(Arc<Mutex<HashMap<String, Whose>>>);

impl Tokens {
    pub fn know(&self, token: &str, login: &'static str, scopes: &'static str) {
        self.0.lock().unwrap().insert(
            token.to_string(),
            Whose {
                login,
                scopes: Some(scopes),
            },
        );
    }

    /// A token the forge knows, and says nothing of the scopes of: no `X-OAuth-Scopes`.
    pub fn know_without_scopes(&self, token: &str, login: &'static str) {
        self.0.lock().unwrap().insert(
            token.to_string(),
            Whose {
                login,
                scopes: None,
            },
        );
    }

    /// The reach a sign-in checks a token over: a transport that answers for the token it carries.
    pub fn reach(
        &self,
    ) -> impl Fn(&Forge, Arc<dyn TokenSource>) -> Result<Arc<dyn Transport>, ForgeError> + use<>
    {
        let table = self.0.clone();
        move |_forge: &Forge, tokens: Arc<dyn TokenSource>| {
            Ok(Arc::new(ByToken {
                table: table.clone(),
                tokens,
            }) as Arc<dyn Transport>)
        }
    }
}

struct ByToken {
    table: Arc<Mutex<HashMap<String, Whose>>>,
    tokens: Arc<dyn TokenSource>,
}

impl Transport for ByToken {
    fn send(&self, forge: &Forge, _call: &Call) -> Result<Reply, NoAnswer> {
        let token = self.tokens.token().unwrap().expose_secret().to_string();
        let known = self.table.lock().unwrap().get(&token).cloned();
        Ok(match known {
            Some(who) => {
                let login_key = match forge.kind {
                    Kind::GitHub => "login",
                    Kind::GitLab => "username",
                };
                Reply {
                    code: 0,
                    out: format!(r#"{{"{login_key}": "{}"}}"#, who.login),
                    err: String::new(),
                    status: Some(200),
                    headers: who
                        .scopes
                        .map(|scopes| vec![("x-oauth-scopes".to_string(), scopes.to_string())])
                        .unwrap_or_default(),
                }
            }
            None => Reply {
                code: 1,
                out: String::new(),
                err: "HTTP 401: Bad credentials".to_string(),
                status: Some(401),
                headers: Vec::new(),
            },
        })
    }

    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        Ok(())
    }
}

/// What a device or PKCE grant holds: the access token and refresh token, as the forge's reply
/// has them.
pub fn grant(access: &str, refresh: &str, scope: &str) -> Granted {
    Answer::of(
        200,
        &format!(
            r#"{{"access_token": "{access}", "refresh_token": "{refresh}", "scope": "{scope}",
                 "expires_in": 7200}}"#
        ),
    )
    .granted()
    .unwrap()
}

/// A grant of an OAuth app whose tokens never expire: no refresh token, no `expires_in`.
pub fn grant_that_does_not_expire(access: &str, scope: &str) -> Granted {
    Answer::of(
        200,
        &format!(r#"{{"access_token": "{access}", "token_type": "bearer", "scope": "{scope}"}}"#),
    )
    .granted()
    .unwrap()
}
