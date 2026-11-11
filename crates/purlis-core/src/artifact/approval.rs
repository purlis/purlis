// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each person's approval of an artifact's script** (ADR 0122 §4, #5959), on their own
//! machine: `<config>/artifact-approvals.json` (`docs/plane-format.md`, Machine tier).
//!
//! - **A page with no script asks nothing** ([`Standing::ScriptFree`]); a page with script runs
//!   it only once the person approved **this template**: the pin it was approved under and the
//!   fingerprint of every path and byte of its folder but its data ([`super::serve::Snapshot`]).
//!   New data never asks again; any other change does ([`Standing::Changed`]).
//! - **A pin whose folder, command or working folder changed is a new pin** (its id is drawn
//!   from the three, `pins.rs`), so it is untrusted until the person confirms it
//!   ([`Standing::PinChanged`]), and the bar names who changed it ([`super::pinned_by`]).
//! - **Keyed by where the workspace is and the folder**, so a fork of the workspace, or the
//!   plane cloned elsewhere, asks again; and one approval per folder, so approving a new
//!   version replaces the old.
//! - **The fingerprint is the one served, handed back, never recomputed** at approval
//!   (charter-app#123): the bytes approved are the bytes the person was shown.
//! - **Machine state, never the plane's**: a teammate's approval never reaches it, nor does a
//!   file a teammate committed. Every unreadable state of the record (missing, malformed, a
//!   link, too big) reads as no approval at all, so it can only ask again.
//! - **Where the isolation is not proven** (every platform but macOS and Linux, ADR 0122 §10,
//!   amended 2026-11-11) script never runs and is not asked about: the page renders script-free
//!   ([`Standing::NotHere`]).

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::serve::{self, Snapshot};

/// The record, in purlis's folder of the config home ([`crate::machine::dir`]).
pub const RECORD: &str = "artifact-approvals.json";

/// The most the record may be: one short entry per approved folder.
const MOST_BYTES: u64 = 1 << 20;

/// The platforms, as [`std::env::consts::OS`] names them, whose recorded run of the five
/// isolation tests of ADR 0122 §3 is all green and whose app is not recorded failing its own
/// (`tools/artifact-isolation-probe/results/`): macOS, by hand on 2026-11-09, and Linux, whose
/// app's own tests ran green once a page's script was served as JavaScript (#6129, ADR 0122
/// amended 2026-11-11). Not Windows, where test 4 failed on WebView2.
pub const PROVEN: [&str; 2] = ["macos", "linux"];

/// **Whether an artifact's script may run on this platform at all** (ADR 0122 §10): only on
/// one of [`PROVEN`]. Any other platform, Windows and every other unix among them, renders
/// script-free.
pub const SCRIPT_RUNS_HERE: bool = cfg!(any(target_os = "macos", target_os = "linux"));

/// What the gate answers in this build: [`SCRIPT_RUNS_HERE`], except that a test build
/// (`test-support`, which no shipped build and no scenario-test build has) judges every unix as
/// proven, so the record's tests, which are unix's, run on any unix they are built on.
const RUNS: bool = SCRIPT_RUNS_HERE || cfg!(all(unix, any(test, feature = "test-support")));

/// Where an artifact stands on this machine: whether its script runs, and if not, why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// No page of its template holds script: it opens without asking.
    ScriptFree,
    /// Approved here, this pin and this template: its script runs.
    Approved,
    /// Never approved here: rendered script-free, and asks.
    New,
    /// Approved here, and its template changed since: rendered script-free, and asks again.
    Changed,
    /// Approved here under another pin, before its folder or command changed: a new pin,
    /// rendered script-free, and asks again.
    PinChanged,
    /// Script, on a platform where an artifact's script never runs: rendered script-free, and
    /// asks nothing.
    NotHere,
}

impl Standing {
    /// Whether the page's script runs.
    pub fn runs(self) -> bool {
        self == Self::Approved
    }

    /// Whether the bar asks the person to approve it.
    pub fn asks(self) -> bool {
        matches!(self, Self::New | Self::Changed | Self::PinChanged)
    }

    /// Its name, as the command line and the window spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScriptFree => "script-free",
            Self::Approved => "approved",
            Self::New => "new",
            Self::Changed => "changed",
            Self::PinChanged => "pin-changed",
            Self::NotHere => "not-here",
        }
    }
}

fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(RECORD)
}

/// **The record's key for folder `folder` of the workspace at `workspace`**: where the
/// workspace really is, and the folder. A copy elsewhere is another key.
pub fn key(workspace: &Path, folder: &str) -> String {
    let real = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    serve::origin_host(&real, folder)
}

/// **The host artifact folder `folder`'s page is served from** in view `view` ("" for now,
/// "as-of" for History), served as `snapshot`: its origin, and so its web storage (ADR 0122,
/// amended 2026-11-10).
///
/// A page whose script does not run here touches no storage, and is served from its folder's
/// own host ([`serve::origin_host`]). **A page approved here is served from an origin drawn
/// from its approval**: the random `storage` drawn when it was approved, its pin and its
/// template's fingerprint. So a changed template, a changed pin, or an approval forgotten
/// (an unpin here) and given again is a new, empty origin, and never reads what an earlier
/// approval's page kept; while the same approval keeps its origin on every load, and across a
/// rename of its workspace, which carries the approval. An approval recorded before the
/// `storage` was drawn is served from its pin and fingerprint alone.
pub fn page_host(
    config_root: Option<&Path>,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    snapshot: &Snapshot,
    view: &str,
) -> String {
    standing_and_host(config_root, workspace, folder, pin_id, snapshot, view).1
}

/// **Where the artifact stands and the host its page is served from, decided by one read of
/// the record** (#6120): [`standing`] and [`page_host`] together. Read apart, an approval
/// forgotten between the two reads would leave the page's script running while it is served
/// from its folder's origin, whose storage other approvals of the folder share.
pub fn standing_and_host(
    config_root: Option<&Path>,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    snapshot: &Snapshot,
    view: &str,
) -> (Standing, String) {
    let (standing, entry) = judged(config_root, workspace, folder, pin_id, snapshot);
    let real = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let own = if view.is_empty() {
        folder.to_owned()
    } else {
        format!("{view}\u{0}{folder}")
    };
    if standing != Standing::Approved {
        return (standing, serve::origin_host(&real, &own));
    }
    let storage = entry
        .as_ref()
        .and_then(|entry| entry.get("storage"))
        .and_then(Value::as_str)
        .filter(|drawn| is_storage(drawn))
        .unwrap_or_default();
    let mut hash = Sha256::new();
    let mut frame = |name: &str, body: &[u8]| {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((body.len() as u64).to_be_bytes());
        hash.update(body);
    };
    frame("purlis-artifact-origin", b"1");
    frame("view", view.as_bytes());
    frame("storage", storage.as_bytes());
    frame("pin", pin_id.as_bytes());
    frame("fingerprint", snapshot.fingerprint().as_bytes());
    let host = hash
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    (standing, host)
}

/// A new `storage` for an approval: 128 random bits, as 32 hex digits.
fn drawn_storage() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|why| format!("purlis could not draw the page's storage ({why})"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn is_storage(text: &str) -> bool {
    text.len() == 32 && is_hex(text)
}

/// One folder's approval: the pin and the template approved.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Approved {
    pin: String,
    fingerprint: String,
}

/// Every approval the record holds, by key, each entry as written; nothing when it cannot be
/// read, and no entry that is not a pin's id and a sha256.
fn entries(config_root: &Path) -> serde_json::Map<String, Value> {
    let at = path(config_root);
    let Ok(file) = crate::contain::open_no_link(config_root, &at) else {
        return serde_json::Map::new();
    };
    let plain = file
        .metadata()
        .is_ok_and(|meta| meta.is_file() && meta.len() <= MOST_BYTES);
    let mut text = String::new();
    if !plain || file.take(MOST_BYTES).read_to_string(&mut text).is_err() {
        return serde_json::Map::new();
    }
    let Ok(Value::Object(mut doc)) = serde_json::from_str::<Value>(&text) else {
        return serde_json::Map::new();
    };
    let Some(Value::Object(mut approved)) = doc.remove("approved") else {
        return serde_json::Map::new();
    };
    approved.retain(|_, entry| approval_of(entry).is_some());
    approved
}

/// The pin and the template an entry approved, when it names both. A pin of twelve digits was
/// recorded before ids were 128 bits: it is kept, so it reads as a pin that changed
/// ([`Standing::PinChanged`]) and asks again, never as approved.
fn approval_of(entry: &Value) -> Option<Approved> {
    let pin = entry.get("pin")?.as_str()?;
    let fingerprint = entry.get("fingerprint")?.as_str()?;
    let recorded = is_pin_id(pin) || (pin.len() == OLD_ID_DIGITS && is_hex(pin));
    (recorded && is_fingerprint(fingerprint)).then(|| Approved {
        pin: pin.to_owned(),
        fingerprint: fingerprint.to_owned(),
    })
}

/// **Where the artifact pinned as `pin_id` at `folder` stands**, served as `snapshot`, on the
/// machine whose config home is `config_root` (none: no record, so it asks).
pub fn standing(
    config_root: Option<&Path>,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    snapshot: &Snapshot,
) -> Standing {
    judged(config_root, workspace, folder, pin_id, snapshot).0
}

/// Where the artifact stands, with the record's entry for it that decided so: the one read of
/// the record [`standing_and_host`] needs.
fn judged(
    config_root: Option<&Path>,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    snapshot: &Snapshot,
) -> (Standing, Option<Value>) {
    if !snapshot.holds_script() {
        return (Standing::ScriptFree, None);
    }
    if !RUNS {
        return (Standing::NotHere, None);
    }
    let Some(config_root) = config_root else {
        return (Standing::New, None);
    };
    let entry = entries(config_root).remove(&key(workspace, folder));
    let standing = match entry.as_ref().and_then(approval_of) {
        None => Standing::New,
        Some(approved) if approved.pin != pin_id || !is_pin_id(pin_id) => Standing::PinChanged,
        Some(approved) if approved.fingerprint != snapshot.fingerprint() => Standing::Changed,
        Some(_) => Standing::Approved,
    };
    (standing, entry)
}

/// **Records the person's approval** of folder `folder` of the workspace at `workspace`, pinned
/// as `pin_id`, for the template whose fingerprint they were served. Replaces any approval of
/// the folder before it. Only purlis's bar calls this: never a command a chat can run.
pub fn approve(
    config_root: &Path,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    fingerprint: &str,
) -> Result<(), String> {
    if !RUNS {
        return Err(
            "an artifact's script does not run on this platform yet (ADR 0122 \u{a7}10), so \
             there is nothing to approve: it renders without it"
                .into(),
        );
    }
    if !is_fingerprint(fingerprint) {
        return Err("an approval is recorded against a sha256, and that is not one".into());
    }
    if !is_pin_id(pin_id) {
        return Err(format!(
            "'{}' is not a pin's id",
            crate::personas::one_line(pin_id)
        ));
    }
    let real = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    // Drawn afresh on every approval: the page's origin, so its storage, is this approval's.
    let storage = drawn_storage()?;
    change(config_root, |entries| {
        entries.insert(
            key(workspace, folder),
            json!({
                "workspace": real.to_string_lossy(),
                "folder": folder,
                "pin": pin_id,
                "fingerprint": fingerprint,
                "storage": storage,
            }),
        );
    })
}

/// **Grants the page approved as `pin_id` and `fingerprint` its plane-scoped queries** (ADR 0122
/// §7, #5969): a grant of its own, beside its approval and only on top of it, which a new
/// approval of the folder (a changed template or pin) drops. Only purlis's bar calls this.
pub fn grant_plane(
    config_root: &Path,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    fingerprint: &str,
) -> Result<(), String> {
    let at = key(workspace, folder);
    let mut granted = false;
    change(config_root, |entries| {
        if let Some(Value::Object(entry)) = entries.get_mut(&at) {
            let same = entry.get("pin").and_then(Value::as_str) == Some(pin_id)
                && entry.get("fingerprint").and_then(Value::as_str) == Some(fingerprint);
            if same {
                entry.insert("plane".into(), Value::Bool(true));
                granted = true;
            }
        }
    })?;
    if granted {
        Ok(())
    } else {
        Err("approve the page's script first: plane-wide reads are granted on top of it".into())
    }
}

/// **Whether the artifact pinned as `pin_id` at `folder`, served as `snapshot`, may read the
/// whole plane**: approved here as it is, and granted plane-wide reads on top.
pub fn plane_granted(
    config_root: Option<&Path>,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    snapshot: &Snapshot,
) -> bool {
    if standing(config_root, workspace, folder, pin_id, snapshot) != Standing::Approved {
        return false;
    }
    config_root.is_some_and(|config| {
        entries(config)
            .get(&key(workspace, folder))
            .and_then(|entry| entry.get("plane"))
            .and_then(Value::as_bool)
            == Some(true)
    })
}

/// **Forgets the approval of folder `folder`** of the workspace at `workspace`: its script stops
/// running at its next load and it asks again. Whether there was one.
pub fn forget(config_root: &Path, workspace: &Path, folder: &str) -> Result<bool, String> {
    let mut had = false;
    change(config_root, |entries| {
        had = entries.remove(&key(workspace, folder)).is_some();
    })?;
    Ok(had)
}

/// **Carries every approval of the workspace that was at `from` to where it is now, `to`**: a
/// rename keeps its approvals (#5960), where a fork, a copy elsewhere, asks again. Only an
/// entry recorded at exactly `from` moves, so a workspace whose name merely begins with the
/// old one's keeps its own. How many moved; none writes nothing, so a rename finished twice,
/// or of a workspace nobody approved anything in, creates no record.
pub fn moved(config_root: &Path, from: &Path, to: &Path) -> Result<usize, String> {
    let from = from.to_string_lossy();
    let at_from = |entry: &Value| {
        entry.get("workspace").and_then(Value::as_str) == Some(&*from)
            && entry.get("folder").and_then(Value::as_str).is_some()
    };
    if !entries(config_root).values().any(at_from) {
        return Ok(0);
    }
    let real = to.canonicalize().unwrap_or_else(|_| to.to_path_buf());
    let mut count = 0;
    change(config_root, |entries| {
        let keys: Vec<String> = entries
            .iter()
            .filter(|(_, entry)| at_from(entry))
            .map(|(key, _)| key.clone())
            .collect();
        for old in keys {
            let Some(mut entry) = entries.remove(&old) else {
                continue;
            };
            let folder = entry["folder"].as_str().unwrap_or_default().to_owned();
            entry["workspace"] = Value::String(real.to_string_lossy().into_owned());
            entries.insert(key(&real, &folder), entry);
            count += 1;
        }
    })?;
    Ok(count)
}

/// Reads the record's entries, changes them, and writes the record whole, private to the
/// person. A record that cannot be read is written afresh: what it held is lost, which only
/// asks again.
fn change(
    config_root: &Path,
    edit: impl FnOnce(&mut serde_json::Map<String, Value>),
) -> Result<(), String> {
    let mut entries = entries(config_root);
    edit(&mut entries);
    let doc = json!({ "version": 1, "approved": Value::Object(entries) });
    let words = |e: std::io::Error| {
        format!(
            "purlis could not keep the approval ({})",
            crate::rewrite::os_words(&e)
        )
    };
    crate::machine::private_dir(config_root).map_err(words)?;
    let text = crate::pyjson::dumps_indent2(&doc) + "\n";
    crate::machine::write_beside(config_root, &path(config_root), text.as_bytes()).map_err(words)
}

fn is_fingerprint(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// How many digits a pin's id had before it was 128 bits (2026-11-10).
const OLD_ID_DIGITS: usize = 12;

/// Whether `text` is a pin's id: 32 lowercase hex digits ([`super::pins::id_of`]).
pub(super) fn is_pin_id(text: &str) -> bool {
    text.len() == 2 * super::pins::ID_BYTES && is_hex(text)
}

fn is_hex(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    struct Here {
        _dir: tempfile::TempDir,
        config: PathBuf,
        workspace: PathBuf,
    }

    impl Here {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let base = dir.path().canonicalize().unwrap();
            let workspace = base.join("plane/workspaces/alpha");
            std::fs::create_dir_all(workspace.join("artifacts/board/data")).unwrap();
            let here = Self {
                _dir: dir,
                config: base.join("config"),
                workspace,
            };
            here.write("index.html", "<h1>Board</h1><script src=app.js></script>");
            here.write("app.js", "draw()");
            here
        }

        fn write(&self, file: &str, text: &str) {
            std::fs::write(self.workspace.join("artifacts/board").join(file), text).unwrap();
        }

        fn snapshot(&self) -> Snapshot {
            Snapshot::read(&self.workspace.join("artifacts/board"), "board").unwrap()
        }

        fn standing(&self, pin: &str) -> Standing {
            standing(
                Some(&self.config),
                &self.workspace,
                "board",
                pin,
                &self.snapshot(),
            )
        }

        fn approve(&self, pin: &str) {
            let shown = self.snapshot().fingerprint().to_owned();
            approve(&self.config, &self.workspace, "board", pin, &shown).unwrap();
        }
    }

    const PIN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_page_without_script_opens_without_asking() {
        let here = Here::new();
        here.write(
            "index.html",
            r#"<script type="application/json">{}</script>"#,
        );
        assert_eq!(here.standing(PIN), Standing::ScriptFree);
        assert!(!Standing::ScriptFree.asks());
        assert!(!Standing::ScriptFree.runs());
    }

    #[test]
    fn plane_wide_reads_are_a_grant_of_their_own_on_top_of_the_approval() {
        let here = Here::new();
        let plane = || {
            plane_granted(
                Some(&here.config),
                &here.workspace,
                "board",
                PIN,
                &here.snapshot(),
            )
        };
        let grant = || {
            let shown = here.snapshot().fingerprint().to_owned();
            grant_plane(&here.config, &here.workspace, "board", PIN, &shown)
        };
        assert!(grant().is_err(), "nothing approved to grant on top of");
        here.approve(PIN);
        assert!(!plane(), "approving the script grants no plane-wide reads");
        grant().unwrap();
        assert!(plane());
        assert_eq!(here.standing(PIN), Standing::Approved);

        // A changed template asks again, and its new approval holds no plane grant.
        here.write("app.js", "draw(2)");
        assert!(!plane());
        here.approve(PIN);
        assert!(!plane());
    }

    #[test]
    fn a_page_with_script_asks_until_approved_and_then_runs() {
        let here = Here::new();
        assert_eq!(here.standing(PIN), Standing::New);
        assert!(Standing::New.asks());
        here.approve(PIN);
        assert_eq!(here.standing(PIN), Standing::Approved);
        assert!(Standing::Approved.runs());
    }

    #[test]
    fn new_data_never_asks_again_and_any_change_to_the_template_does() {
        let here = Here::new();
        here.approve(PIN);
        here.write("data.json", r#"{"open": 12}"#);
        std::fs::write(here.workspace.join("artifacts/board/data/week.json"), "[]").unwrap();
        assert_eq!(here.standing(PIN), Standing::Approved);

        for (file, text) in [
            ("app.js", "draw(); steal()"),
            ("style.css", "h1 { color: red }"),
        ] {
            let here = Here::new();
            here.approve(PIN);
            here.write(file, text);
            assert_eq!(here.standing(PIN), Standing::Changed, "{file}");
        }
    }

    #[test]
    fn a_pin_whose_folder_or_command_changed_is_untrusted_until_confirmed() {
        let here = Here::new();
        here.approve(PIN);
        assert_eq!(
            here.standing("ba9876543210fedcba9876543210fedc"),
            Standing::PinChanged
        );
        here.approve("ba9876543210fedcba9876543210fedc");
        assert_eq!(
            here.standing("ba9876543210fedcba9876543210fedc"),
            Standing::Approved
        );
        assert_eq!(here.standing(PIN), Standing::PinChanged);
    }

    /// An approval recorded before ids were 128 bits names the pin by twelve digits: it reads
    /// as a pin that changed, asking again, and never as approved.
    #[test]
    fn an_approval_under_an_old_short_id_asks_again_as_a_changed_pin() {
        let here = Here::new();
        let shown = here.snapshot().fingerprint().to_owned();
        crate::machine::private_dir(&here.config).unwrap();
        let entry = json!({ "version": 1, "approved": {
            key(&here.workspace, "board"): {
                "workspace": here.workspace.canonicalize().unwrap().to_string_lossy(),
                "folder": "board", "pin": &PIN[..12], "fingerprint": shown,
            }
        }});
        std::fs::write(path(&here.config), entry.to_string()).unwrap();
        assert_eq!(here.standing(PIN), Standing::PinChanged);
        assert_eq!(here.standing(&PIN[..12]), Standing::PinChanged);
        let why = approve(&here.config, &here.workspace, "board", &PIN[..12], &shown).unwrap_err();
        assert!(why.contains("not a pin's id"), "{why}");
    }

    #[test]
    fn a_copy_of_the_workspace_elsewhere_asks_again() {
        let here = Here::new();
        here.approve(PIN);
        let fork = here.workspace.with_file_name("alpha-fork");
        std::fs::create_dir_all(fork.join("artifacts")).unwrap();
        std::fs::rename(
            here.workspace.join("artifacts/board"),
            fork.join("artifacts/board"),
        )
        .unwrap();
        let snapshot = Snapshot::read(&fork.join("artifacts/board"), "board").unwrap();
        assert_eq!(
            standing(Some(&here.config), &fork, "board", PIN, &snapshot),
            Standing::New
        );
    }

    #[test]
    fn an_unreadable_record_is_no_approval_and_approving_writes_it_afresh() {
        let here = Here::new();
        here.approve(PIN);
        let record = path(&here.config);
        for bad in ["not json", "{\"approved\": []}", "[]"] {
            std::fs::write(&record, bad).unwrap();
            assert_eq!(here.standing(PIN), Standing::New, "{bad}");
        }
        std::fs::remove_file(&record).unwrap();
        let elsewhere = here.config.join("elsewhere.json");
        std::fs::write(&elsewhere, "{}").unwrap();
        std::os::unix::fs::symlink(&elsewhere, &record).unwrap();
        assert_eq!(here.standing(PIN), Standing::New, "a link");
        std::fs::remove_file(&record).unwrap();

        std::fs::write(&record, "not json").unwrap();
        here.approve(PIN);
        assert_eq!(here.standing(PIN), Standing::Approved);
    }

    /// #6120: where the page stands and the origin it is served from are decided by one read
    /// of the record, so they always agree: served from an approval's own origin exactly when
    /// its script runs, in every state the record can be in.
    #[test]
    fn standing_and_host_agree_in_every_state_of_the_record() {
        let here = Here::new();
        let both = |pin: &str| {
            standing_and_host(
                Some(&here.config),
                &here.workspace,
                "board",
                pin,
                &here.snapshot(),
                "",
            )
        };
        let folder_host = serve::origin_host(&here.workspace.canonicalize().unwrap(), "board");
        let agree = |pin: &str, want: Standing| {
            let (standing, host) = both(pin);
            assert_eq!(standing, want);
            assert_eq!(standing, here.standing(pin));
            assert_eq!(
                host,
                page_host(
                    Some(&here.config),
                    &here.workspace,
                    "board",
                    pin,
                    &here.snapshot(),
                    ""
                )
            );
            assert_eq!(
                host == folder_host,
                standing != Standing::Approved,
                "{standing:?}"
            );
        };
        agree(PIN, Standing::New);
        here.approve(PIN);
        agree(PIN, Standing::Approved);
        agree("ba9876543210fedcba9876543210fedc", Standing::PinChanged);
        here.write("app.js", "drawAgain()");
        agree(PIN, Standing::Changed);
        here.approve(PIN);
        agree(PIN, Standing::Approved);
        forget(&here.config, &here.workspace, "board").unwrap();
        agree(PIN, Standing::New);
    }

    /// The exploit of the second review of #5978: v1, approved, keeps what its queries answered
    /// in its storage; v2, approved for other reads, finds it there. An artifact's storage is its
    /// origin's, so each approval is served from an origin of its own: a changed template, a
    /// changed pin or an approval forgotten and given again never reaches the storage before it.
    #[test]
    fn each_approval_is_served_from_an_origin_of_its_own() {
        let here = Here::new();
        let host = |pin: &str, view: &str| {
            page_host(
                Some(&here.config),
                &here.workspace,
                "board",
                pin,
                &here.snapshot(),
                view,
            )
        };
        let unapproved = host(PIN, "");
        here.approve(PIN);
        let v1 = host(PIN, "");
        assert_eq!(v1, host(PIN, ""), "the same approval keeps its origin");
        assert_ne!(v1, unapproved, "approving is a new origin");
        assert_ne!(
            host(PIN, "as-of"),
            v1,
            "History's view is an origin of its own"
        );

        here.write("app.js", "draw(2)");
        assert_ne!(host(PIN, ""), v1, "a changed template, unapproved");
        here.approve(PIN);
        let v2 = host(PIN, "");
        assert_ne!(v2, v1, "a changed template, approved again");

        here.approve("ba9876543210fedcba9876543210fedc");
        assert_ne!(
            host("ba9876543210fedcba9876543210fedc", ""),
            v2,
            "another pin"
        );

        here.approve(PIN);
        let again = host(PIN, "");
        assert!(forget(&here.config, &here.workspace, "board").unwrap());
        here.approve(PIN);
        assert_ne!(
            host(PIN, ""),
            again,
            "an approval forgotten and given again"
        );
        for one in [&v1, &v2, &again] {
            assert!(
                one.len() == 32 && one.chars().all(|c| c.is_ascii_hexdigit()),
                "{one}"
            );
        }
    }

    #[test]
    fn no_config_home_is_no_approval() {
        let here = Here::new();
        let snapshot = here.snapshot();
        assert_eq!(
            standing(None, &here.workspace, "board", PIN, &snapshot),
            Standing::New
        );
    }

    #[test]
    fn an_approval_is_of_a_sha256_and_a_pin_id_only() {
        let here = Here::new();
        let shown = here.snapshot().fingerprint().to_owned();
        let why = approve(&here.config, &here.workspace, "board", PIN, "abc").unwrap_err();
        assert!(why.contains("sha256"), "{why}");
        let why = approve(&here.config, &here.workspace, "board", "nope", &shown).unwrap_err();
        assert!(why.contains("not a pin's id"), "{why}");
        assert_eq!(here.standing(PIN), Standing::New);
    }

    #[test]
    fn forgetting_an_approval_asks_again_and_keeps_the_others() {
        let here = Here::new();
        here.approve(PIN);
        std::fs::create_dir_all(here.workspace.join("artifacts/other")).unwrap();
        let other = here.workspace.join("artifacts/other");
        std::fs::write(other.join("index.html"), "<script>x()</script>").unwrap();
        let shown = Snapshot::read(&other, "other").unwrap();
        approve(
            &here.config,
            &here.workspace,
            "other",
            PIN,
            shown.fingerprint(),
        )
        .unwrap();

        assert!(forget(&here.config, &here.workspace, "board").unwrap());
        assert!(!forget(&here.config, &here.workspace, "board").unwrap());
        assert_eq!(here.standing(PIN), Standing::New);
        assert_eq!(
            standing(Some(&here.config), &here.workspace, "other", PIN, &shown),
            Standing::Approved
        );
    }

    #[test]
    fn a_renamed_workspace_keeps_its_approvals_and_no_other_workspace_s_move() {
        let here = Here::new();
        here.approve(PIN);
        // A workspace whose name begins with the renamed one's is another workspace.
        let other = here.workspace.with_file_name("alphabet");
        std::fs::create_dir_all(other.join("artifacts/board")).unwrap();
        std::fs::write(
            other.join("artifacts/board/index.html"),
            "<script>x()</script>",
        )
        .unwrap();
        let others = Snapshot::read(&other.join("artifacts/board"), "board").unwrap();
        approve(&here.config, &other, "board", PIN, others.fingerprint()).unwrap();

        let renamed = here.workspace.with_file_name("beta");
        std::fs::rename(&here.workspace, &renamed).unwrap();
        assert_eq!(moved(&here.config, &here.workspace, &renamed).unwrap(), 1);

        let snapshot = Snapshot::read(&renamed.join("artifacts/board"), "board").unwrap();
        assert_eq!(
            standing(Some(&here.config), &renamed, "board", PIN, &snapshot),
            Standing::Approved
        );
        assert_eq!(
            standing(Some(&here.config), &other, "board", PIN, &others),
            Standing::Approved
        );
        let text = std::fs::read_to_string(path(&here.config)).unwrap();
        assert!(!text.contains("\"alpha\""), "{text}");
        assert!(!text.contains("/alpha\""), "{text}");

        // Finished once, a second run has nothing to move.
        assert_eq!(moved(&here.config, &here.workspace, &renamed).unwrap(), 0);
    }

    #[test]
    fn a_rename_with_nothing_approved_writes_no_record() {
        let here = Here::new();
        let renamed = here.workspace.with_file_name("beta");
        std::fs::rename(&here.workspace, &renamed).unwrap();
        assert_eq!(moved(&here.config, &here.workspace, &renamed).unwrap(), 0);
        assert!(!path(&here.config).exists());
    }

    #[test]
    fn the_record_is_private_to_the_person() {
        use std::os::unix::fs::PermissionsExt;
        let here = Here::new();
        here.approve(PIN);
        let mode = std::fs::metadata(path(&here.config))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
        let text = std::fs::read_to_string(path(&here.config)).unwrap();
        assert!(text.contains("\"folder\": \"board\""), "{text}");
    }
}

/// The gate is evidence: script runs on exactly the platforms whose recorded run of the five
/// isolation tests is all green, and where no record says the app's own isolation tests fail
/// (ADR 0122 §3, §10, amended 2026-11-10 and 2026-11-11).
#[cfg(test)]
mod gate {
    use super::*;

    /// The five tests of ADR 0122 §3, as the probe's RESULT lines name them.
    const FIVE: [&str; 5] = ["reach", "escape", "loop", "speak", "walk"];

    /// Every recorded run on `os`, oldest first: the files are named `<os>-<yyyy-mm-dd>.txt`.
    fn records(os: &str) -> Vec<String> {
        let results = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/artifact-isolation-probe/results");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&results)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{os}-"))
            })
            .map(|e| e.path())
            .collect();
        files.sort();
        files
            .iter()
            .map(|file| std::fs::read_to_string(file).unwrap())
            .collect()
    }

    /// Whether a recorded run of the probe on `os` has all five green for the chosen rendering.
    fn probe_green(os: &str) -> bool {
        records(os).iter().any(|text| {
            FIVE.iter().all(|test| {
                text.contains(&format!("RESULT webview {test} PASS"))
                    && !text.contains(&format!("RESULT webview {test} FAIL"))
            })
        })
    }

    /// Whether the newest record on `os` of the app's own isolation tests
    /// (`app/e2e/specs/artifact-isolation.e2e.ts`) has one failing.
    fn app_red(os: &str) -> bool {
        records(os)
            .iter()
            .rev()
            .find(|text| text.contains("RESULT app "))
            .is_some_and(|text| text.contains(" FAIL"))
    }

    /// Whether `os` is proven: its probe all green, and its app not recorded red.
    fn proven(os: &str) -> bool {
        probe_green(os) && !app_red(os)
    }

    #[test]
    fn script_runs_only_where_a_recorded_run_passed_all_five_isolation_tests() {
        for os in PROVEN {
            assert!(proven(os), "{os} is not proven by its records");
        }
        assert!(!probe_green("windows"), "record windows, then prove it");
        assert!(!PROVEN.contains(&"windows"));
        assert_eq!(SCRIPT_RUNS_HERE, PROVEN.contains(&std::env::consts::OS));
    }

    /// Linux was recorded red in the app (`linux-2026-11-10.txt`) while a page's `app.js` was
    /// served as bytes under `nosniff`; a newer record of the app's own tests is green (#6129).
    #[test]
    fn linux_runs_script_once_a_newer_record_of_the_app_s_own_tests_is_green() {
        assert!(probe_green("linux"));
        assert!(!app_red("linux"));
        assert!(PROVEN.contains(&"linux"));
    }
}
