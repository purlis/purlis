// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No existing secret path resolves a your-keys entry** (T1a-72, R-772).
//!
//! Your keys (`purlis/@your-keys`) is read by `yourkeys::read_key` alone. Each path that hands a
//! vault value to something else is given a your-keys entry, by the vault name `@your-keys` and
//! by its full service `purlis/@your-keys`, with the entry's id as the key: `purlis secret exec`,
//! a chat's `secret_ask` (its line and a Link answer to it), the browser's secret fill (its gate
//! and its read), and a tool value sent to a runner (the desktop's gate and its read). Each
//! refuses, names the entry as your keys and not its own, and reads nothing from the stub
//! keyring the entry is held in.
//!
//! Every value here is synthetic, and every read reaches the fenced stub keyring only.

use std::path::Path;
use std::time::Duration;

use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::seam::{Record, RecordingCredentials};
use purlis_core::secrets::ask::{self, Answer, Asks, Chat, Need, SecretAsk};
use purlis_core::secrets::cmd::{Io, Say};
use purlis_core::secrets::exec::{Request, exec};
use purlis_core::secrets::keyring::{self, FileStore, STUB_FILE, Store};
use purlis_core::secrets::links::Target;
use purlis_core::secrets::{Ctx, Env, brokered, cmd};
use purlis_core::yourkeys::{self, Checked, Product, SERVICE, Saved};

/// The words every refusal names the entry by: your keys, which purlis alone reads.
const NOT_THEIRS: [&str; 2] = ["is your keys", "purlis alone reads"];

#[derive(Default)]
struct Heard(Vec<String>);

impl Io for Heard {
    fn say(&mut self, line: Say) {
        self.0.push(format!("{line:?}"));
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
    fn read_hidden(&mut self, _: &str) -> String {
        String::new()
    }
}

/// A project whose persona `qa` has a vault of its own, `qa`, holding one value.
fn project(root: &Path) {
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "qa": {"provider": "plain-file", "config": {"file": "qa.json"}, "persona": "qa"},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("qa.json"),
        serde_json::json!({"TOKEN": "synthetic-qa-value"}).to_string(),
    )
    .unwrap();
}

fn ctx(root: &Path) -> Ctx {
    let home = root.join("home");
    Ctx::new(root, Env::of(&[("PURLIS_HOME", &home.to_string_lossy())]))
}

/// Whether what one path said is a refusal naming the entry as your keys: `None` if it is, or
/// what is wrong, so every path is judged before the test fails, each by its own name.
fn refused_as_your_keys(path: &str, said: Result<String, String>) -> Option<String> {
    let said = match said {
        Ok(answered) => {
            return Some(format!(
                "{path} did not refuse a your-keys entry: {answered}"
            ));
        }
        Err(why) => why,
    };
    NOT_THEIRS
        .iter()
        .find(|words| !said.contains(*words))
        .map(|words| {
            format!("{path} refused without naming the entry as your keys ({words:?}): {said}")
        })
}

#[test]
fn no_existing_secret_path_resolves_a_your_keys_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    project(root);
    let ctx = ctx(root);
    // The entry, added as the window adds it, into the very stub keyring every path here reads.
    let stub = ctx.state.join(STUB_FILE);
    let config = tempfile::tempdir().unwrap();
    let credentials =
        RecordingCredentials::new(Box::new(FileStore::at(stub.clone())), Record::default());
    let id = match yourkeys::add(
        config.path(),
        &credentials,
        Product::anthropic(),
        Checked::Passed,
        "synthetic-your-keys-value",
    ) {
        Ok(Saved::Added(id)) => id.to_string(),
        other => panic!("the key was not added: {other:?}"),
    };
    assert!(
        FileStore::at(stub.clone())
            .get(SERVICE, &id)
            .unwrap()
            .is_some(),
        "the entry is held where the paths read"
    );
    let mut misses: Vec<String> = Vec::new();
    let mut reads = keyring::stub_reads(&stub);
    let mut unread = |path: &str, misses: &mut Vec<String>| {
        let now = keyring::stub_reads(&stub);
        if now != reads {
            misses.push(format!("{path} read from the keyring that holds {SERVICE}"));
        }
        reads = now;
    };
    let asker = brokered::Asker {
        root: root.to_owned(),
        env: ctx.env.clone(),
        chat: 7,
        persona: Some("qa".to_owned()),
        folder: None,
        confines: None,
        harness: None,
        ends_seen: None,
    };

    for vault in ["@your-keys", SERVICE] {
        // `purlis secret exec`.
        let mut heard = Heard::default();
        let request = Request {
            vault: vault.to_owned(),
            env: vec![format!("KEY={id}")],
            file: Vec::new(),
            dotenv: Vec::new(),
            stream: false,
            exec: false,
            command: vec!["/usr/bin/true".to_owned()],
            argv0: None,
        };
        let code = exec(&ctx, &request, &mut heard);
        misses.extend(refused_as_your_keys(
            &format!("secret exec {vault}"),
            if code == 0 {
                Ok(heard.0.join("\n"))
            } else {
                Err(heard.0.join("\n"))
            },
        ));
        unread("secret exec", &mut misses);

        // The `secret_ask` tool's line.
        let asks = Asks::new(root);
        let line = SecretAsk {
            chat: 7,
            key: id.clone(),
            purpose: "to call the model".to_owned(),
            vault: Some(vault.to_owned()),
            replace: None,
            program: None,
        };
        let chat = Chat {
            chat: 7,
            persona: Some("qa"),
            harness: None,
            asker: None,
        };
        misses.extend(refused_as_your_keys(
            &format!("secret_ask into {vault}"),
            ask::raised_within(&ctx, &asks, chat, &line, Duration::ZERO, &|| {})
                .map(|said| format!("{said:?}")),
        ));
        assert!(asks.open().is_empty(), "no ask was raised into {vault}");
        unread("secret_ask", &mut misses);

        // A Link answer to a Secret ask, picking the entry.
        asks.hold(
            Need {
                chat: Some(7),
                persona: "qa".to_owned(),
                vault: "qa".to_owned(),
                key: "MISSING".to_owned(),
                tool: None,
                purpose: None,
                declared: None,
                names_no_vault: false,
                reason: None,
                no_allow: None,
            },
            Duration::ZERO,
            &mut |_: &str| {},
        );
        let open = asks.open();
        let [asked] = open.as_slice() else {
            panic!("one ask is open: {open:?}")
        };
        misses.extend(refused_as_your_keys(
            &format!("a Link answer to {vault}"),
            asks.answer(
                &ctx,
                asked.id,
                Answer::Link(Target {
                    vault: vault.to_owned(),
                    key: id.clone(),
                }),
                Answerer::admitted(Admitted::LocalUi).unwrap(),
            )
            .map(|answered| format!("{answered:?}")),
        ));
        assert_eq!(
            asks.open().len(),
            1,
            "the ask is still open, nothing linked"
        );
        unread("a Link answer", &mut misses);

        // The browser's secret fill: its gate before the notice, and its read after an Allow.
        misses.extend(refused_as_your_keys(
            &format!("the fill's gate for {vault}"),
            purlis_core::secretfill::judged(&ctx, Some("qa"), vault, &id)
                .map(|warned| format!("{warned:?}")),
        ));
        misses.extend(refused_as_your_keys(
            &format!("the fill's read from {vault}"),
            brokered::fill_value(&asker, vault, &id).map(|got| format!("{}", got.is_some())),
        ));
        unread("the browser's secret fill", &mut misses);

        // A tool value sent to a runner: the desktop's gate on the run's vault, and its read.
        misses.extend(refused_as_your_keys(
            &format!("a runner's gate for {vault}"),
            brokered::authorise(&ctx, Some("qa"), vault).map(|()| "authorised".to_owned()),
        ));
        misses.extend(refused_as_your_keys(
            &format!("a runner's read from {vault}"),
            cmd::provider(&ctx, vault)
                .and_then(|v| cmd::get_value(&ctx, &v, &id))
                .map(|_| "a value".to_owned())
                .map_err(|e| e.message.to_string()),
        ));
        unread("a tool value sent to a runner", &mut misses);
    }
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}
