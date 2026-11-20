// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The OAuth applications a person registered on their own hosts** (ADR 0128 D1: GitHub
//! Enterprise Server and self-hosted GitLab).
//!
//! purlis's compiled-in registrations ([`super::registration::TABLE`]) exist on github.com and
//! gitlab.com only. A GHES host or a self-hosted GitLab knows purlis by nothing until somebody
//! makes an application there and gives purlis its **client id**: on GHES one OAuth app or GitHub
//! App with the device flow ticked, on a self-hosted GitLab an OAuth application owned by the
//! person's own profile (no administrator, no instance setting). This file keeps those client
//! ids, one per kind and host.
//!
//! **Nothing here is secret.** A client id is public, and a registration is a public client: the
//! device flow and PKCE send no client secret, and neither does a refresh. So the file is a
//! Machine-tier store a person's sync may carry (`docs/plane-format.md`), and a restored copy
//! asks nothing of the keyring.
//!
//! **Only the host writes it.** [`register`] takes a [`HostScope`], refused inside a chat's
//! process tree, as a sign-in does (ADR 0128 D5): a chat cannot point the sign-in at an
//! application of its own.
//!
//! **A compiled-in host cannot be overridden.** A registration on github.com or gitlab.com is
//! refused, so this file can only add hosts, never redirect a sign-in purlis ships.
//!
//! **A registration's flow follows its forge**: the device flow on GitHub, which needs the client
//! id only, and PKCE with the loopback redirect on GitLab ([`super::registration::redirect_uri`]
//! are the addresses the person's application must list).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::accounts::host_named;
use super::registration::{self, Flow, Registration};
use super::route::HostScope;
use super::{Kind, host_ok};

/// The registrations file, in purlis's directory of the config home.
pub const FILE: &str = "forge-registrations.json";

const VERSION: u64 = 1;

/// The most [`FILE`] may be. A machine has a handful of hosts.
const MOST_BYTES: u64 = 1 << 20;

/// The longest client id a forge issues, with room to spare.
const MOST_ID: usize = 128;

/// Where [`FILE`] is under the config home `config_root`.
pub fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FILE)
}

/// One writer at a time, whatever the file.
static WRITING: Mutex<()> = Mutex::new(());

/// The file as written: every entry as it was, so a field a later purlis adds survives a write
/// by this one.
#[derive(Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    hosts: Vec<Value>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    kind: String,
    host: String,
    client_id: String,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// Registrations already handed out, kept for the life of the process: a [`Registration`] is
/// `'static` (the sign-in flows take one by reference, and the compiled-in table is a constant),
/// so each distinct one is made once and kept. A machine has a handful.
static HANDED_OUT: Mutex<Vec<&'static Registration>> = Mutex::new(Vec::new());

fn handed_out(kind: Kind, host: &str, client_id: &str) -> &'static Registration {
    let mut kept = HANDED_OUT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(same) = kept
        .iter()
        .find(|r| r.kind == kind && r.host == host && r.client_id == client_id)
    {
        return same;
    }
    let made: &'static Registration = Box::leak(Box::new(Registration {
        kind,
        host: Box::leak(host.to_string().into_boxed_str()),
        client_id: Box::leak(client_id.to_string().into_boxed_str()),
        flow: match kind {
            Kind::GitHub => Flow::Device,
            Kind::GitLab => Flow::Pkce,
        },
    }));
    kept.push(made);
    made
}

fn id_ok(client_id: &str) -> bool {
    !client_id.is_empty()
        && client_id.len() <= MOST_ID
        && client_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn read(config_root: &Path) -> Result<OnDisk, String> {
    let at = path(config_root);
    let file = match std::fs::File::open(&at) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(OnDisk::default()),
        Err(e) => return Err(format!("purlis cannot read {}: {e}", at.display())),
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(file, MOST_BYTES + 1), &mut text)
        .map_err(|e| format!("purlis cannot read {}: {e}", at.display()))?;
    if text.len() as u64 > MOST_BYTES {
        return Err(format!(
            "{} is larger than a list of hosts can be",
            at.display()
        ));
    }
    serde_json::from_str(&text).map_err(|e| {
        format!(
            "{} is not a list of registered forge hosts: {e}",
            at.display()
        )
    })
}

fn write(config_root: &Path, mut file: OnDisk) -> Result<(), String> {
    file.version = VERSION;
    let at = path(config_root);
    let dir = crate::machine::private_dir(config_root)
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))?;
    let text = serde_json::to_string_pretty(&file).unwrap_or_default() + "\n";
    crate::machine::write_beside(config_root, &dir.join(FILE), text.as_bytes())
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))
}

/// The registration the person made for `kind` on `host`, or `None` when there is none. An entry
/// this purlis cannot use (an unknown forge, a host that is not a host, a client id with a
/// character none has) is skipped, never half-read. A file that cannot be read is an error, so a
/// caller never takes "unreadable" for "none".
pub fn find(
    config_root: &Path,
    kind: Kind,
    host: &str,
) -> Result<Option<&'static Registration>, String> {
    let file = read(config_root)?;
    Ok(file
        .hosts
        .iter()
        .filter_map(|value| serde_json::from_value::<Entry>(value.clone()).ok())
        .find(|e| {
            Kind::parse(&e.kind) == Some(kind)
                && e.host == e.host.to_ascii_lowercase()
                && host_ok(&e.host)
                && e.host.eq_ignore_ascii_case(host)
                && id_ok(&e.client_id)
        })
        .map(|e| handed_out(kind, &e.host, &e.client_id)))
}

/// Every host the person registered, as `(kind, host)`, in the order they were added.
pub fn hosts(config_root: &Path) -> Result<Vec<(Kind, String)>, String> {
    Ok(read(config_root)?
        .hosts
        .iter()
        .filter_map(|value| serde_json::from_value::<Entry>(value.clone()).ok())
        .filter_map(|e| Some((Kind::parse(&e.kind)?, e.host)))
        .collect())
}

/// Registers `client_id` for `kind` on `host`, replacing the one already there. The host is
/// normalised as a sign-in's is ([`host_named`]); a host purlis ships a registration on is
/// refused. Returns the host as kept.
pub fn register(
    _host: &HostScope,
    config_root: &Path,
    kind: Kind,
    typed_host: &str,
    client_id: &str,
) -> Result<String, String> {
    let host = host_named(kind, typed_host)?;
    if registration::on(kind, &host).is_some() {
        return Err(format!(
            "purlis already signs in to {host} with its own registration, and that one cannot \
             be replaced. Nothing was changed."
        ));
    }
    let client_id = client_id.trim();
    if !id_ok(client_id) {
        return Err(
            "A client id is letters, digits, dashes, underscores and dots, as the forge shows it \
             on the application's page. Copy it again."
                .to_string(),
        );
    }
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let entry = serde_json::json!({
        "kind": kind.word(),
        "host": host,
        "client_id": client_id,
    });
    let same = |value: &Value| {
        serde_json::from_value::<Entry>(value.clone())
            .is_ok_and(|e| Kind::parse(&e.kind) == Some(kind) && e.host == host)
    };
    match file.hosts.iter().position(same) {
        Some(at) => file.hosts[at] = entry,
        None => file.hosts.push(entry),
    }
    write(config_root, file)?;
    Ok(host)
}

/// Takes `host`'s registration away. An account signed in through it stays listed and keeps
/// its token until it expires; it can no longer be renewed.
pub fn unregister(
    _host: &HostScope,
    config_root: &Path,
    kind: Kind,
    typed_host: &str,
) -> Result<(), String> {
    let host = host_named(kind, typed_host)?;
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let before = file.hosts.len();
    file.hosts.retain(|value| {
        !serde_json::from_value::<Entry>(value.clone())
            .is_ok_and(|e| Kind::parse(&e.kind) == Some(kind) && e.host == host)
    });
    if file.hosts.len() == before {
        return Ok(());
    }
    write(config_root, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> HostScope {
        HostScope::for_a_test()
    }

    #[test]
    fn a_ghes_host_registered_with_a_client_id_signs_in_by_the_device_flow() {
        let home = tempfile::tempdir().unwrap();
        let kept = register(
            &scope(),
            home.path(),
            Kind::GitHub,
            " https://GHE.Example.com/ ",
            "Iv1.abc123",
        )
        .unwrap();
        assert_eq!(kept, "ghe.example.com");
        let found = find(home.path(), Kind::GitHub, "GHE.example.com")
            .unwrap()
            .unwrap();
        assert_eq!(found.flow, Flow::Device);
        assert_eq!(found.client_id, "Iv1.abc123");
        assert_eq!(found.host, "ghe.example.com");
    }

    #[test]
    fn a_self_hosted_gitlab_registered_with_a_user_owned_application_signs_in_by_pkce() {
        let home = tempfile::tempdir().unwrap();
        register(
            &scope(),
            home.path(),
            Kind::GitLab,
            "git.example.com",
            "f00dbabe",
        )
        .unwrap();
        let found = find(home.path(), Kind::GitLab, "git.example.com")
            .unwrap()
            .unwrap();
        assert_eq!(found.flow, Flow::Pkce);
        assert_eq!(found.kind, Kind::GitLab);
    }

    #[test]
    fn a_registration_is_for_its_own_kind_and_host_only() {
        let home = tempfile::tempdir().unwrap();
        register(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "abc",
        )
        .unwrap();
        for (kind, host) in [
            (Kind::GitLab, "ghe.example.com"),
            (Kind::GitHub, "other.example.com"),
            (Kind::GitHub, "example.com"),
            (Kind::GitHub, "ghe.example.com.evil.net"),
        ] {
            assert_eq!(
                find(home.path(), kind, host).unwrap().map(|r| r.client_id),
                None,
                "{kind:?} on {host}"
            );
        }
    }

    #[test]
    fn a_host_purlis_ships_a_registration_on_cannot_be_overridden() {
        let home = tempfile::tempdir().unwrap();
        for (kind, host) in [(Kind::GitHub, "github.com"), (Kind::GitLab, "GitLab.com")] {
            let why = register(&scope(), home.path(), kind, host, "abc").unwrap_err();
            assert!(why.contains("cannot be replaced"), "{why}");
        }
        assert!(!path(home.path()).exists());
    }

    #[test]
    fn a_client_id_with_anything_but_letters_digits_and_dashes_is_refused() {
        let home = tempfile::tempdir().unwrap();
        for bad in ["", "  ", "a b", "a\nb", "a/b", "a;b", &"x".repeat(129)] {
            assert!(
                register(&scope(), home.path(), Kind::GitHub, "ghe.example.com", bad).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn registering_again_replaces_the_id_and_unregistering_takes_it_away() {
        let home = tempfile::tempdir().unwrap();
        register(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "first",
        )
        .unwrap();
        register(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "second",
        )
        .unwrap();
        assert_eq!(hosts(home.path()).unwrap().len(), 1);
        assert_eq!(
            find(home.path(), Kind::GitHub, "ghe.example.com")
                .unwrap()
                .unwrap()
                .client_id,
            "second"
        );
        unregister(&scope(), home.path(), Kind::GitHub, "ghe.example.com").unwrap();
        assert_eq!(
            find(home.path(), Kind::GitHub, "ghe.example.com").unwrap(),
            None
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_never_no_registration() {
        let home = tempfile::tempdir().unwrap();
        let at = path(home.path());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "not json").unwrap();
        assert!(find(home.path(), Kind::GitHub, "ghe.example.com").is_err());
        assert!(
            register(
                &scope(),
                home.path(),
                Kind::GitHub,
                "ghe.example.com",
                "abc"
            )
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "not json");
    }

    #[test]
    fn an_entry_this_purlis_cannot_use_is_skipped_and_kept_when_the_file_is_written() {
        let home = tempfile::tempdir().unwrap();
        let at = path(home.path());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            &at,
            r#"{"version":1,"hosts":[{"kind":"forgejo","host":"x.example.com","client_id":"a"},
               {"kind":"github","host":"Bad Host","client_id":"a"}],"note":"kept"}"#,
        )
        .unwrap();
        assert_eq!(
            find(home.path(), Kind::GitHub, "x.example.com").unwrap(),
            None
        );
        register(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "abc",
        )
        .unwrap();
        let text = std::fs::read_to_string(&at).unwrap();
        assert!(
            text.contains("forgejo") && text.contains("\"note\""),
            "{text}"
        );
    }

    #[test]
    fn the_same_registration_is_handed_out_as_the_same_reference() {
        let home = tempfile::tempdir().unwrap();
        register(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "abc",
        )
        .unwrap();
        let a = find(home.path(), Kind::GitHub, "ghe.example.com")
            .unwrap()
            .unwrap();
        let b = find(home.path(), Kind::GitHub, "ghe.example.com")
            .unwrap()
            .unwrap();
        assert!(std::ptr::eq(a, b));
    }
}
