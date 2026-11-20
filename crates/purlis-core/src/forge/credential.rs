// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where purlis's forge credential comes from** (ADR 0128 D1 and D3, "one login", slice C).
//!
//! The forge proxy (slice A) acts for a chat with the person's credential and a chat never
//! holds it. It asks one question: [`CredentialSource::credential_for`] a host. Two sources
//! answer it, behind that one trait:
//!
//! - **[`SignedIn`]**, the interactive sign-in (ADR 0077): the account the person signed in to in
//!   the window, whose token is in the keyring. github.com and GHES by the device flow, gitlab.com
//!   and a self-hosted GitLab by PKCE (the application the person registered on that host,
//!   [`super::registered`]), a pasted token as the fallback. Its OAuth grants expire, and
//!   [`renewal::Renewal`] renews them before they do, without the person.
//! - **[`TokenConnection`]**, a token given without a person: a CI job, `purlis serve`, a pool
//!   runner, the operator's Secret. It is read from named variables
//!   (`PURLIS_FORGE_TOKEN__<HEX>`, or the forge CLI's own `GH_TOKEN`, `GITHUB_TOKEN`,
//!   `GITLAB_TOKEN`, `CI_JOB_TOKEN`) or from a file a variable names (a mounted Secret), and its
//!   kind is read from its shape. A per-job token is never renewed and never warned about; a
//!   long-lived one that says when it expires is warned about before it does ([`Expiry`]).
//!
//! # The precedence rule
//!
//! **The mode picks the source, and it picks one. [`Credentials`] never falls from one to
//! another because the first failed.**
//!
//! | Mode | What answers | A failure |
//! |---|---|---|
//! | [`Mode::Interactive`] (the window) | The signed-in account only. A token connection is **not read at all**: a `GITHUB_TOKEN` in the shell the window was started from is nobody's choice of identity. | Ends there. |
//! | [`Mode::Headless`] (CI, `purlis serve`, a runner) | A token connection **for that host** wins. Only when none is given for the host does the signed-in account answer, if the machine has one. | A token connection that is given and unusable (malformed, expired, of the wrong forge) is an error, and the account is **not** tried. A sign-in the forge ended is an error, and no token connection is tried in its place. |
//!
//! A refresh the forge refuses is the one thing that raises the Notice ("sign in again to
//! `<host>`", [`super::refresh::tell_when_ended`]); it is raised once, as the account is marked
//! signed out, and a later sweep or request finds it signed out and says so without raising it
//! again. The error carries no credential.
//!
//! # What is kept, and where
//!
//! The token stays in the keyring or in this process's memory, as a [`SecretString`]: it is
//! zeroed when dropped, kept out of the log from the moment it is read, and absent from every
//! `Debug` and error sentence here. Nothing in this module writes one anywhere, and a chat's
//! process tree never reaches it ([`HostScope`] guards the writes, and the proxy is the app's).
//!
//! # What was read to build this (2026-11-20, `origin/main` `4568a3c7b4`)
//!
//! - **`purlis serve`** touches no forge credential: it starts the session host and follows the
//!   mint port ([`super::mintport`]). A headless `serve` gets a credential from [`Credentials`] in
//!   [`Mode::Headless`].
//! - **Pool tools** (`app/src-tauri/src/pooltools.rs`) hand a runner chat's tool a GitHub App
//!   installation token as `GH_TOKEN` ([`super::apptoken`]). That is the per-agent token ADR 0128
//!   replaces: a chat holds no token, so it is not routed through here; it goes when the proxy
//!   does. A runner that is given a token (`PURLIS_FORGE_TOKEN__<HEX>`, or what the operator mints
//!   for it) reads it through [`TokenConnection`].
//! - **The operator** (`operator/`) is a workspace of its own that links nothing of this crate
//!   (its `Cargo.toml` says so, to keep its Kubernetes client tree out of every build). It mints
//!   an installation token from the organisation's App Secret and answers it to a vouched
//!   runner; it holds no person's credential and has nothing to route. The token it mints is an
//!   installation token (`ghs_…`), which [`TokenConnection`] recognises as per-job.

use std::sync::Arc;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

use super::Kind;
use super::accounts::Method;

mod connection;
pub mod renewal;
mod signedin;

pub use connection::{TokenConnection, file_variable_for, host_key, variable_for};
pub use signedin::SignedIn;

/// How long before a long-lived token's known expiry it is warned about: two weeks, enough to
/// make a new one without an outage.
pub const WARN_BEFORE: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// Whom purlis is acting as a person for, and so where its credential comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The window, with a person who signs in.
    Interactive,
    /// No person: a CI job, `purlis serve`, a pool runner.
    Headless,
}

/// Which source answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    SignedIn,
    TokenConnection,
}

/// What a credential is, as far as its shape and where it came from say.
///
/// GitLab's personal, project and group access tokens share the `glpat-` prefix and cannot be
/// told apart without asking the forge, so they are one kind, [`TokenKind::GitLabAccess`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// An OAuth grant from the person's browser sign-in: renewed by purlis.
    SignIn,
    /// A token the person pasted, or imported from a forge CLI.
    Pasted,
    /// GitHub: an installation token (`ghs_`), which is also what Actions' `GITHUB_TOKEN` is.
    /// Lasts an hour or a job.
    GitHubInstallation,
    /// GitHub: a fine-grained personal access token (`github_pat_`).
    GitHubFineGrained,
    /// GitHub: a user-to-server or OAuth token (`ghu_`, `gho_`), eight hours, with a refresh
    /// token a token connection does not have.
    GitHubUser,
    /// GitLab: a CI job's token (`CI_JOB_TOKEN`, `glcbt-`), for the length of the job.
    GitLabJob,
    /// GitLab: a personal, project or group access token (`glpat-`).
    GitLabAccess,
    /// A token of no shape purlis knows (a GitLab token from before prefixes, a GHES one that
    /// names none), taken as long-lived.
    Unrecognised,
}

/// How long a credential is meant to last, and so what purlis does about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    /// Purlis renews it ([`renewal`]).
    Renewed,
    /// Made for a job or an hour: never renewed, never warned about.
    PerJob,
    /// Lasts until someone replaces it: warned about when its expiry is known and near.
    LongLived,
}

impl TokenKind {
    pub fn lifetime(self) -> Lifetime {
        match self {
            TokenKind::SignIn => Lifetime::Renewed,
            TokenKind::GitHubInstallation | TokenKind::GitHubUser | TokenKind::GitLabJob => {
                Lifetime::PerJob
            }
            TokenKind::Pasted
            | TokenKind::GitHubFineGrained
            | TokenKind::GitLabAccess
            | TokenKind::Unrecognised => Lifetime::LongLived,
        }
    }

    /// The kind of a sign-in's token, by how it was signed in.
    pub(super) fn of_method(method: Method) -> TokenKind {
        match method {
            Method::Device | Method::Pkce => TokenKind::SignIn,
            Method::Pat | Method::Import => TokenKind::Pasted,
        }
    }
}

/// Where a credential stands against its expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expiry {
    /// Purlis renews it.
    Renewed,
    /// Made for a job or an hour: nothing to watch.
    PerJob,
    /// Long-lived, and nothing says when it ends.
    Unknown,
    /// More than [`WARN_BEFORE`] left.
    Fine,
    /// Less than [`WARN_BEFORE`] left: the warning.
    Soon { left: Duration },
    /// It has ended.
    Passed,
}

/// One forge credential, for one host. Its token is a secret: it is never in `Debug`, never in
/// an error, and zeroed when this is dropped. There is no `Clone`: each use asks the source again.
pub struct Credential {
    kind: Kind,
    host: String,
    token: SecretString,
    source: Source,
    token_kind: TokenKind,
    login: Option<String>,
    expires_at: Option<u64>,
    from: String,
}

impl Credential {
    pub(super) fn new(
        kind: Kind,
        host: &str,
        token: SecretString,
        source: Source,
        token_kind: TokenKind,
        from: String,
    ) -> Credential {
        Credential {
            kind,
            host: host.to_ascii_lowercase(),
            token,
            source,
            token_kind,
            login: None,
            expires_at: None,
            from,
        }
    }

    pub(super) fn with_login(mut self, login: &str) -> Credential {
        self.login = Some(login.to_string());
        self
    }

    pub(super) fn expiring_at(mut self, at: Option<u64>) -> Credential {
        self.expires_at = at;
        self
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The host this credential is for, lower-case.
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn source(&self) -> Source {
        self.source
    }

    pub fn token_kind(&self) -> TokenKind {
        self.token_kind
    }

    /// The forge login, where the source knows it (a sign-in does, a token connection does not).
    pub fn login(&self) -> Option<&str> {
        self.login.as_deref()
    }

    /// When the token ends, in seconds since the epoch, where known.
    pub fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }

    /// Where it came from, for a sentence: an account's host, or the name of the variable (never
    /// its value).
    pub fn from(&self) -> &str {
        &self.from
    }

    /// The token, for the one request it authorises. Only the forge proxy and the brokered push
    /// call this, and never write it to a place a chat reads.
    pub fn token(&self) -> &SecretString {
        &self.token
    }

    /// The token as text, for the request that carries it.
    pub fn expose(&self) -> &str {
        self.token.expose_secret()
    }

    /// Where this credential stands at `now`, seconds since the epoch.
    pub fn expiry(&self, now: u64) -> Expiry {
        match self.token_kind.lifetime() {
            Lifetime::Renewed => Expiry::Renewed,
            Lifetime::PerJob => Expiry::PerJob,
            Lifetime::LongLived => match self.expires_at {
                None => Expiry::Unknown,
                Some(at) if at <= now => Expiry::Passed,
                Some(at) => {
                    let left = Duration::from_secs(at - now);
                    if left <= WARN_BEFORE {
                        Expiry::Soon { left }
                    } else {
                        Expiry::Fine
                    }
                }
            },
        }
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("kind", &self.kind)
            .field("host", &self.host)
            .field("source", &self.source)
            .field("token_kind", &self.token_kind)
            .field("login", &self.login)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

/// Why a source gave no credential. Every sentence is built from a host, a login and a variable's
/// name; none carries a token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CredentialError {
    /// Nothing was given for this host.
    #[error("purlis has no forge credential for {host}. {how}")]
    None { host: String, how: String },
    /// The forge ended purlis's sign-in, or would not renew it: the person signs in again.
    #[error("{said}")]
    SignInAgain { host: String, said: String },
    /// A renewal could not be asked now and the token has run out.
    #[error("{said}")]
    Unreachable { host: String, said: String },
    /// A token connection's token has ended.
    #[error(
        "The token {variable} gives purlis for {host} ended on its expiry date. Replace it, \
         and start purlis again."
    )]
    Expired { host: String, variable: String },
    /// A credential is given and purlis cannot use it.
    #[error("{said}")]
    Unusable { host: String, said: String },
}

impl CredentialError {
    /// The host this is about.
    pub fn host(&self) -> &str {
        match self {
            CredentialError::None { host, .. }
            | CredentialError::SignInAgain { host, .. }
            | CredentialError::Unreachable { host, .. }
            | CredentialError::Expired { host, .. }
            | CredentialError::Unusable { host, .. } => host,
        }
    }

    /// Whether the person is to sign in again to `host()`: what the Notice is raised for.
    pub fn is_sign_in_again(&self) -> bool {
        matches!(self, CredentialError::SignInAgain { .. })
    }

    /// Whether nothing at all was given for the host, as against something unusable.
    pub fn is_none_given(&self) -> bool {
        matches!(self, CredentialError::None { .. })
    }
}

/// Something that answers, for a host, with the credential purlis acts there with.
pub trait CredentialSource: Send + Sync {
    /// The credential for `host` (lower- or upper-case, with a port when it has one), or why
    /// there is none. Called for each request: a source renews and re-reads as it needs.
    fn credential_for(&self, host: &str) -> Result<Credential, CredentialError>;
}

/// The two sources behind the one rule in this module's table.
pub struct Credentials {
    mode: Mode,
    signed_in: Option<Arc<dyn CredentialSource>>,
    connection: Option<Arc<dyn CredentialSource>>,
}

impl Credentials {
    /// The sources for `mode`. A source that does not exist here (a headless machine with no
    /// keyring, a window that reads no variables) is `None`.
    pub fn new(
        mode: Mode,
        signed_in: Option<Arc<dyn CredentialSource>>,
        connection: Option<Arc<dyn CredentialSource>>,
    ) -> Credentials {
        Credentials {
            mode,
            signed_in,
            connection,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    fn account(&self, host: &str) -> Result<Credential, CredentialError> {
        match &self.signed_in {
            Some(source) => source.credential_for(host),
            None => Err(CredentialError::None {
                host: host.to_ascii_lowercase(),
                how: no_account_how(self.mode, host),
            }),
        }
    }
}

/// What a person does when nothing is given for `host`, by mode.
pub(super) fn no_account_how(mode: Mode, host: &str) -> String {
    match mode {
        Mode::Interactive => format!("Sign in to {host} in {}.", super::signin::SIGN_IN_AT),
        Mode::Headless => format!("Give it a token with {}.", connection::variable_for(host)),
    }
}

impl CredentialSource for Credentials {
    fn credential_for(&self, host: &str) -> Result<Credential, CredentialError> {
        match self.mode {
            // The window acts as the person who signed in. A token in the environment is not a
            // choice anyone made in the window, so it is not read.
            Mode::Interactive => self.account(host),
            Mode::Headless => match self.connection.as_deref().map(|c| c.credential_for(host)) {
                Some(Ok(given)) => Ok(given),
                // Nothing given for this host: the machine's own sign-in, if it has one.
                Some(Err(e)) if e.is_none_given() => self.account(host),
                // Given and unusable, or expired: that is the answer. No other identity stands in.
                Some(Err(e)) => Err(e),
                None => self.account(host),
            },
        }
    }
}

#[cfg(test)]
mod tests;
