// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::secrets::registry;

/// A plane with a keyring vault `ops` and a plain-file vault `files`, both registered
/// locally. A test build keeps the keyring in `<state>/keyring-stub.json`.
fn plane() -> (tempfile::TempDir, Ctx) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    registry::add_vault(
        &ctx,
        "ops",
        "keyring",
        Default::default(),
        None,
        false,
        false,
    )
    .unwrap();
    register_file_vault(&ctx, "files", "plain-file");
    (dir, ctx)
}

/// Register `name` as a vault kept in a file under the state directory.
fn register_file_vault(ctx: &Ctx, name: &str, provider: &str) {
    let file = ctx.vaults_dir().join(format!("{name}.json"));
    let mut config = serde_json::Map::new();
    config.insert(
        "file".into(),
        serde_json::Value::String(file.to_string_lossy().into_owned()),
    );
    registry::add_vault(ctx, name, provider, config, None, false, false).unwrap();
}

#[test]
fn the_list_names_each_vault_with_its_provider_and_how_many_secrets_it_holds() {
    let (_dir, ctx) = plane();
    add(&ctx, "ops", "A", &SecretValue::from("list-value-a-3e1")).unwrap();
    add(&ctx, "ops", "B", &SecretValue::from("list-value-b-7f2")).unwrap();

    let listed = list(&ctx).unwrap();

    let shown: Vec<(&str, &str, Option<u32>, bool)> = listed
        .iter()
        .map(|v| (v.name.as_str(), v.provider.as_str(), v.count, v.health.ok))
        .collect();
    assert_eq!(
        shown,
        [
            ("files", "plain-file", Some(0), true),
            ("ops", "keyring", Some(2), true)
        ]
    );
}

/// The list marks what it cannot read as a vault, each by what is wrong with it, and goes
/// on listing the rest: an entry left over on this machine, a vault of a kind this purlis
/// does not know, and a vault this machine has not set up yet.
#[test]
fn the_list_marks_an_entry_left_over_a_kind_not_known_and_a_vault_not_set_up_here() {
    let (_dir, ctx) = plane();
    std::fs::write(
        ctx.shared_registry(),
        r#"{"vaults": {
            "newer": {"provider": "carrier-pigeon"},
            "theirs": {"provider": "plain-file"}
        }}"#,
    )
    .unwrap();
    let mut local = registry::load_local(&ctx).unwrap();
    local["vaults"]["gone"] = serde_json::json!({"config": {"account": "me@example.test"}});
    registry::save_local(&ctx, &local).unwrap();

    let listed = list(&ctx).unwrap();

    let shown: Vec<(&str, bool, &str)> = listed
        .iter()
        .map(|v| (v.name.as_str(), v.health.ok, v.health.detail.as_str()))
        .collect();
    assert_eq!(
        [shown[1], shown[2], shown[4]],
        [
            (
                "gone",
                false,
                "left over on this machine: it names no kind of vault. Remove it"
            ),
            (
                "newer",
                false,
                "a vault of a kind this purlis does not know: update purlis"
            ),
            (
                "theirs",
                false,
                "not set up on this machine: say where its file is, or remove the vault"
            ),
        ]
    );
    // The rest is listed as it always was.
    assert_eq!((shown[0].0, shown[0].1), ("files", true));
    assert_eq!((shown[3].0, shown[3].1), ("ops", true));
}

/// #1660: the list says, per vault, whether opening its tab reads a secret, from the core's
/// own knowledge of the provider. One it cannot read the entry of says it does: a put-back
/// tab of it waits for a press.
#[test]
fn the_list_says_which_vaults_read_a_secret_when_opened() {
    let (_dir, ctx) = plane();
    register_file_vault(&ctx, "refs", "reference");
    alike(&ctx, "team");
    let listed = list(&ctx).unwrap();
    let reads: Vec<(&str, bool)> = listed
        .iter()
        .map(|v| (v.name.as_str(), v.reads_on_open))
        .collect();
    assert_eq!(
        reads,
        [
            ("files", false),
            ("ops", false),
            ("refs", false),
            ("team", true)
        ]
    );
}

#[test]
fn listing_or_opening_a_keyring_vault_reads_the_index_and_never_the_store() {
    let (dir, ctx) = plane();
    add(
        &ctx,
        "ops",
        "API_TOKEN",
        &SecretValue::from("open-value-0123456789"),
    )
    .unwrap();
    // A store that cannot be read: an open that asked it anything would fail.
    std::fs::write(dir.path().join(".charter/keyring-stub.json"), "not json").unwrap();

    let opened = open(&ctx, "ops").unwrap();
    let listed = list(&ctx).unwrap();

    assert_eq!(
        (opened.name.as_str(), opened.provider.as_str()),
        ("ops", "keyring")
    );
    assert_eq!(opened.count, 1);
    assert!(opened.health.ok, "{:?}", opened.health);
    // Each entry is a key's name, size band and time, never a value: only those are printed.
    let [entry] = opened.secrets.as_slice() else {
        panic!("{} entries, not one", opened.secrets.len())
    };
    assert_eq!(entry.key, "API_TOKEN");
    assert_eq!(entry.size.as_deref(), Some("16–31 bytes"));
    assert!(
        entry.updated.as_deref().is_some_and(|t| t.ends_with('Z')),
        "updated at {:?}",
        entry.updated
    );
    let ops = listed.iter().find(|v| v.name == "ops").unwrap();
    assert_eq!((ops.count, ops.health.ok), (Some(1), true), "{ops:?}");
}

#[test]
fn a_vault_charter_does_not_track_sizes_for_lists_names_alone() {
    let (_dir, ctx) = plane();
    add(
        &ctx,
        "files",
        "DB_URL",
        &SecretValue::from("postgres://open-fixture"),
    )
    .unwrap();

    let opened = open(&ctx, "files").unwrap();

    assert_eq!(opened.count, 1);
    assert_eq!(
        opened.secrets,
        [VaultSecret {
            key: "DB_URL".into(),
            size: None,
            updated: None
        }]
    );
}

#[test]
fn opening_a_vault_the_plane_does_not_register_says_so() {
    let (_dir, ctx) = plane();
    let err = open(&ctx, "nope").unwrap_err();
    assert!(err.contains("nope"), "{err}");
}
fn keys(opened: &VaultContents) -> Vec<&str> {
    opened.secrets.iter().map(|s| s.key.as_str()).collect()
}

fn value(ctx: &Ctx, vault: &str, key: &str) -> String {
    let v = cmd::provider(ctx, vault).unwrap();
    cmd::get_value(ctx, &v, key).unwrap()
}

#[test]
fn adding_a_secret_answers_with_the_vault_as_it_now_is() {
    let (_dir, ctx) = plane();
    let opened = add(&ctx, "ops", "API_TOKEN", &SecretValue::from("add-value-5a")).unwrap();
    assert_eq!(keys(&opened), ["API_TOKEN"]);
    assert_eq!(value(&ctx, "ops", "API_TOKEN"), "add-value-5a");
}

#[test]
fn adding_a_key_the_vault_already_holds_is_refused_and_the_value_stays() {
    let (_dir, ctx) = plane();
    add(
        &ctx,
        "ops",
        "API_TOKEN",
        &SecretValue::from("first-value-11"),
    )
    .unwrap();

    let err = add(
        &ctx,
        "ops",
        "API_TOKEN",
        &SecretValue::from("second-value-22"),
    )
    .unwrap_err();

    assert!(err.contains("API_TOKEN"), "{err}");
    assert_eq!(value(&ctx, "ops", "API_TOKEN"), "first-value-11");
}

#[test]
fn an_empty_value_or_a_key_that_is_not_a_name_is_refused_before_anything_is_written() {
    let (dir, ctx) = plane();
    for (key, val) in [("EMPTY", ""), ("", "v-1"), ("  ", "v-2"), ("A\nB", "v-3")] {
        assert!(
            add(&ctx, "ops", key, &SecretValue::from(val)).is_err(),
            "{key:?}"
        );
    }
    assert!(!dir.path().join(".charter/keyring-stub.json").exists());
    assert_eq!(open(&ctx, "ops").unwrap().count, 0);
}

#[test]
fn setting_a_value_replaces_the_one_a_held_key_had() {
    let (_dir, ctx) = plane();
    add(&ctx, "files", "DB_URL", &SecretValue::from("old-value-33")).unwrap();

    let opened = set(&ctx, "files", "DB_URL", &SecretValue::from("new-value-44")).unwrap();

    assert_eq!(keys(&opened), ["DB_URL"]);
    assert_eq!(value(&ctx, "files", "DB_URL"), "new-value-44");
}

#[test]
fn setting_a_key_the_vault_does_not_hold_is_refused_rather_than_added() {
    let (_dir, ctx) = plane();
    assert!(set(&ctx, "ops", "NEVER_ADDED", &SecretValue::from("v-55")).is_err());
    assert_eq!(open(&ctx, "ops").unwrap().count, 0);
}

#[test]
fn renaming_and_deleting_answer_with_the_vault_as_it_now_is() {
    let (_dir, ctx) = plane();
    add(&ctx, "ops", "OLD", &SecretValue::from("moving-value-66")).unwrap();
    add(&ctx, "ops", "GONE", &SecretValue::from("deleted-value-77")).unwrap();

    assert_eq!(
        keys(&rename(&ctx, "ops", "OLD", "NEW").unwrap()),
        ["GONE", "NEW"]
    );
    assert_eq!(value(&ctx, "ops", "NEW"), "moving-value-66");
    assert_eq!(keys(&delete(&ctx, "ops", "GONE").unwrap()), ["NEW"]);
}
/// Every value the no-value test writes, one per provider and one per write.
const FIXTURES: [&str; 6] = [
    "fixture-keyring-5f0c9a2e71d4",
    "fixture-keyring-edited-8b3e0f",
    "fixture-plain-2c7d51e9a0b6",
    "fixture-plain-edited-4e19a7",
    "op://Fixture Vault/fixture-item/fixture-field",
    "fixture-refused-duplicate-93aa",
];

/// Whatever crossed to the window, as the window receives it.
fn wire<T: serde::Serialize>(said: &Result<T, String>) -> String {
    match said {
        Ok(it) => serde_json::to_string(it).unwrap(),
        Err(why) => serde_json::to_string(why).unwrap(),
    }
}

#[test]
fn no_list_open_refresh_write_or_copy_ever_answers_with_a_value() {
    let (dir, ctx) = plane();
    register_file_vault(&ctx, "refs", "reference");
    let v = |s: &str| SecretValue::from(s);

    let mut answers = vec![
        wire(&add(&ctx, "ops", "API_TOKEN", &v(FIXTURES[0]))),
        wire(&set(&ctx, "ops", "API_TOKEN", &v(FIXTURES[1]))),
        wire(&add(&ctx, "files", "DB_URL", &v(FIXTURES[2]))),
        wire(&set(&ctx, "files", "DB_URL", &v(FIXTURES[3]))),
        wire(&add(&ctx, "refs", "DEPLOY", &v(FIXTURES[4]))),
        // Refused writes answer too, and a refusal is the classic place a value leaks.
        wire(&add(&ctx, "ops", "API_TOKEN", &v(FIXTURES[5]))),
        wire(&set(&ctx, "files", "MISSING", &v(FIXTURES[5]))),
        wire(&set(&ctx, "nope", "X", &v(FIXTURES[5]))),
        wire(&rename(&ctx, "ops", "API_TOKEN", "TOKEN")),
        wire(&rename(&ctx, "files", "DB_URL", "DATABASE_URL")),
        wire(&rename(&ctx, "refs", "DEPLOY", "DEPLOY_REF")),
        wire(&list(&ctx)),
    ];
    for vault in ["ops", "files", "refs"] {
        answers.push(wire(&open(&ctx, vault)));
        answers.push(wire(&open(&ctx, vault))); // what `vault_refresh` answers
    }
    // A copy puts a value on the clipboard and answers the window with nothing (the
    // command's `()`); what crosses back is only a refusal. Only `reveal` answers with a value,
    // and it is not asked here.
    let pasting = Pasteboard::default().pasting();
    for (vault, key) in [
        ("ops", "TOKEN"),
        ("files", "DATABASE_URL"),
        ("refs", "DEPLOY_REF"),
        ("ops", "MISSING"),
    ] {
        answers.push(wire(&copy(&ctx, vault, key, &pasting).map(|_| ())));
    }
    answers.push(format!("{:?}", pasting.lock().unwrap()));
    answers.push(wire(&delete(&ctx, "ops", "TOKEN")));
    answers.push(wire(&delete(&ctx, "files", "DATABASE_URL")));
    answers.push(wire(&delete(&ctx, "refs", "DEPLOY_REF")));

    // The writes happened, so the absence below is not an absence of values to leak.
    assert!(
        std::fs::read_to_string(dir.path().join(".charter/vaults/files.json"))
            .is_ok_and(|t| !t.contains("fixture-plain")),
        "the plain-file vault was never written"
    );
    for answer in &answers {
        for fixture in FIXTURES {
            assert!(!answer.contains(fixture), "{fixture} in {answer}");
        }
    }
}

/// Every line the plane's trace holds, for a test run outside any session.
fn traced(ctx: &Ctx) -> Vec<serde_json::Value> {
    let file = purlis_core::trace::file(&ctx.root, purlis_core::trace::NO_SESSION);
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_reveal_answers_with_that_one_value_and_is_recorded_as_secret_get_reveal_is() {
    let (_dir, ctx) = plane();
    add(
        &ctx,
        "ops",
        "API_TOKEN",
        &SecretValue::from("reveal-value-0c41"),
    )
    .unwrap();
    add(
        &ctx,
        "ops",
        "OTHER",
        &SecretValue::from("neighbour-value-7d"),
    )
    .unwrap();

    let shown = reveal(&ctx, "ops", "API_TOKEN");

    assert_eq!(wire(&shown), "\"reveal-value-0c41\"");
    let lines = traced(&ctx);
    let [line] = lines.as_slice() else {
        panic!("{lines:?}")
    };
    assert_eq!(line["event"], "secret-reveal");
    assert_eq!(line["vault"], "ops");
    assert_eq!(line["key_names"], serde_json::json!(["API_TOKEN"]));
    assert_eq!(line["to"], "window");
    assert!(!line.to_string().contains("reveal-value"), "{line}");
}

#[test]
fn a_reveal_of_a_key_the_vault_does_not_hold_is_refused_and_records_nothing() {
    let (_dir, ctx) = plane();
    assert!(reveal(&ctx, "ops", "NEVER_ADDED").is_err());
    assert!(traced(&ctx).is_empty());
}

/// A clipboard held in memory, so no test touches the operator's. Cloned, it is the same
/// clipboard: one clone goes to the core, and the test reads and writes through the other.
#[derive(Clone, Default)]
struct Pasteboard(std::sync::Arc<std::sync::Mutex<Option<String>>>);

impl Pasteboard {
    fn holding(text: &str) -> Self {
        let board = Self::default();
        board.put(text);
        board
    }
    fn now(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
    /// What the operator copies, from anywhere.
    fn put(&self, text: &str) {
        *self.0.lock().unwrap() = Some(text.to_owned());
    }
    /// The core's view of it.
    fn pasting(&self) -> std::sync::Arc<std::sync::Mutex<Pasting>> {
        std::sync::Arc::new(std::sync::Mutex::new(Pasting::new(Box::new(self.clone()))))
    }
}

impl Clipboard for Pasteboard {
    fn text(&mut self) -> Option<String> {
        self.now()
    }
    fn set_secret(&mut self, text: &str) -> Result<(), String> {
        self.put(text);
        Ok(())
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        self.put(text);
        Ok(())
    }
    fn clear(&mut self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn a_copy_puts_the_value_on_the_clipboard_answers_with_nothing_and_is_recorded() {
    let (_dir, ctx) = plane();
    add(
        &ctx,
        "files",
        "DB_URL",
        &SecretValue::from("copied-value-2e9b"),
    )
    .unwrap();
    let board = Pasteboard::default();
    let pasting = board.pasting();

    copy(&ctx, "files", "DB_URL", &pasting).unwrap();

    assert_eq!(board.now().as_deref(), Some("copied-value-2e9b"));
    let lines = traced(&ctx);
    let [line] = lines.as_slice() else {
        panic!("{lines:?}")
    };
    assert_eq!(line["event"], "secret-reveal");
    assert_eq!(line["vault"], "files");
    assert_eq!(line["key_names"], serde_json::json!(["DB_URL"]));
    assert_eq!(line["forced"], false);
    assert_eq!(line["to"], "clipboard");
    assert!(!line.to_string().contains("copied-value"), "{line}");
    let kept = format!("{:?}", pasting.lock().unwrap());
    assert!(!kept.contains("copied-value"), "{kept}");
}

#[test]
fn a_copy_of_a_key_the_vault_does_not_hold_leaves_the_clipboard_alone() {
    let (_dir, ctx) = plane();
    let board = Pasteboard::holding("the operator's own");
    let pasting = board.pasting();

    assert!(copy(&ctx, "ops", "NEVER_ADDED", &pasting).is_err());

    assert_eq!(board.now().as_deref(), Some("the operator's own"));
    assert_eq!(clear_now(&pasting), Ok(false));
    assert_eq!(board.now().as_deref(), Some("the operator's own"));
    assert!(traced(&ctx).is_empty());
}

/// A plane whose keyring vault `ops` holds `A` and `B`, and a clipboard for it.
fn copying() -> (
    tempfile::TempDir,
    Ctx,
    Pasteboard,
    std::sync::Arc<std::sync::Mutex<Pasting>>,
) {
    let (dir, ctx) = plane();
    add(&ctx, "ops", "A", &SecretValue::from("copied-value-a-51")).unwrap();
    add(&ctx, "ops", "B", &SecretValue::from("copied-value-b-73")).unwrap();
    let board = Pasteboard::default();
    let pasting = board.pasting();
    (dir, ctx, board, pasting)
}

/// Move the paused clock on by `secs`, and nothing more: the clock is only ever moved here.
async fn after(secs: u64) {
    tokio::time::advance(std::time::Duration::from_secs(secs)).await;
}

/// Whether `clear` has finished by now, given real time for its blocking half. Busy, so the
/// paused clock never jumps ahead on its own while this waits.
async fn done(clear: &tokio::task::JoinHandle<()>) -> bool {
    for _ in 0..500 {
        if clear.is_finished() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        tokio::task::yield_now().await;
    }
    false
}

/// Copy `key` as the command does: the copy, then its clear, started — so its minute runs
/// from now.
async fn copy_as_the_window_asks(
    ctx: &Ctx,
    key: &str,
    pasting: &std::sync::Arc<std::sync::Mutex<Pasting>>,
) -> tokio::task::JoinHandle<()> {
    let n = copy(ctx, "ops", key, pasting).unwrap();
    let clear = tokio::spawn(clear_later(std::sync::Arc::clone(pasting), n));
    tokio::task::yield_now().await;
    clear
}

#[tokio::test(start_paused = true)]
async fn the_clipboard_is_cleared_a_minute_after_a_copy() {
    let (_dir, ctx, board, pasting) = copying();

    let clear = copy_as_the_window_asks(&ctx, "A", &pasting).await;
    after(59).await;
    assert!(!done(&clear).await);
    assert_eq!(board.now().as_deref(), Some("copied-value-a-51"));
    after(1).await;

    assert!(done(&clear).await);
    assert_eq!(board.now(), None);
}

#[tokio::test(start_paused = true)]
async fn what_the_operator_copied_since_is_left_on_the_clipboard() {
    let (_dir, ctx, board, pasting) = copying();

    let clear = copy_as_the_window_asks(&ctx, "A", &pasting).await;
    after(10).await;
    board.put("something the operator copied since");
    after(50).await;

    assert!(done(&clear).await);
    assert_eq!(
        board.now().as_deref(),
        Some("something the operator copied since")
    );
}

#[tokio::test(start_paused = true)]
async fn a_path_copied_after_a_secret_is_left_on_the_clipboard_by_the_secrets_clear() {
    let (_dir, ctx, board, pasting) = copying();

    let clear = copy_as_the_window_asks(&ctx, "A", &pasting).await;
    after(10).await;
    put_text(&pasting, "/work/svc/src/lib.rs").unwrap();
    after(50).await;

    assert!(done(&clear).await);
    assert_eq!(board.now().as_deref(), Some("/work/svc/src/lib.rs"));
}

#[tokio::test(start_paused = true)]
async fn a_later_copy_is_cleared_on_its_own_minute_and_not_the_earlier_ones() {
    let (_dir, ctx, board, pasting) = copying();

    let first = copy_as_the_window_asks(&ctx, "A", &pasting).await;
    after(30).await;
    let second = copy_as_the_window_asks(&ctx, "B", &pasting).await;
    after(30).await;
    assert!(done(&first).await);
    assert_eq!(board.now().as_deref(), Some("copied-value-b-73"));
    after(30).await;

    assert!(done(&second).await);
    assert_eq!(board.now(), None);
}

#[test]
fn at_exit_the_clipboard_is_cleared_only_while_it_still_holds_what_charter_copied() {
    let (_dir, ctx, board, pasting) = copying();

    copy(&ctx, "ops", "A", &pasting).unwrap();
    assert_eq!(clear_now(&pasting), Ok(true));
    assert_eq!(board.now(), None);

    copy(&ctx, "ops", "A", &pasting).unwrap();
    board.put("something the operator copied since");
    assert_eq!(clear_now(&pasting), Ok(false));
    assert_eq!(
        board.now().as_deref(),
        Some("something the operator copied since")
    );

    // Nothing charter copied is waiting, so nothing is cleared, even the same text.
    board.put("copied-value-a-51");
    assert_eq!(clear_now(&pasting), Ok(false));
    assert_eq!(board.now().as_deref(), Some("copied-value-a-51"));
}

// --- a 1Password identity, moved into the keyring (#237) ---------------------------- //

/// A fabricated service-account token.
const TOKEN: &str = "ops_fixture-window-move-6c02da";

/// A plane with a 1Password vault `team` read through `$OP_TEAM_TOKEN`, an `op` on `PATH`
/// that lists one field, and a process environment of `carrying`.
fn team(carrying: &[(&str, &str)]) -> (tempfile::TempDir, Ctx) {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    // The project is a folder of its own beside it: no provider's program is run from
    // inside a project (#1516).
    std::fs::create_dir_all(dir.path().join("plane")).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let op = bin.join("op");
    std::fs::write(
        &op,
        "#!/bin/sh\n[ \"$1 $2\" = 'item get' ] && { printf '%s' '{\"fields\":[{\"label\":\"DEPLOY\",\"value\":\"x\"}]}'; exit 0; }\nexit 1\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&op, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let mut vars = vec![("PATH", path.as_str())];
    vars.extend_from_slice(carrying);
    let ctx = Ctx::new(&dir.path().join("plane"), Env::of(&vars));
    let mut config = serde_json::Map::new();
    config.insert("op-vault".into(), serde_json::json!("Fixture"));
    config.insert(
        "env".into(),
        serde_json::json!({"OP_SERVICE_ACCOUNT_TOKEN": "OP_TEAM_TOKEN"}),
    );
    registry::add_vault(&ctx, "team", "1password", config, None, false, false).unwrap();
    (dir, ctx)
}

fn identity_of(opened: &VaultContents) -> Vec<(&str, IdentityHeld)> {
    opened
        .identity
        .iter()
        .map(|i| (i.variable.as_str(), i.held))
        .collect()
}

#[test]
fn a_vault_read_through_a_token_in_the_environment_says_so_and_names_the_variable() {
    let (_dir, ctx) = team(&[("OP_TEAM_TOKEN", TOKEN)]);

    let opened = open(&ctx, "team").unwrap();

    assert_eq!(
        identity_of(&opened),
        [("OP_TEAM_TOKEN", IdentityHeld::Environment)]
    );
    assert!(open(&ctx, "team").is_ok_and(|o| o.health.ok), "{opened:?}");
}

#[test]
fn moving_the_token_answers_with_the_vault_reading_it_from_the_keyring() {
    let (dir, ctx) = team(&[("OP_TEAM_TOKEN", TOKEN)]);

    let moved = move_identity(&ctx, "team").unwrap();

    assert_eq!(
        identity_of(&moved),
        [("OP_TEAM_TOKEN", IdentityHeld::Keyring)]
    );
    // A chat's `charter`, which has no `$OP_TEAM_TOKEN`, now finds it.
    let path = format!("{}:/usr/bin:/bin", dir.path().join("bin").display());
    let bare = Ctx::new(
        &dir.path().join("plane"),
        Env::of(&[("PATH", path.as_str())]),
    );
    let reopened = open(&bare, "team").unwrap();
    assert_eq!(
        identity_of(&reopened),
        [("OP_TEAM_TOKEN", IdentityHeld::Keyring)]
    );
    assert!(reopened.health.ok, "{reopened:?}");
}

#[test]
fn the_vaults_panel_every_window_draws_as_it_opens_never_asks_the_keychain() {
    // #1654: each plane window lists its vaults as it opens, and a 1Password vault's line
    // ran `op` with its kept token, which after an update made the Keychain ask once per
    // vault per window.
    let (dir, ctx) = team(&[("OP_TEAM_TOKEN", TOKEN)]);
    move_identity(&ctx, "team").unwrap();
    let path = format!("{}:/usr/bin:/bin", dir.path().join("bin").display());
    let opening = Ctx::new(
        &dir.path().join("plane"),
        Env::of(&[("PATH", path.as_str())]),
    );
    let stub = opening.state.join(keyring::STUB_FILE);
    let before = keyring::stub_reads(&stub);

    let listed = list(&opening).unwrap();

    assert_eq!(keyring::stub_reads(&stub) - before, 0);
    let team = listed.iter().find(|v| v.name == "team").expect("team");
    assert!(team.health.ok, "{team:?}");
    assert!(team.health.detail.contains("vault verify team"), "{team:?}");
}

#[test]
fn the_vaults_panel_never_runs_op_for_a_vault_signed_in_through_the_1password_app() {
    // #1654: `op` reads the 1Password app's data, which makes macOS ask the person, so
    // drawing the panel as a window opens never runs it.
    let (dir, ctx) = team(&[]);
    let ran = dir.path().join("op-ran");
    std::fs::write(
        dir.path().join("bin/op"),
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", ran.display()),
    )
    .unwrap();
    let mut config = serde_json::Map::new();
    config.insert("op-vault".into(), serde_json::json!("Fixture"));
    registry::add_vault(&ctx, "desk", "1password", config, None, false, false).unwrap();

    let listed = list(&ctx).unwrap();

    assert!(!ran.exists(), "op ran to draw the list");
    let desk = listed.iter().find(|v| v.name == "desk").expect("desk");
    assert!(desk.health.ok, "{desk:?}");
    assert!(desk.health.detail.contains("open the vault"), "{desk:?}");
}

#[test]
fn a_pasted_token_is_put_in_the_keyring_and_never_seen_in_the_app_environment() {
    // #271 review U3. The token comes in through the command, not the app's environment, so
    // `identity_in_app_env` stays empty: no chat can read it from charter's process.
    let (dir, ctx) = team(&[]);

    let put = put_identity(&ctx, "team", &SecretValue::from("ops_fixture-pasted-3f")).unwrap();

    assert_eq!(
        identity_of(&put),
        [("OP_TEAM_TOKEN", IdentityHeld::Keyring)]
    );
    assert!(put.identity_in_app_env.is_empty(), "{put:?}");
    let path = format!("{}:/usr/bin:/bin", dir.path().join("bin").display());
    let bare = Ctx::new(
        &dir.path().join("plane"),
        Env::of(&[("PATH", path.as_str())]),
    );
    assert_eq!(
        identity_of(&open(&bare, "team").unwrap()),
        [("OP_TEAM_TOKEN", IdentityHeld::Keyring)]
    );
}

#[test]
fn a_move_from_the_app_environment_warns_that_the_export_is_still_there() {
    // #271 review U3. The move leaves the export in the app's process, so the answer names it
    // in `identity_in_app_env` for the tab to warn about.
    let (_dir, ctx) = team(&[("OP_TEAM_TOKEN", TOKEN)]);

    let moved = move_identity(&ctx, "team").unwrap();

    assert_eq!(moved.identity_in_app_env, ["OP_TEAM_TOKEN"]);
}

#[test]
fn a_vault_whose_token_is_nowhere_says_so_and_a_move_is_refused() {
    let (_dir, ctx) = team(&[]);

    // Its keys are `op`'s to list, and `op` is not run under an identity nobody declared.
    let opened = open(&ctx, "team").unwrap();
    let refused = opened.refused.as_ref().map_or("", |r| r.why.as_str());
    assert!(
        refused.contains("$OP_TEAM_TOKEN, which is unset"),
        "{opened:?}"
    );
    let err = move_identity(&ctx, "team").unwrap_err();
    assert!(err.contains("$OP_TEAM_TOKEN"), "{err}");
}

// --- #1526: the identity is answered when the contents cannot be read ------------------ //

/// Replace the project's stand-in `op` with one that runs `script`, and counts its runs in
/// the file answered.
fn op_now(dir: &tempfile::TempDir, script: &str) -> std::path::PathBuf {
    let ran = dir.path().join("op-ran");
    std::fs::write(
        dir.path().join("bin/op"),
        format!("#!/bin/sh\necho ran >> '{}'\n{script}\n", ran.display()),
    )
    .unwrap();
    ran
}

/// A second 1Password vault `name`, read through `$OP_TEAM_TOKEN` as `team` is.
fn alike(ctx: &Ctx, name: &str) {
    let mut config = serde_json::Map::new();
    config.insert("op-vault".into(), serde_json::json!("Edge"));
    config.insert(
        "env".into(),
        serde_json::json!({"OP_SERVICE_ACCOUNT_TOKEN": "OP_TEAM_TOKEN"}),
    );
    registry::add_vault(ctx, name, "1password", config, None, false, false).unwrap();
}

#[test]
fn a_vault_whose_token_is_nowhere_still_answers_the_identity_it_declares() {
    let (dir, ctx) = team(&[]);
    // An `op` that would betray being run at all.
    let ran = op_now(&dir, "exit 1");

    let opened = open(&ctx, "team").unwrap();

    assert_eq!(
        identity_of(&opened),
        [("OP_TEAM_TOKEN", IdentityHeld::Unset)]
    );
    assert_eq!(
        opened.refused,
        Some(Unread {
            why: "vault 'team' is read through $OP_TEAM_TOKEN, which is unset. purlis will \
                  not fall back to an ambient $OP_SERVICE_ACCOUNT_TOKEN: that would read \
                  this vault under an identity it does not declare, and the failure would \
                  look like a missing secret rather than a wrong credential. Paste the \
                  token into the box below: it goes straight into the Keychain."
                .into(),
            kind: UnreadFor::NoToken,
        })
    );
    assert!(opened.secrets.is_empty() && opened.count == 0, "{opened:?}");
    assert!(!opened.health.ok, "{opened:?}");
    assert!(opened.identity_in_app_env.is_empty(), "{opened:?}");
    assert_eq!(
        (opened.name.as_str(), opened.provider.as_str()),
        ("team", "1password")
    );
    assert!(!ran.exists(), "op ran under an identity nobody declared");
}

#[test]
fn a_token_pasted_for_a_vault_whose_token_was_nowhere_answers_with_its_secrets() {
    let (_dir, ctx) = team(&[]);
    assert!(open(&ctx, "team").unwrap().refused.is_some());

    let put = put_identity(&ctx, "team", &SecretValue::from(TOKEN)).unwrap();

    assert_eq!(put.refused, None, "{put:?}");
    assert_eq!(keys(&put), ["DEPLOY"]);
    assert_eq!(
        identity_of(&put),
        [("OP_TEAM_TOKEN", IdentityHeld::Keyring)]
    );
    assert!(put.identity_in_app_env.is_empty(), "{put:?}");
}

/// #1528: a store whose replaced tokens could not all be removed from the keyring says how
/// many in its answer, for the tab to say; one that removed them all says nothing.
#[test]
fn a_store_that_left_old_tokens_behind_says_how_many() {
    let (_dir, ctx) = team(&[]);

    let left = stored(
        &ctx,
        "team",
        Ok(identity::Stored {
            sources: vec!["OP_TEAM_TOKEN".into()],
            left: 2,
        }),
    )
    .unwrap();
    let clean = stored(
        &ctx,
        "team",
        Ok(identity::Stored {
            sources: vec!["OP_TEAM_TOKEN".into()],
            left: 0,
        }),
    )
    .unwrap();

    assert_eq!(left.left, Some(2), "{left:?}");
    assert_eq!(clean.left, None, "{clean:?}");
    assert_eq!(open(&ctx, "team").unwrap().left, None);
    let refused = stored(&ctx, "team", Err(VaultError::new("the keyring is locked")));
    assert_eq!(refused.unwrap_err(), "the keyring is locked");
}

/// #1180: a vault with nothing moved under the access rule carries no note, however often
/// it is opened; the window's words for the note name the vault and the purlis command.
#[test]
fn the_moved_note_is_said_in_the_windows_words_only_when_due() {
    let (_dir, ctx) = plane();
    add(&ctx, "ops", "A", &SecretValue::from("note-fixture-value-1")).unwrap();

    assert_eq!(open(&ctx, "ops").unwrap().note, None);
    assert_eq!(
        moved_note("ops"),
        "purlis moved the secrets of vault 'ops' under its access rule in your system \
         keychain: from now on only purlis's app reads them without asking you. Any other \
         program, the purlis command included, makes the system ask you first."
    );
}

#[test]
fn a_store_gives_no_other_vault_the_token_and_names_those_that_still_have_none() {
    // D-1526-7: a token is stored per vault, from its own tab. The answer only points.
    let (_dir, ctx) = team(&[]);
    alike(&ctx, "edge");

    let put = put_identity(&ctx, "team", &SecretValue::from(TOKEN)).unwrap();

    assert_eq!(put.identity_unset_elsewhere, ["edge"]);
    let edge = open(&ctx, "edge").unwrap();
    assert_eq!(
        edge.refused.as_ref().map(|r| r.kind),
        Some(UnreadFor::NoToken)
    );
    assert_eq!(identity_of(&edge), [("OP_TEAM_TOKEN", IdentityHeld::Unset)]);
    // team's token is kept, so edge has no other vault to point at.
    assert!(edge.identity_unset_elsewhere.is_empty(), "{edge:?}");

    // Put in from its own tab, it reads, and team has nothing left to point at.
    let edge = put_identity(&ctx, "edge", &SecretValue::from(TOKEN)).unwrap();
    assert_eq!(edge.refused, None, "{edge:?}");
    assert!(
        open(&ctx, "team")
            .unwrap()
            .identity_unset_elsewhere
            .is_empty()
    );
}

/// `team`, its token kept, opened against an `op` that now runs `script`: what kept it from
/// being read, and how many times `op` was run to find out.
fn unread_with(script: &str) -> (VaultContents, usize) {
    let (dir, ctx) = team(&[]);
    put_identity(&ctx, "team", &SecretValue::from(TOKEN)).unwrap();
    let ran = op_now(&dir, script);
    let opened = open(&ctx, "team").unwrap();
    let runs = std::fs::read_to_string(ran)
        .unwrap_or_default()
        .lines()
        .count();
    (opened, runs)
}

#[test]
fn a_stored_token_that_does_not_read_the_vault_still_answers_the_identity() {
    // The tab that stored the token must be able to replace it, so this is answered with
    // why and the identity, not refused.
    let (opened, _) = unread_with("echo '[ERROR] something else' >&2; exit 1");

    assert_eq!(
        opened.refused.as_ref().map(|r| r.kind),
        Some(UnreadFor::Other),
        "{opened:?}"
    );
    assert!(opened.secrets.is_empty(), "{opened:?}");
    assert_eq!(
        identity_of(&opened),
        [("OP_TEAM_TOKEN", IdentityHeld::Keyring)]
    );
}

#[test]
fn a_failure_that_is_not_the_tokens_is_not_called_the_tokens() {
    for (stderr, kind) in [
        ("[ERROR] 429: rate-limited", UnreadFor::TryAgain),
        (
            "[ERROR] dial tcp: lookup my.1password.com: no such host",
            UnreadFor::TryAgain,
        ),
        (
            "[ERROR] You do not have permission to do this",
            UnreadFor::SignIn,
        ),
        ("[ERROR] no accounts configured", UnreadFor::SignIn),
    ] {
        let (opened, _) = unread_with(&format!("echo '{stderr}' >&2; exit 1"));
        assert_eq!(
            opened.refused.as_ref().map(|r| r.kind),
            Some(kind),
            "{stderr}: {opened:?}"
        );
    }
}

#[test]
fn a_failure_kind_is_the_one_the_core_diagnosed_never_one_a_name_spells() {
    // #1528: the refusal names the vault's identity variable, which the committed half
    // sets. A variable named with the words of a known failure must not change the kind.
    let (dir, ctx) = team(&[]);
    for (name, words) in [
        ("crafted-wait", "rate-limited this client"),
        ("crafted-sign", "refused this as unauthorised"),
    ] {
        let mut config = serde_json::Map::new();
        config.insert("op-vault".into(), serde_json::json!("Fixture"));
        config.insert(
            "env".into(),
            serde_json::json!({ "OP_SERVICE_ACCOUNT_TOKEN": words }),
        );
        registry::add_vault(&ctx, name, "1password", config, None, false, false).unwrap();
    }
    op_now(&dir, "echo '[ERROR] something else' >&2; exit 1");
    let path = ctx.env.get("PATH").unwrap();
    let carrying = Ctx::new(
        &ctx.root,
        Env::of(&[
            ("PATH", path.as_str()),
            ("rate-limited this client", TOKEN),
            ("refused this as unauthorised", TOKEN),
        ]),
    );

    for name in ["crafted-wait", "crafted-sign"] {
        let opened = open(&carrying, name).unwrap();
        let unread = opened.refused.expect("unread");
        assert!(
            unread.why.contains("rate-limited this client")
                || unread.why.contains("refused this as unauthorised"),
            "the sentence names the variable: {}",
            unread.why
        );
        assert_eq!(unread.kind, UnreadFor::Other, "{name}: {}", unread.why);
    }
}

#[test]
fn a_pinned_program_that_is_gone_is_said_to_be_the_program() {
    let (dir, ctx) = team(&[]);
    put_identity(&ctx, "team", &SecretValue::from(TOKEN)).unwrap();
    std::fs::remove_file(dir.path().join("bin/op")).unwrap();

    let opened = open(&ctx, "team").unwrap();

    let unread = opened.refused.expect("unread");
    assert_eq!(unread.kind, UnreadFor::Program);
    assert!(unread.why.contains("pinned"), "{}", unread.why);
}

#[test]
fn an_unreadable_vault_is_opened_without_asking_the_provider_for_its_health_too() {
    // The read asks for the item and, when that fails, whether the item is there: two runs.
    // The health line is the refusal, not a third run to hear it again.
    let (opened, runs) = unread_with("exit 1");
    assert_eq!(runs, 2, "{opened:?}");
    let unread = opened.refused.expect("unread");
    assert_eq!(
        opened.health,
        VaultHealth {
            ok: false,
            detail: unread.why.lines().next().unwrap().to_owned()
        }
    );
}

#[test]
fn no_answer_about_a_vault_that_cannot_be_read_carries_the_token() {
    // A provider that prints the token it was handed, on both streams, failing and as a
    // body that is not what was asked for: if any of it reached an answer, this would see.
    const ECHO: &str = "printf '%s\n' \"$OP_SERVICE_ACCOUNT_TOKEN\"; \
                        printf '%s\n' \"$OP_SERVICE_ACCOUNT_TOKEN\" >&2";
    let (dir, ctx) = team(&[]);
    let mut answers = vec![wire(&open(&ctx, "team")), wire(&list(&ctx))];
    answers.push(wire(&put_identity(&ctx, "team", &SecretValue::from(TOKEN))));
    let ran = op_now(&dir, &format!("{ECHO}; exit 1"));
    answers.push(wire(&open(&ctx, "team")));
    answers.push(wire(&list(&ctx)));
    answers.push(wire(&put_identity(&ctx, "team", &SecretValue::from(TOKEN))));
    assert!(
        ran.exists(),
        "the provider was never run, so it echoed nothing"
    );
    op_now(&dir, &format!("{ECHO}; exit 0"));
    answers.push(wire(&open(&ctx, "team")));
    answers.push(wire(&list(&ctx)));
    // A keyring that cannot be written: the refusal comes with the token in hand.
    std::fs::write(ctx.state.join(keyring::STUB_FILE), "not json").unwrap();
    answers.push(wire(&put_identity(&ctx, "team", &SecretValue::from(TOKEN))));
    answers.push(wire(&open(&ctx, "team")));
    answers.extend(traced(&ctx).iter().map(ToString::to_string));

    assert!(answers[0].contains("\"no-token\""), "{}", answers[0]);
    assert!(answers[2].contains("\"keyring\""), "{}", answers[2]);
    for answer in &answers {
        assert!(!answer.contains(TOKEN), "{answer}");
    }
}

// --- #1527: the guided set-up ------------------------------------------------------- //

/// Made-up tokens: the stand-in signs in with any that holds `works`.
const GIVEN: &str = "fixture-word-that-works-1527-app";
const WRONG: &str = "fixture-word-refused-1527-app";

/// A stand-in `op` that signs in with a token holding `works`, and otherwise fails
/// printing the token it was handed on both streams.
const SIGNS_IN: &str = "case \"$OP_SERVICE_ACCOUNT_TOKEN\" in *works*) ;; *) \
    printf '%s\n' \"$OP_SERVICE_ACCOUNT_TOKEN\"; \
    printf '%s\n' \"$OP_SERVICE_ACCOUNT_TOKEN\" >&2; exit 1;; esac\n\
    case \"$1 $2\" in\n\
    'vault list') printf '%s' '[{\"name\":\"Engineering\"}]';;\n\
    'item list') printf '%s' '[{\"title\":\"other\"}]';;\n\
    'item get') printf '%s' '{\"fields\":[{\"label\":\"DEPLOY\",\"value\":\"x\"}]}';;\n\
    esac";

/// The main window, on the project `ctx` is for.
fn id_of(ctx: &Ctx) -> At {
    At::new("main", PlaneId::for_tests(&ctx.root))
}

/// A new vault `fresh`, its items in the 1Password vault `Engineering`.
fn fresh() -> SetupNew {
    SetupNew {
        vault: "fresh".into(),
        op_vault: "Engineering".into(),
        op_item: None,
    }
}

#[test]
fn a_set_up_is_given_its_token_once_tested_and_made_and_no_answer_holds_the_token() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let (setups, plane) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));
    let mut answers = Vec::new();

    let begun = setup_begin(
        &ctx,
        &setups,
        &plane,
        Some(&SecretValue::from(format!("{GIVEN}\n").as_str())),
        None,
        None,
    );
    answers.push(wire(&begun));
    let begun = begun.unwrap();
    assert_eq!(begun.op_vaults, ["Engineering"]);
    assert_eq!(begun.listing, None);

    let tested = setup_test(
        &ctx,
        &setups,
        &plane,
        begun.setup,
        "fresh",
        Some("Engineering"),
        None,
    );
    answers.push(wire(&tested));
    let tested = tested.unwrap();
    assert_eq!(
        (tested.items, tested.item.as_str(), tested.item_there),
        (1, "charter-fresh", false)
    );
    assert_eq!(tested.failed, None);
    // Nothing is registered by a test.
    assert!(open(&ctx, "fresh").is_err());

    let made = setup_create(&ctx, &setups, &plane, begun.setup, &fresh(), Vec::new());
    answers.push(wire(&made));
    let made = made.unwrap();
    assert_eq!(made.contents.refused, None, "{made:?}");
    assert_eq!(keys(&made.contents), ["DEPLOY"]);
    assert_eq!(made.contents.identity.len(), 1);
    assert!(made.contents.identity[0].kept);
    assert_eq!(made.contents.identity[0].held, IdentityHeld::Keyring);
    // The app lets go of the token once the vault is made.
    assert!(setups.given(&plane, begun.setup).is_err());
    assert!(format!("{setups:?}").len() < 20);

    answers.push(wire(&list(&ctx)));
    answers.extend(traced(&ctx).iter().map(ToString::to_string));
    answers.push(std::fs::read_to_string(ctx.local_registry()).unwrap());
    for answer in &answers {
        assert!(!answer.contains(GIVEN), "{answer}");
    }
}

#[test]
fn a_token_that_does_not_sign_in_is_said_by_kind_and_nothing_is_made_by_the_test() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let (setups, plane) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));

    let begun = setup_begin(
        &ctx,
        &setups,
        &plane,
        Some(&SecretValue::from(WRONG)),
        None,
        None,
    );
    let tested = setup_test(
        &ctx,
        &setups,
        &plane,
        begun.as_ref().unwrap().setup,
        "fresh",
        Some("Engineering"),
        None,
    );

    // The listing failed, so the name is typed; the test says why by kind.
    assert!(begun.as_ref().unwrap().op_vaults.is_empty());
    assert_eq!(
        begun.as_ref().unwrap().listing.as_ref().map(|f| f.kind),
        Some(UnreadFor::Other)
    );
    assert_eq!(
        tested.as_ref().unwrap().failed.as_ref().map(|f| f.kind),
        Some(UnreadFor::Other)
    );
    assert!(open(&ctx, "fresh").is_err());
    assert!(!ctx.state.join(keyring::STUB_FILE).exists());
    for answer in [wire(&begun), wire(&tested)] {
        assert!(!answer.contains(WRONG), "{answer}");
    }
}

#[test]
fn a_set_up_is_held_for_its_own_project_and_until_another_is_begun_or_it_is_cancelled() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let (setups, plane) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));
    let first = setup_begin(
        &ctx,
        &setups,
        &plane,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;
    let elsewhere = At::new("main", PlaneId::for_tests(dir.path()));

    assert!(setups.given(&elsewhere, first).is_err());
    assert!(setups.given(&plane, first).is_ok());
    let second = setup_begin(&ctx, &setups, &plane, None, Some("acme.1password.eu"), None)
        .unwrap()
        .setup;
    assert_eq!(setups.given(&plane, first).unwrap_err(), SETUP_GONE);
    setups.end("main", second);
    assert!(setups.given(&plane, second).is_err());
    // A create with nothing held makes nothing.
    let refused = setup_create(&ctx, &setups, &plane, second, &fresh(), vec![]);
    assert_eq!(refused.unwrap_err(), SETUP_GONE);
    assert!(open(&ctx, "fresh").is_err());
}

#[test]
fn a_set_up_is_held_for_the_window_that_began_it_alone() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let (setups, main) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));
    let split = At::new("window-2", main.plane.clone());
    let number = setup_begin(
        &ctx,
        &setups,
        &main,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;

    // Another window of the same project cannot use it, nor let go of it.
    assert_eq!(setups.given(&split, number).unwrap_err(), SETUP_GONE);
    assert!(setup_change(&ctx, &setups, &split, number, "team", vec![]).is_err());
    setups.end("window-2", number);
    assert!(setups.given(&main, number).is_ok());
    // The window that began it going, or its page reloading, lets go of the token.
    setups.forget("window-2");
    assert!(setups.given(&main, number).is_ok());
    setups.forget("main");
    assert_eq!(setups.given(&main, number).unwrap_err(), SETUP_GONE);
    assert!(
        open(&ctx, "team").unwrap().refused.is_some(),
        "nothing was stored"
    );
}

#[test]
fn a_held_token_is_let_go_of_when_its_time_is_up_with_no_call_from_the_window() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let setups = std::sync::Arc::new(Setups::default());
    let main = id_of(&ctx);
    let number = setup_begin(
        &ctx,
        &setups,
        &main,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;

    setups.expire_after(number, std::time::Duration::from_millis(50));
    assert!(setups.given(&main, number).is_ok());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    // Asked of the holder itself, never through `given`, whose own check would hide a
    // timer that never ran.
    while setups.held().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "the timer never let go"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // A later set-up is not let go of by an earlier one's timer.
    let later = setup_begin(
        &ctx,
        &setups,
        &main,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;
    setups.expire_after(number, std::time::Duration::from_millis(1));
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(setups.given(&main, later).is_ok());
}

#[test]
fn a_set_up_is_let_go_of_after_fifteen_minutes_of_real_time_even_across_a_sleep() {
    // The monotonic clock stops while the machine sleeps; the wall clock does not.
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let (setups, main) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));
    let number = setup_begin(
        &ctx,
        &setups,
        &main,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;
    assert!(setups.given(&main, number).is_ok());
    // As after a lid closed for sixteen minutes: the wall clock moved, the other did not.
    if let Some(held) = setups.held().as_mut() {
        held.given_at -= std::time::Duration::from_secs(16 * 60);
        assert!(held.since.elapsed() < A_SETUP_IS_HELD_FOR);
    }
    assert_eq!(setups.given(&main, number).unwrap_err(), SETUP_GONE);

    // A wall clock set back before the token was given is a doubt, and keeps nothing.
    let later = setup_begin(
        &ctx,
        &setups,
        &main,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;
    if let Some(held) = setups.held().as_mut() {
        held.given_at += std::time::Duration::from_secs(60 * 60);
    }
    assert_eq!(setups.given(&main, later).unwrap_err(), SETUP_GONE);
}

#[test]
fn a_held_set_up_has_its_timer_from_the_moment_it_is_held() {
    // The timer starts inside `begin`, not after the command's answer, so a window that
    // reloads while the set-up is being begun cannot leave it held with no timer.
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let setups = std::sync::Arc::new(Setups::default());
    let main = id_of(&ctx);
    setup_begin(
        &ctx,
        &setups,
        &main,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap();
    // Two references: the one here, and the timer's.
    assert_eq!(std::sync::Arc::strong_count(&setups), 2);
}

#[test]
fn a_window_that_is_gone_or_reloaded_lets_go_of_the_set_up_it_began() {
    // What the app's `Destroyed` and page-load hooks call (`lib.rs`), on an app that holds
    // the set-ups as the real one does. Tauri's mock runtime sends no window events, so
    // the hooks themselves are wired in `lib.rs` and not driven here.
    use tauri::Manager as _;
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    let setups = std::sync::Arc::new(Setups::default());
    let held = At::new("held", id_of(&ctx).plane);
    let number = setup_begin(
        &ctx,
        &setups,
        &held,
        Some(&SecretValue::from(GIVEN)),
        None,
        None,
    )
    .unwrap()
    .setup;
    let app = tauri::test::mock_builder()
        .manage(std::sync::Arc::clone(&setups))
        .build(tauri_context!(test = true))
        .expect("a mock app");

    window_gone(&app, "window-2");
    assert!(
        app.state::<std::sync::Arc<Setups>>()
            .given(&held, number)
            .is_ok()
    );
    window_gone(&app, "held");
    assert!(setups.held().is_none(), "the window's token was kept");
    // An app that holds no set-ups at all is not troubled by a window going.
    let bare = tauri::test::mock_builder()
        .build(tauri_context!(test = true))
        .expect("a mock app");
    window_gone(&bare, "main");
}

#[test]
fn a_vault_bound_to_a_variable_is_converted_from_its_tab_and_read_at_once() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    alike(&ctx, "edge");
    let (setups, plane) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));
    assert!(open(&ctx, "team").unwrap().refused.is_some());

    let begun = setup_begin(
        &ctx,
        &setups,
        &plane,
        Some(&SecretValue::from(GIVEN)),
        None,
        Some("team"),
    )
    .unwrap();
    // The other vault bound to the same variable is offered, with what would be pinned.
    assert_eq!(begun.alike.len(), 1);
    assert_eq!(
        (
            begun.alike[0].name.as_str(),
            begun.alike[0].op_vault.as_str()
        ),
        ("edge", "Edge")
    );
    assert!(
        begun.alike[0].ticked,
        "this machine's own, with no token yet"
    );
    // Tested where the vault's items already live: no 1Password vault is named.
    let tested = setup_test(&ctx, &setups, &plane, begun.setup, "team", None, None).unwrap();
    assert_eq!(tested.failed, None);
    assert_eq!(tested.item, "charter-team");

    // Stored with no tick: `edge` is left alone.
    let done = setup_change(&ctx, &setups, &plane, begun.setup, "team", vec![]).unwrap();

    assert_eq!(done.contents.refused, None, "{done:?}");
    assert_eq!(keys(&done.contents), ["DEPLOY"]);
    assert!(done.contents.identity[0].kept);
    assert!(done.marked.is_empty() && done.skipped.is_empty());
    assert!(
        done.no_longer_read.is_empty(),
        "edge still reads the variable, so its export is still needed"
    );
    assert!(open(&ctx, "edge").unwrap().refused.is_some());
}

#[test]
fn a_tick_whose_vault_changed_since_it_was_shown_is_answered_as_skipped() {
    let (dir, ctx) = team(&[]);
    op_now(&dir, SIGNS_IN);
    alike(&ctx, "edge");
    let (setups, plane) = (std::sync::Arc::new(Setups::default()), id_of(&ctx));
    let begun = setup_begin(
        &ctx,
        &setups,
        &plane,
        Some(&SecretValue::from(GIVEN)),
        None,
        Some("team"),
    )
    .unwrap();
    let tick = SetupTick {
        name: "edge".into(),
        digest: "not-what-was-shown".into(),
    };

    let done = setup_change(&ctx, &setups, &plane, begun.setup, "team", vec![tick]).unwrap();

    assert!(done.marked.is_empty());
    assert_eq!(
        done.skipped,
        [SetupSkipped {
            name: "edge".into(),
            why: "changed".into(),
            said: None
        }]
    );
}

#[test]
fn a_1password_vault_with_no_token_of_its_own_that_cannot_be_read_still_opens() {
    // Its tab is where how it signs in is changed, so a refusal alone would hide the way out.
    let (dir, ctx) = team(&[]);
    op_now(&dir, "exit 1");
    let mut config = serde_json::Map::new();
    config.insert("op-vault".into(), serde_json::json!("Mine"));
    registry::add_vault(&ctx, "mine", "1password", config, None, false, false).unwrap();

    let opened = open(&ctx, "mine").unwrap();

    assert!(opened.identity.is_empty());
    assert!(opened.refused.is_some());
    assert!(opened.secrets.is_empty());
}

#[test]
fn a_vault_with_no_identity_lists_none_and_has_nothing_to_move() {
    let (_dir, ctx) = plane();
    assert!(open(&ctx, "ops").unwrap().identity.is_empty());
    assert!(move_identity(&ctx, "ops").is_err());
}

#[test]
fn no_answer_or_refusal_of_a_move_carries_the_token() {
    let (_dir, ctx) = team(&[("OP_TEAM_TOKEN", TOKEN)]);
    let mut answers = vec![wire(&open(&ctx, "team")), wire(&list(&ctx))];
    answers.push(wire(&move_identity(&ctx, "team")));
    answers.push(wire(&move_identity(&ctx, "nope")));
    answers.push(wire(&open(&ctx, "team")));
    answers.push(wire(&list(&ctx)));
    // A keyring that cannot be written: the refusal comes after the token was read.
    // In the project's own state folder, which is where the move wrote it.
    std::fs::write(ctx.state.join(keyring::STUB_FILE), "not json").unwrap();
    answers.push(wire(&move_identity(&ctx, "team")));
    answers.push(wire(&open(&ctx, "team")));
    answers.extend(traced(&ctx).iter().map(ToString::to_string));

    // The move happened, so the absence below is not an absence of a token to leak.
    assert!(answers[2].contains("\"keyring\""), "{}", answers[2]);
    for answer in &answers {
        assert!(!answer.contains(TOKEN), "{answer}");
    }
}

#[test]
fn a_value_from_the_window_arrives_as_a_string_and_never_prints_in_debug() {
    let value: SecretValue = serde_json::from_str("\"debug-fixture-61c2\"").unwrap();
    assert_eq!(format!("{value:?}"), "SecretValue(***)");
}

#[test]
fn a_new_vault_is_a_keyring_vault_unless_another_provider_is_asked_for() {
    let (_dir, ctx) = plane();

    let made = create(&ctx, "fresh", None, None).unwrap();
    let file = create(&ctx, "papers", Some("plain-file"), None).unwrap();

    assert_eq!(
        (made.name.as_str(), made.provider.as_str(), made.count),
        ("fresh", "keyring", 0)
    );
    assert_eq!(file.provider, "plain-file");
    let names: Vec<String> = list(&ctx).unwrap().into_iter().map(|v| v.name).collect();
    assert_eq!(names, ["files", "fresh", "ops", "papers"]);
}

#[test]
fn a_vault_name_charter_would_not_accept_or_one_already_registered_is_refused() {
    let (_dir, ctx) = plane();

    let bad = create(&ctx, "../escape", None, None).unwrap_err();
    let taken = create(&ctx, "ops", Some("plain-file"), None).unwrap_err();
    let unknown = create(&ctx, "odd", Some("carrier-pigeon"), None).unwrap_err();

    assert!(bad.contains("not a vault name"), "{bad}");
    assert!(taken.contains("already registered"), "{taken}");
    assert!(unknown.contains("unknown provider"), "{unknown}");
    assert_eq!(open(&ctx, "ops").unwrap().provider, "keyring");
}

#[test]
fn a_1password_vault_needs_the_1password_vault_it_keeps_its_items_in() {
    let (_dir, ctx) = plane();
    let err = create(&ctx, "team", Some("1password"), None).unwrap_err();
    assert!(err.contains("1Password vault"), "{err}");
    assert!(open(&ctx, "team").is_err());
}

#[test]
fn a_plain_file_vault_that_git_would_commit_is_not_written() {
    let (dir, ctx) = plane();
    purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["init", "-q"]),
    )
    .unwrap();
    let mut file = serde_json::Map::new();
    file.insert(
        "file".into(),
        serde_json::Value::String("secrets.json".into()),
    );
    registry::add_vault(&ctx, "tracked", "plain-file", file, None, false, false).unwrap();

    let err = add(&ctx, "tracked", "K", &SecretValue::from("tracked-value-1")).unwrap_err();

    assert!(err.contains("NOT gitignored"), "{err}");
    assert!(!dir.path().join("secrets.json").exists());
}

#[test]
fn deleting_a_keyring_vault_destroys_its_secrets_and_answers_with_their_names_only() {
    let (dir, ctx) = plane();
    add(&ctx, "ops", "B", &SecretValue::from("removed-value-b-7f2")).unwrap();
    add(&ctx, "ops", "A", &SecretValue::from("removed-value-a-3e1")).unwrap();

    let gone = remove(&ctx, "ops").unwrap();

    assert_eq!(
        gone,
        VaultRemoved {
            name: "ops".into(),
            provider: "keyring".into(),
            destroyed: vec!["A".into(), "B".into()],
        }
    );
    let answer = wire(&Ok::<_, String>(gone));
    assert!(!answer.contains("removed-value"), "{answer}");
    let stub =
        std::fs::read_to_string(dir.path().join(".charter/keyring-stub.json")).unwrap_or_default();
    assert!(
        !stub.contains("removed-value"),
        "the keyring still holds a value"
    );
    assert_eq!(
        list(&ctx)
            .unwrap()
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        ["files"]
    );
}

#[test]
fn deleting_a_vault_the_plane_does_not_register_is_refused() {
    let (_dir, ctx) = plane();
    let err = remove(&ctx, "nope").unwrap_err();
    assert!(err.contains("nope"), "{err}");
    assert_eq!(list(&ctx).unwrap().len(), 2);
}
