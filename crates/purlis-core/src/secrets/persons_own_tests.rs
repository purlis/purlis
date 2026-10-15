// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each vault verb a sandboxed chat leaves to the person, run as the chat runs it: refused
//! before anything is read, with the way that works named (#1969).

use serde_json::json;

use crate::secrets::cmd::tests::{Plane, Rec};
use crate::secrets::cmd::{self, SetFrom};
use crate::secrets::vaultcmd::{self, AddRequest};

const VALUE: &str = "kept-in-the-vault-value";

/// A plane whose chat is sandboxed, with a plain-file vault `team` holding `TOKEN`.
fn chat_plane() -> (Plane, std::path::PathBuf) {
    let plane = Plane::new(&[(crate::hookwire::SANDBOXED_ENV, "1")]);
    let file = team(&plane);
    (plane, file)
}

/// A plain-file vault `team` holding `TOKEN`, in a file of the plane's own root.
fn team(plane: &Plane) -> std::path::PathBuf {
    let file = plane.root().join("team.json");
    std::fs::write(&file, json!({ "TOKEN": VALUE }).to_string()).unwrap();
    plane.register(
        "team",
        "plain-file",
        json!({"file": file.to_string_lossy()}),
        None,
    );
    file
}

/// What a refused verb leaves behind: status 1, nothing on stdout, no value said, and the
/// sentence that names the way. Returns what was said, for the verb's own words.
fn refused(code: i32, rec: &Rec) -> String {
    let said = rec.said();
    assert_eq!(code, 1, "{said}");
    assert!(rec.out.is_empty(), "nothing printed: {}", rec.out());
    assert!(!said.contains(VALUE), "{said}");
    assert!(said.contains("sandboxed chat"), "{said}");
    assert!(
        said.contains("own terminal"),
        "names the person's terminal: {said}"
    );
    said
}

fn registry_of(plane: &Plane) -> String {
    std::fs::read_to_string(plane.ctx.local_registry()).unwrap()
}

#[test]
fn secret_set_in_a_sandboxed_chat_is_the_persons_and_nothing_is_read_or_written() {
    let (plane, file) = chat_plane();
    let before = std::fs::read_to_string(&file).unwrap();
    let mut rec = Rec {
        stdin: "new-value".into(),
        ..Default::default()
    };
    let code = cmd::set(&plane.ctx, "team", "TOKEN", &SetFrom::default(), &mut rec);
    let said = refused(code, &rec);
    assert!(said.contains("purlis secret set team TOKEN"), "{said}");
    assert!(said.contains("vault's tab in the app"), "{said}");
    assert_eq!(rec.stdin, "new-value", "the value was never read");
    assert!(rec.prompts.is_empty(), "nothing was asked");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);

    // Decided before the registry is read: a vault it does not hold gets the same sentence.
    let mut rec = Rec::default();
    let code = cmd::set(&plane.ctx, "nowhere", "K", &SetFrom::default(), &mut rec);
    let said = refused(code, &rec);
    assert!(said.contains("purlis secret set nowhere K"), "{said}");
}

#[test]
fn secret_rm_in_a_sandboxed_chat_is_the_persons_and_the_key_stays() {
    let (plane, file) = chat_plane();
    let before = std::fs::read_to_string(&file).unwrap();
    let mut rec = Rec::default();
    let code = cmd::rm(&plane.ctx, "team", "TOKEN", &mut rec);
    let said = refused(code, &rec);
    assert!(said.contains("purlis secret rm team TOKEN"), "{said}");
    assert!(said.contains("vault's tab in the app"), "{said}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);

    let mut rec = Rec::default();
    let said = refused(cmd::rm(&plane.ctx, "nowhere", "K", &mut rec), &rec);
    assert!(said.contains("purlis secret rm nowhere K"), "{said}");
}

#[test]
fn secret_audit_in_a_sandboxed_chat_names_the_persons_terminal_before_any_read() {
    let (plane, _file) = chat_plane();
    let mut rec = Rec::default();
    let said = refused(cmd::audit(&plane.ctx, "team", 90, &mut rec), &rec);
    assert!(said.contains("purlis secret audit team"), "{said}");

    // A vault the registry does not hold is not looked up: the same sentence.
    let mut rec = Rec::default();
    let said = refused(cmd::audit(&plane.ctx, "nowhere", 90, &mut rec), &rec);
    assert!(said.contains("purlis secret audit nowhere"), "{said}");
    assert!(
        !said.contains("not found") && !said.contains("no vault"),
        "{said}"
    );
}

#[test]
fn vault_add_in_a_sandboxed_chat_is_the_persons_and_the_registry_is_untouched() {
    let (plane, _file) = chat_plane();
    let before = registry_of(&plane);
    let request = AddRequest {
        name: "fresh".into(),
        provider: "plain-file".into(),
        ..Default::default()
    };
    let mut rec = Rec::default();
    let said = refused(vaultcmd::add(&plane.ctx, &request, &mut rec), &rec);
    assert!(said.contains("purlis vault add fresh"), "{said}");
    assert!(said.contains("New vault"), "names the app's way: {said}");
    assert_eq!(registry_of(&plane), before);
    assert!(!plane.ctx.vaults_dir().join("fresh.json").exists());

    // With a token on stdin, the token is never read.
    let request = AddRequest {
        name: "fresh".into(),
        provider: "1password".into(),
        op_vault: Some("Engineering".into()),
        token_stdin: true,
        ..Default::default()
    };
    let mut rec = Rec {
        stdin: "a-token".into(),
        ..Default::default()
    };
    refused(vaultcmd::add(&plane.ctx, &request, &mut rec), &rec);
    assert_eq!(rec.stdin, "a-token", "the token was never read");
    assert!(rec.prompts.is_empty());
    assert_eq!(registry_of(&plane), before);
}

#[test]
fn vault_remove_in_a_sandboxed_chat_is_the_persons_and_the_vault_stays_registered() {
    let (plane, file) = chat_plane();
    let before = registry_of(&plane);
    let mut rec = Rec::default();
    let said = refused(vaultcmd::remove(&plane.ctx, "team", &mut rec), &rec);
    assert!(said.contains("purlis vault remove team"), "{said}");
    assert_eq!(registry_of(&plane), before);
    assert!(file.exists());
}

#[test]
fn vault_verify_in_a_sandboxed_chat_names_the_vaults_panel_and_the_persons_terminal() {
    let (plane, _file) = chat_plane();
    let mut rec = Rec::default();
    let said = refused(vaultcmd::verify(&plane.ctx, Some("team"), &mut rec), &rec);
    assert!(said.contains("purlis vault verify team"), "{said}");
    assert!(said.contains("Vaults panel"), "{said}");
    assert!(
        said.contains("purlis vault list"),
        "names what this chat can run: {said}"
    );

    let mut rec = Rec::default();
    let said = refused(vaultcmd::verify(&plane.ctx, None, &mut rec), &rec);
    assert!(said.contains("`purlis vault verify`"), "{said}");
}

#[test]
fn outside_a_sandboxed_chat_each_verb_runs_as_before() {
    let plane = Plane::new(&[]);
    let file = team(&plane);
    let set = SetFrom {
        value: Some("next".into()),
        ..Default::default()
    };
    let mut rec = Rec::default();
    assert_eq!(
        cmd::set(&plane.ctx, "team", "NEW", &set, &mut rec),
        0,
        "{}",
        rec.said()
    );
    let mut rec = Rec::default();
    assert_eq!(
        cmd::audit(&plane.ctx, "team", 90, &mut rec),
        0,
        "{}",
        rec.said()
    );
    let mut rec = Rec::default();
    assert_eq!(
        vaultcmd::verify(&plane.ctx, Some("team"), &mut rec),
        0,
        "{}",
        rec.said()
    );
    let mut rec = Rec::default();
    assert_eq!(
        cmd::rm(&plane.ctx, "team", "NEW", &mut rec),
        0,
        "{}",
        rec.said()
    );
    let mut rec = Rec::default();
    assert_eq!(
        vaultcmd::remove(&plane.ctx, "team", &mut rec),
        0,
        "{}",
        rec.said()
    );
    assert!(!registry_of(&plane).contains("\"team\""));
    assert!(file.exists());

    // A chat that is not sandboxed reads and writes as the person does (the mark is "1").
    let plane = Plane::new(&[(crate::hookwire::SANDBOXED_ENV, "0")]);
    team(&plane);
    let mut rec = Rec::default();
    assert_eq!(
        cmd::rm(&plane.ctx, "team", "TOKEN", &mut rec),
        0,
        "{}",
        rec.said()
    );
}
