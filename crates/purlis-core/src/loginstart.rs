// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a login's sign-in starts, where the chat names it** (#6291, the operator's ruling of
//! 2026-11-12, ADR 0067 as amended that day).
//!
//! A login opens its declared page. A chat that has to start an OAuth or OpenID Connect sign-in
//! with its own request (a client id, a `redirect_uri`, a `state`, a PKCE challenge) names the
//! address with `browser_login`'s `start`, for a login its persona declares or one the person
//! allowed, never with a suggestion. [`start_of`] says whether it may:
//!
//! - **On the login's own sites alone**: its origin, as a browser reads the address, is the
//!   login's page's or the one its form is on. Nothing a browser reads otherwise (a user or
//!   password, a backslash, a space or a control character) passes.
//! - **Its way back is the login's page's site**: every `redirect_uri` in its query, decoded as
//!   the identity provider decodes it, is an address on that origin. A `request` or a
//!   `request_uri` carries a redirect purlis can't read, so it is refused.
//! - **Kept as written**: its query and fragment, percent-encoding included, are opened as they
//!   are, for that one sign-in. Everything else of the login stays as it is: where it's filled,
//!   what the browser reaches and how long the sign-in lasts.
//!
//! A refusal names the origin and the parameter, never a value of the query: a `state` or a code
//! there is the chat's own, and isn't said back.

use crate::sandbox::tools::login;

/// The longest `start` taken, in bytes: room for any authorize request, and a bound on a notice.
pub const MOST_START: usize = 8 * 1024;

/// The parameters whose value is a redirect purlis can't read: a signed request object, and a
/// reference to one pushed before (RFC 9101, RFC 9126).
const UNREADABLE: [&str; 2] = ["request", "request_uri"];

/// **The address a sign-in of the login whose page is `page`, and whose form is on `form` where
/// that's another origin, starts at**, as `start` names it, or why it may not (module docs).
pub fn start_of(start: &str, page: &str, form: Option<&str>) -> Result<String, String> {
    let start = start.trim();
    if start.is_empty() || start.len() > MOST_START {
        return Err(format!(
            "give `start` the https address the sign-in begins at, at most {MOST_START} bytes"
        ));
    }
    if start
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return Err("`start` has a space, a backslash or a control character in it".to_owned());
    }
    let parsed = url::Url::parse(start)
        .map_err(|_| "`start` is not an address a browser opens".to_owned())?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err("`start` is not an https address".to_owned());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(
            "`start` carries a user or a password; the login comes from the vault".to_owned(),
        );
    }
    let origin = parsed.origin().ascii_serialization();
    let page_origin = login::origin_of(page)?;
    let form_origin = form.map(login::form_of).transpose()?;
    if origin != page_origin && form_origin.as_deref() != Some(origin.as_str()) {
        let sites = match &form_origin {
            Some(form) if *form != page_origin => format!("{page_origin} or {form}"),
            _ => page_origin.clone(),
        };
        return Err(format!(
            "`start` is on {origin}, and this login's sign-in starts only on {sites}"
        ));
    }
    for (key, value) in parsed.query_pairs() {
        let key = key.to_ascii_lowercase();
        if UNREADABLE.contains(&key.as_str()) {
            return Err(format!(
                "`start` has a `{key}`, whose redirect purlis can't read, so it can't tell the \
                 sign-in comes back to {page_origin}: give the request's parameters in the query \
                 instead"
            ));
        }
        if key == "redirect_uri" {
            let back = url::Url::parse(&value)
                .ok()
                .map(|back| back.origin().ascii_serialization());
            if back.as_deref() != Some(page_origin.as_str()) {
                return Err(format!(
                    "`start` has a `redirect_uri` that isn't an address on {page_origin}, the \
                     login's page's site, so the sign-in would not come back to it"
                ));
            }
        }
    }
    Ok(start.to_owned())
}

#[cfg(test)]
mod tests;
