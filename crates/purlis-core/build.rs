// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Compiles `docs/` into the binary.
//!
//! The documentation pages follow one rule: *the page a user reads should come from the same
//! install as the behaviour, so `charter docs show secrets` cannot describe a vault the running
//! CLI does not have.* An app installed from a signed bundle has no checkout to fall back to, so
//! the pages are `include_str!`ed and there is no second source to drift from. They are this
//! app's own, written here (M2.21, ADR 0044).
//!
//! **The file list is read from the directory rather than written down.** A checked-in list of
//! `include_str!` lines would be a second statement of what is in `docs/`, free to disagree with
//! the directory the day somebody adds a page.
//!
//! The order is the topic's, sorted: `docsrc.topics()` is `sorted(...)` of the stems, and a stem
//! differs from its filename only by a suffix every page shares.
//!
//! `templates/` is compiled in the same way and for the same reason: a project template is data
//! (FR-17), and the template a first run lays out has to be the one this install ships. Its
//! files are nested, so they are walked rather than listed.
//!
//! (`news/`, the Python charter's frozen news corpus, was compiled in here too until `charter
//! news` became the app's own CHANGELOG.md, #352.)

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    // `charter news` compiles the repository's CHANGELOG.md in with `include_str!`, which
    // cargo already tracks; nothing to generate for it.
    compile(
        "docs",
        "docs_pages.rs",
        "PAGES",
        "Every documentation page, as `(topic, text)`, in topic order.",
        // The TOPIC, which is `Path.stem`: `charter docs show secrets`, never `secrets.md`.
        // `strip_suffix` and not `trim_end_matches`, which takes the suffix off REPEATEDLY
        // and would key `a.md.md` as `a` where Python keys it `a.md`.
        |name| name.strip_suffix(".md").unwrap_or(name).to_string(),
    );
    compile_tree(
        "templates",
        "template_files.rs",
        "FILES",
        "Every project template's files, as `(<template>/<path>, text)`, sorted by path.",
    );
    families();
    println!("cargo:rustc-env=PURLIS_BUILD_PROFILE={}", profile());
    println!(
        "cargo:rustc-env=PURLIS_RELEASE_VERSION={}",
        release_version()
    );
}

/// **The version this build says it is** (#2749): the one a release stamps it with,
/// `PURLIS_VERSION_STAMP` (`release.yml`'s `plan`, a dev build's `<version>-dev.<run>`), so the
/// desktop and the runner image built for it say the same; else the crate's own. A stamp that is
/// not the crate's version, or a prerelease of it, stops the build.
fn release_version() -> String {
    println!("cargo:rerun-if-env-changed=PURLIS_VERSION_STAMP");
    let own = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let stamp = std::env::var("PURLIS_VERSION_STAMP").unwrap_or_default();
    if stamp.is_empty() {
        return own;
    }
    let plain = stamp
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let of_it = stamp == own
        || stamp
            .strip_prefix(&own)
            .is_some_and(|rest| rest.starts_with('-'));
    assert!(
        plain && of_it,
        "PURLIS_VERSION_STAMP {stamp:?} is not {own} or a prerelease of it"
    );
    stamp
}

/// The Cargo profile this build is made with, by the name Cargo files it under: `release`,
/// `dev-release`, and `debug` for `dev` and `test` (ADR 0092, #1429).
///
/// Cargo hands a build script `PROFILE`, but that is only ever `debug` or `release`: a custom
/// profile reports the one it inherits from, so a `dev-release` build would call itself
/// `release`, which is the one thing the version line exists to tell apart. The directory is
/// the name. `OUT_DIR` is `<target>[/<triple>]/<profile>/build/<crate>-<hash>/out`, and when
/// it is not that shape the answer falls back to `PROFILE` rather than to a guess.
fn profile() -> String {
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let mut above = Path::new(&out).ancestors().skip(2);
    let build = above.next().and_then(Path::file_name);
    let named = above.next().and_then(Path::file_name);
    match (
        build.and_then(|b| b.to_str()),
        named.and_then(|n| n.to_str()),
    ) {
        (Some("build"), Some(name)) if !name.is_empty() => name.to_owned(),
        _ => std::env::var("PROFILE").expect("cargo sets PROFILE"),
    }
}

/// Every file under `<crate>/<dir>`, at any depth, as a `&[(&str, &str)]` named `konst` keyed
/// by its path below `dir` with `/` between segments, written to `OUT_DIR`.
fn compile_tree(dir: &str, out: &str, konst: &str, doc: &str) {
    fn walk(at: &Path, under: &str, found: &mut Vec<(String, std::path::PathBuf)>) {
        println!("cargo:rerun-if-changed={}", at.display());
        let entries =
            std::fs::read_dir(at).unwrap_or_else(|e| panic!("cannot read {}: {e}", at.display()));
        for entry in entries {
            let entry =
                entry.unwrap_or_else(|e| panic!("cannot read an entry of {}: {e}", at.display()));
            let name = entry.file_name().to_string_lossy().into_owned();
            let key = if under.is_empty() {
                name
            } else {
                format!("{under}/{name}")
            };
            let path = entry.path();
            if path.is_dir() {
                walk(&path, &key, found);
            } else {
                found.push((key, path));
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut found = Vec::new();
    walk(&root, "", &mut found);
    found.sort();
    assert!(!found.is_empty(), "{} holds no files", root.display());

    let mut text =
        format!("/// {doc} Generated by `build.rs`.\npub static {konst}: &[(&str, &str)] = &[\n");
    for (key, path) in &found {
        println!("cargo:rerun-if-changed={}", path.display());
        writeln!(
            text,
            "    ({key:?}, include_str!({:?})),",
            path.to_string_lossy()
        )
        .expect("writing to a String cannot fail");
    }
    text.push_str("];\n");
    let target = Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join(out);
    std::fs::write(&target, text)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", target.display()));
}

/// Every `*.md` in `<crate>/<dir>` as a `&[(&str, &str)]` named `konst`, written to `OUT_DIR`.
///
/// `key` is what each entry is looked up by: the topic, for docs.
fn compile(dir: &str, out: &str, konst: &str, doc: &str, key: impl Fn(&str) -> String) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("cannot read an entry of {}: {e}", dir.display()))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".md"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "{} holds no entries", dir.display());

    let mut text =
        format!("/// {doc} Generated by `build.rs`.\npub static {konst}: &[(&str, &str)] = &[\n");
    for name in &names {
        let path = dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        // Rust string escapes for a path and a key. Real names are `[0-9a-z.-]` and the
        // directory is the crate's own, but a generator that assumes its input is quote-free
        // is a generator that emits code that will not compile the day it is not.
        writeln!(
            text,
            "    ({:?}, include_str!({:?})),",
            key(name),
            path.to_string_lossy()
        )
        .expect("writing to a String cannot fail");
    }
    text.push_str("];\n");

    let target = Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join(out);
    std::fs::write(&target, text)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", target.display()));
}

/// **What the first twelve manifest versions shipped, in the order they shipped it** (ADR 0103):
/// the order `purlis theme parts` lists them in and version 12's digest was taken over. Frozen: a
/// family that came in later is placed by the version it came in, then its name ([`families`]),
/// so no family is ever added here.
const SHIPPED: [&str; 18] = [
    "button",
    "field",
    "text-input",
    "textarea",
    "tabs",
    "editor",
    "panel",
    "panel-section",
    "empty-state",
    "status-item",
    "tooltip",
    "popover",
    "menu",
    "dialog",
    "rail",
    "activity",
    "list",
    "titlebar",
];

/// One family a family's file declares: its module, its constant, its name and the version it
/// came in (`u32::MAX` for `super::NEXT`, which no release has given a number yet).
struct Declared {
    module: String,
    konst: String,
    name: String,
    since: u32,
}

/// **The component parts manifest, assembled from each family's own file** (spec #2897, the
/// operator's ruling of 2026-10-26 that two family pull requests never touch the same lines).
///
/// Every `src/theme/parts_<module>.rs` that is not a test is a module of `theme::parts`, and each
/// `pub(super) const <NAME>: Family = Family {` at the start of a line in it is a family of
/// `theme::parts::MANIFEST`: the family's own `name:` and `since:` are its lines at one
/// indent inside it. Every `src/theme/parts_<module>_tests.rs` is a test module
/// of `theme`. Both are written to `OUT_DIR`, which `parts.rs` and `theme.rs` include.
///
/// The order is what [`SHIPPED`] says for what it names, then by `since` and name, NEXT last: a
/// family's place depends on nothing another pull request writes.
fn families() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/theme");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| {
            let entry =
                entry.unwrap_or_else(|e| panic!("cannot read an entry of {}: {e}", dir.display()));
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            )
        })
        .filter(|(name, _)| name.starts_with("parts_") && name.ends_with(".rs"))
        .collect();
    files.sort();

    let mut modules = String::new();
    let mut tests = String::new();
    let mut declared: Vec<Declared> = Vec::new();
    for (name, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = &name["parts_".len()..name.len() - ".rs".len()];
        let plain = stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        assert!(
            plain,
            "{name}: a family file is named parts_<snake_case>.rs"
        );
        // The engine's own tests, which `theme.rs` declares.
        if stem == "tests" {
            continue;
        }
        if stem.ends_with("_tests") {
            writeln!(
                tests,
                "#[cfg(test)]\n#[path = {:?}]\nmod parts_{stem};",
                path.to_string_lossy()
            )
            .expect("writing to a String cannot fail");
            continue;
        }
        writeln!(
            modules,
            "#[path = {:?}]\nmod {stem};",
            path.to_string_lossy()
        )
        .expect("writing to a String cannot fail");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        declared.extend(declared_in(stem, name, &text));
    }

    for (at, one) in declared.iter().enumerate() {
        assert!(
            declared[..at].iter().all(|other| other.name != one.name),
            "two families are named {:?}",
            one.name
        );
    }
    let place = |one: &Declared| {
        let shipped = SHIPPED.iter().position(|name| *name == one.name);
        (
            shipped.unwrap_or(SHIPPED.len()),
            one.since,
            one.name.clone(),
        )
    };
    declared.sort_by_key(place);
    for name in SHIPPED {
        assert!(
            declared.iter().any(|one| one.name == name),
            "{name} shipped in the manifest and no family file declares it: a released family is \
             never taken away"
        );
    }

    let mut text =
        String::from("// Generated by `build.rs` (`families`) from `src/theme/parts_*.rs`.\n\n");
    text.push_str(&modules);
    text.push_str(
        "\n/// **The closed manifest**: every family a theme may name, each declared in its own file\n\
         /// (`parts_<family>.rs`) and assembled by `build.rs`: what the first twelve versions\n\
         /// shipped in the order they shipped it, then each later family by the version it came in\n\
         /// and its name.\npub const MANIFEST: &[Family] = &[\n",
    );
    for one in &declared {
        writeln!(text, "    {}::{},", one.module, one.konst)
            .expect("writing to a String cannot fail");
    }
    text.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    for (file, body) in [
        ("theme_parts_families.rs", text),
        ("theme_parts_tests.rs", tests),
    ] {
        let target = out.join(file);
        std::fs::write(&target, body)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", target.display()));
    }
}

/// Every family `text`, the file `name` of module `module`, declares.
fn declared_in(module: &str, name: &str, text: &str) -> Vec<Declared> {
    let mut found = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some(konst) = line
            .strip_prefix("pub(super) const ")
            .and_then(|rest| rest.strip_suffix(": Family = Family {"))
        else {
            // A family declared in another shape would be left out of the manifest in silence.
            assert!(
                !line.contains(": Family = Family"),
                "{name}: {line:?} declares a family build.rs cannot read: write it \
                 `pub(super) const <NAME>: Family = Family {{` alone on its line"
            );
            continue;
        };
        let mut family_name = None;
        let mut since = None;
        for inside in lines.by_ref() {
            if inside == "};" {
                break;
            }
            if let Some(said) = inside.strip_prefix("    name: ") {
                family_name
                    .get_or_insert_with(|| said.trim_end_matches(',').trim_matches('"').to_owned());
            } else if let Some(said) = inside.strip_prefix("    since: ") {
                let said = said.trim_end_matches(',');
                since.get_or_insert_with(|| {
                    if said == "super::NEXT" {
                        u32::MAX
                    } else {
                        said.parse().unwrap_or_else(|_| {
                            panic!("{name}: {konst}'s since is {said:?}: a number or super::NEXT")
                        })
                    }
                });
            }
        }
        found.push(Declared {
            module: module.to_owned(),
            konst: konst.to_owned(),
            name: family_name
                .unwrap_or_else(|| panic!("{name}: {konst} has no `name:` of its own")),
            since: since.unwrap_or_else(|| panic!("{name}: {konst} has no `since:` of its own")),
        });
    }
    found
}
