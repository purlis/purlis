// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A token connection: a token given without a person** (ADR 0128 D1, "one login" slice C).
//!
//! A CI job, `purlis serve`, a pool runner and the operator's Secret have no window to sign in
//! in. They give purlis a token, and this source reads it. What it reads, for a host, in this
//! order, the **first one set** deciding (a variable that is set and unusable is an error, and
//! the later ones are not tried):
//!
//! 1. `PURLIS_FORGE_TOKEN_<HOST>`, the token itself, where `<HOST>` is [`host_key`]'s spelling:
//!    `GITHUB_COM`, `GHE_EXAMPLE_COM`, `GIT__EXAMPLE_COM` for `git-example.com`.
//! 2. `PURLIS_FORGE_TOKEN_<HOST>_FILE`, the path of a file holding it: a Kubernetes or CI secret
//!    mounted as a file, which keeps the token out of the process's environment.
//! 3. The forge CLI's own variable, on the host it names:
//!    - GitHub: `GH_TOKEN`, then `GITHUB_TOKEN` on github.com (and on the host of
//!      `GITHUB_SERVER_URL`, which Actions on GHES sets); `GH_ENTERPRISE_TOKEN`, then
//!      `GITHUB_ENTERPRISE_TOKEN`, on the host `GH_HOST` names.
//!    - GitLab: `GITLAB_TOKEN` on `GITLAB_HOST` (gitlab.com when it is unset), then
//!      `CI_JOB_TOKEN` on `CI_SERVER_HOST`.
//!
//! **A token is only ever offered to the host its variable names.** `GITHUB_TOKEN` answers
//! github.com and never `ghe.example.com`, so a job's token is not sent to another host.
//!
//! **The forge and the kind are read from the token's shape**: GitHub `ghp_`, `github_pat_`,
//! `ghs_` (an installation token, which is what Actions' `GITHUB_TOKEN` is), `ghu_`/`gho_`; GitLab
//! `glpat-` (a personal, project or group access token, which a prefix cannot tell apart) and
//! `glcbt-` or `CI_JOB_TOKEN` (a job's). A token of no known shape on a host that is not
//! github.com or gitlab.com needs `PURLIS_FORGE_KIND_<HOST>` (`github` or `gitlab`); where the
//! kind and the shape or the host disagree the token is refused, never guessed at.
//!
//! **Expiry.** A per-job kind is never renewed and never warned about. A long-lived one is
//! warned about ([`super::Expiry::Soon`]) when `PURLIS_FORGE_TOKEN_<HOST>_EXPIRES_AT` (unix
//! seconds, or an RFC 3339 time) says it ends within [`super::WARN_BEFORE`], and refused once
//! that time has passed. Purlis cannot read a token's expiry from the token, and does not ask the
//! forge for it here: no variable, no warning.
//!
//! **The token is read into a secret and wiped.** The variable's text is wiped once the secret
//! is made, and the secret is kept out of the log from then on. Errors name variables, never
//! values.

use std::str::FromStr;

use secrecy::SecretString;
use zeroize::Zeroizing;

use super::{Credential, CredentialError, CredentialSource, Source, TokenKind};
use crate::forge::{KINDS, Kind, host_ok};

/// The longest token purlis takes: forges' own are a few hundred characters at most.
const MOST_TOKEN: usize = 4096;

/// The most a file of a token may hold.
const MOST_FILE: u64 = 16 * 1024;

const PREFIX: &str = "PURLIS_FORGE_TOKEN_";

/// `host` as it is spelled in a variable's name: letters and digits upper-case, `.` as `_`, `-` as
/// `__` and the port's `:` as `___`. **Two different hosts never share a spelling**: a host name
/// has no `_`, and its dots, dashes and colon cannot sit next to each other, so a run of one
/// underscore is a dot, an even run is dashes and a run of three is the colon.
pub fn host_key(host: &str) -> String {
    let mut key = String::with_capacity(host.len() + 4);
    for c in host.chars() {
        match c {
            '.' => key.push('_'),
            '-' => key.push_str("__"),
            ':' => key.push_str("___"),
            c => key.push(c.to_ascii_uppercase()),
        }
    }
    key
}

/// The variable that gives purlis a token for `host`.
pub fn variable_for(host: &str) -> String {
    format!("{PREFIX}{}", host_key(&host.to_ascii_lowercase()))
}

/// Reads a token given through variables.
pub struct TokenConnection {
    env: Reading,
}

/// How a variable is read.
type Reading = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

impl TokenConnection {
    /// This process's environment.
    pub fn of_process() -> TokenConnection {
        TokenConnection::reading(crate::envvar::var)
    }

    /// Reads variables through `env`: a test's own, or a runner's.
    pub fn reading(
        env: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> TokenConnection {
        TokenConnection { env: Box::new(env) }
    }

    fn set(&self, name: &str) -> Option<Zeroizing<String>> {
        (self.env)(name)
            .filter(|v| !v.trim().is_empty())
            .map(Zeroizing::new)
    }

    /// The host a variable of a URL or a host names, lower-case, or `None`.
    fn host_named_by(&self, name: &str) -> Option<String> {
        let value = self.set(name)?;
        let trimmed = value.trim().to_ascii_lowercase();
        let host = trimmed
            .split_once("://")
            .map_or(trimmed.as_str(), |(_, rest)| rest)
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string();
        host_ok(&host).then_some(host)
    }

    /// Every variable that may give `host` a token, in order, with what it says of the kind.
    fn candidates(&self, host: &str) -> Vec<Candidate> {
        let key = host_key(host);
        let mut found = vec![
            Candidate::named(format!("{PREFIX}{key}"), None, false),
            Candidate::named(format!("{PREFIX}{key}_FILE"), None, true),
        ];
        let is = |other: Option<String>| other.as_deref() == Some(host);
        if host == Kind::GitHub.default_host() {
            found.push(Candidate::forge("GH_TOKEN", Kind::GitHub));
            found.push(Candidate::forge("GITHUB_TOKEN", Kind::GitHub));
        } else {
            if is(self.host_named_by("GH_HOST")) {
                found.push(Candidate::forge("GH_ENTERPRISE_TOKEN", Kind::GitHub));
                found.push(Candidate::forge("GITHUB_ENTERPRISE_TOKEN", Kind::GitHub));
            }
            if is(self.host_named_by("GITHUB_SERVER_URL")) {
                found.push(Candidate::forge("GITHUB_TOKEN", Kind::GitHub));
            }
        }
        let gitlab_host = self.host_named_by("GITLAB_HOST");
        let gitlab_default = gitlab_host.is_none() && host == Kind::GitLab.default_host();
        if gitlab_default || is(gitlab_host) {
            found.push(Candidate::forge("GITLAB_TOKEN", Kind::GitLab));
        }
        if is(self.host_named_by("CI_SERVER_HOST")) {
            found.push(Candidate {
                job: true,
                ..Candidate::forge("CI_JOB_TOKEN", Kind::GitLab)
            });
        }
        found
    }

    /// [`CredentialSource::credential_for`], with the time given.
    pub fn credential_at(&self, host: &str, now: u64) -> Result<Credential, CredentialError> {
        let host = host.trim().to_ascii_lowercase();
        if !host_ok(&host) {
            return Err(unusable(&host, "That is not a host name.".to_string()));
        }
        let Some((candidate, token)) = self.first_given(&host)? else {
            return Err(CredentialError::None {
                how: format!(
                    "Give it a token with {} (or {}_FILE, for a file).",
                    variable_for(&host),
                    variable_for(&host)
                ),
                host,
            });
        };
        let token_text = token.trim();
        if token_text.is_empty()
            || token_text.len() > MOST_TOKEN
            || !token_text.bytes().all(|b| (0x21..=0x7e).contains(&b))
        {
            return Err(unusable(
                &host,
                format!(
                    "{} holds something that is not a token: a token is one run of printable \
                     characters. Check the value, and start purlis again.",
                    candidate.var
                ),
            ));
        }
        let (kind, token_kind) = self.shape(&host, &candidate, token_text)?;
        let expires_at = self.expiry_of(&host)?;
        let secret = SecretString::from(token_text.to_string());
        crate::applog::keep_out(&secret);
        let credential = Credential::new(
            kind,
            &host,
            secret,
            Source::TokenConnection,
            token_kind,
            candidate.var.clone(),
        )
        .expiring_at(expires_at);
        if credential.expiry(now) == super::Expiry::Passed {
            return Err(CredentialError::Expired {
                host,
                variable: candidate.var,
            });
        }
        Ok(credential)
    }

    /// The first variable that is set for `host`, and its token.
    fn first_given(
        &self,
        host: &str,
    ) -> Result<Option<(Candidate, Zeroizing<String>)>, CredentialError> {
        for candidate in self.candidates(host) {
            let Some(value) = self.set(&candidate.var) else {
                continue;
            };
            if !candidate.file {
                return Ok(Some((candidate, value)));
            }
            let text = read_secret_file(value.trim())
                .map_err(|why| unusable(host, format!("{} is set, but {why}", candidate.var)))?;
            return Ok(Some((candidate, text)));
        }
        Ok(None)
    }

    /// The forge and the kind of `token`, from where it came and what it looks like.
    fn shape(
        &self,
        host: &str,
        from: &Candidate,
        token: &str,
    ) -> Result<(Kind, TokenKind), CredentialError> {
        let by_prefix = kind_by_prefix(token);
        let hinted = match self.set(&format!("PURLIS_FORGE_KIND_{}", host_key(host))) {
            None => None,
            Some(word) => Some(Kind::parse(&word.trim().to_ascii_lowercase()).ok_or_else(
                || {
                    unusable(
                        host,
                        format!(
                            "PURLIS_FORGE_KIND_{} is \"github\" or \"gitlab\". Correct it.",
                            host_key(host)
                        ),
                    )
                },
            )?),
        };
        let shipped = KINDS.into_iter().find(|k| k.default_host() == host);
        let said = [from.kind, hinted, by_prefix, shipped];
        let mut agreed: Option<Kind> = None;
        for kind in said.into_iter().flatten() {
            if let Some(already) = agreed.filter(|already| *already != kind) {
                return Err(unusable(
                    host,
                    format!(
                        "The token in {} and what purlis knows of {host} name different forges, \
                         {} and {}. purlis uses neither. Check the variable.",
                        from.var,
                        already.word(),
                        kind.word(),
                    ),
                ));
            }
            agreed = Some(kind);
        }
        let Some(kind) = agreed else {
            return Err(unusable(
                host,
                format!(
                    "purlis cannot tell which forge {host} is from the token in {}. Set \
                     PURLIS_FORGE_KIND_{} to github or gitlab.",
                    from.var,
                    host_key(host)
                ),
            ));
        };
        let token_kind = match (kind, from.job) {
            (Kind::GitLab, true) => TokenKind::GitLabJob,
            (Kind::GitHub, _) => github_kind(token),
            (Kind::GitLab, false) => gitlab_kind(token),
        };
        // A CLI's own variable is ambient: a shell or a CI job exports it for its own use. Only a
        // token of a shape purlis knows is taken from one; any other must be named for the host.
        if from.kind.is_some() && token_kind == TokenKind::Unrecognised {
            return Err(unusable(
                host,
                format!(
                    "The token in {} is of no shape purlis knows (github_pat_, ghp_, ghs_, glpat-, \
                     glcbt-), and purlis does not take an unknown one from a variable another \
                     tool reads. Give it for this host with {}.",
                    from.var,
                    variable_for(host)
                ),
            ));
        }
        Ok((kind, token_kind))
    }

    /// `PURLIS_FORGE_TOKEN_<HOST>_EXPIRES_AT`, as seconds since the epoch.
    fn expiry_of(&self, host: &str) -> Result<Option<u64>, CredentialError> {
        let name = format!("{PREFIX}{}_EXPIRES_AT", host_key(host));
        let Some(text) = self.set(&name) else {
            return Ok(None);
        };
        let text = text.trim();
        let seconds = text.parse::<u64>().ok().or_else(|| {
            jiff::Timestamp::from_str(text)
                .ok()
                .and_then(|at| u64::try_from(at.as_second()).ok())
        });
        seconds.map(Some).ok_or_else(|| {
            unusable(
                host,
                format!(
                    "{name} is a time as unix seconds or like 2027-03-01T00:00:00Z. Correct it."
                ),
            )
        })
    }
}

impl CredentialSource for TokenConnection {
    fn credential_for(&self, host: &str) -> Result<Credential, CredentialError> {
        self.credential_at(host, crate::forge::accounts::now())
    }
}

/// One variable that may hold a token.
struct Candidate {
    var: String,
    /// The forge the variable's own name says.
    kind: Option<Kind>,
    /// The variable names a file, not the token.
    file: bool,
    /// A CI job's token, whatever it looks like.
    job: bool,
}

impl Candidate {
    fn named(var: String, kind: Option<Kind>, file: bool) -> Candidate {
        Candidate {
            var,
            kind,
            file,
            job: false,
        }
    }

    fn forge(var: &str, kind: Kind) -> Candidate {
        Candidate::named(var.to_string(), Some(kind), false)
    }
}

fn unusable(host: &str, said: String) -> CredentialError {
    CredentialError::Unusable {
        host: host.to_string(),
        said,
    }
}

fn kind_by_prefix(token: &str) -> Option<Kind> {
    const GITHUB: [&str; 6] = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"];
    const GITLAB: [&str; 2] = ["glpat-", "glcbt-"];
    if GITHUB.iter().any(|p| token.starts_with(p)) {
        Some(Kind::GitHub)
    } else if GITLAB.iter().any(|p| token.starts_with(p)) {
        Some(Kind::GitLab)
    } else {
        None
    }
}

fn github_kind(token: &str) -> TokenKind {
    match token {
        t if t.starts_with("ghs_") => TokenKind::GitHubInstallation,
        t if t.starts_with("github_pat_") => TokenKind::GitHubFineGrained,
        t if t.starts_with("ghp_") => TokenKind::GitHubClassic,
        t if t.starts_with("ghu_") || t.starts_with("gho_") => TokenKind::GitHubUser,
        _ => TokenKind::Unrecognised,
    }
}

fn gitlab_kind(token: &str) -> TokenKind {
    match token {
        t if t.starts_with("glcbt-") => TokenKind::GitLabJob,
        t if t.starts_with("glpat-") => TokenKind::GitLabAccess,
        _ => TokenKind::Unrecognised,
    }
}

/// A secret kept in a file, read whole and wiped by its owner. The file is read as it is, and
/// only the sentence's reason leaves here, never the content.
fn read_secret_file(path: &str) -> Result<Zeroizing<String>, String> {
    let file =
        std::fs::File::open(path).map_err(|_| "purlis cannot open that file.".to_string())?;
    let meta = file
        .metadata()
        .map_err(|_| "purlis cannot read that file.".to_string())?;
    if !meta.is_file() {
        return Err("it is not a file.".to_string());
    }
    if meta.len() > MOST_FILE {
        return Err("that file is larger than a token can be.".to_string());
    }
    let mut text = Zeroizing::new(String::new());
    std::io::Read::read_to_string(&mut std::io::Read::take(file, MOST_FILE), &mut text)
        .map_err(|_| "purlis cannot read that file as text.".to_string())?;
    Ok(text)
}
