// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the operator asks of GitHub for a mint** (#4206): which installation of the App holds an
//! owner's repo (`GET /repos/{owner}/{repo}/installation`), and an installation token limited to
//! named repos and named permissions (`POST /app/installations/{id}/access_tokens`). The same two
//! requests as the desktop's mint (`crates/purlis-core/src/forge/apptoken.rs`).
//!
//! GitHub reads an omitted `repositories` as every repo of the installation, so both lists are
//! always sent, and what comes back is held to them ([`checked`]): a token that says it covers
//! anything else is revoked and not handed over. No answer's body and no token is ever logged or
//! put in an error.

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::time::Duration;

/// Where GitHub's API is.
pub const API: &str = "https://api.github.com";

/// The variable that puts another API in [`API`]'s place, for the kind job's stand-in for GitHub.
/// Not a value of the chart: only someone who may already change the operator's Deployment, and so
/// read its keys, can set it.
pub const API_VAR: &str = "PURLIS_FORGE_API_URL";

/// How long one request to GitHub is waited for.
const ANSWER_WITHIN: Duration = Duration::from_secs(15);

/// The most of an answer that is read.
const MOST_ANSWER: usize = 1024 * 1024;

/// **The permissions a token may ask for, whatever the installation holds**: repo-level ones that
/// administer nothing. Anything else, an organisation's above all, which a list of repos does not
/// narrow, is never asked for.
pub const MINTABLE: [&str; 11] = [
    "actions",
    "checks",
    "contents",
    "deployments",
    "discussions",
    "issues",
    "metadata",
    "packages",
    "pull_requests",
    "repository_projects",
    "statuses",
];

/// What of the installation's `held` permissions a token asks for, by name.
pub fn mintable(held: &Map<String, Value>) -> Map<String, Value> {
    held.iter()
        .filter(|(name, _)| MINTABLE.contains(&name.as_str()))
        .map(|(name, level)| (name.clone(), level.clone()))
        .collect()
}

/// The installation holding a repo, as GitHub answers.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Installation {
    pub id: u64,
    pub account: Account,
    /// What the organisation gave the installation: what a token asks for, by name.
    #[serde(default)]
    pub permissions: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Account {
    pub login: String,
}

/// A token as GitHub answers. Its `Debug` leaves the token out.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Token {
    pub token: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub repository_selection: Option<String>,
    #[serde(default)]
    pub repositories: Vec<NamedRepo>,
    #[serde(default)]
    pub permissions: Map<String, Value>,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token")
            .field("expires_at", &self.expires_at)
            .field("repository_selection", &self.repository_selection)
            .field("repositories", &self.repositories)
            .field("permissions", &self.permissions)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NamedRepo {
    pub name: String,
}

/// The body of a token request: always both lists.
pub fn token_request(names: &[String], permissions: &Map<String, Value>) -> Value {
    json!({ "repositories": names, "permissions": permissions })
}

/// Why `token` is not what was asked for, if it is not: limited to selected repos, exactly `names`
/// (case aside), and no permission beyond `permissions`.
pub fn checked(
    token: &Token,
    names: &[String],
    permissions: &Map<String, Value>,
) -> Result<(), String> {
    if token.repository_selection.as_deref() != Some("selected") {
        return Err("GitHub's token is not limited to selected repos".to_owned());
    }
    let lower = |name: &str| name.to_ascii_lowercase();
    let mut said: Vec<String> = token.repositories.iter().map(|r| lower(&r.name)).collect();
    let mut asked: Vec<String> = names.iter().map(|name| lower(name)).collect();
    said.sort();
    asked.sort();
    if said != asked {
        return Err("GitHub's token does not cover exactly the repos asked for".to_owned());
    }
    if token.permissions.is_empty()
        || token
            .permissions
            .iter()
            .any(|(name, level)| permissions.get(name) != Some(level))
    {
        return Err("GitHub's token does not hold exactly the permissions asked for".to_owned());
    }
    Ok(())
}

/// The way to GitHub.
pub struct GitHub {
    api: String,
    http: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
}

impl GitHub {
    /// GitHub's API, or the one [`API_VAR`] names, trusting the certificate authorities the
    /// operator's image holds.
    pub fn new() -> Result<GitHub, String> {
        let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_provider_and_native_roots(provider)
            .map_err(|why| format!("no certificate authorities to trust GitHub by: {why}"))?
            .https_or_http()
            .enable_http1()
            .build();
        let api = std::env::var(API_VAR)
            .ok()
            .filter(|api| !api.is_empty())
            .unwrap_or_else(|| API.to_owned());
        if api == API {
            tracing::info!(%api, "minting against GitHub");
        } else {
            tracing::warn!(%api, "minting against another API than GitHub's: {API_VAR} is set");
        }
        Ok(GitHub {
            api,
            http: Client::builder(TokioExecutor::new()).build(connector),
        })
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        bearer: &str,
        body: Option<&Value>,
    ) -> Result<(StatusCode, Bytes), String> {
        let request = Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.api))
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28")
            .header("user-agent", "purlis-cluster-operator")
            .header("authorization", format!("Bearer {bearer}"))
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(
                body.map(Value::to_string).unwrap_or_default(),
            )))
            .map_err(|_| "a request to GitHub could not be made".to_owned())?;
        let asked = async {
            let answer = self
                .http
                .request(request)
                .await
                .map_err(|_| "GitHub could not be reached".to_owned())?;
            let status = answer.status();
            let body = Limited::new(answer.into_body(), MOST_ANSWER)
                .collect()
                .await
                .map_err(|_| "GitHub's answer could not be read".to_owned())?
                .to_bytes();
            Ok::<_, String>((status, body))
        };
        tokio::time::timeout(ANSWER_WITHIN, asked)
            .await
            .map_err(|_| "GitHub did not answer in time".to_owned())?
    }

    /// The installation of the App on `owner`'s `repo`. A repo that moved answers with a redirect,
    /// which is not followed, and an installation of another account is refused.
    pub async fn installation(
        &self,
        jwt: &str,
        owner: &str,
        repo: &str,
    ) -> Result<Installation, String> {
        let path = format!("/repos/{owner}/{repo}/installation");
        let (status, body) = self.call(Method::GET, &path, jwt, None).await?;
        if status != StatusCode::OK {
            return Err(format!(
                "the GitHub App is not installed on {owner}/{repo} (GitHub answered {})",
                status.as_u16()
            ));
        }
        let held: Installation = serde_json::from_slice(&body)
            .map_err(|_| "GitHub's word on the installation could not be read".to_owned())?;
        if !held.account.login.eq_ignore_ascii_case(owner) {
            return Err(format!(
                "{owner}/{repo} is on an installation of another account"
            ));
        }
        Ok(held)
    }

    /// A token of installation `id` for exactly `names` and `permissions`. One that says otherwise
    /// is revoked here and refused.
    pub async fn token(
        &self,
        jwt: &str,
        id: u64,
        names: &[String],
        permissions: &Map<String, Value>,
    ) -> Result<Token, String> {
        let path = format!("/app/installations/{id}/access_tokens");
        let body = token_request(names, permissions);
        let (status, answer) = self.call(Method::POST, &path, jwt, Some(&body)).await?;
        if status != StatusCode::CREATED {
            return Err(format!(
                "GitHub refused the token (it answered {})",
                status.as_u16()
            ));
        }
        let token: Token = serde_json::from_slice(&answer)
            .map_err(|_| "GitHub's answer holds no token that could be read".to_owned())?;
        if let Err(why) = checked(&token, names, permissions) {
            // Best effort: it is in no one's hands, and lives its hour at most if this fails.
            let _ = self
                .call(Method::DELETE, "/installation/token", &token.token, None)
                .await;
            return Err(why);
        }
        Ok(token)
    }
}
