// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::secrets::Env;

const VALUE: &str = "kr-unit-4b9d27e1f0a3";

fn vault(name: &str) -> Vault {
    Vault {
        name: name.into(),
        provider: "keyring".into(),
        persona: None,
        config: Map::new(),
    }
}

#[test]
fn two_planes_with_a_vault_of_one_name_never_read_each_others_items() {
    // The keyring is the machine's and a vault name is only the plane's: one store, two
    // planes, one name.
    let shared = tempfile::tempdir().expect("a directory");
    let store = FileStore::at(shared.path().join("machine-keyring.json"));
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (ctx_a, ctx_b) = (
        Ctx::new(a.path(), Env::of(&[])),
        Ctx::new(b.path(), Env::of(&[])),
    );
    let ops = vault("ops");

    set_with(
        &store,
        &ctx_a,
        &ops,
        "API_TOKEN",
        VALUE,
        "2026-09-24T00:00:00Z",
    )
    .unwrap();
    set_with(
        &store,
        &ctx_b,
        &ops,
        "OTHER",
        "b-value",
        "2026-09-24T00:00:00Z",
    )
    .unwrap();

    assert_eq!(get_with(&store, &ctx_a, &ops, "API_TOKEN").unwrap(), VALUE);
    let err = get_with(&store, &ctx_b, &ops, "API_TOKEN").unwrap_err();
    assert_eq!(err.kind, crate::secrets::Kind::NotFound);
    assert_ne!(
        load_index(&ctx_a, &ops).unwrap().service,
        load_index(&ctx_b, &ops).unwrap().service
    );
}

#[test]
fn a_key_that_could_not_name_an_item_is_refused_before_the_store_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    let store = FileStore::at(dir.path().join("k.json"));
    for key in ["", "A\nB", "tab\there"] {
        assert!(set_with(&store, &ctx, &vault("ops"), key, VALUE, "t").is_err());
    }
    assert!(!dir.path().join("k.json").exists(), "the store was written");
}

#[test]
fn nothing_that_holds_a_value_prints_it_in_debug() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::at(dir.path().join("k.json"));
    store.set("charter/ops/x", "K", VALUE).unwrap();
    for shown in [
        format!("{:?}", Secret::new(VALUE.into())),
        format!("{store:?}"),
        format!("{:?}", store.get("charter/ops/x", "K").unwrap()),
        format!("{OsStore:?}"),
    ] {
        assert!(!shown.contains(VALUE), "{shown}");
    }
}

#[test]
fn each_store_and_a_value_debug_print_as_what_they_are() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.json");
    assert_eq!(format!("{:?}", Secret::new(VALUE.into())), "Secret(***)");
    assert_eq!(format!("{OsStore:?}"), "OsStore");
    assert_eq!(
        format!("{:?}", FileStore::at(path.clone())),
        format!("FileStore {{ path: {path:?} }}")
    );
}

#[test]
fn each_keyring_failure_says_the_operation_the_service_and_the_stores_reason() {
    let said = |e: ::keyring::Error| failure("write", "charter/ops/x", &e).message;
    let head = "purlis could not write 'charter/ops/x' in the system keyring: ";
    let platform = || -> Box<dyn std::error::Error + Send + Sync> {
        "User interaction is not allowed.".into()
    };
    for (e, why) in [
        (
            ::keyring::Error::BadEncoding(VALUE.as_bytes().to_vec()),
            "the stored entry is not UTF-8 text",
        ),
        (
            ::keyring::Error::NoStorageAccess(platform()),
            "User interaction is not allowed.",
        ),
        (
            ::keyring::Error::PlatformFailure(platform()),
            "User interaction is not allowed.",
        ),
        (
            ::keyring::Error::NoDefaultStore,
            "this platform has no keyring purlis can use",
        ),
        (
            ::keyring::Error::TooLong("service".into(), 255),
            "the service is longer than the store allows",
        ),
        (
            ::keyring::Error::Invalid("account".into(), "is empty".into()),
            "the account is empty",
        ),
        (::keyring::Error::NoEntry, "the store refused it"),
    ] {
        assert_eq!(said(e), format!("{head}{why}"));
    }
}

#[test]
fn a_stub_keyring_that_cannot_be_read_is_an_error_and_never_an_empty_one() {
    // A directory where the file belongs: read fails, and not with "not found".
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::at(dir.path().to_path_buf());
    let err = store.get("charter/ops/x", "K").unwrap_err();
    assert!(err.message.contains("cannot be read"), "{}", err.message);
    // A missing file is an empty keyring.
    let missing = FileStore::at(dir.path().join("none.json"));
    assert!(missing.get("charter/ops/x", "K").unwrap().is_none());
}

#[test]
fn deleting_from_the_stub_keyring_says_whether_there_was_an_entry_and_drops_only_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore::at(dir.path().join("k.json"));
    store.set("charter/ops/x", "K", VALUE).unwrap();
    store.set("charter/ops/x", "L", "other").unwrap();
    assert!(store.delete("charter/ops/x", "K").unwrap());
    assert!(!store.delete("charter/ops/x", "K").unwrap());
    assert!(store.get("charter/ops/x", "K").unwrap().is_none());
    assert_eq!(
        store
            .get("charter/ops/x", "L")
            .unwrap()
            .unwrap()
            .into_inner(),
        "other"
    );
}

#[test]
fn a_vault_service_is_the_products_for_that_vault_with_one_plain_segment_after_it() {
    // Both prefixes during the rename window (#1261), and nothing else.
    assert!(service_ok_for("purlis/ops/3f9a2c1b", "ops"));
    assert!(service_ok_for("charter/ops/3f9a2c1b", "ops"));
    for bad in [
        "charter/ops/",
        "purlis/ops/",
        "charter/ops/a/b",
        "purlis/ops/a/b",
        "charter/ops/a\nb",
        "charter/other/3f9a2c1b",
        "purlis/other/3f9a2c1b",
        "charter/identity",
        "purlis/@identity/3f9a2c1b",
        "charter/@identity/3f9a2c1b",
        "someone-else/ops/3f9a2c1b",
        // Lookalikes of the two prefixes.
        "charterx/ops/3f9a2c1b",
        "purlisx/ops/3f9a2c1b",
        "purlis-/ops/3f9a2c1b",
        "Purlis/ops/3f9a2c1b",
        "CHARTER/ops/3f9a2c1b",
        "/purlis/ops/3f9a2c1b",
        " charter/ops/3f9a2c1b",
        "purlis//ops/3f9a2c1b",
        "charter/../ops/3f9a2c1b",
        "purlis/charter/ops/3f9a2c1b",
        "edm/ops/3f9a2c1b",
    ] {
        assert!(!service_ok_for(bad, "ops"), "{bad:?}");
    }
}

#[test]
fn the_products_own_services_are_under_exactly_the_two_prefixes() {
    assert_eq!(OWN_PREFIXES, ["purlis/", "charter/"]);
    for good in [
        "purlis/ops/3f9a2c1b",
        "charter/ops/3f9a2c1b",
        "purlis/@identity/0123456789abcdef",
        "charter/@identity/0123456789abcdef",
        "charter/my.vault_1-x/3f9a2c1b",
    ] {
        assert!(own_service(good), "{good:?}");
    }
    for bad in [
        "charterx/ops/3f9a2c1b",
        "purlis-/ops/3f9a2c1b",
        "charter/../x",
        "purlis/../x",
        "charter/./x",
        "charter/ops",
        "charter/ops/",
        "charter//x",
        "charter/ops/a/b",
        "charter/ops/a\nb",
        "charter/@identity",
        "charter/@identity/",
        "charter/@other/x",
        "charter/.hidden/x",
        "Claude Code-credentials",
        "",
    ] {
        assert!(!own_service(bad), "{bad:?}");
    }
}

#[test]
fn a_secret_stored_under_the_old_charter_prefix_is_still_found_and_kept_there() {
    // An install from before the rename: its index names a `charter/` service.
    let (_dir, ctx, ops) = plane();
    write_index(
        &ctx,
        &ops,
        r#"{"service": "charter/ops/3f9a2c1b", "keys": {"API_TOKEN": {"size": "", "updated": ""}}}"#,
    );
    let stub = FileStore::at(ctx.state.join(STUB_FILE));
    stub.set("charter/ops/3f9a2c1b", "API_TOKEN", VALUE)
        .unwrap();

    assert_eq!(get(&ctx, &ops, "API_TOKEN").unwrap(), VALUE);

    // One vault keeps one service until the keychain copy moves it (RN-6): a key added
    // now sits beside its siblings, never split across two prefixes.
    set(&ctx, &ops, "OTHER", "other").unwrap();
    assert_eq!(
        load_index(&ctx, &ops).unwrap().service.as_deref(),
        Some("charter/ops/3f9a2c1b")
    );
    assert_eq!(
        stub.get("charter/ops/3f9a2c1b", "OTHER")
            .unwrap()
            .unwrap()
            .into_inner(),
        "other"
    );
}

#[test]
fn a_new_vault_writes_its_secrets_under_the_purlis_prefix() {
    let (_dir, ctx, ops) = plane();
    set(&ctx, &ops, "API_TOKEN", VALUE).unwrap();

    let service = load_index(&ctx, &ops).unwrap().service.unwrap();
    let tail = service.strip_prefix("purlis/ops/").expect(&service);
    assert!(
        tail.len() == 8 && tail.chars().all(|c| c.is_ascii_hexdigit()),
        "{service}"
    );
    let stub = FileStore::at(ctx.state.join(STUB_FILE));
    assert_eq!(
        stub.get(&service, "API_TOKEN")
            .unwrap()
            .unwrap()
            .into_inner(),
        VALUE
    );
    assert_eq!(get(&ctx, &ops, "API_TOKEN").unwrap(), VALUE);
}

/// A keyring vault `ops` in a fresh plane.
fn plane() -> (tempfile::TempDir, Ctx, Vault) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    (dir, ctx, vault("ops"))
}

fn write_index(ctx: &Ctx, v: &Vault, doc: &str) {
    let p = index_path(ctx, v);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, doc).unwrap();
}

#[test]
fn an_index_naming_another_vaults_service_is_refused_as_corrupt() {
    let (_dir, ctx, ops) = plane();
    write_index(
        &ctx,
        &ops,
        r#"{"service": "charter/other/3f9a2c1b", "keys": {}}"#,
    );
    let err = load_index(&ctx, &ops).unwrap_err();
    assert!(err.message.contains("is corrupt"), "{}", err.message);
    let (ok, detail) = health(&ctx, &ops);
    assert!(!ok);
    assert_eq!(detail, err.message);
}

#[test]
fn an_index_that_cannot_be_read_is_an_error_and_never_an_empty_vault() {
    let (_dir, ctx, ops) = plane();
    // A directory where the index belongs.
    std::fs::create_dir_all(index_path(&ctx, &ops)).unwrap();
    let err = load_index(&ctx, &ops).unwrap_err();
    assert!(err.message.contains("cannot be read"), "{}", err.message);
}

#[test]
fn a_keyring_vault_lists_each_key_with_its_size_band_and_when_it_was_written() {
    let (dir, ctx, ops) = plane();
    let store = FileStore::at(dir.path().join("k.json"));
    set_with(&store, &ctx, &ops, "B", VALUE, "2026-09-20T10:00:00Z").unwrap();
    set_with(&store, &ctx, &ops, "A", "x", "2026-09-24T00:00:00Z").unwrap();
    assert_eq!(
        listed(&ctx, &ops).unwrap(),
        [
            Listed {
                key: "A".into(),
                size: "1–15 bytes".into(),
                updated: "2026-09-24T00:00:00Z".into(),
            },
            Listed {
                key: "B".into(),
                size: "16–31 bytes".into(),
                updated: "2026-09-20T10:00:00Z".into(),
            },
        ]
    );
}

#[test]
fn a_keys_age_is_whole_days_since_it_was_written_and_none_when_the_time_is_unreadable() {
    let (dir, ctx, ops) = plane();
    let store = FileStore::at(dir.path().join("k.json"));
    set_with(&store, &ctx, &ops, "OLD", VALUE, "2026-09-20T23:59:59Z").unwrap();
    set_with(&store, &ctx, &ops, "TODAY", VALUE, "2026-09-24T00:00:00Z").unwrap();
    set_with(&store, &ctx, &ops, "UNDATED", VALUE, "not a time").unwrap();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 24).unwrap();
    assert_eq!(
        ages(&ctx, &ops, today).unwrap(),
        [
            ("OLD".to_string(), Some(4)),
            ("TODAY".to_string(), Some(0)),
            ("UNDATED".to_string(), None),
        ]
    );
}

#[test]
fn a_keyring_vaults_health_counts_its_keys_and_names_its_service_from_the_index() {
    let (dir, ctx, ops) = plane();
    assert_eq!(
        health(&ctx, &ops),
        (true, "no secrets yet in the system keyring".to_string())
    );
    let store = FileStore::at(dir.path().join("k.json"));
    set_with(&store, &ctx, &ops, "A", VALUE, "t").unwrap();
    set_with(&store, &ctx, &ops, "B", VALUE, "t").unwrap();
    let service = load_index(&ctx, &ops).unwrap().service.unwrap();
    assert_eq!(
        health(&ctx, &ops),
        (
            true,
            format!("2 secrets in the system keyring, service '{service}'")
        )
    );
}

#[test]
fn a_write_through_the_builds_own_store_is_stamped_with_the_time_it_was_made() {
    let (_dir, ctx, ops) = plane();
    let before = chrono::Utc::now().timestamp();
    set(&ctx, &ops, "A", VALUE).unwrap();
    let after = chrono::Utc::now().timestamp();
    let updated = &listed(&ctx, &ops).unwrap()[0].updated;
    assert_eq!(updated.len(), "2026-09-24T11:32:17Z".len(), "{updated}");
    assert!(updated.ends_with('Z'), "{updated}");
    let at = chrono::DateTime::parse_from_rfc3339(updated)
        .unwrap()
        .timestamp();
    assert!((before..=after).contains(&at), "{updated}");
}

#[test]
fn a_keyring_failure_never_quotes_the_bytes_the_store_returned() {
    let e = ::keyring::Error::BadEncoding(VALUE.as_bytes().to_vec());
    let said = failure("read", "charter/ops/x", &e).message;
    assert!(!said.contains(VALUE), "{said}");
    assert!(said.contains("charter/ops/x"), "{said}");
}

/// A store that refuses every write.
struct Refusing;

impl Store for Refusing {
    fn get(&self, _: &str, _: &str) -> Result<Option<Secret>, VaultError> {
        Ok(None)
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<Held, VaultError> {
        Err(VaultError::new("refused"))
    }
    fn delete(&self, _: &str, _: &str) -> Result<bool, VaultError> {
        Err(VaultError::new("refused"))
    }
}

#[test]
fn a_vaults_service_is_kept_before_its_first_item_is_written() {
    // Were it kept only after, an item written by a store that then failed the index
    // write would sit in the keyring under a service nothing records.
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    let ops = vault("ops");

    assert!(set_with(&Refusing, &ctx, &ops, "API_TOKEN", VALUE, "t").is_err());

    let index = load_index(&ctx, &ops).unwrap();
    assert!(index.service.is_some_and(|s| s.starts_with("purlis/ops/")));
    assert!(index.keys.is_empty(), "a key the store refused was listed");
}

/// A store that holds items to charter's app, as macOS's does, over the stub: it counts the
/// writes it was asked for, and answers each with `answer`.
struct Holding {
    under: FileStore,
    writes: std::sync::atomic::AtomicUsize,
    answer: Held,
}

impl Holding {
    fn at(dir: &std::path::Path, answer: Held) -> Self {
        Self {
            under: FileStore::at(dir.join("k.json")),
            writes: std::sync::atomic::AtomicUsize::new(0),
            answer,
        }
    }

    fn writes(&self) -> usize {
        self.writes.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Store for Holding {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        self.under.get(service, account)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.writes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.under.set(service, account, value)?;
        Ok(self.answer)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        self.under.delete(service, account)
    }
    fn holds(&self) -> bool {
        true
    }
}

#[test]
fn a_write_to_a_store_that_holds_it_marks_the_key_held() {
    let (dir, ctx, ops) = plane();
    let store = Holding::at(dir.path(), Held::ToTheApp);
    set_with(&store, &ctx, &ops, "A", VALUE, "t").unwrap();
    assert!(load_index(&ctx, &ops).unwrap().keys["A"].held);
    // Nothing was moved, so there is nothing to say.
    assert_eq!(take_note(&ctx, &ops), None);
}

#[test]
fn an_item_written_before_the_store_held_it_is_held_on_its_first_read_and_still_resolves() {
    let (dir, ctx, ops) = plane();
    // Written before: the same items, by a store that set no rule.
    set_with(
        &FileStore::at(dir.path().join("k.json")),
        &ctx,
        &ops,
        "A",
        VALUE,
        "t",
    )
    .unwrap();
    assert!(!load_index(&ctx, &ops).unwrap().keys["A"].held);

    let store = Holding::at(dir.path(), Held::ToTheApp);
    assert_eq!(get_with(&store, &ctx, &ops, "A").unwrap(), VALUE);
    assert_eq!(store.writes(), 1, "the item was not written again");
    assert!(load_index(&ctx, &ops).unwrap().keys["A"].held);

    assert_eq!(get_with(&store, &ctx, &ops, "A").unwrap(), VALUE);
    assert_eq!(store.writes(), 1, "a held item was written again");
}

#[test]
fn a_vault_whose_items_were_moved_says_so_once() {
    let (dir, ctx, ops) = plane();
    let before = FileStore::at(dir.path().join("k.json"));
    set_with(&before, &ctx, &ops, "A", VALUE, "t").unwrap();
    set_with(&before, &ctx, &ops, "B", VALUE, "t").unwrap();
    let store = Holding::at(dir.path(), Held::ToTheApp);

    get_with(&store, &ctx, &ops, "A").unwrap();
    assert_eq!(
        take_note(&ctx, &ops).as_deref(),
        Some(
            "purlis moved the secrets of vault 'ops' under its access rule in the system \
             keyring: from now on only purlis's app reads them without asking you. Any \
             other program, this command included, makes the system ask you first."
        )
    );
    assert_eq!(take_note(&ctx, &ops), None);
    // A later key moved is not said again.
    get_with(&store, &ctx, &ops, "B").unwrap();
    assert_eq!(take_note(&ctx, &ops), None);
}

#[test]
fn an_item_the_store_could_not_hold_yet_is_tried_again_at_the_next_read() {
    let (dir, ctx, ops) = plane();
    set_with(
        &FileStore::at(dir.path().join("k.json")),
        &ctx,
        &ops,
        "A",
        VALUE,
        "t",
    )
    .unwrap();
    let store = Holding::at(dir.path(), Held::NotYet);

    assert_eq!(get_with(&store, &ctx, &ops, "A").unwrap(), VALUE);
    assert_eq!(get_with(&store, &ctx, &ops, "A").unwrap(), VALUE);
    assert_eq!(store.writes(), 2);
    assert!(!load_index(&ctx, &ops).unwrap().keys["A"].held);
    assert_eq!(take_note(&ctx, &ops), None);
}

#[test]
fn a_store_that_keeps_no_rule_never_writes_on_a_read() {
    let (dir, ctx, ops) = plane();
    let store = FileStore::at(dir.path().join("k.json"));
    set_with(&store, &ctx, &ops, "A", VALUE, "t").unwrap();
    let before = std::fs::read(index_path(&ctx, &ops)).unwrap();
    assert_eq!(get_with(&store, &ctx, &ops, "A").unwrap(), VALUE);
    assert_eq!(std::fs::read(index_path(&ctx, &ops)).unwrap(), before);
}

/// Another charter's `secret set` of `key` to `value` at `at`, started on its own thread now:
/// a writer outside the one under test, which finishes when the vault's lock lets it.
fn another_set(
    root: &std::path::Path,
    key: &str,
    value: &'static str,
    at: &'static str,
) -> std::thread::JoinHandle<Result<(), VaultError>> {
    let (root, key) = (root.to_path_buf(), key.to_owned());
    std::thread::spawn(move || {
        set_with(
            &FileStore::at(root.join("k.json")),
            &Ctx::new(&root, Env::of(&[])),
            &vault("ops"),
            &key,
            value,
            at,
        )
    })
}

/// A holding store over which another charter's `secret set` of the same key starts right
/// after each read.
struct Racing {
    holding: Holding,
    root: PathBuf,
    set: std::sync::Mutex<Option<std::thread::JoinHandle<Result<(), VaultError>>>>,
}

impl Store for Racing {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        let read = self.holding.get(service, account)?;
        *self.set.lock().unwrap() = Some(another_set(
            &self.root,
            account,
            "newer-value-9d2",
            "2026-10-04T12:00:00Z",
        ));
        Ok(read)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.holding.set(service, account, value)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        self.holding.delete(service, account)
    }
    fn holds(&self) -> bool {
        true
    }
}

#[test]
fn a_move_never_writes_the_value_it_read_over_a_newer_one() {
    let (dir, ctx, ops) = plane();
    set_with(
        &FileStore::at(dir.path().join("k.json")),
        &ctx,
        &ops,
        "A",
        VALUE,
        "t",
    )
    .unwrap();
    let racing = Racing {
        holding: Holding::at(dir.path(), Held::ToTheApp),
        root: dir.path().to_path_buf(),
        set: std::sync::Mutex::new(None),
    };

    assert_eq!(get_with(&racing, &ctx, &ops, "A").unwrap(), VALUE);
    // The set that started during the read lands once the move is done (#1180).
    let set = racing.set.lock().unwrap().take().expect("a set started");
    set.join().unwrap().unwrap();

    let after = FileStore::at(dir.path().join("k.json"));
    assert_eq!(
        get_with(&after, &ctx, &ops, "A").unwrap(),
        "newer-value-9d2",
        "the move wrote over the newer value"
    );
    let entry = &load_index(&ctx, &ops).unwrap().keys["A"];
    assert_eq!(entry.updated, "2026-10-04T12:00:00Z");
    assert!(!entry.held, "the newer value was marked held by the move");
}

#[test]
fn a_write_the_store_could_not_hold_is_told_apart_from_one_it_held() {
    let (dir, ctx, ops) = plane();
    let held = Holding::at(dir.path(), Held::ToTheApp);
    set_with(&held, &ctx, &ops, "A", VALUE, "t").unwrap();
    assert!(!left_unheld_in(&held, &ctx, &ops, "A"));

    let not_yet = Holding::at(dir.path(), Held::NotYet);
    set_with(&not_yet, &ctx, &ops, "B", VALUE, "t").unwrap();
    assert!(left_unheld_in(&not_yet, &ctx, &ops, "B"));

    // A store with no rule never says it.
    let stub = FileStore::at(dir.path().join("k.json"));
    set_with(&stub, &ctx, &ops, "C", VALUE, "t").unwrap();
    assert!(!left_unheld_in(&stub, &ctx, &ops, "C"));
    assert_eq!(
        unheld_sentence("C"),
        "purlis could not hold 'C' to its app, so the program that wrote it reads it \
         without asking you. purlis tries again the next time the purlis command reads it."
    );
}

/// A holding store where another charter's `secret set` of the same key, which could not be
/// held, starts while the move is being made.
struct SetDuringTheMove {
    under: FileStore,
    root: PathBuf,
    set: std::sync::Mutex<Option<std::thread::JoinHandle<Result<(), VaultError>>>>,
}

impl Store for SetDuringTheMove {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        self.under.get(service, account)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        *self.set.lock().unwrap() = Some(another_set(
            &self.root,
            account,
            "newer-value-4c8",
            "2026-10-04T13:00:00Z",
        ));
        self.under.set(service, account, value)?;
        Ok(Held::ToTheApp)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        self.under.delete(service, account)
    }
    fn holds(&self) -> bool {
        true
    }
}

#[test]
fn a_newer_write_started_during_a_move_lands_after_it_and_is_never_marked_held_by_it() {
    let (dir, ctx, ops) = plane();
    set_with(
        &FileStore::at(dir.path().join("k.json")),
        &ctx,
        &ops,
        "A",
        VALUE,
        "t",
    )
    .unwrap();
    let store = SetDuringTheMove {
        under: FileStore::at(dir.path().join("k.json")),
        root: dir.path().to_path_buf(),
        set: std::sync::Mutex::new(None),
    };

    assert_eq!(get_with(&store, &ctx, &ops, "A").unwrap(), VALUE);
    let set = store.set.lock().unwrap().take().expect("a set started");
    set.join().unwrap().unwrap();

    let entry = &load_index(&ctx, &ops).unwrap().keys["A"];
    assert_eq!(entry.updated, "2026-10-04T13:00:00Z");
    assert!(!entry.held, "the newer value was marked held by the move");
    assert_eq!(
        get_with(&FileStore::at(dir.path().join("k.json")), &ctx, &ops, "A").unwrap(),
        "newer-value-4c8"
    );
}
