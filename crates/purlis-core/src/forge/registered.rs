// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The OAuth application a person pinned on each of their own hosts** (ADR 0128 D1.3 rule 2:
//! GitHub Enterprise Server and self-hosted GitLab).
//!
//! purlis's compiled-in registrations ([`super::registration::TABLE`]) exist on github.com and
//! gitlab.com only. A GHES host or a self-hosted GitLab knows purlis by nothing until an **owner**
//! makes an application there (owned by an organisation, an enterprise or a group, or instance-wide,
//! never by one person's own account) and the person **pins its client id on this machine**, by
//! pressing *Use this application* in the window. This file keeps those pins, one per kind and host.
//!
//! **The pin is this machine's, and is never synced** (`docs/plane-format.md`: Machine,
//! device-bound). A teammate or a workspace may *suggest* a client id; a suggestion is not in this
//! file until the person presses. A sync that carried a pin would be an attacker's way to point
//! every teammate's sign-in at an application of their own. Nothing in it is secret (a client id is
//! public, and a registration is a public client: the device flow and PKCE send no client secret,
//! and neither does a refresh), but it is a decision of this person, on this machine.
//!
//! **Only the window's press writes it.** [`pin`] and [`unpin`] take a [`HostScope`], which a
//! process inside a chat cannot have ([`HostScope::claim`] refuses it), and nothing else in
//! purlis writes the file: no command, no socket call, no sync. The file is in purlis's folder of
//! the config home, which a chat's sandbox does not write. [`pin`] says what it replaced, so the
//! window can raise a Notice that the application changed: **a changed id is a new press**.
//!
//! **A sign-in keeps the id it was made with.** The account records the client id it signed in
//! through ([`super::accounts::ForgeAccount::client_id`]) and is renewed with that one; if the pin
//! no longer says the same, the renewal stops ([`super::refresh`]). So re-pinning a host cannot make
//! a refresh token, which belongs to one application, go to another.
//!
//! **A compiled-in host cannot be pinned.** A pin on github.com or gitlab.com is refused, so this
//! file can only add hosts, never redirect a sign-in purlis ships.
//!
//! **A registration's flow follows its forge**: the device flow on GitHub, which needs the client
//! id only, and PKCE with the loopback redirect on GitLab ([`super::registration::redirect_uri`]
//! are the addresses the owner's application must list).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::accounts::host_named;
use super::registration::{self, Flow, Registration};
use super::route::HostScope;
use super::{Kind, host_ok};

mod owner;
pub use owner::{OwnerPinned, consent_check, owner_of, owner_request, pin_with_owner};

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
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a list of pinned forge hosts: {e}", at.display()))
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

/// The application the person pinned for `kind` on `host`, or `None` when there is none. An entry
/// this purlis cannot use (an unknown forge, a host that is not a host, a client id with a
/// character none has) is skipped, never half-read. A file that cannot be read is an error, so a
/// caller never takes "unreadable" for "none".
pub fn pinned(
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

/// Every host the person pinned, as `(kind, host)`, in the order they were added.
pub fn pinned_hosts(config_root: &Path) -> Result<Vec<(Kind, String)>, String> {
    Ok(read(config_root)?
        .hosts
        .iter()
        .filter_map(|value| serde_json::from_value::<Entry>(value.clone()).ok())
        .filter_map(|e| Some((Kind::parse(&e.kind)?, e.host)))
        .collect())
}

/// What a press pinned.
#[derive(Debug, PartialEq, Eq)]
pub struct Pinned {
    /// The host as kept.
    pub host: String,
    /// The client id the host was pinned to before, when it was and it was another: what the
    /// window's Notice says changed.
    pub replaced: Option<String>,
}

/// Pins `client_id` for `kind` on `host`, replacing the one already there. **Only the window's
/// press calls this** (the scope is the proof that it is not a chat). The host is normalised as a
/// sign-in's is ([`host_named`]); a host purlis ships a registration on is refused.
pub fn pin(
    _host: &HostScope,
    config_root: &Path,
    kind: Kind,
    typed_host: &str,
    client_id: &str,
) -> Result<Pinned, String> {
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
    let mut replaced = None;
    match file.hosts.iter().position(same) {
        Some(at) => {
            replaced = serde_json::from_value::<Entry>(file.hosts[at].clone())
                .ok()
                .map(|e| e.client_id)
                .filter(|was| was != client_id);
            file.hosts[at] = entry;
        }
        None => file.hosts.push(entry),
    }
    write(config_root, file)?;
    Ok(Pinned { host, replaced })
}

/// Takes `host`'s pin away. An account signed in through it stays listed and keeps its token
/// until it expires; it can no longer be renewed.
pub fn unpin(
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
    fn a_ghes_host_pinned_to_a_client_id_signs_in_by_the_device_flow() {
        let home = tempfile::tempdir().unwrap();
        let kept = pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            " https://GHE.Example.com/ ",
            "Iv1.abc123",
        )
        .unwrap();
        assert_eq!(kept.host, "ghe.example.com");
        assert_eq!(kept.replaced, None);
        let found = pinned(home.path(), Kind::GitHub, "GHE.example.com")
            .unwrap()
            .unwrap();
        assert_eq!(found.flow, Flow::Device);
        assert_eq!(found.client_id, "Iv1.abc123");
        assert_eq!(found.host, "ghe.example.com");
    }

    #[test]
    fn a_self_hosted_gitlab_pinned_to_an_application_signs_in_by_pkce() {
        let home = tempfile::tempdir().unwrap();
        pin(
            &scope(),
            home.path(),
            Kind::GitLab,
            "git.example.com",
            "f00dbabe",
        )
        .unwrap();
        let found = pinned(home.path(), Kind::GitLab, "git.example.com")
            .unwrap()
            .unwrap();
        assert_eq!(found.flow, Flow::Pkce);
        assert_eq!(found.kind, Kind::GitLab);
    }

    #[test]
    fn a_registration_is_for_its_own_kind_and_host_only() {
        let home = tempfile::tempdir().unwrap();
        pin(
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
                pinned(home.path(), kind, host)
                    .unwrap()
                    .map(|r| r.client_id),
                None,
                "{kind:?} on {host}"
            );
        }
    }

    #[test]
    fn a_host_purlis_ships_a_registration_on_cannot_be_overridden() {
        let home = tempfile::tempdir().unwrap();
        for (kind, host) in [(Kind::GitHub, "github.com"), (Kind::GitLab, "GitLab.com")] {
            let why = pin(&scope(), home.path(), kind, host, "abc").unwrap_err();
            assert!(why.contains("cannot be replaced"), "{why}");
        }
        assert!(!path(home.path()).exists());
    }

    #[test]
    fn a_client_id_with_anything_but_letters_digits_and_dashes_is_refused() {
        let home = tempfile::tempdir().unwrap();
        for bad in ["", "  ", "a b", "a\nb", "a/b", "a;b", &"x".repeat(129)] {
            assert!(
                pin(&scope(), home.path(), Kind::GitHub, "ghe.example.com", bad).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn pinning_again_replaces_the_id_and_unpinning_takes_it_away() {
        let home = tempfile::tempdir().unwrap();
        pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "first",
        )
        .unwrap();
        pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "second",
        )
        .unwrap();
        assert_eq!(pinned_hosts(home.path()).unwrap().len(), 1);
        assert_eq!(
            pinned(home.path(), Kind::GitHub, "ghe.example.com")
                .unwrap()
                .unwrap()
                .client_id,
            "second"
        );
        unpin(&scope(), home.path(), Kind::GitHub, "ghe.example.com").unwrap();
        assert_eq!(
            pinned(home.path(), Kind::GitHub, "ghe.example.com").unwrap(),
            None
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_never_no_registration() {
        let home = tempfile::tempdir().unwrap();
        let at = path(home.path());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "not json").unwrap();
        assert!(pinned(home.path(), Kind::GitHub, "ghe.example.com").is_err());
        assert!(
            pin(
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
            pinned(home.path(), Kind::GitHub, "x.example.com").unwrap(),
            None
        );
        pin(
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
        pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "abc",
        )
        .unwrap();
        let a = pinned(home.path(), Kind::GitHub, "ghe.example.com")
            .unwrap()
            .unwrap();
        let b = pinned(home.path(), Kind::GitHub, "ghe.example.com")
            .unwrap()
            .unwrap();
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn pinning_a_different_id_says_what_it_replaced_and_the_same_id_replaces_nothing() {
        let home = tempfile::tempdir().unwrap();
        let first = pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "one",
        )
        .unwrap();
        assert_eq!(first.replaced, None);
        let same = pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "one",
        )
        .unwrap();
        assert_eq!(same.replaced, None);
        let other = pin(
            &scope(),
            home.path(),
            Kind::GitHub,
            "ghe.example.com",
            "two",
        )
        .unwrap();
        assert_eq!(other.replaced.as_deref(), Some("one"));
    }

    #[test]
    fn a_process_inside_a_chat_has_no_scope_and_so_cannot_pin() {
        let in_a_chat = |name: &str| (name == crate::hookwire::CHAT_ENV).then(|| "1".to_string());
        assert!(HostScope::claim_in(&in_a_chat).is_none());
        assert!(HostScope::claim_in(&|_| None).is_some());
    }

    #[test]
    fn nothing_but_this_file_writes_the_pin_file() {
        // `pin` and `unpin` are the only writers, each taking the host's scope. No other source
        // file of the crate or the app names the file at all.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut named_elsewhere = Vec::new();
        for dir in [root.join("src"), root.join("../../app/src-tauri/src")] {
            visit(&dir, &mut |path, text| {
                if !path.ends_with("forge/registered.rs") && text.contains("forge-registrations") {
                    named_elsewhere.push(path.display().to_string());
                }
            });
        }
        assert_eq!(named_elsewhere, Vec::<String>::new());
    }

    fn visit(dir: &Path, with: &mut dyn FnMut(&Path, &str)) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, with);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                with(&path, &text);
            }
        }
    }
}
