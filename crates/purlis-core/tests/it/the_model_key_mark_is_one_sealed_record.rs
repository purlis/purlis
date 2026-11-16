// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The model-key mark is one sealed record per entry, under a device key** (T1a-10, #6754;
//! R-174, R-175, R-176, R-177, R-179, R-182, R-620, R-649).
//!
//! Every test runs against the fenced stub keyring behind the recording seam (T1a-69), with
//! synthetic values only: no test here reaches the operating system's keychain or a real key.
//! A write "by another path" is [`RecordingCredentials::write_outside`], on every OS (X7).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use hmac::{KeyInit as _, Mac as _};
use purlis_core::seam::{Kind, Record, RecordingCredentials, RecordingFiles, Target};
use purlis_core::secrets::keyring::{FileStore, STUB_FILE, Secret, Store};
use purlis_core::yourkeys::mark::{
    self, Binding, ModelKeyError, Unmarked, origins_allowed, read_model_key, route_allowed,
};
use purlis_core::yourkeys::pools::NoneYet;
use purlis_core::yourkeys::{self, Checked, Product, Replaced, SERVICE, Saved, add, replace};
use ulid::Ulid;

/// A config home and the stub keyring beside it, behind a recording seam.
struct Here {
    config: tempfile::TempDir,
    record: Record,
    credentials: RecordingCredentials,
}

impl Here {
    fn new() -> Self {
        let config = tempfile::tempdir().unwrap();
        let record = Record::default();
        let credentials = RecordingCredentials::new(Box::new(stub(config.path())), record.clone());
        Self {
            config,
            record,
            credentials,
        }
    }

    fn root(&self) -> &Path {
        self.config.path()
    }

    /// Adds a key for `product` through the window's write and returns its entry id.
    fn added_for(&self, product: Product, value: &str) -> Ulid {
        match add(
            self.root(),
            &self.credentials,
            product,
            Checked::Passed,
            value,
        ) {
            Ok(Saved::Added(id)) => id,
            other => panic!("the key was not added: {other:?}"),
        }
    }

    /// Adds an Anthropic key and marks it for the Anthropic built-in's origins.
    fn marked(&self, value: &str) -> Ulid {
        let id = self.added_for(Product::anthropic(), value);
        mark::mark(self.root(), &self.credentials, id, &anthropic())
            .unwrap_or_else(|why| panic!("the key was not marked: {why}"));
        id
    }

    fn read(&self, id: Ulid) -> Result<(String, Binding), ModelKeyError> {
        read_model_key(
            self.root(),
            &RecordingFiles::new(self.record.clone()),
            &self.credentials,
            id,
        )
        .map(|(value, binding)| (Secret::into_inner(value), binding))
    }

    fn seal(&self, id: Ulid) -> PathBuf {
        mark::path(self.root(), id)
    }

    fn device_service(&self) -> String {
        let device = purlis_core::machine::known_device_id(self.root()).expect("a device id");
        format!("{}/{device}", mark::SERVICE)
    }

    /// The device key's bytes, as the stub keeps them.
    fn device_key(&self) -> Vec<u8> {
        let hex = stub(self.root())
            .get(&self.device_service(), mark::ACCOUNT)
            .unwrap()
            .expect("the device key is kept")
            .into_inner();
        (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
            .collect()
    }
}

/// The stub keyring every test build uses, straight, with no seam in front of it.
fn stub(config: &Path) -> FileStore {
    FileStore::at(config.join(STUB_FILE))
}

/// The Anthropic built-in's binding: its reference and its origins.
fn anthropic() -> Binding {
    let declaration = purlis_core::model_provider::built_ins()
        .iter()
        .find(|d| d.id == "anthropic")
        .expect("the Anthropic built-in ships");
    Binding::of(declaration)
}

fn origins(of: &[&str]) -> BTreeSet<String> {
    of.iter().map(|o| (*o).to_owned()).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unmarked(read: Result<(String, Binding), ModelKeyError>) -> Unmarked {
    match read {
        Err(ModelKeyError::Unmarked(why)) => why,
        other => panic!("the entry reads as a model key: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The record and its seal (R-174, R-175)

#[test]
fn the_mark_verifies_under_the_device_key_and_fails_when_tampered() {
    let here = Here::new();
    let id = here.marked("synthetic-tamper-value");
    assert_eq!(
        here.read(id).unwrap(),
        ("synthetic-tamper-value".to_owned(), anthropic())
    );

    let sealed = std::fs::read(here.seal(id)).unwrap();
    for at in 0..sealed.len() {
        let mut changed = sealed.clone();
        changed[at] ^= 0x01;
        std::fs::write(here.seal(id), &changed).unwrap();
        assert!(
            here.read(id).is_err(),
            "the record verified with byte {at} changed: {:?}",
            String::from_utf8_lossy(&changed)
        );
    }
    // A byte more, at the end, is a change too.
    let mut longer = sealed.clone();
    longer.push(b'\n');
    std::fs::write(here.seal(id), &longer).unwrap();
    assert!(here.read(id).is_err(), "a byte more verified");

    // The same record under another device key does not verify.
    std::fs::write(here.seal(id), &sealed).unwrap();
    assert!(here.read(id).is_ok());
    stub(here.root())
        .set(&here.device_service(), mark::ACCOUNT, &"ab".repeat(32))
        .unwrap();
    assert!(
        matches!(unmarked(here.read(id)), Unmarked::Broken(_)),
        "the record verified under another device key"
    );
    // And with no device key at all (a restored install), the mark is inert.
    stub(here.root())
        .delete(&here.device_service(), mark::ACCOUNT)
        .unwrap();
    assert_eq!(unmarked(here.read(id)), Unmarked::NoDeviceKey);
}

#[test]
fn the_seal_holds_an_hmac_of_the_value_never_the_value() {
    let here = Here::new();
    let value = "synthetic-QZx7Wv-never-in-the-seal-KpL93m";
    let id = here.marked(value);
    let sealed = std::fs::read_to_string(here.seal(id)).unwrap();

    // No form of the value: no run of eight of its characters, as written, in hex or in base64.
    let forms = [
        value.to_owned(),
        hex(value.as_bytes()),
        base64::engine::general_purpose::STANDARD.encode(value),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value),
    ];
    for form in &forms {
        let chars: Vec<char> = form.chars().collect();
        for window in chars.windows(8) {
            let run: String = window.iter().collect();
            assert!(
                !sealed.contains(&run),
                "the seal holds `{run}` of a form of the value: {sealed}"
            );
        }
    }

    // Its fingerprint is the value's HMAC under the device key, behind its own prefix.
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&here.device_key()).unwrap();
    mac.update(b"purlis/model-key-value/1\n");
    mac.update(value.as_bytes());
    let expected = hex(&mac.finalize().into_bytes());
    let body: serde_json::Value =
        serde_json::from_str(sealed.lines().next().expect("a body line")).unwrap();
    assert_eq!(body["fingerprint"], expected.as_str(), "{sealed}");
    // And the seal covers what the entry is and resolves through, and what it is bound to.
    assert_eq!(body["entry"], id.to_string().as_str());
    assert_eq!(body["kind"], "keychain");
    assert_eq!(body["declaration"]["from"], "built-in");
    assert_eq!(body["declaration"]["id"], "anthropic");
    assert_eq!(
        body["origins"],
        serde_json::to_value(&anthropic().origins).unwrap()
    );
}

#[test]
fn a_changed_declaration_breaks_the_mark() {
    let here = Here::new();
    let id = here.marked("synthetic-declaration-value");
    let sealed = std::fs::read_to_string(here.seal(id)).unwrap();
    for (from, to) in [
        ("\"id\":\"anthropic\"", "\"id\":\"anthropix\""),
        ("\"from\":\"built-in\"", "\"from\":\"project\""),
    ] {
        assert!(sealed.contains(from), "{sealed}");
        std::fs::write(here.seal(id), sealed.replacen(from, to, 1)).unwrap();
        assert!(
            matches!(unmarked(here.read(id)), Unmarked::Broken(_)),
            "{to} verified"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// How it changes (R-175, R-176)

#[test]
fn a_replace_through_the_window_keeps_the_mark() {
    let here = Here::new();
    let id = here.marked("synthetic-before-rotation");
    let before = std::fs::read(here.seal(id)).unwrap();

    assert_eq!(
        replace(
            here.root(),
            &here.credentials,
            &NoneYet,
            id,
            "synthetic-after-rotation",
            Checked::Passed,
        ),
        Ok(Replaced::Marked)
    );
    assert_eq!(
        here.read(id).unwrap(),
        ("synthetic-after-rotation".to_owned(), anthropic())
    );
    assert_ne!(
        std::fs::read(here.seal(id)).unwrap(),
        before,
        "the mark was not sealed again"
    );
}

#[test]
fn a_value_written_by_another_path_unmarks() {
    let here = Here::new();
    let id = here.marked("synthetic-window-value");
    here.record.clear();

    here.credentials
        .write_outside(SERVICE, &id.to_string(), "synthetic-outside-value")
        .unwrap();
    assert_eq!(here.record.count(Kind::OutsideWrite), 1);
    assert_eq!(here.record.count(Kind::CredentialWrite), 0);
    // The next start asks to mark again: the key was changed outside purlis (W13, T1a-27).
    assert_eq!(unmarked(here.read(id)), Unmarked::ChangedOutside);

    // A Replace after it writes the value and leaves the entry unmarked: a seal is never made
    // again over a record the value no longer matches, so the person marks it again.
    assert_eq!(
        replace(
            here.root(),
            &here.credentials,
            &NoneYet,
            id,
            "synthetic-replaced",
            Checked::Passed,
        ),
        Ok(Replaced::Unmarked)
    );
    assert_eq!(unmarked(here.read(id)), Unmarked::ChangedOutside);
}

#[test]
fn a_seal_copied_to_another_entry_fails() {
    let here = Here::new();
    // The same value under both, so only the entry the seal names tells them apart.
    let a = here.marked("synthetic-shared-value");
    let b = here.added_for(
        Product {
            from: "project".to_owned(),
            id: "anthropic".to_owned(),
        },
        "synthetic-shared-value",
    );
    assert_eq!(unmarked(here.read(b)), Unmarked::NoMark);

    std::fs::copy(here.seal(a), here.seal(b)).unwrap();
    assert!(
        matches!(unmarked(here.read(b)), Unmarked::Broken(_)),
        "entry a's seal marked entry b"
    );
    assert!(here.read(a).is_ok());
}

#[test]
fn a_line_of_a_kind_this_build_cant_read_cannot_be_marked() {
    let here = Here::new();
    here.added_for(Product::anthropic(), "synthetic-held-value");
    // A line of a kind 1a never writes: your keys hold only keychain entries, so it is unread.
    let unread = Ulid::generate();
    let at = yourkeys::index_path(here.root());
    let mut index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&at).unwrap()).unwrap();
    index["entries"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": unread.to_string(),
            "kind": "unheard-of",
            "product": { "from": "built-in", "id": "anthropic" },
            "approval": "off",
            "checked": "passed",
        }));
    std::fs::write(&at, serde_json::to_vec_pretty(&index).unwrap()).unwrap();

    let said = mark::mark(here.root(), &here.credentials, unread, &anthropic())
        .expect_err("a line this build can't read was marked");
    assert_eq!(
        said, "this purlis can't read this key's line in your keys, so it didn't mark it",
        "the refusal doesn't say the line can't be read"
    );
    assert!(!here.seal(unread).exists(), "a seal was written");
}

#[test]
fn a_value_shaped_as_a_sealed_body_does_not_fingerprint_as_its_seal() {
    let here = Here::new();
    let a = here.marked("synthetic-first-value");
    let sealed = std::fs::read_to_string(here.seal(a)).unwrap();
    let (body, seal) = sealed
        .strip_suffix('\n')
        .and_then(|lines| lines.split_once('\n'))
        .expect("a record and its seal");

    // A value that is the seal's input for entry a's record, re-sealed through the window: its
    // fingerprint must not be that record's seal, or a mark of such a value would forge it.
    let shaped = format!("purlis/model-key-mark/1\n{body}");
    assert_eq!(
        replace(here.root(), &here.credentials, a, &shaped),
        Ok(Replaced::Marked)
    );
    let resealed = std::fs::read_to_string(here.seal(a)).unwrap();
    let now: serde_json::Value =
        serde_json::from_str(resealed.lines().next().expect("a body line")).unwrap();
    assert_ne!(
        now["fingerprint"], seal,
        "a value's fingerprint is a record's seal"
    );
}

// ---------------------------------------------------------------------------------------------
// How it is used (R-177, R-179, R-182)

#[test]
fn only_a_marked_entry_resolves_as_a_model_key() {
    let here = Here::new();
    let id = here.added_for(Product::anthropic(), "synthetic-unmarked-value");
    // Another source holding a key under the same name: never tried.
    stub(here.root())
        .set(
            "purlis/ops/3f9a2c1b",
            "ANTHROPIC_API_KEY",
            "synthetic-project-vault-value",
        )
        .unwrap();
    here.record.clear();

    assert_eq!(unmarked(here.read(id)), Unmarked::NoMark);
    let ops = here.record.ops();
    assert!(
        ops.iter().all(|op| match &op.target {
            Target::Path(path) =>
                *path == yourkeys::index_path(here.root()) || *path == here.seal(id),
            Target::Entry { service, .. } =>
                service == SERVICE || service.starts_with(mark::SERVICE),
            Target::Program(_) => false,
        }),
        "the read reached past your keys and the mark: {ops:?}"
    );
    assert!(
        !ops.iter().any(|op| matches!(
            &op.target,
            Target::Entry { service, .. } if service.starts_with("purlis/ops")
        )),
        "{ops:?}"
    );
}

#[test]
fn the_origin_compare_refuses_a_differing_origin() {
    let marked = origins(&["https://api.anthropic.com"]);
    assert_eq!(
        origins_allowed(&origins(&["https://api.anthropic.com"]), &marked),
        Ok(())
    );
    for (route, differs) in [
        ("https://api.anthropic.dev", "host"),
        ("https://api.anthropic.com:8443", "port"),
        ("http://api.anthropic.com", "scheme"),
    ] {
        let why = origins_allowed(&origins(&[route]), &marked)
            .expect_err(&format!("a differing {differs} was allowed"));
        assert!(
            why.contains(route) && why.contains("https://api.anthropic.com"),
            "the reason for a differing {differs} names neither origin: {why}"
        );
    }
    // A route reaching a marked origin and one more is refused too.
    assert!(
        origins_allowed(
            &origins(&["https://api.anthropic.com", "https://elsewhere.example"]),
            &marked
        )
        .is_err()
    );
    // And a route that names no origin is never allowed.
    assert!(origins_allowed(&BTreeSet::new(), &marked).is_err());
}

#[test]
fn a_key_marked_for_a_built_in_is_refused_on_another_declaration() {
    let marked = anthropic();
    assert_eq!(route_allowed(&marked, &anthropic()), Ok(()));
    for from in ["project", "yours"] {
        let route = Binding {
            declaration: Product {
                from: from.to_owned(),
                id: "anthropic".to_owned(),
            },
            origins: marked.origins.clone(),
        };
        let why = route_allowed(&marked, &route).expect_err(&format!(
            "a key marked for the built-in went to a {from} route"
        ));
        assert!(why.contains(from), "{why}");
    }
}

// ---------------------------------------------------------------------------------------------
// The device key (R-649)

#[test]
fn the_first_mark_makes_the_device_key() {
    let here = Here::new();
    let a = here.added_for(Product::anthropic(), "synthetic-first-value");
    assert!(
        purlis_core::machine::known_device_id(here.root()).is_none_or(|device| stub(here.root())
            .get(&format!("{}/{device}", mark::SERVICE), mark::ACCOUNT)
            .unwrap()
            .is_none()),
        "a device key is there before the first mark"
    );
    here.record.clear();

    mark::mark(here.root(), &here.credentials, a, &anthropic()).unwrap();
    let service = here.device_service();
    let made = here.device_key();
    assert_eq!(made.len(), 32, "the device key is 32 bytes");
    let device_writes = |record: &Record| {
        record
            .ops()
            .into_iter()
            .filter(|op| {
                op.kind == Kind::CredentialWrite
                    && op.target
                        == Target::Entry {
                            service: service.clone(),
                            account: mark::ACCOUNT.to_owned(),
                        }
            })
            .count()
    };
    assert_eq!(device_writes(&here.record), 1);

    // A second mark reuses it.
    here.record.clear();
    let b = here.added_for(
        Product {
            from: "project".to_owned(),
            id: "acme".to_owned(),
        },
        "synthetic-second-value",
    );
    let acme = Binding {
        declaration: Product {
            from: "project".to_owned(),
            id: "acme".to_owned(),
        },
        origins: origins(&["https://router.acme.dev"]),
    };
    mark::mark(here.root(), &here.credentials, b, &acme).unwrap();
    assert_eq!(
        device_writes(&here.record),
        0,
        "a second device key was made"
    );
    assert_eq!(here.device_key(), made);
    assert!(here.read(a).is_ok() && here.read(b).is_ok());
}

// ---------------------------------------------------------------------------------------------
// One write path (R-620)

/// The files that may name the mark's writer: the mark itself, your keys' store (its *Replace*
/// seals again through it) and the window's commands for your keys (T1a-11 and T1a-13 add it).
const MAY_CALL_THE_WRITER: [&str; 3] = [
    "crates/purlis-core/src/yourkeys/mark.rs",
    "crates/purlis-core/src/yourkeys.rs",
    "app/src-tauri/src/yourkeyscmds.rs",
];

/// Whether `text` names the mark's writer: `mark::mark`, a `mark::{…}` group holding `mark`, a
/// glob of the mark's module, or an alias of it.
fn names_the_writer(text: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    for (at, _) in text.match_indices("mark::") {
        if text[..at].chars().next_back().is_some_and(word) {
            continue;
        }
        let after = &text[at + "mark::".len()..];
        if after.starts_with('*') {
            return true;
        }
        if let Some(group) = after.strip_prefix('{') {
            let group = &group[..group.find('}').unwrap_or(group.len())];
            if group.split(|c: char| !word(c)).any(|w| w == "mark") {
                return true;
            }
            continue;
        }
        let name = &after[..after.find(|c: char| !word(c)).unwrap_or(after.len())];
        if name == "mark" {
            return true;
        }
    }
    text.contains("yourkeys::mark as") || text.contains("yourkeys::{mark as")
}

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        let path = item.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n == "tests" || n == "target" || n == "node_modules")
            {
                continue;
            }
            rust_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.file_name().is_some_and(|n| n == "tests.rs")
        {
            into.push(path);
        }
    }
}

#[test]
fn there_is_one_write_path_to_the_mark() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (text, named) in [
        ("crate::yourkeys::mark::mark(root, c, id, &b)", true),
        ("use purlis_core::yourkeys::mark::{self, mark};", true),
        ("use purlis_core::yourkeys::mark::*;", true),
        ("use purlis_core::yourkeys::mark as m;", true),
        (
            "use purlis_core::yourkeys::mark::{Binding, read_model_key};",
            false,
        ),
        ("crate::yourkeys::mark::path(root, id)", false),
        ("personamark::mark(", false),
    ] {
        assert_eq!(names_the_writer(text), named, "{text}");
    }

    // In the mark's own file, one function writes a record, and only the writer calls it.
    let own = std::fs::read_to_string(root.join(MAY_CALL_THE_WRITER[0])).unwrap();
    assert!(
        own.contains("pub fn mark("),
        "the mark has no writer `mark`"
    );
    assert_eq!(
        own.matches("write_beside(").count(),
        1,
        "the mark's file writes in more than one place"
    );
    assert_eq!(
        own.matches("seal_into(").count(),
        2,
        "the record's one writing function is defined and called once"
    );
    let writer = &own[own.find("pub fn mark(").unwrap()..];
    let writer = &writer[..writer[1..]
        .find("\npub fn ")
        .map_or(writer.len(), |at| at + 1)];
    assert!(
        writer.contains("seal_into("),
        "the record is written outside `mark`"
    );
    // Your keys reaches it from Replace alone.
    let store = std::fs::read_to_string(root.join(MAY_CALL_THE_WRITER[1])).unwrap();
    assert_eq!(
        store.matches("mark::mark(").count(),
        1,
        "{}",
        MAY_CALL_THE_WRITER[1]
    );

    let mut offenders = Vec::new();
    let mut walked = 0;
    for surface in ["crates", "app/src-tauri/src"] {
        let mut files = Vec::new();
        rust_files(&root.join(surface), &mut files);
        for file in files {
            let at = file.strip_prefix(&root).unwrap_or(&file).to_path_buf();
            if MAY_CALL_THE_WRITER.iter().any(|ok| at == Path::new(ok)) {
                continue;
            }
            walked += 1;
            let text = std::fs::read_to_string(&file).unwrap();
            if names_the_writer(&text) || text.contains(mark::SERVICE) {
                offenders.push(at.display().to_string());
            }
        }
    }
    assert!(walked > 100, "the walk read only {walked} files");
    assert!(
        offenders.is_empty(),
        "only the window's command writes the mark or names its device key:\n{}",
        offenders.join("\n")
    );
}
