// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Keyring items held to charter's app (ruling V90a, ADR 0047 as amended).
//!
//! **The rule is the Keychain's own default.** An item made with no access object of its own
//! lets the program that created it read it without asking, by that program's code signature,
//! and makes every other program ask the person or fail (ADR 0047, measured). So charter holds
//! an item to its app by having **the app's own binary create it**:
//!
//! - in the app ([`this_process_is_the_app`]), the item is deleted and made again here;
//! - anywhere else (the `charter` command), the app's binary beside this one is started again
//!   with [`HOLD_ARG`], is handed the item on its standard input, makes it and exits
//!   ([`serve_if_asked`]). It writes and never reads, so starting it gives nothing back.
//!
//! The item is deleted first because a replaced item keeps the access it had, and one the
//! person once let another program read ("Always Allow") would keep letting it. **A program may
//! delete only an item it owns**, so an item the `charter` command made before (where no app sat
//! beside it, or before this rule) is moved by the command: it deletes its own item and the app
//! makes it again, the next time the command reads or writes it. The app cannot move such an
//! item itself. If the app cannot make it, the command writes it back, and says the item is not
//! held yet ([`Held::NotYet`]); the next read through the command tries again. The termination
//! signals a person or a closing terminal sends are blocked for that window ([`Shield`]) and
//! delivered after it, so only a kill that cannot be blocked loses the value there.
//!
//! **What it rests on.** Signed builds (#606) make the app's identity survive an update. Until
//! then an ad-hoc signed app is trusted by its exact build, so the first read after an update
//! asks the person once, and neither the new app nor the new command owns an item an older build
//! made. Where no app sits beside the command (a command-line install, a dev build of the command
//! alone), the command writes the item itself, and it stays the command's until a command with
//! the app beside it reads it. The command is found through any link to it: the app is looked
//! for beside the real file, never beside a link on `PATH`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use super::VaultError;
use super::keyring::{Held, OWN_PREFIXES, own_service};
use crate::tui::Markup;

/// The argument that starts the app's binary as the one-item writer.
pub const HOLD_ARG: &str = "charter-hold-a-keyring-item";

/// The name of the app's binary, beside the `charter` command in the app's bundle and in a
/// build's target directory.
pub const APP_BINARY: &str = "charter-app";

/// The most bytes the writer reads from its standard input: an item, never more.
const MOST_INPUT: u64 = 1024 * 1024;

/// How long the command waits for the writer before it writes the item itself.
const PATIENCE: Duration = Duration::from_secs(20);

/// The writer's exit code when the item there is owned by another program.
const OWNED: i32 = 3;

static THE_APP: AtomicBool = AtomicBool::new(false);

/// Called once by the app as it starts: this process is charter's app, so it holds an item by
/// writing it itself.
pub fn this_process_is_the_app() {
    THE_APP.store(true, Ordering::SeqCst);
}

/// Whether this process is charter's app: an item it writes is held to it, so it reads that
/// item back without the person being asked.
pub fn is_the_app() -> bool {
    THE_APP.load(Ordering::SeqCst)
}

/// One item to write, as the writer is handed it.
#[derive(Debug, PartialEq, Eq)]
pub struct Item {
    pub service: String,
    pub account: String,
    pub value: String,
}

/// The item `input` names, or why the writer refuses it: only an item under charter's own
/// services, so the app's binary is never a deputy for writing another program's item.
pub fn asked(input: &[u8]) -> Result<Item, String> {
    let doc: Value =
        serde_json::from_slice(input).map_err(|_| "the item is not a JSON object".to_owned())?;
    let field = |name: &str| {
        doc.get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("the item has no {name}"))
    };
    let item = Item {
        service: field("service")?,
        account: field("account")?,
        value: field("value")?,
    };
    // Under `purlis/` or, during the rename window, `charter/`, and shaped as a vault's or an
    // identity's item: nothing else (#1261).
    if !own_service(&item.service)
        || item.account.is_empty()
        || item.account.chars().any(char::is_control)
    {
        return Err(format!(
            "purlis writes only its own items, under '{}…' or '{}…'",
            OWN_PREFIXES[0], OWN_PREFIXES[1]
        ));
    }
    Ok(item)
}

/// Started again as the writer: write the one item on standard input and exit. `None` when
/// this process was not started as one. Called first thing by the app's `main`.
pub fn serve_if_asked() -> Option<i32> {
    if std::env::args_os().nth(1)? != HOLD_ARG {
        return None;
    }
    let mut input = Vec::new();
    if std::io::stdin()
        .take(MOST_INPUT)
        .read_to_end(&mut input)
        .is_err()
    {
        return Some(1);
    }
    let item = match asked(&input) {
        Ok(item) => item,
        Err(why) => {
            let _ = writeln!(std::io::stderr(), "{why}");
            return Some(1);
        }
    };
    Some(match write_quietly(&item) {
        Ok(()) => 0,
        Err(Refused::Owned) => OWNED,
        Err(Refused::Failed(e)) => {
            let _ = writeln!(std::io::stderr(), "{}", e.message);
            1
        }
    })
}

/// Why this process could not make an item.
enum Refused {
    /// The item there is another program's, and only its owner may delete it.
    Owned,
    Failed(VaultError),
}

/// The writer's write, with the Keychain's dialogs off: one it may not make fails, and the
/// command that started it decides what to do next.
#[cfg(target_os = "macos")]
fn write_quietly(item: &Item) -> Result<(), Refused> {
    let _quiet = security_framework::os::macos::keychain::SecKeychain::disable_user_interaction()
        .map_err(|e| Refused::Failed(VaultError::new(e.to_string())))?;
    make_here(&item.service, &item.account, &item.value)
}

#[cfg(not(target_os = "macos"))]
fn write_quietly(_item: &Item) -> Result<(), Refused> {
    Err(Refused::Failed(VaultError::new(
        "only macOS holds a keyring item to purlis's app",
    )))
}

/// `errSecInvalidOwnerEdit`: the Keychain refused a delete because another program owns the
/// item (measured on a delete by a program the item does not trust).
#[cfg(target_os = "macos")]
const NOT_THE_OWNER: i32 = -25244;

/// The two calls an item is made fresh with: a delete, and an add that never updates.
trait Items {
    /// Delete the item; `Ok` when there was none.
    fn delete(&self, service: &str, account: &str) -> Result<(), Refused>;
    /// Make a NEW item: [`Refused::Owned`] when one is there already, which is then left
    /// exactly as it is (D-RN6-9).
    fn add(&self, service: &str, account: &str, value: &str) -> Result<(), Refused>;
}

/// The item deleted and made again, add-only: one put there between the delete and the add,
/// by anyone, is never updated in place and reported as this program's own.
///
/// **The value is never only in this process's memory meanwhile** (#1309): a spare item holds
/// it first ([`spare_of`]), and goes only once the item is made again. A crash, a kill or a
/// refused add in between leaves the value at rest in the spare, never lost. Where the spare
/// cannot be made, the item is left as it was and nothing is deleted.
fn remake(items: &dyn Items, service: &str, account: &str, value: &str) -> Result<(), Refused> {
    let spare = spare_of(service);
    // One left by an earlier write that did not finish is this program's, and holds an older
    // value than this one.
    items
        .delete(&spare, account)
        .and_then(|()| items.add(&spare, account, value))
        .map_err(|e| no_spare(service, e))?;
    if let Err(e) = items.delete(service, account) {
        let _ = items.delete(&spare, account);
        return Err(e);
    }
    // A refused add keeps the spare: the value stays at rest there.
    items.add(service, account, value)?;
    let _ = items.delete(&spare, account);
    Ok(())
}

/// The spare item [`remake`] keeps `service`'s value in while it makes the item again: one
/// more segment under it, which no vault's or identity's item has, and which the app's writer
/// refuses ([`asked`]), so nothing asks it to write or replace a spare.
fn spare_of(service: &str) -> String {
    format!("{service}/{SPARE}")
}

/// The last segment of [`spare_of`]'s spare item.
const SPARE: &str = "spare";

/// The spare item a command keeps the value of its own item in, while that item is deleted for
/// the app to make again ([`set`]): another than the app's own spare, which the app's writer
/// makes and deletes meanwhile.
fn commands_spare_of(service: &str) -> String {
    format!("{service}/{SPARE}-of-the-command")
}

/// Every spare item a write of `service` may leave behind when it is cut short ([`spare_of`],
/// [`commands_spare_of`]): a delete of the item deletes these with it, so a value the person
/// deleted is not kept at rest in a spare.
pub(super) fn spares_of(service: &str) -> [String; 2] {
    [spare_of(service), commands_spare_of(service)]
}

/// Why nothing was written: the spare could not be made, so the item was left as it was.
fn no_spare(service: &str, e: Refused) -> Refused {
    let why = match e {
        Refused::Owned => Markup::from("another program's item is where it keeps it"),
        Refused::Failed(e) => e.message,
    };
    Refused::Failed(VaultError::new(
        Markup::from(format!(
            "purlis could not keep a spare copy of '{service}' while it writes it again ("
        ))
        .then(why)
        .then(Markup::from("), so it left the item as it was")),
    ))
}

/// The operating system's store, as [`remake`] asks it.
struct OsItems;

impl Items for OsItems {
    fn delete(&self, service: &str, account: &str) -> Result<(), Refused> {
        let entry = ::keyring::Entry::new(service, account)
            .map_err(|e| Refused::Failed(super::keyring::failure("reach", service, &e)))?;
        match entry.delete_credential() {
            Ok(()) | Err(::keyring::Error::NoEntry) => Ok(()),
            Err(e) if owned_by_another(&e) => Err(Refused::Owned),
            Err(e) => Err(Refused::Failed(super::keyring::failure(
                "write", service, &e,
            ))),
        }
    }

    #[cfg(target_os = "macos")]
    fn add(&self, service: &str, account: &str, value: &str) -> Result<(), Refused> {
        use security_framework::os::macos::keychain::{SecKeychain, SecPreferencesDomain};
        let failed = |e: security_framework::base::Error| {
            let said = format!(
                "purlis could not write '{service}' in {}: {e}",
                super::keyring::STORE_NAME
            );
            // Its dialogs off, and this write would have asked (#1306).
            Refused::Failed(if e.code() == INTERACTION_NOT_ALLOWED {
                VaultError::would_ask(said)
            } else {
                VaultError::new(said)
            })
        };
        // The user's keychain, the one the `keyring` crate reads and writes.
        let added = SecKeychain::default_for_domain(SecPreferencesDomain::User)
            .and_then(|keychain| keychain.add_generic_password(service, account, value.as_bytes()));
        match added {
            Ok(()) => Ok(()),
            Err(e) if e.code() == DUPLICATE => Err(Refused::Owned),
            Err(e) => Err(failed(e)),
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn add(&self, service: &str, account: &str, value: &str) -> Result<(), Refused> {
        ::keyring::Entry::new(service, account)
            .and_then(|entry| entry.set_password(value))
            .map_err(|e| Refused::Failed(super::keyring::failure("write", service, &e)))
    }
}

/// `errSecDuplicateItem`: an add found the item there already.
#[cfg(target_os = "macos")]
const DUPLICATE: i32 = -25299;

/// `errSecInteractionNotAllowed`: the write would have asked the person, with dialogs off.
#[cfg(target_os = "macos")]
const INTERACTION_NOT_ALLOWED: i32 = -25308;

/// The item deleted and made again by this process, so it is this program's alone.
fn make_here(service: &str, account: &str, value: &str) -> Result<(), Refused> {
    remake(&OsItems, service, account, value)
}

/// Whether the store refused a delete because another program owns the item.
#[cfg(target_os = "macos")]
fn owned_by_another(e: &::keyring::Error) -> bool {
    match e {
        ::keyring::Error::NoStorageAccess(inner) | ::keyring::Error::PlatformFailure(inner) => {
            inner
                .downcast_ref::<security_framework::base::Error>()
                .is_some_and(|e| e.code() == NOT_THE_OWNER)
        }
        _ => false,
    }
}

#[cfg(not(target_os = "macos"))]
fn owned_by_another(_e: &::keyring::Error) -> bool {
    false
}

/// The termination signals a person or a closing terminal sends (SIGINT, SIGHUP, SIGTERM),
/// blocked on this thread while the command's own item is gone between its delete and its new
/// write, and unblocked after: one that arrived meanwhile is then delivered under whatever
/// handling the process has, which by default ends it. Blocked, never handled, so the process's
/// handling of them is the same afterwards. Only the command puts it up: the app keeps its own
/// handling, and the writer runs in a process group of its own, where a terminal's signals do
/// not reach it.
pub struct Shield {
    #[cfg(unix)]
    before: Option<nix::sys::signal::SigSet>,
}

impl Shield {
    /// Block the signals from now.
    pub fn up() -> Self {
        #[cfg(unix)]
        {
            use nix::sys::signal::{SigSet, SigmaskHow, Signal, pthread_sigmask};
            let mut held = SigSet::empty();
            for sig in [Signal::SIGINT, Signal::SIGHUP, Signal::SIGTERM] {
                held.add(sig);
            }
            let mut before = SigSet::empty();
            let blocked =
                pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&held), Some(&mut before)).is_ok();
            Self {
                before: blocked.then_some(before),
            }
        }
        #[cfg(not(unix))]
        {
            Self {}
        }
    }
}

impl Drop for Shield {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(before) = self.before.take() {
            let _ = nix::sys::signal::pthread_sigmask(
                nix::sys::signal::SigmaskHow::SIG_SETMASK,
                Some(&before),
                None,
            );
        }
    }
}

/// The app's binary beside this one, where there is one and this process is not it.
fn the_app() -> Option<PathBuf> {
    app_beside(&std::env::current_exe().ok()?)
}

/// Whether the app's binary sits beside this one, so an item this process writes is made by the
/// app ([`set`]).
pub fn app_is_beside() -> bool {
    the_app().is_some()
}

/// The app's binary beside the program at `exe`, as the kernel names it, where there is one
/// and `exe` is not it. A link on `PATH` (`/usr/local/bin/charter`, made by the app's Install
/// on PATH) is followed to the bundle, and nothing beside the link is ever looked at.
pub fn app_beside(exe: &std::path::Path) -> Option<PathBuf> {
    let here = exe.canonicalize().ok()?;
    let app = here.parent()?.join(APP_BINARY);
    (app != here && app.is_file()).then_some(app)
}

/// Why an item is written: a new value, or the same value moved under the rule (ruling V90d),
/// which is pointless to write again in place when it cannot be held, or a copy the keychain
/// copy makes (RN-6), which must never be written into another program's item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Write,
    Move,
    /// A fresh item or nothing: an item another program owns is an error, and nothing is
    /// written to it. One this command owns is still deleted and made again by the app.
    Copy,
}

/// What a [`Why::Copy`] write says when another program's item is in the way.
fn not_ours(service: &str) -> VaultError {
    VaultError::new(format!(
        "another program's item is under '{service}', so purlis wrote nothing there"
    ))
}

/// The item made fresh by this process itself, never through the app's writer, and never
/// written into an item another program owns (`Store::make_own`, #1306): held to the app when
/// this is the app, and this command's own anywhere else ([`Held::NotYet`]). A fenced build
/// refuses, as [`set`] does.
pub fn make_own(service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
    if crate::fence::FENCED {
        return Err(fenced(service));
    }
    match make_here(service, account, value) {
        Ok(()) if THE_APP.load(Ordering::SeqCst) => Ok(Held::ToTheApp),
        Ok(()) => Ok(Held::NotYet),
        Err(Refused::Owned) => Err(not_ours(service)),
        Err(Refused::Failed(e)) => Err(e),
    }
}

/// What a fenced build says instead of reaching the operating system's store.
fn fenced(service: &str) -> VaultError {
    VaultError::new(format!(
        "a test build never writes '{service}' in {}; its store is the stub",
        super::keyring::STORE_NAME
    ))
}

/// Where the item cannot be held: write the value in place, leave it for a move, and for a copy
/// write nothing, never into an item this process did not just make.
fn in_place(service: &str, account: &str, value: &str, why: Why) -> Result<Held, VaultError> {
    match why {
        Why::Write => super::keyring::set_here(service, account, value).map(|()| Held::NotYet),
        Why::Move => Ok(Held::NotYet),
        Why::Copy => Err(not_ours(service)),
    }
}

/// Write the item held to charter's app, from whichever charter this is. See the module header.
/// A fenced build (every test build) refuses before anything is started or written: its store
/// is the stub, and no test may start a real writer (`crate::fence`).
pub fn set(service: &str, account: &str, value: &str, why: Why) -> Result<Held, VaultError> {
    if crate::fence::FENCED {
        return Err(fenced(service));
    }
    let in_place = || in_place(service, account, value, why);
    if THE_APP.load(Ordering::SeqCst) {
        return match make_here(service, account, value) {
            Ok(()) => Ok(Held::ToTheApp),
            // The command's own item, which only the command can delete.
            Err(Refused::Owned) => in_place(),
            Err(Refused::Failed(e)) => Err(e),
        };
    }
    let Some(app) = the_app() else {
        return in_place();
    };
    let item = Item {
        service: service.to_owned(),
        account: account.to_owned(),
        value: value.to_owned(),
    };
    match through(&app, &item) {
        Ok(()) => return Ok(Held::ToTheApp),
        Err(code) if code != OWNED => return in_place(),
        Err(_) => {}
    }
    // This command's own item, made before: it deletes it, the app makes it again, and if the
    // app cannot, the command writes it back. No caught signal ends it in between, and a spare
    // of the command's own holds the value meanwhile, so a kill there leaves it at rest (#1309).
    // With no spare, the item is left as it was.
    let _shield = Shield::up();
    let spare = commands_spare_of(service);
    let kept = OsItems
        .delete(&spare, account)
        .and_then(|()| OsItems.add(&spare, account, value));
    if kept.is_err() {
        return in_place();
    }
    let drop_spare = || {
        let _ = OsItems.delete(&spare, account);
    };
    if let Ok(entry) = ::keyring::Entry::new(service, account) {
        match entry.delete_credential() {
            Ok(()) | Err(::keyring::Error::NoEntry) => {}
            // Not the command's own either: another program's, which a copy never writes into.
            Err(_) => {
                drop_spare();
                return in_place();
            }
        }
    }
    let written = match through(&app, &item) {
        Ok(()) => Ok(Held::ToTheApp),
        // A copy writes nothing more: the value is still where it was copied from.
        Err(_) if why == Why::Copy => Err(not_ours(service)),
        // The command's own item is gone by now, so this makes a new one, the command's: never
        // another program's.
        Err(_) => super::keyring::set_here(service, account, value).map(|()| Held::NotYet),
    };
    // A write that failed keeps the spare: the value stays at rest there.
    if written.is_ok() || why == Why::Copy {
        drop_spare();
    }
    written
}

/// `item` written by the app's binary at `app`: `Err` with its exit code (`-1` when it could
/// not be run or did not finish in time).
fn through(app: &std::path::Path, item: &Item) -> Result<(), i32> {
    let mut doc = Map::new();
    doc.insert("service".into(), Value::String(item.service.clone()));
    doc.insert("account".into(), Value::String(item.account.clone()));
    doc.insert("value".into(), Value::String(item.value.clone()));
    let input = serde_json::to_vec(&Value::Object(doc)).map_err(|_| -1)?;
    let mut command = Command::new(app);
    command
        .arg(HOLD_ARG)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(home) = std::env::var_os("HOME") {
        command.env("HOME", home);
    }
    // Its own process group: a Ctrl-C to the terminal's group never ends it mid-write.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = crate::forklock::spawn(&mut command).map_err(|_| -1)?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&input);
    }
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(status.code().unwrap_or(-1))
                };
            }
            Ok(None) if started.elapsed() < PATIENCE => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(-1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_writer_takes_an_item_under_charters_own_services() {
        let item = asked(br#"{"service": "charter/ops/3f9a2c1b", "account": "K", "value": "v"}"#)
            .expect("taken");
        assert_eq!(
            item,
            Item {
                service: "charter/ops/3f9a2c1b".into(),
                account: "K".into(),
                value: "v".into(),
            }
        );
    }

    #[test]
    fn the_writer_takes_items_under_both_of_the_products_prefixes_and_their_identities() {
        for service in [
            "purlis/ops/3f9a2c1b",
            "charter/ops/3f9a2c1b",
            "purlis/@identity/0123456789abcdef",
            "charter/@identity/0123456789abcdef",
        ] {
            let input = format!(r#"{{"service": "{service}", "account": "K", "value": "v"}}"#);
            assert_eq!(asked(input.as_bytes()).expect(service).service, service);
        }
    }

    #[test]
    fn the_writer_refuses_a_lookalike_of_the_products_prefixes() {
        for service in [
            "charterx/ops/3f9a2c1b",
            "purlis-/ops/3f9a2c1b",
            "purlisx/ops/3f9a2c1b",
            "Purlis/ops/3f9a2c1b",
            "charter/../x",
            "purlis/../x",
            "charter/ops",
            "purlis/ops/a/b",
            "charter//x",
            "purlis/@identity",
            "purlis/@other/x",
            "edm/ops/3f9a2c1b",
        ] {
            let input = format!(r#"{{"service": "{service}", "account": "K", "value": "v"}}"#);
            let refused = asked(input.as_bytes()).expect_err(service);
            assert!(
                refused.contains("'purlis/…'") && refused.contains("'charter/…'"),
                "{refused}"
            );
        }
    }

    #[test]
    fn the_writer_refuses_another_programs_item_and_a_name_it_could_not_state() {
        for input in [
            br#"{"service": "Claude Code-credentials", "account": "K", "value": "v"}"#.as_slice(),
            br#"{"service": "charter/ops/x\n", "account": "K", "value": "v"}"#,
            br#"{"service": "charter/ops/x", "account": "", "value": "v"}"#,
            br#"{"service": "charter/ops/x", "account": "K"}"#,
            b"not json",
        ] {
            assert!(asked(input).is_err(), "{}", String::from_utf8_lossy(input));
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_command_run_through_a_link_on_path_finds_the_app_in_its_bundle() {
        // `/usr/local/bin/charter` is a link into the bundle (`clipath`), and macOS reports the
        // program as run, by the link's path, so the app is looked for beside the real file.
        let dir = tempfile::tempdir().expect("a directory");
        let bundle = dir.path().join("charter.app/Contents/MacOS");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bundle).expect("the bundle");
        std::fs::create_dir_all(&bin).expect("a bin");
        std::fs::write(bundle.join("charter"), "").expect("the command");
        std::fs::write(bundle.join(APP_BINARY), "").expect("the app");
        std::os::unix::fs::symlink(bundle.join("charter"), bin.join("charter")).expect("a link");
        assert_eq!(
            app_beside(&bin.join("charter")),
            Some(bundle.canonicalize().unwrap().join(APP_BINARY))
        );
        // A program planted beside the link is never the one handed a value.
        std::fs::write(bin.join(APP_BINARY), "").expect("a planted app");
        assert_eq!(
            app_beside(&bin.join("charter")),
            Some(bundle.canonicalize().unwrap().join(APP_BINARY))
        );
    }

    #[test]
    fn the_app_is_not_its_own_writer_and_a_lone_command_has_none() {
        let dir = tempfile::tempdir().expect("a directory");
        std::fs::write(dir.path().join(APP_BINARY), "").expect("the app");
        assert_eq!(app_beside(&dir.path().join(APP_BINARY)), None);
        assert_eq!(app_beside(&dir.path().join("missing")), None);
        let lone = tempfile::tempdir().expect("a directory");
        std::fs::write(lone.path().join("charter"), "").expect("the command");
        assert_eq!(app_beside(&lone.path().join("charter")), None);
    }

    /// What the child runs: the window up and down, with a SIGTERM raised inside it when
    /// `CHARTER_TEST_SIGNAL_IN_WINDOW` is set, then a line, then a wait for SIGTERM.
    #[cfg(unix)]
    #[test]
    #[ignore = "run by a_process_still_ends_on_sigterm_after_the_window, in a child"]
    fn the_window_in_a_child() {
        if std::env::var_os("CHARTER_TEST_WINDOW_CHILD").is_none() {
            return;
        }
        {
            let _shield = Shield::up();
            if std::env::var_os("CHARTER_TEST_SIGNAL_IN_WINDOW").is_some() {
                let _ = signal_hook::low_level::raise(signal_hook::consts::SIGTERM);
            }
        }
        println!("after the window");
        std::thread::sleep(Duration::from_secs(10));
        println!("survived");
    }

    /// The child's exit, after it said it left the window, with a SIGTERM sent to it then
    /// unless one was raised inside the window.
    #[cfg(unix)]
    fn child_after_the_window(raised_inside: bool) -> (std::process::ExitStatus, String) {
        use std::io::BufRead;
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "secrets::keyhold::tests::the_window_in_a_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("CHARTER_TEST_WINDOW_CHILD", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if raised_inside {
            command.env("CHARTER_TEST_SIGNAL_IN_WINDOW", "1");
        }
        let mut child = crate::forklock::spawn(&mut command).unwrap();
        let mut lines = std::io::BufReader::new(child.stdout.take().unwrap()).lines();
        let mut said = String::new();
        for line in lines.by_ref() {
            let line = line.unwrap_or_default();
            said.push_str(&line);
            said.push('\n');
            if line.contains("after the window") {
                if !raised_inside {
                    let pid = i32::try_from(child.id()).unwrap();
                    nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid),
                        nix::sys::signal::Signal::SIGTERM,
                    )
                    .unwrap();
                }
                break;
            }
        }
        for line in lines {
            said.push_str(&line.unwrap_or_default());
            said.push('\n');
        }
        (child.wait().unwrap(), said)
    }

    #[cfg(unix)]
    #[test]
    fn a_process_still_ends_on_sigterm_after_the_window() {
        use std::os::unix::process::ExitStatusExt;
        // N1: holding the signals off must never leave the process deaf to them afterwards,
        // whether one arrived inside the window (it is delivered as it leaves) or not.
        for raised_inside in [true, false] {
            let (status, said) = child_after_the_window(raised_inside);
            assert_eq!(
                status.signal(),
                Some(nix::sys::signal::Signal::SIGTERM as i32),
                "raised inside: {raised_inside}; {status:?}; said:\n{said}"
            );
            assert!(!said.contains("survived"), "{said}");
        }
    }

    const SERVICE: &str = "purlis/ops/x";

    fn key(service: &str) -> (String, String) {
        (service.to_owned(), "K".to_owned())
    }

    /// Items in memory, as another program and the store itself may answer: `planted` put at
    /// the item by another program right after its delete, `owned` items another program made
    /// (which this one may not delete), and `refused` items whose add fails. Each delete of the
    /// item itself checks that its value is at rest somewhere else meanwhile.
    #[derive(Default)]
    struct Stub {
        items: std::sync::Mutex<std::collections::BTreeMap<(String, String), String>>,
        planted: Option<&'static str>,
        owned: Vec<String>,
        refused: Vec<String>,
        /// The value being written, which a delete of the item must find at rest elsewhere.
        writing: Option<&'static str>,
    }

    impl Items for Stub {
        fn delete(&self, service: &str, account: &str) -> Result<(), Refused> {
            if self.owned.iter().any(|s| s == service) {
                return Err(Refused::Owned);
            }
            let mut items = self.items.lock().unwrap();
            if service == SERVICE
                && let Some(value) = self.writing
            {
                assert!(
                    items
                        .iter()
                        .any(|(k, v)| k.0 != SERVICE && v.as_str() == value),
                    "the item is deleted while its value is only in memory: {items:?}"
                );
            }
            items.remove(&key(service));
            if service == SERVICE
                && let Some(planted) = self.planted
            {
                items.insert(key(service), planted.to_owned());
            }
            let _ = account;
            Ok(())
        }
        fn add(&self, service: &str, account: &str, value: &str) -> Result<(), Refused> {
            if self.refused.iter().any(|s| s == service) {
                return Err(Refused::Failed(VaultError::new(
                    "the store refused the add",
                )));
            }
            let mut items = self.items.lock().unwrap();
            let k = (service.to_owned(), account.to_owned());
            if items.contains_key(&k) {
                return Err(Refused::Owned);
            }
            items.insert(k, value.to_owned());
            Ok(())
        }
    }

    impl Stub {
        fn with_old() -> Self {
            let stub = Self {
                writing: Some("new"),
                ..Self::default()
            };
            stub.items
                .lock()
                .unwrap()
                .insert(key(SERVICE), "old".to_owned());
            stub
        }

        fn items(&self) -> std::collections::BTreeMap<(String, String), String> {
            self.items.lock().unwrap().clone()
        }
    }

    #[test]
    fn an_item_is_made_again_add_only_and_one_planted_meanwhile_is_never_written_into() {
        let calm = Stub::with_old();
        assert!(remake(&calm, SERVICE, "K", "new").is_ok());
        assert_eq!(
            calm.items(),
            [(key(SERVICE), "new".to_owned())].into(),
            "the item made again, and nothing else left"
        );

        let raced = Stub {
            planted: Some("planted"),
            ..Stub::with_old()
        };
        assert!(matches!(
            remake(&raced, SERVICE, "K", "new"),
            Err(Refused::Owned)
        ));
        assert_eq!(
            raced.items()[&key(SERVICE)],
            "planted",
            "the planted item was written into"
        );
    }

    #[test]
    fn the_value_is_never_only_in_memory_while_its_item_is_made_again() {
        // #1309: between the delete and the add the value was only in this process. A refused
        // add (or a crash, which the stub's delete stands in for by checking) lost it.
        let refused = Stub {
            refused: vec![SERVICE.to_owned()],
            ..Stub::with_old()
        };
        assert!(matches!(
            remake(&refused, SERVICE, "K", "new"),
            Err(Refused::Failed(_))
        ));
        let left = refused.items();
        assert!(!left.contains_key(&key(SERVICE)));
        assert_eq!(
            left.values().collect::<Vec<_>>(),
            ["new"],
            "the value is kept at rest: {left:?}"
        );

        // The spare is never an item the app's writer can be asked to write, or one a vault or
        // an identity names.
        let spare = left.keys().next().unwrap().0.clone();
        assert!(spare.starts_with(SERVICE), "{spare}");
        let input = format!(r#"{{"service": "{spare}", "account": "K", "value": "v"}}"#);
        assert!(asked(input.as_bytes()).is_err(), "{spare}");
    }

    #[test]
    fn a_spare_that_cannot_be_made_leaves_the_item_as_it_was() {
        // Another program's item where the spare goes, or a store that refuses it: nothing is
        // deleted, and the old item stays.
        let spare = || spare_of(SERVICE);
        for stub in [
            Stub {
                owned: vec![spare()],
                ..Stub::with_old()
            },
            Stub {
                refused: vec![spare()],
                ..Stub::with_old()
            },
        ] {
            let Err(Refused::Failed(e)) = remake(&stub, SERVICE, "K", "new") else {
                panic!("a remake with no spare went ahead");
            };
            assert!(!e.message.contains("new"), "{}", e.message);
            assert_eq!(stub.items()[&key(SERVICE)], "old");
        }

        // A spare left by an earlier write that did not finish is this program's, and older.
        let stale = Stub::with_old();
        stale
            .items
            .lock()
            .unwrap()
            .insert(key(&spare()), "older".to_owned());
        assert!(remake(&stale, SERVICE, "K", "new").is_ok());
        assert_eq!(stale.items(), [(key(SERVICE), "new".to_owned())].into());
    }

    #[test]
    fn a_copy_never_falls_back_to_a_write_in_place() {
        // Where an item cannot be held, a move is left and a copy writes nothing. (A write in
        // place is not asked here: it would reach the operating system's store.)
        let refused = in_place("purlis/ops/3f9a2c1b", "K", "v", Why::Copy).unwrap_err();
        assert!(
            refused.message.contains("wrote nothing"),
            "{}",
            refused.message
        );
        assert_eq!(
            in_place("purlis/ops/3f9a2c1b", "K", "v", Why::Move),
            Ok(Held::NotYet)
        );
    }

    #[test]
    fn a_test_build_never_starts_a_writer_or_reaches_the_real_store() {
        // purlis-core's own tests run fenced.
        const _: () = assert!(crate::fence::FENCED);
        for why in [Why::Write, Why::Move, Why::Copy] {
            let refused = set("purlis/ops/3f9a2c1b", "K", "v", why).unwrap_err();
            assert!(
                refused.message.contains("test build"),
                "{}",
                refused.message
            );
        }
    }

    #[test]
    fn a_process_not_started_as_the_writer_is_left_alone() {
        assert_eq!(serve_if_asked(), None);
    }
}
