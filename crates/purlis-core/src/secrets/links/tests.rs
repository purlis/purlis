// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Linked entries read through the vault seam ([`cmd`]): what a key that links to another
//! entry reads, lists, refuses and reports, in a temp plane. The keyring here is the fenced
//! build's stub, so nothing reaches this machine's keychain.

use serde_json::{Value, json};

use crate::secrets::cmd::{self, Io, Say};
use crate::secrets::registry::{self, Vault};
use crate::secrets::{Ctx, Env, Kind, vaultcmd};

use super::*;

struct Plane {
    dir: tempfile::TempDir,
    ctx: Ctx,
}

impl Plane {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ctx = Ctx::new(dir.path(), Env::of(&[]));
        Self { dir, ctx }
    }

    fn register(&self, name: &str, provider: &str, config: Value) -> Vault {
        let path = self.ctx.local_registry();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut doc: Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| json!({"vaults": {}}));
        doc["vaults"][name] = json!({"provider": provider, "persona": null, "config": config});
        std::fs::write(&path, doc.to_string()).unwrap();
        registry::vault(&self.ctx, name).unwrap()
    }

    fn keyring(&self, name: &str) -> Vault {
        self.register(name, "keyring", json!({}))
    }

    fn plain(&self, name: &str, secrets: Value) -> Vault {
        let file = self.ctx.vaults_dir().join(format!("{name}.json"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, secrets.to_string()).unwrap();
        self.register(name, "plain-file", json!({"file": file.to_string_lossy()}))
    }

    fn stub(&self) -> String {
        std::fs::read_to_string(self.dir.path().join(".charter/keyring-stub.json"))
            .unwrap_or_default()
    }
}

#[derive(Default)]
struct Said(Vec<String>);

impl Io for Said {
    fn say(&mut self, line: Say) {
        self.0.push(match line {
            Say::Info(t) => format!("info: {t}"),
            Say::Ok(t) => format!("ok: {t}"),
            Say::Warn(t) => format!("warn: {t}"),
            Say::Err(t) => format!("err: {t}"),
        });
    }
    fn out(&mut self, bytes: &[u8]) {
        self.0.push(String::from_utf8_lossy(bytes).into_owned());
    }
    fn err(&mut self, bytes: &[u8]) {
        self.0.push(String::from_utf8_lossy(bytes).into_owned());
    }
    fn stdout_is_terminal(&self) -> bool {
        false
    }
    fn stdin_is_terminal(&self) -> bool {
        false
    }
    fn read_stdin(&mut self) -> String {
        String::new()
    }
    fn read_hidden(&mut self, _prompt: &str) -> String {
        String::new()
    }
}

const STRIPE: &str = "sk_fixture-linked-5069-aa";
const ROTATED: &str = "sk_fixture-rotated-5069-bb";

/// A persona's keyring vault `ops`, and the person's own `team` vault holding `STRIPE`.
fn ops_and_team() -> (Plane, Vault, Vault) {
    let plane = Plane::new();
    let ops = plane.keyring("ops");
    let team = plane.plain("team", json!({"STRIPE": STRIPE, "OTHER": "o"}));
    (plane, ops, team)
}

#[test]
fn a_link_reads_its_targets_current_value_also_after_the_target_rotates() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    assert_eq!(
        cmd::get_value(&plane.ctx, &ops, "STRIPE_KEY").unwrap(),
        STRIPE
    );

    cmd::set_value(&plane.ctx, &team, "STRIPE", ROTATED).unwrap();
    assert_eq!(
        cmd::get_value(&plane.ctx, &ops, "STRIPE_KEY").unwrap(),
        ROTATED
    );
}

#[test]
fn a_link_is_listed_among_its_vaults_keys() {
    let (plane, ops, _team) = ops_and_team();
    cmd::set_value(&plane.ctx, &ops, "OWN", "v").unwrap();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    assert_eq!(cmd::keys(&plane.ctx, &ops).unwrap(), ["OWN", "STRIPE_KEY"]);
    assert_eq!(
        of(&plane.ctx, "ops", "STRIPE_KEY").unwrap(),
        Some(Target {
            vault: "team".into(),
            key: "STRIPE".into()
        })
    );
    assert_eq!(of(&plane.ctx, "ops", "OWN").unwrap(), None);
}

#[test]
fn a_link_in_a_keyring_vault_makes_no_keyring_item() {
    let (plane, ops, _team) = ops_and_team();
    let before = plane.stub();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    assert_eq!(plane.stub(), before, "a link wrote to the keyring");
    assert!(!plane.stub().contains("STRIPE_KEY"));
    let index = crate::secrets::keyring::load_index(&plane.ctx, &ops).unwrap();
    assert!(index.keys.is_empty(), "{index:?}");
}

#[test]
fn a_link_to_a_link_is_refused() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    let e = link(&plane.ctx, &team, "AGAIN", "ops", "STRIPE_KEY").unwrap_err();
    assert!(e.message.contains("is itself a linked entry"), "{e}");
    assert!(cmd::get_value(&plane.ctx, &team, "AGAIN").is_err());
}

#[test]
fn a_key_other_keys_link_to_cannot_become_a_link_itself() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    let e = link(&plane.ctx, &team, "STRIPE", "team", "OTHER").unwrap_err();
    assert!(e.message.contains("1 linked entry reads it"), "{e}");
}

#[test]
fn a_link_to_an_entry_that_is_not_there_is_refused() {
    let (plane, ops, _team) = ops_and_team();
    let gone = link(&plane.ctx, &ops, "K", "team", "NOPE").unwrap_err();
    assert!(gone.message.contains("holds no 'NOPE'"), "{gone}");
    let unknown = link(&plane.ctx, &ops, "K", "nowhere", "STRIPE").unwrap_err();
    assert_eq!(unknown.kind, Kind::NotConfigured, "{unknown}");
    let itself = link(&plane.ctx, &ops, "K", "ops", "K").unwrap_err();
    assert!(itself.message.contains("to itself"), "{itself}");
    assert!(cmd::keys(&plane.ctx, &ops).unwrap().is_empty());
}

#[test]
fn a_link_over_a_value_the_key_holds_is_refused() {
    let (plane, ops, _team) = ops_and_team();
    cmd::set_value(&plane.ctx, &ops, "STRIPE_KEY", "mine").unwrap();
    let e = link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap_err();
    assert!(e.message.contains("already holds a value"), "{e}");
    assert_eq!(
        cmd::get_value(&plane.ctx, &ops, "STRIPE_KEY").unwrap(),
        "mine"
    );
}

#[test]
fn a_link_is_re_pointed_by_linking_it_again() {
    let (plane, ops, _team) = ops_and_team();
    link(&plane.ctx, &ops, "K", "team", "STRIPE").unwrap();
    link(&plane.ctx, &ops, "K", "team", "OTHER").unwrap();
    assert_eq!(cmd::get_value(&plane.ctx, &ops, "K").unwrap(), "o");
}

#[test]
fn a_broken_link_is_reported_on_read_without_naming_where_it_pointed() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    cmd::delete(&plane.ctx, &team, "STRIPE").unwrap();

    let e = cmd::get_value(&plane.ctx, &ops, "STRIPE_KEY").unwrap_err();
    assert_eq!(e.kind, Kind::BrokenLink, "{e}");
    assert!(
        e.message
            .contains("'STRIPE_KEY' in vault 'ops' is a linked entry"),
        "{e}"
    );
    assert!(e.message.contains("purlis vault verify ops"), "{e}");
    // What a chat hears of a read never says where the value lives.
    assert!(!e.message.contains("team"), "{e}");
}

#[test]
fn a_link_into_a_vault_that_is_gone_is_broken_too() {
    let (plane, ops, _team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    registry::remove_vault(&plane.ctx, "team").unwrap();
    let e = cmd::get_value(&plane.ctx, &ops, "STRIPE_KEY").unwrap_err();
    assert_eq!(e.kind, Kind::BrokenLink, "{e}");
}

#[test]
fn vault_verify_reports_a_broken_link_and_names_the_entry_it_pointed_at() {
    let (plane, ops, team) = ops_and_team();
    cmd::set_value(&plane.ctx, &ops, "OWN", "v").unwrap();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    let mut io = Said::default();
    assert_eq!(
        vaultcmd::verify(&plane.ctx, Some("ops"), &mut io),
        0,
        "{:?}",
        io.0
    );

    cmd::delete(&plane.ctx, &team, "STRIPE").unwrap();
    let mut io = Said::default();
    assert_eq!(vaultcmd::verify(&plane.ctx, Some("ops"), &mut io), 1);
    let said = io.0.join("\n");
    assert!(said.contains("1 of 2 references did NOT resolve"), "{said}");
    assert!(said.contains("STRIPE_KEY"), "{said}");
    assert!(
        said.contains("'STRIPE' in vault 'team', which is gone"),
        "{said}"
    );
}

#[test]
fn deleting_a_link_leaves_the_entry_it_reads() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    cmd::delete(&plane.ctx, &ops, "STRIPE_KEY").unwrap();
    assert!(cmd::keys(&plane.ctx, &ops).unwrap().is_empty());
    assert_eq!(cmd::get_value(&plane.ctx, &team, "STRIPE").unwrap(), STRIPE);
}

#[test]
fn the_links_that_read_an_entry_are_counted() {
    let (plane, ops, _team) = ops_and_team();
    let web = plane.keyring("web");
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    link(&plane.ctx, &web, "PAYMENTS", "team", "STRIPE").unwrap();
    link(&plane.ctx, &web, "ELSE", "team", "OTHER").unwrap();
    assert_eq!(
        linking_to(&plane.ctx, "team", "STRIPE").unwrap(),
        [
            ("ops".to_string(), "STRIPE_KEY".to_string()),
            ("web".to_string(), "PAYMENTS".to_string())
        ]
    );
    assert!(linking_to(&plane.ctx, "team", "NONE").unwrap().is_empty());
    let read = readers_in(&plane.ctx, "team").unwrap();
    assert_eq!(read.get("STRIPE"), Some(&2));
    assert_eq!(read.get("OTHER"), Some(&1));
}

#[test]
fn a_value_set_on_a_link_replaces_the_link() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    cmd::set_value(&plane.ctx, &ops, "STRIPE_KEY", "own").unwrap();
    assert_eq!(of(&plane.ctx, "ops", "STRIPE_KEY").unwrap(), None);
    assert_eq!(
        cmd::get_value(&plane.ctx, &ops, "STRIPE_KEY").unwrap(),
        "own"
    );
    assert_eq!(cmd::get_value(&plane.ctx, &team, "STRIPE").unwrap(), STRIPE);
}

#[test]
fn renaming_a_link_moves_it_and_renaming_its_target_re_points_it() {
    let (plane, ops, team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    cmd::rename(&plane.ctx, &ops, "STRIPE_KEY", "PAY").unwrap();
    assert_eq!(cmd::keys(&plane.ctx, &ops).unwrap(), ["PAY"]);
    assert_eq!(cmd::get_value(&plane.ctx, &ops, "PAY").unwrap(), STRIPE);

    cmd::rename(&plane.ctx, &team, "STRIPE", "STRIPE_LIVE").unwrap();
    assert_eq!(cmd::get_value(&plane.ctx, &ops, "PAY").unwrap(), STRIPE);
    assert_eq!(
        of(&plane.ctx, "ops", "PAY").unwrap().unwrap().key,
        "STRIPE_LIVE"
    );
}

#[test]
fn a_link_never_reaches_a_github_apps_key() {
    let (plane, ops, _team) = ops_and_team();
    let reserved = format!("{}KEY", crate::sandbox::tools::mint::RESERVED_PREFIX);
    let e = link(&plane.ctx, &ops, "K", "team", &reserved).unwrap_err();
    assert!(!e.message.is_empty());
    let e = link(&plane.ctx, &ops, &reserved, "team", "STRIPE").unwrap_err();
    assert!(!e.message.is_empty());
}

#[test]
fn a_destroyed_vault_takes_its_links_with_it() {
    let (plane, ops, _team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    vaultcmd::destroy(&plane.ctx, "ops").unwrap();
    plane.keyring("ops");
    assert_eq!(of(&plane.ctx, "ops", "STRIPE_KEY").unwrap(), None);
    assert!(linking_to(&plane.ctx, "team", "STRIPE").unwrap().is_empty());
}

#[test]
fn the_links_index_holds_names_only_and_is_private() {
    let (plane, ops, _team) = ops_and_team();
    link(&plane.ctx, &ops, "STRIPE_KEY", "team", "STRIPE").unwrap();
    let p = path(&plane.ctx);
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(!text.contains(STRIPE), "{text}");
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        doc,
        json!({"links": {"ops": {"STRIPE_KEY": {"vault": "team", "key": "STRIPE"}}}})
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn a_corrupt_links_index_is_refused_and_named() {
    let (plane, ops, _team) = ops_and_team();
    std::fs::create_dir_all(path(&plane.ctx).parent().unwrap()).unwrap();
    std::fs::write(
        path(&plane.ctx),
        r#"{"links": {"ops": {"K": {"vault": 3}}}}"#,
    )
    .unwrap();
    let e = cmd::get_value(&plane.ctx, &ops, "K").unwrap_err();
    assert!(e.message.contains("vault-links.json is corrupt"), "{e}");
}
