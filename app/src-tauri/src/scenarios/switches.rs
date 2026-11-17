// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every test-only switch on a scenario's path is forced or listed** (T1.1-06; spec principle
//! 1). The scenario host is a test build: purlis-app's own `cfg(test)` is on, and purlis-core,
//! the host and the protocol are built with `fenced` and `test-support`. Each place that build
//! runs differently from what a person runs is a switch, and each is either forced to its
//! production branch by the scenario host (a test proves it) or listed in `switches.json` with
//! its reason and a named cover for each OS it applies to: the separately built host process
//! (`host-process`, Linux), ticket 54's Linux release-profile job (`linux-release-profile`), or a
//! live check in ticket 37's brief run on a macOS release build (`macos-live-check`).
//!
//! **What counts.** In purlis-app's sources, every `cfg(test)`, `cfg!(test)`, `test-support` and
//! `fenced` mention, and every use of `fence::FENCED`. In the crates it depends on, only the
//! features (`test-support`, `fenced`) and `fence::FENCED`: their own `cfg(test)` is off in the
//! scenario host's build, which links them as dependencies. Test files (`tests.rs`, `*_tests.rs`,
//! anything under a `tests` folder) and test modules' declarations are not a path a scenario
//! runs.
//!
//! **A definition is not a switch.** A function, type, field, constant or static only a test
//! build has changes no production path by itself: production cannot call it, and the code that
//! does is itself counted here. So a gated definition is inert unless it is one side of a pair
//! (`cfg(not(test))`, `cfg(not(any(test, …)))`, `cfg(not(feature = …))`), which swaps what runs
//! and is a switch.
//!
//! An entry is keyed by its file and the code line it gates (the line after the attribute, or the
//! line itself for an expression), so it survives lines moving. A switch the lint finds that the
//! list does not name fails it, and so does an entry that names nothing any more.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The roots a scenario runs through, from the repo's root.
const ROOTS: &[&str] = &[
    "app/src-tauri/src",
    "crates/purlis-core/src",
    "crates/host/src",
    "crates/session-protocol/src",
    "crates/linux-sandbox/src",
    "crates/same-user/src",
    "crates/purlis-cli/src",
];

/// The crate whose own `cfg(test)` is on in the scenario host's build.
const THIS_CRATE: &str = "app/src-tauri/src";

/// The covers an entry may name.
const COVERS: &[&str] = &["host-process", "linux-release-profile", "macos-live-check"];

/// Covers that run on each OS.
fn runs_on(cover: &str) -> &'static [&'static str] {
    match cover {
        "host-process" | "linux-release-profile" => &["linux"],
        "macos-live-check" => &["macos"],
        _ => &[],
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    /// What each cover is, in words.
    covers: BTreeMap<String, String>,
    /// What the scenario host stands in for besides the switches, each with its covers.
    substitutions: Vec<Substitution>,
    switches: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Substitution {
    name: String,
    reason: String,
    covers: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    file: String,
    at: String,
    #[serde(default = "one")]
    count: usize,
    /// `switch` or `seam`.
    kind: String,
    reason: String,
    /// How the scenario host forces its production branch, and the test that shows it.
    #[serde(default)]
    forced: Option<Forced>,
    /// The OSes it applies to; both where not said.
    #[serde(default)]
    applies: Option<Vec<String>>,
    #[serde(default)]
    covers: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Forced {
    how: String,
    test: String,
}

fn one() -> usize {
    1
}

/// One switch the scan found: where, the line it gates, and whether it is a definition alone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Found {
    pub file: String,
    pub at: String,
    pub inert: bool,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repo")
}

/// Whether `relative` is a test file, which no scenario path runs.
fn a_test_file(relative: &str) -> bool {
    let mut parts: Vec<&str> = relative.split('/').collect();
    let name = parts.pop().unwrap_or_default();
    name == "tests.rs" || name.ends_with("_tests.rs") || parts.contains(&"tests")
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Whether `line` mentions a switch, for a file of this crate (`own`) or a dependency's.
fn mentions(line: &str, own: bool) -> bool {
    let features = line.contains("feature = \"test-support\"")
        || line.contains("feature = \"fenced\"")
        || line.contains("fence::FENCED");
    features
        || (own
            && (line.contains("cfg(test)")
                || line.contains("cfg!(test)")
                || line.contains("cfg(any(test")
                || line.contains("cfg(not(test")
                || line.contains("cfg(not(any(test")
                || line.contains("cfg(all(test")))
}

/// Whether the code line `at` begins a definition.
fn a_definition(at: &str) -> bool {
    let mut rest = at;
    for prefix in ["pub(crate) ", "pub(super) ", "pub ", "async ", "const fn "] {
        rest = rest.strip_prefix(prefix).unwrap_or(rest);
    }
    [
        "fn ",
        "struct ",
        "enum ",
        "impl ",
        "impl<",
        "const ",
        "static ",
        "type ",
        "trait ",
        "use ",
        "mod ",
        "thread_local!",
        "macro_rules!",
    ]
    .iter()
    .any(|start| rest.starts_with(start))
}

/// A test module's declaration: `mod tests;`, or `mod <name> {` under a test gate.
fn a_test_module(at: &str) -> bool {
    let rest = at
        .strip_prefix("pub(crate) ")
        .or_else(|| at.strip_prefix("pub "))
        .unwrap_or(at);
    rest.starts_with("mod ") && (rest.ends_with(';') || rest.starts_with("mod tests"))
}

/// **The modules a crate's root declares for its tests alone** (`#[cfg(test)] mod x;`, or under
/// `cfg(all(test, …))`), as paths from the repo's root: `<root>/x.rs` and `<root>/x/`, or the
/// `#[path]` it names. They are the suites themselves (this one among them), which no scenario
/// path runs.
fn test_modules(repo: &Path, root: &str) -> Vec<String> {
    let mut out = Vec::new();
    for top in ["lib.rs", "main.rs"] {
        let Ok(text) = std::fs::read_to_string(repo.join(root).join(top)) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (i, line) in lines.iter().enumerate() {
            if !(line.starts_with("#[cfg(test)]") || line.starts_with("#[cfg(all(test")) {
                continue;
            }
            let mut path = None;
            for next in &lines[i + 1..] {
                if let Some(named) = next
                    .strip_prefix("#[path = \"")
                    .and_then(|rest| rest.strip_suffix("\"]"))
                {
                    path = Some(named.to_owned());
                } else if next.starts_with("#[") || next.is_empty() {
                } else {
                    let name = next
                        .trim_start_matches("pub(crate) ")
                        .trim_start_matches("pub ")
                        .strip_prefix("mod ")
                        .and_then(|rest| rest.strip_suffix(';'));
                    if let Some(name) = name {
                        let stem = path.unwrap_or_else(|| format!("{name}.rs"));
                        let dir = stem.trim_end_matches(".rs").to_owned();
                        out.push(format!("{root}/{stem}"));
                        out.push(format!("{root}/{dir}/"));
                    }
                    break;
                }
            }
        }
    }
    out
}

/// **Every switch on a scenario's path**, found in the sources under `repo`.
pub(crate) fn scan(repo: &Path) -> Vec<Found> {
    let mut found = Vec::new();
    for root in ROOTS {
        let mut files = Vec::new();
        sources(&repo.join(root), &mut files);
        files.sort();
        let suites = test_modules(repo, root);
        for file in files {
            let relative = file
                .strip_prefix(repo)
                .expect("under the repo")
                .to_string_lossy()
                .replace('\\', "/");
            if a_test_file(&relative)
                || suites.iter().any(|suite| {
                    relative == *suite || (suite.ends_with('/') && relative.starts_with(suite))
                })
            {
                continue;
            }
            let own = relative.starts_with(THIS_CRATE);
            let text = std::fs::read_to_string(&file).expect("a source");
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") || !mentions(line, own) {
                    continue;
                }
                let attribute = trimmed.starts_with("#[");
                let at = if attribute {
                    lines[i + 1..]
                        .iter()
                        .map(|l| l.trim())
                        .find(|l| !l.is_empty() && !l.starts_with("#[") && !l.starts_with("//"))
                        .unwrap_or_default()
                } else {
                    trimmed
                };
                if attribute && a_test_module(at) {
                    continue;
                }
                let pair = trimmed.contains("not(");
                found.push(Found {
                    file: relative.clone(),
                    at: at.to_owned(),
                    inert: attribute && !pair && a_definition(at),
                });
            }
        }
    }
    found
}

fn list() -> List {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/scenarios/switches.json"),
    )
    .expect("switches.json");
    serde_json::from_str(&text).expect("switches.json reads")
}

/// What is wrong with `entry`'s reason, forcing and covers, if anything.
fn checked(entry: &Entry, covers: &BTreeMap<String, String>) -> Vec<String> {
    let mut wrong = Vec::new();
    let name = format!("{} `{}`", entry.file, entry.at);
    if entry.reason.trim().len() < 12 {
        wrong.push(format!("{name}: its reason says too little"));
    }
    match entry.kind.as_str() {
        "seam" => {}
        "switch" => {
            let applies = entry
                .applies
                .clone()
                .unwrap_or_else(|| vec!["linux".to_owned(), "macos".to_owned()]);
            if let Some(forced) = &entry.forced {
                if forced.how.trim().is_empty() || forced.test.trim().is_empty() {
                    wrong.push(format!(
                        "{name}: forced, but says neither how nor by which test"
                    ));
                }
            } else {
                for os in &applies {
                    let named = entry.covers.get(os).cloned().unwrap_or_default();
                    if !named.iter().any(|c| runs_on(c).contains(&os.as_str())) {
                        wrong.push(format!("{name}: no cover that runs on {os}"));
                    }
                }
            }
            for named in entry.covers.values().flatten() {
                if !covers.contains_key(named) || !COVERS.contains(&named.as_str()) {
                    wrong.push(format!("{name}: names `{named}`, which is no cover"));
                }
            }
        }
        other => wrong.push(format!("{name}: kind `{other}` is neither switch nor seam")),
    }
    wrong
}

#[test]
fn every_test_only_switch_on_a_scenario_path_is_forced_or_listed() {
    let list = list();
    let found = scan(&repo());
    let mut counted: BTreeMap<(String, String), usize> = BTreeMap::new();
    for one in found.iter().filter(|f| !f.inert) {
        *counted
            .entry((one.file.clone(), one.at.clone()))
            .or_default() += 1;
    }
    let mut wrong = Vec::new();
    let mut listed: BTreeMap<(String, String), &Entry> = BTreeMap::new();
    for entry in &list.switches {
        let key = (entry.file.clone(), entry.at.clone());
        if listed.insert(key.clone(), entry).is_some() {
            wrong.push(format!("{} `{}` is listed twice", key.0, key.1));
        }
        wrong.extend(checked(entry, &list.covers));
    }
    let mut unlisted = Vec::new();
    for ((file, at), count) in &counted {
        match listed.get(&(file.clone(), at.clone())) {
            None => unlisted.push(serde_json::json!({
                "file": file, "at": at, "count": count, "kind": "switch", "reason": "",
                "covers": {"linux": [], "macos": []},
            })),
            Some(entry) if entry.count != *count => wrong.push(format!(
                "{file} `{at}`: its entry's count is {}, and the scan found {count}",
                entry.count
            )),
            Some(_) => {}
        }
    }
    for (file, at) in listed.keys() {
        if !counted.contains_key(&(file.clone(), at.clone())) {
            wrong.push(format!(
                "{file} `{at}` is listed, but no such switch is there now: take it off the list"
            ));
        }
    }
    for substitution in &list.substitutions {
        for os in ["linux", "macos"] {
            let named = substitution.covers.get(os).cloned().unwrap_or_default();
            if !named.iter().any(|c| runs_on(c).contains(&os)) {
                wrong.push(format!(
                    "the substitution `{}` has no cover that runs on {os}",
                    substitution.name
                ));
            }
        }
        if substitution.reason.trim().len() < 12 {
            wrong.push(format!(
                "the substitution `{}` says too little",
                substitution.name
            ));
        }
    }
    assert!(
        unlisted.is_empty() && wrong.is_empty(),
        "every test-only switch on a scenario's path is forced to production by the scenario host \
         or listed in app/src-tauri/src/scenarios/switches.json with its reason and a cover for \
         each OS.\n{}\nNot listed (each as an entry to fill in):\n{}",
        wrong.join("\n"),
        serde_json::to_string_pretty(&unlisted).expect("JSON")
    );
}

/// The lint finds what it is meant to: a switch in a body, a pair's other side, an expression,
/// and a dependency's feature, and it leaves a definition, a test module and a dependency's own
/// `cfg(test)` alone.
#[test]
fn the_switch_scan_tells_a_switch_from_a_definition() {
    let repo = tempfile::tempdir().expect("a repo");
    let app = repo.path().join(THIS_CRATE);
    let core = repo.path().join("crates/purlis-core/src");
    std::fs::create_dir_all(&app).expect("the app");
    std::fs::create_dir_all(&core).expect("the core");
    std::fs::write(
        app.join("one.rs"),
        "#[cfg(test)]\nmod tests;\n#[cfg(test)]\nfn helper() {}\nfn body() {\n    #[cfg(test)]\n    \
         let x = 1;\n    #[cfg(not(test))]\n    let x = 2;\n    if cfg!(test) {}\n}\n",
    )
    .expect("a file");
    std::fs::write(
        core.join("two.rs"),
        "#[cfg(test)]\nfn only_its_own_tests() {}\nfn body() {\n    #[cfg(any(test, feature = \
         \"test-support\"))]\n    let y = 1;\n    if crate::fence::FENCED {}\n}\n",
    )
    .expect("a file");
    let found = scan(repo.path());
    let switches: Vec<(&str, &str)> = found
        .iter()
        .filter(|f| !f.inert)
        .map(|f| (f.file.as_str(), f.at.as_str()))
        .collect();
    assert_eq!(
        switches,
        [
            ("app/src-tauri/src/one.rs", "let x = 1;"),
            ("app/src-tauri/src/one.rs", "let x = 2;"),
            ("app/src-tauri/src/one.rs", "if cfg!(test) {}"),
            ("crates/purlis-core/src/two.rs", "let y = 1;"),
            (
                "crates/purlis-core/src/two.rs",
                "if crate::fence::FENCED {}"
            ),
        ]
    );
    assert!(
        found.iter().any(|f| f.inert && f.at == "fn helper() {}"),
        "{found:?}"
    );
}
