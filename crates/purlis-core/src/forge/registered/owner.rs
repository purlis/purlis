// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The owner the person expects** (ADR 0128 D1.3 rule 1), on top of the pin ([`super::pin`]).
//!
//! No API returns an OAuth application's owner to purlis: the device-flow and token replies carry
//! none. So purlis cannot print the owner from a response and must not pretend to. The owner is the
//! line the forge's own authorization page shows (*"owned by acme-platform"*), and the person
//! compares it with the one they pinned. *Use this application* therefore asks for it with the
//! client id ([`pin_with_owner`]), keeps it in the same entry, and the window shows
//! [`consent_check`] before it opens the forge's page.
//!
//! A pin made without one (by [`super::pin`] alone, as an older build did) still signs in: the
//! window says no owner was named, and a press of *Use this application* names one.

use super::*;

/// The longest owner path a forge allows, with room to spare.
const MOST_OWNER: usize = 255;

/// An owner as a forge's page spells it: an organization's login, or a group's path with
/// subgroups. Letters, digits, `-`, `_`, `.` and `/` between segments, none empty.
fn owner_ok(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= MOST_OWNER
        && !owner.starts_with('/')
        && !owner.ends_with('/')
        && !owner.contains("//")
        && owner
            .split('/')
            .all(|segment| segment != "." && segment != "..")
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/'))
}

/// What a press of *Use this application* with an owner changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerPinned {
    /// The host as kept.
    pub host: String,
    /// The client id this replaced, when the host was pinned to another: the window raises a
    /// Notice, because accounts signed in through the old application cannot be renewed through
    /// the new one.
    pub replaced: Option<String>,
    /// The owner this replaced, when the host was pinned with another.
    pub owner_was: Option<String>,
}

/// The owner the person pinned for `kind` on `host`, or `None` where the pin names none (or there
/// is no pin). A file that cannot be read is an error, never "none".
pub fn owner_of(config_root: &Path, kind: Kind, host: &str) -> Result<Option<String>, String> {
    let file = read(config_root)?;
    Ok(file
        .hosts
        .iter()
        .filter_map(|value| serde_json::from_value::<Entry>(value.clone()).ok())
        .find(|e| Kind::parse(&e.kind) == Some(kind) && e.host.eq_ignore_ascii_case(host))
        .and_then(|e| {
            e.rest
                .get("owner")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|owner| owner_ok(owner)))
}

/// **The person's press of *Use this application***: [`super::pin`] with the `owner` the person
/// expects the forge's page to show. An owner no forge spells is refused before anything is
/// written.
pub fn pin_with_owner(
    host: &HostScope,
    config_root: &Path,
    kind: Kind,
    typed_host: &str,
    client_id: &str,
    owner: &str,
) -> Result<OwnerPinned, String> {
    let kept = host_named(kind, typed_host)?;
    let owner = owner.trim();
    if !owner_ok(owner) {
        return Err(format!(
            "Name the organization or group that owns the application on {kept}, as its page \
             spells it: letters, digits, dashes, underscores and dots, with / between a group's \
             subgroups."
        ));
    }
    let was = owner_of(config_root, kind, &kept)?;
    let pinned = pin(host, config_root, kind, typed_host, client_id)?;
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let at = file.hosts.iter().position(|value| {
        serde_json::from_value::<Entry>(value.clone())
            .is_ok_and(|e| Kind::parse(&e.kind) == Some(kind) && e.host == pinned.host)
    });
    let Some(Value::Object(entry)) = at.and_then(|at| file.hosts.get_mut(at)) else {
        return Err(format!(
            "purlis pinned the application on {} but could not record its owner. Pin it again with \
             its owner.",
            pinned.host
        ));
    };
    entry.insert("owner".to_string(), Value::String(owner.to_string()));
    write(config_root, file)?;
    Ok(OwnerPinned {
        host: pinned.host,
        replaced: pinned.replaced,
        owner_was: was.filter(|was| was != owner),
    })
}

/// **The sentence the window shows before the sign-in opens the forge's page** (D1.3 rule 1):
/// what the person compares with the line the forge's own page shows. `owner` is the pinned one.
pub fn consent_check(owner: Option<&str>, host: &str) -> String {
    match owner {
        Some(owner) => format!(
            "On the next page, check that the application is owned by {owner} on {host}. Sign in \
             only if it is."
        ),
        None => format!(
            "No owner was named for the application on {host}. On the next page, check that it \
             is owned by the organization or group you expect, and pin the application again with \
             its owner to have it said here."
        ),
    }
}

/// **The request an owner can be sent** (D1.3 rule 3): what to enter when making the application
/// on `host`, as text to copy. GitHub: an OAuth app owned by an organization with *Enable Device
/// Flow* ticked. GitLab: an application owned by a group, not confidential, with the redirect
/// addresses and the `api` scope.
pub fn owner_request(kind: Kind, host: &str) -> String {
    match kind {
        Kind::GitHub => format!(
            "Please register an OAuth app for purlis on {host}.\n\
             \n\
             Owner: an organization (or the enterprise), not a person's own account.\n\
             Settings > Developer settings > OAuth Apps > New OAuth App\n\
             Application name: purlis\n\
             Homepage URL: any address of your own (GitHub requires one; purlis never opens it)\n\
             Authorization callback URL: http://127.0.0.1/purlis/signin (the device flow never uses it)\n\
             Tick: Enable Device Flow\n\
             \n\
             Then send back the client id, shown on the app's page. No client secret is needed."
        ),
        Kind::GitLab => format!(
            "Please register an OAuth application for purlis on {host}.\n\
             \n\
             Owner: a group (Group > Settings > Applications), or the instance (Admin > \
             Applications), not a person's own.\n\
             Name: purlis\n\
             Redirect URIs, one per line:\n{}\n\
             Confidential: off\n\
             Scopes: api\n\
             \n\
             Then send back the application ID. No secret is needed.",
            registration::REDIRECT_PORTS
                .map(registration::redirect_uri)
                .join("\n")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> HostScope {
        HostScope::for_a_test()
    }

    #[test]
    fn pressing_use_this_application_pins_the_client_id_with_the_owner_the_person_expects() {
        let home = tempfile::tempdir().unwrap();
        let kept = pin_with_owner(
            &scope(),
            home.path(),
            Kind::GitHub,
            "GHE.example.com",
            "Iv1.abc",
            "acme-platform",
        )
        .unwrap();
        assert_eq!(kept.host, "ghe.example.com");
        assert_eq!((kept.replaced, kept.owner_was), (None, None));
        assert_eq!(
            owner_of(home.path(), Kind::GitHub, "ghe.example.com").unwrap(),
            Some("acme-platform".into())
        );
        assert_eq!(
            pinned(home.path(), Kind::GitHub, "ghe.example.com")
                .unwrap()
                .unwrap()
                .client_id,
            "Iv1.abc"
        );
    }

    #[test]
    fn an_owner_no_forge_spells_is_refused_and_nothing_is_written() {
        let home = tempfile::tempdir().unwrap();
        for bad in [
            "", "  ", "a b", "/acme", "acme/", "a//b", "../x", "a/./b", "a;b", "a\nb",
        ] {
            assert!(
                pin_with_owner(
                    &scope(),
                    home.path(),
                    Kind::GitLab,
                    "git.example.com",
                    "abc",
                    bad
                )
                .is_err(),
                "{bad:?}"
            );
        }
        assert!(!path(home.path()).exists());
        // A group's subgroups are an owner.
        pin_with_owner(
            &scope(),
            home.path(),
            Kind::GitLab,
            "git.example.com",
            "abc",
            "platform/tools",
        )
        .unwrap();
    }

    #[test]
    fn a_changed_client_id_or_owner_is_a_new_press_that_says_what_it_replaced() {
        let home = tempfile::tempdir().unwrap();
        let press = |id: &str, owner: &str| {
            pin_with_owner(
                &scope(),
                home.path(),
                Kind::GitHub,
                "ghe.example.com",
                id,
                owner,
            )
            .unwrap()
        };
        let first = press("first", "acme");
        assert_eq!((first.replaced, first.owner_was), (None, None));
        let same = press("first", "acme");
        assert_eq!((same.replaced, same.owner_was), (None, None));
        let id = press("second", "acme");
        assert_eq!(id.replaced.as_deref(), Some("first"));
        assert_eq!(id.owner_was, None);
        let owner = press("second", "acme-platform");
        assert_eq!(owner.replaced, None);
        assert_eq!(owner.owner_was.as_deref(), Some("acme"));
        assert_eq!(pinned_hosts(home.path()).unwrap().len(), 1);
    }

    #[test]
    fn a_pin_made_without_an_owner_still_signs_in_and_the_window_says_none_was_named() {
        let home = tempfile::tempdir().unwrap();
        pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "Iv1.old",
        )
        .unwrap();
        assert!(
            pinned(home.path(), Kind::GitHub, "ghe.example.com")
                .unwrap()
                .is_some()
        );
        assert_eq!(
            owner_of(home.path(), Kind::GitHub, "ghe.example.com").unwrap(),
            None
        );
        assert!(consent_check(None, "ghe.example.com").contains("No owner was named"));
    }

    #[test]
    fn the_sentence_before_the_forge_page_names_the_pinned_owner_and_the_host() {
        assert_eq!(
            consent_check(Some("acme-platform"), "ghe.example.com"),
            "On the next page, check that the application is owned by acme-platform on \
             ghe.example.com. Sign in only if it is."
        );
    }

    #[test]
    fn the_request_to_an_owner_asks_for_an_organization_or_group_owned_application() {
        let github = owner_request(Kind::GitHub, "ghe.example.com");
        assert!(github.contains("organization") && github.contains("Enable Device Flow"));
        let gitlab = owner_request(Kind::GitLab, "git.example.com");
        assert!(gitlab.contains("Confidential: off") && gitlab.contains("Scopes: api"));
        for port in registration::REDIRECT_PORTS {
            assert!(
                gitlab.contains(&registration::redirect_uri(port)),
                "{gitlab}"
            );
        }
        assert!(gitlab.contains("not a person's own"));
    }

    #[test]
    fn a_re_pin_by_c_alone_drops_the_owner_so_the_press_must_name_it_again() {
        let home = tempfile::tempdir().unwrap();
        pin_with_owner(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "a",
            "acme",
        )
        .unwrap();
        pin(&scope(), home.path(), Kind::GitHub, "ghe.example.com", "b").unwrap();
        assert_eq!(
            owner_of(home.path(), Kind::GitHub, "ghe.example.com").unwrap(),
            None
        );
    }
}
