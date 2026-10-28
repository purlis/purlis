// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No old name is spelled outside `purlis_core::names`** (#1262, RN-2z).
//!
//! The rename to purlis is an expand–contract refactor. RN-1 put every renamed name in one
//! module, [`purlis_core::names`], with the purlis spelling to write and the old spellings
//! still read. RN-2a…d moved the callers onto it. This is the contract step: a string literal
//! in the repository's Rust that spells an old name — `.charter`, `charter.toml`,
//! `charter.local.toml`, `.charter-scan-allow.toml`, a `CHARTER_` variable, a `charter/`
//! keychain or branch prefix, a `Charter-` trailer, an old marker — fails this test, so the
//! old name cannot leak back in one call site at a time.
//!
//! **What is scanned.** Every `.rs` file under `crates/` and `app/src-tauri/`, string literals
//! only (comments and doc comments are prose about the names and are not read). The spellings
//! looked for are not listed here: they are every old spelling (`reads` and `history`) of the
//! entries in [`names::ALL`] whose kind is a file, folder, marker, keychain prefix, variable
//! prefix, branch prefix or trailer — so a name added there is guarded here the day it is
//! added. A bare word (`charter`, the config home's and the 1Password tag's old spelling) is
//! not looked for: it is the product's name, and every sentence that says it is RN-11a's.
//!
//! **The name module is the only code exception.** `names.rs` and `names/` hold the old
//! spellings because saying them is their job. Everything else that still spells one is on
//! one of two allow-lists below:
//!
//! * [`STILL_SPELLED`] — shipped code, file by file, with the exact number of literals and a
//!   [`Why`] each. The counts are exact, so the list can only SHRINK: a file that spells one
//!   more fails until somebody raises its count in review, and a file that spells one fewer
//!   fails until its count is lowered (or its entry removed).
//! * [`FIXTURES`] — test code, by scope. A test of the compatibility window has to build a
//!   project under the old names; that is a fixture, not a leak.
//!
//! **Time-boxed fallbacks.** Some old names are still READ, inside the name module, for a set
//! time after everything shipped stopped writing them. [`TIME_BOXED`] lists each one on
//! purpose, with when it goes, so a fallback is never mistaken for a leftover nobody chose.

use std::collections::BTreeMap;
use std::path::Path;

use crate::old_names;
use old_names::{old_spellings, spells};

use crate::shipped_literals;
use shipped_literals::{
    FIXTURE_FILES, FIXTURES, TestMod, is_fixture_path, outside_tests, repo, shipped,
};

/// Why a shipped file may still spell an old name. Each is a ticket that removes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    /// Text an operator or an agent reads — a refusal, a hint, a help line, a doctor row —
    /// that names an old file or variable. RN-11a (#1269) renames the product's visible words.
    Prose,
    /// A list or a sentence that names both spellings on purpose, so the old one keeps working
    /// through the compatibility window. It goes with the window: RN-14 (#1273).
    Window,
    /// A template of a file committed to a project, written under the old names until the
    /// project is migrated: RN-7 (rename-plane, #1265) rewrites it (#1277).
    CommittedTemplate,
    /// A test-only variable read by shipped code (`CHARTER_TEST_*`, the plane fence, the
    /// probes'): RN-11a moves them with the tests that set them (#1278).
    TestVariable,
    /// A name a shipped artifact outside this code carries: the variable the plugin's hooks
    /// file and the opencode shim read, the shim's own file and text, a bundled extension's
    /// manifest file. It moves with that artifact: RN-8 (plugin, #1266), #1284.
    ArtifactContract,
}

/// Shipped files that still spell an old name, with how many literals do and why. A file
/// whose literals have two reasons has an entry for each; its counts add up.
///
/// **Shrink only.** Lowering a count, or removing an entry, is the whole of finishing one.
/// Raising one, or adding one, is a decision a reviewer makes on purpose; route the name
/// through [`purlis_core::names`] instead wherever it is not visible text.
#[rustfmt::skip]
const STILL_SPELLED: &[(&str, usize, Why)] = &[
    // Visible text: RN-11a (#1269).
    ("app/src-tauri/src/doctor.rs", 1, Why::Prose),
    ("app/src-tauri/src/planes.rs", 1, Why::Prose),
    ("crates/purlis-core/src/adopt.rs", 2, Why::Prose),
    ("crates/purlis-core/src/alerts.rs", 1, Why::Prose),
    ("crates/purlis-core/src/chatenv.rs", 4, Why::Prose),
    ("crates/purlis-core/src/commitguard.rs", 1, Why::Prose),
    ("crates/purlis-core/src/compat.rs", 2, Why::Prose),
    ("crates/purlis-core/src/diffscan.rs", 3, Why::Prose),
    ("crates/purlis-core/src/doctor/config.rs", 9, Why::Prose),
    ("crates/purlis-core/src/doctor/fix.rs", 1, Why::Prose),
    ("crates/purlis-core/src/doctor/git.rs", 2, Why::Prose),
    ("crates/purlis-core/src/doctor/plane.rs", 3, Why::Prose),
    ("crates/purlis-core/src/doctor/profiles.rs", 1, Why::Prose),
    ("crates/purlis-core/src/extension/writes.rs", 1, Why::Prose),
    ("crates/purlis-core/src/fence.rs", 1, Why::Prose),
    ("crates/purlis-core/src/forge.rs", 1, Why::Prose),
    ("crates/purlis-core/src/forge/pr.rs", 1, Why::Prose),
    ("crates/purlis-core/src/gitpolicy.rs", 2, Why::Prose),
    ("crates/purlis-core/src/guest.rs", 1, Why::Prose),
    ("crates/purlis-core/src/harness_declaration.rs", 1, Why::Prose),
    ("crates/purlis-core/src/inventory.rs", 2, Why::Prose),
    ("crates/purlis-core/src/leakguard.rs", 1, Why::Prose),
    ("crates/purlis-core/src/personacmd.rs", 4, Why::Prose),
    ("crates/purlis-core/src/personaverbs/define.rs", 1, Why::Prose),
    ("crates/purlis-core/src/planegit/prsave.rs", 1, Why::Prose),
    ("crates/purlis-core/src/planesave.rs", 1, Why::Prose),
    ("crates/purlis-core/src/profiles.rs", 6, Why::Prose),
    ("crates/purlis-core/src/programs.rs", 1, Why::Prose),
    ("crates/purlis-core/src/renamelocal/busy.rs", 1, Why::Prose),
    ("crates/purlis-core/src/repocmd/clone.rs", 1, Why::Prose),
    ("crates/purlis-core/src/repocmd/status.rs", 1, Why::Prose),
    ("crates/purlis-core/src/sandbox/local.rs", 2, Why::Prose),
    ("crates/purlis-core/src/scaffold/mod.rs", 3, Why::Prose),
    ("crates/purlis-core/src/secrets/cmd.rs", 2, Why::Prose),
    ("crates/purlis-core/src/settings/forges.rs", 3, Why::Prose),
    ("crates/purlis-core/src/wiring.rs", 3, Why::Prose),
    ("crates/purlis-core/src/work/promote.rs", 1, Why::Prose),
    ("crates/purlis-core/src/worktree/mod.rs", 2, Why::Prose),
    ("crates/purlis-core/src/wscmd/select.rs", 3, Why::Prose),
    // Both spellings on purpose, for the window: RN-14 (#1273).
    ("crates/purlis-core/src/chatenv.rs", 1, Why::Window),
    ("crates/purlis-core/src/planegit.rs", 2, Why::Window),
    // Committed-file templates: RN-7 (#1265).
    ("crates/purlis-core/src/scaffold/mod.rs", 1, Why::CommittedTemplate),
    // Test-only variables: #1278.
    ("crates/purlis-cli/src/extensions.rs", 1, Why::TestVariable),
    ("crates/purlis-cli/src/main.rs", 2, Why::TestVariable),
    ("crates/purlis-core/src/fence.rs", 1, Why::TestVariable),
    ("crates/session-protocol/src/bin/charter-session-peer.rs", 1, Why::TestVariable),
    // Names a shipped non-Rust artifact carries: RN-8 (#1266), #1284.
    // The variable the plugin's hooks and the opencode shim read keeps its old name until 1.0
    // (D-RN8-1), as the hook commands do (D-RN7-11).
    ("crates/purlis-core/src/opencode.rs", 2, Why::ArtifactContract),
    ("crates/purlis-core/src/plugin.rs", 1, Why::ArtifactContract),
];

/// Old names the name module still reads on purpose for a set time, by [`purlis_core::names`]
/// id, with the ticket and when the fallback goes. Deleting the name's `reads` ends it, and
/// then the entry goes too ([`the_time_boxed_fallbacks_are_still_read`] fails until it does).
const TIME_BOXED: &[(&str, &str)] = &[(
    "extension-manifest",
    "#1802: an extension's charter-extension.json is read when it has no purlis-extension.json, \
     for one release after the built-in extensions shipped the new name",
)];

// ---- the scan ------------------------------------------------------------------------- //

/// A literal that spells an old name: its line, the spelling, and the literal's text.
type Hit = (usize, String, String);

/// The literals in `src` that spell an old name, outside test-only items: `(line, spelling,
/// literal)`. Also the test-only module files `src` brings in.
fn stray(src: &str, spellings: &[&str]) -> (Vec<Hit>, Vec<TestMod>) {
    let (literals, files) = outside_tests(src);
    let hits = literals
        .into_iter()
        .filter_map(|l| {
            let s = spells(&l.text, spellings)?;
            Some((l.line, s.to_owned(), l.text))
        })
        .collect();
    (hits, files)
}

// ---- the tree ------------------------------------------------------------------------- //

fn is_name_module(rel: &str) -> bool {
    rel == "crates/purlis-core/src/names.rs" || rel.starts_with("crates/purlis-core/src/names/")
}

/// Every shipped file's stray literals, by path relative to the repository.
fn scan(root: &Path) -> (BTreeMap<String, Vec<Hit>>, usize) {
    let spellings = old_spellings();
    let mut found = BTreeMap::new();
    let mut scanned = 0;
    for (rel, literals) in shipped(root) {
        if is_name_module(&rel) {
            continue;
        }
        scanned += 1;
        let hits: Vec<Hit> = literals
            .into_iter()
            .filter_map(|l| {
                let s = spells(&l.text, &spellings)?;
                Some((l.line, s.to_owned(), l.text))
            })
            .collect();
        if !hits.is_empty() {
            found.insert(rel, hits);
        }
    }
    (found, scanned)
}

/// What is wrong with `found` against `allowed` (STILL_SPELLED's shape): a file spelling more
/// than its allowance, with each literal; one spelling fewer, to lower; an entry spelling none.
fn beyond_allowance(
    found: &BTreeMap<String, Vec<Hit>>,
    allowed: &[(&str, usize, Why)],
) -> Vec<String> {
    let mut wrong = Vec::new();
    let allowance = |file: &str| -> usize {
        allowed
            .iter()
            .filter(|(f, ..)| *f == file)
            .map(|(_, n, _)| n)
            .sum()
    };
    for (file, hits) in found {
        let allowed = allowance(file);
        match allowed.cmp(&hits.len()) {
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => wrong.push(format!(
                "{file}: allowed {allowed} but spells {} — lower its count in STILL_SPELLED",
                hits.len()
            )),
            std::cmp::Ordering::Less => {
                wrong.push(format!(
                    "{file}: spells {} old name(s), allowed {allowed} — route it through \
                     purlis_core::names:",
                    hits.len()
                ));
                for (line, spelling, text) in hits {
                    let shown: String = text.chars().take(100).collect();
                    wrong.push(format!("    {file}:{line}: `{spelling}` in \"{shown}\""));
                }
            }
        }
    }
    for (file, _, why) in allowed {
        if !found.contains_key(*file) {
            wrong.push(format!(
                "{file}: allowed ({why:?}) but spells none — remove its entry"
            ));
        }
    }
    wrong
}

#[test]
fn no_shipped_file_spells_an_old_name_beyond_its_allowance() {
    purlis_core::unsteered!();
    let (found, scanned) = scan(&repo());
    assert!(scanned > 300, "the walk scanned {scanned} shipped files");
    let wrong = beyond_allowance(&found, STILL_SPELLED);
    assert!(
        wrong.is_empty(),
        "old names outside purlis_core::names ({FIXTURES} excepted):\n{}",
        wrong.join("\n")
    );
}

// ---- the guard's own tests ------------------------------------------------------------ //

/// A repository of one shipped file holding `body`, plus the name module, scanned.
fn scan_of(body: &str) -> BTreeMap<String, Vec<Hit>> {
    let tree = tempfile::tempdir().expect("a tree");
    let src = tree.path().join("crates/x/src");
    std::fs::create_dir_all(&src).expect("folders");
    std::fs::write(src.join("lib.rs"), body).expect("lib.rs");
    let names = tree.path().join("crates/purlis-core/src/names");
    std::fs::create_dir_all(&names).expect("the name module's folder");
    std::fs::write(
        names.with_extension("rs"),
        "const A: &str = \".charter\";\n",
    )
    .expect("names.rs");
    std::fs::write(
        names.join("window.rs"),
        "const B: &str = \"charter.toml\";\n",
    )
    .expect("window.rs");
    scan(tree.path()).0
}

#[test]
fn a_stray_literal_in_a_shipped_file_is_found_and_the_name_module_is_not() {
    purlis_core::unsteered!();
    let found = scan_of("fn f() -> &'static str {\n    \".charter/vaults\"\n}\n");
    let hits = &found["crates/x/src/lib.rs"];
    assert_eq!(hits.len(), 1, "{found:?}");
    assert_eq!((hits[0].0, hits[0].1.as_str()), (2, ".charter"));
    assert_eq!(
        found.len(),
        1,
        "the name module is never reported: {found:?}"
    );
}

#[test]
fn the_guard_fails_on_a_stray_literal_injected_into_the_tree() {
    purlis_core::unsteered!();
    // The repository as it stands passes; the same, plus one shipped line that spells the old
    // manifest, is reported by the very check the guard runs.
    let (mut found, _) = scan(&repo());
    assert!(beyond_allowance(&found, STILL_SPELLED).is_empty());
    let tree = tempfile::tempdir().expect("a tree");
    let injected = tree.path().join("crates/injected/src");
    std::fs::create_dir_all(&injected).expect("a crate");
    std::fs::write(
        injected.join("lib.rs"),
        "pub fn manifest(dir: &std::path::Path) -> std::path::PathBuf {\n    \
         dir.join(\"charter.toml\")\n}\n",
    )
    .expect("lib.rs");
    let (stray, _) = scan(tree.path());
    found.extend(stray);
    let wrong = beyond_allowance(&found, STILL_SPELLED);
    assert_eq!(
        wrong,
        [
            "crates/injected/src/lib.rs: spells 1 old name(s), allowed 0 — route it through \
             purlis_core::names:",
            "    crates/injected/src/lib.rs:2: `charter.toml` in \"charter.toml\"",
        ],
    );
}

#[test]
fn every_kind_the_ticket_names_is_looked_for_and_the_bare_product_name_is_not() {
    purlis_core::unsteered!();
    let spellings = old_spellings();
    for wanted in [
        ".charter",
        "charter.toml",
        "charter.local.toml",
        ".charter-scan-allow.toml",
        "CHARTER_",
        "charter/",
        "charter/@identity",
        "Charter-Chat",
        "Charter-Persona",
        "Charter-Change",
        "<!-- charter-save -->",
        "# >>> charter merge rules (managed by charter) >>>",
        // The plugin's (RN-8, #1266).
        "charter@inline",
        "charter-app@inline",
        "charter@charter-app",
        "charter:",
        "mcp__charter__",
    ] {
        assert!(spellings.contains(&wanted), "{wanted} is not looked for");
    }
    assert!(!spellings.contains(&"charter"));
    assert!(
        !spellings
            .iter()
            .any(|s| s.starts_with("purlis") || s.starts_with(".purlis"))
    );
}

#[test]
fn a_spelling_counts_only_at_a_word_boundary() {
    purlis_core::unsteered!();
    let s = old_spellings();
    for hit in [
        ".charter",
        "/.charter/",
        "{}.charter-{}",
        "x/charter.toml",
        "$CHARTER_ROOT",
        "CHARTER_ROOT=1",
        "charter/ops/db",
        "Charter-Chat: 3",
        "charter@inline",
        "{\"charter@charter-app\": true}",
        "charter:handoff",
        "mcp__charter__todo_add",
        // The old bundle identifier is a spelling of its own (#1284), not `.charter`'s.
        "dev.charter.app",
        "Logs/dev.charter.app.lock",
    ] {
        assert!(spells(hit, &s).is_some(), "{hit} should count");
    }
    for miss in [
        "dev.purlis.app",
        "dev.charter.apps",
        ".charterx",
        "MY_CHARTER_ROOT",
        "a charter of its own",
        "purlis.toml",
        "PURLIS_ROOT",
        "acharter/",
        "charter: the app did not take this",
        "charter:",
        "purlis:handoff",
        "purlis@inline",
        "mcp__purlis__todo_add",
        "charter-app#274",
    ] {
        assert_eq!(spells(miss, &s), None, "{miss} should not count");
    }
}

#[test]
fn comments_and_test_only_items_are_not_read_and_everything_else_is() {
    purlis_core::unsteered!();
    let s = old_spellings();
    let src = r##"
// a comment about ".charter" is prose
/* so is "charter.toml" in a /* nested */ block */
/// and a doc comment's "CHARTER_ROOT"
const Q: char = '"';
fn f<'a>(x: &'a str) -> &'a str { x }
const RAW: &str = r#"has "charter.local.toml" in it"#;
const BYTES: &[u8] = b"CHARTER_HOME";
#[cfg(test)]
mod tests {
    const T: &str = ".charter";
}
#[cfg(all(test, unix))]
fn helper() -> &'static str { "charter/x" }
#[cfg(test)]
#[path = "elsewhere_helpers.rs"]
mod elsewhere;
#[cfg(not(test))]
const SHIPPED: &str = "Charter-Chat";
struct S {
    #[cfg(test)]
    seen: u8,
    shipped: u8,
}
fn g(x: u8) -> S {
    match x {
        #[cfg(test)]
        0 => {}
        _ => {}
    }
    S {
        #[cfg(test)]
        seen: 0,
        shipped: { let _ = ".charter-scan-allow.toml"; 1 },
    }
}
impl S {
    fn h(&self) -> &str { "charter/ops" }
}
#[cfg(test)]
pub(crate) async fn hidden() -> &'static str { "charter.toml" }
"##;
    let (hits, files) = stray(src, &s);
    let spelled: Vec<&str> = hits.iter().map(|(_, sp, _)| sp.as_str()).collect();
    assert_eq!(
        spelled,
        [
            "charter.local.toml",
            "CHARTER_",
            "Charter-Chat",
            ".charter-scan-allow.toml",
            "charter/"
        ],
        "{hits:?}"
    );
    assert_eq!(
        files,
        [(
            "elsewhere".to_owned(),
            Some("elsewhere_helpers.rs".to_owned())
        )]
    );
}

#[test]
fn the_allow_lists_are_well_formed() {
    purlis_core::unsteered!();
    let root = repo();
    for (file, n, why) in STILL_SPELLED {
        assert!(*n > 0, "{file}: an entry of 0 ({why:?}) is a removed entry");
        assert!(root.join(file).is_file(), "{file} is not a file");
        assert!(
            !is_name_module(file) && !is_fixture_path(file),
            "{file} needs no entry"
        );
    }
    for (path, why) in FIXTURE_FILES {
        assert!(root.join(path).exists(), "{path} ({why}) does not exist");
    }
}

#[test]
fn the_time_boxed_fallbacks_are_still_read() {
    purlis_core::unsteered!();
    for (id, why) in TIME_BOXED {
        let name = purlis_core::names::ALL
            .iter()
            .find(|name| name.id == *id)
            .unwrap_or_else(|| panic!("{id} ({why}) is no name in the name module"));
        assert!(
            !name.reads.is_empty(),
            "{id} reads no old name any more: its fallback is over, so remove its entry ({why})"
        );
    }
}
