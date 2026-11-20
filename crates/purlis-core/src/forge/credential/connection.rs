// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A token connection: a token given without a person** (ADR 0128 D1 and D9, "one login" slice C).
//!
//! A CI job, `purlis serve`, a pool runner and the operator's Secret have no window to sign in
//! in. They give purlis a token, and this source reads it. What it reads, for a host, in this
//! order, the **first one set** deciding (a variable that is set and unusable is an error, and
//! the later ones are not tried):
//!
//! 1. `PURLIS_FORGE_TOKEN__<HEX>`, the token itself, where `<HEX>` is [`host_key`]: the lower-case
//!    hex of the host's UTF-8 bytes (`github.com` is `6769746875622e636f6d`), ADR 0128 D9.2. Hex has
//!    one spelling per host, and no `_`, so a name's family (`PURLIS_FORGE_TOKEN__`,
//!    `PURLIS_FORGE_TOKEN_FILE__`, `PURLIS_FORGE_TOKEN_EXPIRES_AT__`, `PURLIS_FORGE_KIND__`,
//!    `PURLIS_FORGE_ACCEPT__`) is the whole of what comes before the host: no host's spelling can
//!    be another family's name. `purlis doctor forge` and the window show the name for a host.
//! 2. `PURLIS_FORGE_TOKEN_FILE__<HEX>`, the path of a file holding it: a Kubernetes or CI secret
//!    mounted as a file, which keeps the token out of the process's environment. **Both set is a
//!    refusal** naming both (D9.2), and **one set and empty is an error**, never a fallback.
//! 3. The forge CLI's own variable, on the host it names:
//!    - GitHub: `GH_TOKEN` and `GITHUB_TOKEN` on github.com **only when `GITHUB_SERVER_URL` is
//!      unset or names github.com**, and `GITHUB_TOKEN` on the host `GITHUB_SERVER_URL` names (what
//!      Actions on GHES sets); `GH_ENTERPRISE_TOKEN`, then `GITHUB_ENTERPRISE_TOKEN`, on the host
//!      `GH_HOST` names.
//!    - GitLab: `GITLAB_TOKEN` on the host `GITLAB_HOST` names, else `CI_SERVER_HOST`; `CI_JOB_TOKEN`
//!      on `CI_SERVER_HOST`. **With no host named, neither is offered to any host**, gitlab.com
//!      included: only a job on gitlab.com says (`CI_SERVER_HOST`) that its token is gitlab.com's.
//!
//! **A token is only ever offered to the host its variable names.** `GITHUB_TOKEN` answers the host
//! `GITHUB_SERVER_URL` names (github.com when it names none), never another, so a GHES job's
//! token is not sent to github.com. A variable naming a host that is not a host name is read as
//! naming none *and* none is offered: the doubt closes the door.
//!
//! **Which tokens are taken** (D9): by exact prefix and shape (the prefix, then the characters and
//! the length that forge's tokens have), never by `starts_with` alone.
//!
//! | Token | From `GH_TOKEN`, `GITLAB_TOKEN` and the other CLI variables | From the named variable |
//! |---|---|---|
//! | `ghs_` installation, `github_pat_` fine-grained | taken | taken |
//! | `gho_`, `ghu_` user-to-server | refused | taken |
//! | `glpat-`, `glcbt-` | taken | taken |
//! | `ghp_` classic PAT, `ghr_` refresh token | refused | refused |
//! | any other shape | refused | refused unless `PURLIS_FORGE_ACCEPT__<HEX>` is `unrecognised` |
//!
//! `CI_JOB_TOKEN` is a job's token whatever it looks like. A token of no known shape on a host
//! that is not github.com or gitlab.com needs `PURLIS_FORGE_KIND__<HEX>` (`github` or `gitlab`);
//! where the kind and the shape or the host disagree the token is refused, never guessed at.
//!
//! **Expiry.** A per-job kind is never renewed and never warned about. A long-lived one is
//! warned about ([`super::Expiry::Soon`]) when `PURLIS_FORGE_TOKEN_EXPIRES_AT__<HEX>` (unix
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

/// `host` as it is spelled in a variable's name: the lower-case hex of its UTF-8 bytes (ADR 0128
/// D9.2). **Two different hosts never share a spelling**, and a spelling has no `_`, so a name
/// built from it with a family's prefix ([`family`]) cannot be another family's name or another
/// host's.
pub fn host_key(host: &str) -> String {
    use std::fmt::Write;
    host.bytes()
        .fold(String::with_capacity(host.len() * 2), |mut key, b| {
            let _ = write!(key, "{b:02x}");
            key
        })
}

/// The families of variable that carry a host in their name. Each ends in `__`, so what follows
/// is always the host's [`host_key`], whole.
mod family {
    pub const TOKEN: &str = "PURLIS_FORGE_TOKEN__";
    pub const FILE: &str = "PURLIS_FORGE_TOKEN_FILE__";
    pub const EXPIRES_AT: &str = "PURLIS_FORGE_TOKEN_EXPIRES_AT__";
    pub const KIND: &str = "PURLIS_FORGE_KIND__";
    pub const ACCEPT: &str = "PURLIS_FORGE_ACCEPT__";
}

fn named(family: &str, host: &str) -> String {
    format!("{family}{}", host_key(&host.to_ascii_lowercase()))
}

/// The variable that gives purlis a token for `host`.
pub fn variable_for(host: &str) -> String {
    named(family::TOKEN, host)
}

/// The variable that names a file holding a token for `host`.
pub fn file_variable_for(host: &str) -> String {
    named(family::FILE, host)
}

/// Reads a token given through variables.
pub struct TokenConnection {
    env: Reading,
}

/// How a variable is read.
type Reading = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A variable as the process has it.
enum Var {
    Unset,
    /// Set, and nothing but white space.
    Blank,
    Value(Zeroizing<String>),
}

/// A variable that names a host, as it was read.
enum Naming {
    Unset,
    Host(String),
    /// Set, and not a host name: it names no host, and nothing is offered on its account.
    Bad,
}

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

    fn var(&self, name: &str) -> Var {
        match (self.env)(name) {
            None => Var::Unset,
            Some(v) if v.trim().is_empty() => Var::Blank,
            Some(v) => Var::Value(Zeroizing::new(v)),
        }
    }

    /// An ambient variable's value: a shell or a CI job often exports one empty, which is not a
    /// choice. Only a variable purlis names for a host is an error when it is empty.
    fn set(&self, name: &str) -> Option<Zeroizing<String>> {
        match self.var(name) {
            Var::Value(v) => Some(v),
            Var::Unset | Var::Blank => None,
        }
    }

    /// The host a variable of a URL or a host names, lower-case.
    fn naming(&self, name: &str) -> Naming {
        let Some(value) = self.set(name) else {
            return Naming::Unset;
        };
        let trimmed = value.trim().to_ascii_lowercase();
        let host = trimmed
            .split_once("://")
            .map_or(trimmed.as_str(), |(_, rest)| rest)
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string();
        if host_ok(&host) {
            Naming::Host(host)
        } else {
            Naming::Bad
        }
    }

    /// The forge CLIs' own variables that may give `host` a token, in order, each with what its
    /// name says of the kind. **A variable is offered only to the host it belongs to.**
    fn ambient(&self, host: &str) -> Vec<Candidate> {
        let mut found = Vec::new();
        let says = |naming: &Naming| matches!(naming, Naming::Host(h) if h == host);

        // GitHub. Actions' `GITHUB_TOKEN` (and a `GH_TOKEN` set from it) is the token of the server
        // `GITHUB_SERVER_URL` names: github.com when it names none.
        let server = self.naming("GITHUB_SERVER_URL");
        let server_is_dotcom = match &server {
            Naming::Unset => true,
            Naming::Host(h) => h == Kind::GitHub.default_host(),
            Naming::Bad => false,
        };
        if host == Kind::GitHub.default_host() {
            if server_is_dotcom {
                found.push(Candidate::forge("GH_TOKEN", Kind::GitHub));
                found.push(Candidate::forge("GITHUB_TOKEN", Kind::GitHub));
            }
        } else {
            if says(&self.naming("GH_HOST")) {
                found.push(Candidate::forge("GH_ENTERPRISE_TOKEN", Kind::GitHub));
                found.push(Candidate::forge("GITHUB_ENTERPRISE_TOKEN", Kind::GitHub));
            }
            if says(&server) {
                found.push(Candidate::forge("GITHUB_TOKEN", Kind::GitHub));
            }
        }

        // GitLab. `GITLAB_TOKEN` is the token of the host `GITLAB_HOST` names, else the one the job
        // runs on; a job's own token is that of `CI_SERVER_HOST`. With no host named, neither is
        // anyone's.
        let ci_server = self.naming("CI_SERVER_HOST");
        let gitlab_host = match self.naming("GITLAB_HOST") {
            Naming::Unset => self.naming("CI_SERVER_HOST"),
            named => named,
        };
        if says(&gitlab_host) {
            found.push(Candidate::forge("GITLAB_TOKEN", Kind::GitLab));
        }
        if says(&ci_server) {
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
                    "Give it a token with {} (or {}, for a file).",
                    variable_for(&host),
                    file_variable_for(&host)
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

    /// The first variable that is set for `host`, and its token. The two variables purlis names
    /// for a host are an error when both are set, and when the one that is set is empty: a CI
    /// that failed to fill a secret must not be answered by another variable's token.
    fn first_given(
        &self,
        host: &str,
    ) -> Result<Option<(Candidate, Zeroizing<String>)>, CredentialError> {
        let (token_var, file_var) = (variable_for(host), file_variable_for(host));
        let empty = |var: &str| {
            unusable(
                host,
                format!("{var} is set but empty. Give it a value, or unset it."),
            )
        };
        match (self.var(&token_var), self.var(&file_var)) {
            (Var::Unset, Var::Unset) => {}
            (Var::Unset, Var::Blank) => return Err(empty(&file_var)),
            (Var::Unset, Var::Value(path)) => {
                let text = read_secret_file(path.trim())
                    .map_err(|why| unusable(host, format!("{file_var} is set, but {why}")))?;
                return Ok(Some((Candidate::named(file_var), text)));
            }
            (Var::Blank, Var::Unset) => return Err(empty(&token_var)),
            (Var::Value(token), Var::Unset) => {
                return Ok(Some((Candidate::named(token_var), token)));
            }
            (_, _) => {
                return Err(unusable(
                    host,
                    format!(
                        "{token_var} and {file_var} are both set, and purlis does not choose \
                         between two tokens. Unset one, and start purlis again."
                    ),
                ));
            }
        }
        for candidate in self.ambient(host) {
            if let Some(value) = self.set(&candidate.var) {
                return Ok(Some((candidate, value)));
            }
        }
        Ok(None)
    }

    /// The forge and the kind of `token`, from where it came and what it looks like, and
    /// whether this source takes a token of that kind from that variable at all (ADR 0128 D9).
    fn shape(
        &self,
        host: &str,
        from: &Candidate,
        token: &str,
    ) -> Result<(Kind, TokenKind), CredentialError> {
        let class = match classify(token) {
            Ok(class) => class,
            Err(prefix) => {
                return Err(unusable(
                    host,
                    format!(
                        "The token in {} starts like a {prefix} token and is not the shape of \
                         one, so purlis does not take it. Check the value.",
                        from.var
                    ),
                ));
            }
        };
        let ambient = from.kind.is_some();
        if let Some(why) = class.refused(ambient, from.job) {
            return Err(unusable(host, format!("The token in {} {why}", from.var)));
        }
        let hint_var = named(family::KIND, host);
        let hinted = match self.set(&hint_var) {
            None => None,
            Some(word) => Some(Kind::parse(&word.trim().to_ascii_lowercase()).ok_or_else(
                || {
                    unusable(
                        host,
                        format!("{hint_var} is \"github\" or \"gitlab\". Correct it."),
                    )
                },
            )?),
        };
        let shipped = KINDS.into_iter().find(|k| k.default_host() == host);
        let said = [from.kind, hinted, class.kind(), shipped];
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
                     {hint_var} to github or gitlab.",
                    from.var
                ),
            ));
        };
        if class == Class::Unrecognised && !from.job && !self.accepts(host, "unrecognised") {
            return Err(unusable(
                host,
                format!(
                    "The token in {} is of no shape purlis knows (github_pat_, ghs_, glpat-, \
                     glcbt-). To take it anyway, set {} to unrecognised.",
                    from.var,
                    named(family::ACCEPT, host)
                ),
            ));
        }
        let token_kind = match (kind, from.job) {
            (Kind::GitLab, true) => TokenKind::GitLabJob,
            (Kind::GitHub, _) => class.github_kind(),
            (Kind::GitLab, false) => class.gitlab_kind(),
        };
        Ok((kind, token_kind))
    }

    /// Whether the operator opted `what` in for `host`: `PURLIS_FORGE_ACCEPT__<HEX>` is a
    /// comma-separated list of the types it takes beyond the ones purlis knows.
    fn accepts(&self, host: &str, what: &str) -> bool {
        self.set(&named(family::ACCEPT, host)).is_some_and(|list| {
            list.split(',')
                .any(|word| word.trim().eq_ignore_ascii_case(what))
        })
    }

    /// `PURLIS_FORGE_TOKEN_EXPIRES_AT__<HEX>`, as seconds since the epoch.
    fn expiry_of(&self, host: &str) -> Result<Option<u64>, CredentialError> {
        let name = named(family::EXPIRES_AT, host);
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
    /// The forge the variable's own name says: set for a forge CLI's variable, which is
    /// **ambient** (a shell or a CI job exports it for its own use), and not for the variable
    /// purlis names for a host.
    kind: Option<Kind>,
    /// A CI job's token, whatever it looks like.
    job: bool,
}

impl Candidate {
    /// The variable purlis names for a host: the operator's own choice.
    fn named(var: String) -> Candidate {
        Candidate {
            var,
            kind: None,
            job: false,
        }
    }

    fn forge(var: &str, kind: Kind) -> Candidate {
        Candidate {
            var: var.to_string(),
            kind: Some(kind),
            job: false,
        }
    }
}

fn unusable(host: &str, said: String) -> CredentialError {
    CredentialError::Unusable {
        host: host.to_string(),
        said,
    }
}

/// What a token is, by its exact prefix and the shape of what follows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    GitHubClassic,
    GitHubRefresh,
    GitHubUser,
    GitHubOAuth,
    GitHubInstallation,
    GitHubFineGrained,
    GitLabAccess,
    GitLabJob,
    Unrecognised,
}

/// `(prefix, class, the characters beyond letters and digits, the fewest and the most that
/// follow the prefix)`. GitHub's own tokens are 36 characters after a four-letter prefix and
/// say they may grow; a fine-grained one is 82 after `github_pat_`; a GitLab access token is 20
/// and a routable one longer.
const SHAPES: [(&str, Class, &[u8], usize, usize); 8] = [
    ("ghp_", Class::GitHubClassic, b"", 36, 255),
    ("ghr_", Class::GitHubRefresh, b"", 36, 255),
    ("gho_", Class::GitHubOAuth, b"", 36, 255),
    ("ghu_", Class::GitHubUser, b"", 36, 255),
    ("ghs_", Class::GitHubInstallation, b"", 36, 255),
    ("github_pat_", Class::GitHubFineGrained, b"_", 36, 255),
    ("glpat-", Class::GitLabAccess, b"-_.", 20, 255),
    ("glcbt-", Class::GitLabJob, b"-_.", 8, 255),
];

/// The class of `token`, or the prefix it begins with where what follows is not that prefix's
/// shape.
fn classify(token: &str) -> Result<Class, &'static str> {
    for (prefix, class, beyond, fewest, most) in SHAPES {
        let Some(rest) = token.strip_prefix(prefix) else {
            continue;
        };
        let shaped = (fewest..=most).contains(&rest.len())
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || beyond.contains(&b));
        return if shaped { Ok(class) } else { Err(prefix) };
    }
    Ok(Class::Unrecognised)
}

impl Class {
    fn kind(self) -> Option<Kind> {
        match self {
            Class::GitHubClassic
            | Class::GitHubRefresh
            | Class::GitHubUser
            | Class::GitHubOAuth
            | Class::GitHubInstallation
            | Class::GitHubFineGrained => Some(Kind::GitHub),
            Class::GitLabAccess | Class::GitLabJob => Some(Kind::GitLab),
            Class::Unrecognised => None,
        }
    }

    /// Why a token of this class is not taken from this kind of variable, as the end of a
    /// sentence that begins "The token in `VAR`", or `None` when it is. `job` is a CI job's token,
    /// which is whatever the job's server issued.
    fn refused(self, ambient: bool, job: bool) -> Option<&'static str> {
        match self {
            Class::GitHubClassic => Some(
                "is a classic personal access token (ghp_), which reaches every repository its \
                 owner can and cannot be narrowed. Use a fine-grained token (github_pat_) limited \
                 to the repositories purlis works in.",
            ),
            Class::GitHubRefresh => Some(
                "is a refresh token (ghr_), which is not a credential for the API. Give the \
                 access token.",
            ),
            Class::GitHubUser | Class::GitHubOAuth if ambient => Some(
                "is a user-to-server token (ghu_ or gho_), which purlis takes only from the \
                 variable it names for the host, not from one another tool reads.",
            ),
            Class::Unrecognised if ambient && !job => Some(
                "is of no shape purlis knows (github_pat_, ghs_, glpat-, glcbt-), and purlis does \
                 not take an unknown one from a variable another tool reads. Give it for this \
                 host with the variable purlis names.",
            ),
            _ => None,
        }
    }

    fn github_kind(self) -> TokenKind {
        match self {
            Class::GitHubInstallation => TokenKind::GitHubInstallation,
            Class::GitHubFineGrained => TokenKind::GitHubFineGrained,
            Class::GitHubUser | Class::GitHubOAuth => TokenKind::GitHubUser,
            _ => TokenKind::Unrecognised,
        }
    }

    fn gitlab_kind(self) -> TokenKind {
        match self {
            Class::GitLabJob => TokenKind::GitLabJob,
            Class::GitLabAccess => TokenKind::GitLabAccess,
            _ => TokenKind::Unrecognised,
        }
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
