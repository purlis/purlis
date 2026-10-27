// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every write of a file one of the window's lanes reads is in the window's table** (#1949).
//!
//! The window keeps a read and a change of the same file in order only for the commands its
//! lanes hold (`app/src/commandOrder.ts`), and which commands write which file is
//! `app/src/writeSites.json`. This guard reads the crate's own code (its test modules skipped,
//! brace by brace) and holds the table to it:
//!
//! - **every call that writes such a file is a listed site**, by the file and the function it is
//!   in. A write is a call of one of the writers named below; for `sandbox::local`, every
//!   function that is not one of its known reads, so a new primitive there counts as a write;
//! - **every command that reaches a site within two calls is in that site's `by`**: a qualified
//!   call (`module::function(`) to a function of that module's file, or a plain call to a
//!   function of the same file. A command that writes through a method (`held.x().y(`), or
//!   further down than two calls, is not followed: the table's `by` is still the word there.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

/// The calls that write a file one of the window's lanes reads (`app/src/commandOrder.ts`), by
/// the module and the functions each is spelled with: each is `<module>::<function>(`.
const LANE_FILE_WRITES: &[(&str, &[&str])] = &[
    ("settings", &["save", "move_keys"]),
    // A proposed change, applied (#1975): it may be the project's manifest.
    ("proposed", &["write_pinned"]),
    ("forges", &["add", "remove"]),
    (
        "harness_profiles",
        &["add", "remove", "rename", "rename_everywhere"],
    ),
    ("hosts", &["add", "remove", "confirm"]),
    // The project's browser sites (#2443): the manifest, and this machine's record of them.
    ("browser_sites", &["grant", "revoke", "acknowledge"]),
    (
        "dispatchgrant",
        &[
            "never",
            "never_said_as",
            "lift_never",
            "allow_any",
            "revoke_any",
            "accept_any_of_the_project",
            "decline",
            "acknowledge",
            "acknowledge_pair",
            "acknowledge_any",
            "forget_pair",
        ],
    ),
    (
        "dispatchwithin",
        &[
            "grant_yours",
            "revoke_yours",
            "accept",
            "unaccept",
            "set_yours",
        ],
    ),
    (
        "dispatchdormant",
        &["noticed", "judged", "persona_gone", "give_back", "remove"],
    ),
    ("dispatcharrival", &["told_gone"]),
    (
        "machine",
        &[
            "update",
            "forget_project",
            "revoke_approval",
            "locate_project",
        ],
    ),
];

/// The same, where any function of the module whose name begins so writes: `<module>::<start>…(`.
const LANE_FILE_WRITES_BEGINNING: &[(&str, &[&str])] = &[
    ("away", &["dismiss", "mark", "forget"]),
    ("network", &["record_"]),
    ("log", &["append", "link", "unlink"]),
];

/// `sandbox::local`'s functions that only read. Every other function of it writes this
/// machine's sandbox file, so one added later is a write until it is listed here.
const LOCAL_READS: &[&str] = &[
    "allowed_persona_hosts",
    "browser_always",
    "browser_always_here",
    "browsers_always",
    "browser_site_held",
    "browser_site_held_at",
    "browser_sites",
    "browser_sites_in_force",
    "browser_sites_seen",
    "confirmed_hosts",
    "confirmed_levels",
    "dispatch_mine_in",
    "grantable_folders",
    "granted_listens",
    "granted_ports",
    "granted_services",
    "granted_sockets",
    "granted_vaults",
    "granted_writes",
    "hosts_changed",
    "made",
    "made_everywhere",
    "offer_due",
    "presets_changed",
    "services_everywhere",
    "sockets_everywhere",
    "tally",
];

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The identifier that ends right before byte `at` of `line`, if any.
fn ident_before(line: &str, at: usize) -> &str {
    let head = &line[..at];
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_ident(*c))
        .last()
        .map_or(at, |(i, _)| i);
    &head[start..]
}

/// Each `<module>::<name>(` in `line`, as (module, name), the module not part of a longer name.
fn qualified_calls(line: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    for (at, _) in line.match_indices('(') {
        let name = ident_before(line, at);
        if name.is_empty() {
            continue;
        }
        let before_name = at - name.len();
        if !line[..before_name].ends_with("::") {
            continue;
        }
        let module = ident_before(line, before_name - 2);
        if !module.is_empty() {
            out.push((module, name));
        }
    }
    out
}

/// Each plain call `<name>(` in `line`: not a method (`.name(`), not qualified (`x::name(`), and
/// not a macro or a definition.
fn plain_calls(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for (at, _) in line.match_indices('(') {
        let name = ident_before(line, at);
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let before = line[..at - name.len()].trim_end();
        if before.ends_with('.') || before.ends_with("::") || before.ends_with("fn") {
            continue;
        }
        out.push(name);
    }
    out
}

/// Whether `line` writes a file one of the window's lanes reads.
fn writes_a_lane_file(line: &str) -> bool {
    if line.contains("audit_then_record(") {
        return true;
    }
    qualified_calls(line).into_iter().any(|(module, name)| {
        (module == "local" && !LOCAL_READS.contains(&name))
            || LANE_FILE_WRITES
                .iter()
                .any(|(m, names)| *m == module && names.contains(&name))
            || LANE_FILE_WRITES_BEGINNING
                .iter()
                .any(|(m, starts)| *m == module && starts.iter().any(|s| name.starts_with(s)))
    })
}

/// `line` with what is inside its string and character literals and after `//` taken out, so
/// its braces and calls are the code's own.
fn code_of(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            match c {
                '\\' => {
                    chars.next();
                }
                '"' => {
                    in_string = false;
                    out.push('"');
                }
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push('"');
            }
            '/' if chars.peek() == Some(&'/') => break,
            '\'' => {
                // A character literal is at most a few characters; a lifetime is a name.
                let rest: String = chars.clone().take(4).collect();
                if let Some(end) = rest.find('\'')
                    && end <= 2
                {
                    for _ in 0..=end {
                        chars.next();
                    }
                    out.push_str("' '");
                    continue;
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// `text` with what is inside its block comments (`/* … */`, nested) and its raw strings
/// (`r"…"`, `r#"…"#`) blanked, lines kept where they were: what spans lines, which a reading a
/// line at a time cannot tell from code. Ordinary strings and line comments are passed over, so a
/// `/*` or an `r"` inside one starts nothing.
fn without_block_comments_and_raw_strings(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // A line comment: passed over to its end, as it is.
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        // A block comment, nested as Rust nests them.
        if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            continue;
        }
        // A raw string: `r`, any `#`s, a quote, not part of a longer name.
        let starts_raw = c == 'r'
            && !(i > 0 && (chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_'))
            && {
                let mut j = i + 1;
                while chars.get(j) == Some(&'#') {
                    j += 1;
                }
                chars.get(j) == Some(&'"')
            };
        if starts_raw {
            let mut hashes = 0;
            out.push('r');
            i += 1;
            while chars.get(i) == Some(&'#') {
                hashes += 1;
                out.push('#');
                i += 1;
            }
            out.push('"');
            i += 1;
            while i < chars.len() {
                let closes =
                    chars[i] == '"' && (1..=hashes).all(|n| chars.get(i + n) == Some(&'#'));
                if closes {
                    out.push('"');
                    i += 1;
                    for _ in 0..hashes {
                        out.push('#');
                        i += 1;
                    }
                    break;
                }
                out.push(blank(chars[i]));
                i += 1;
            }
            continue;
        }
        // A character literal (`'"'`, `'\''`): passed over, so its quote starts no string. A
        // lifetime (`'a`) has no closing quote this close, and is left as it is.
        if c == '\'' {
            let closing = if next == Some('\\') {
                (i + 3..(i + 12).min(chars.len())).find(|&j| chars[j] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(end) = closing {
                out.extend(&chars[i..=end]);
                i = end + 1;
                continue;
            }
        }
        // An ordinary string: passed over whole, escapes and all.
        if c == '"' {
            out.push(c);
            i += 1;
            while i < chars.len() {
                let d = chars[i];
                out.push(d);
                i += 1;
                if d == '\\' {
                    if let Some(&e) = chars.get(i) {
                        out.push(e);
                        i += 1;
                    }
                } else if d == '"' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// One function of a file: its name, whether it is a window command, and its lines.
struct Function {
    name: String,
    command: bool,
    from: usize,
    to: usize,
}

/// A file's code lines (literals and comments out) with its test modules blanked, brace by
/// brace, and its functions.
fn read_file(text: &str) -> (Vec<String>, Vec<Function>) {
    let text = without_block_comments_and_raw_strings(text);
    let mut lines: Vec<String> = text.lines().map(code_of).collect();
    // A module under `#[cfg(test)]` is blanked to its closing brace, and only it: what follows
    // it in the file is the crate's own code again.
    let mut at = 0;
    while at < lines.len() {
        if lines[at].trim_start().starts_with("#[cfg(test)]") {
            let mut next = at + 1;
            while next < lines.len() && lines[next].trim_start().starts_with("#[") {
                next += 1;
            }
            let opens = lines.get(next).is_some_and(|l| {
                let l = l.trim_start();
                (l.starts_with("mod ") || l.contains(" mod ")) && l.contains('{')
            });
            if opens {
                let mut depth = 0i64;
                let mut end = next;
                for (i, line) in lines.iter().enumerate().skip(next) {
                    depth += line.matches('{').count() as i64;
                    depth -= line.matches('}').count() as i64;
                    end = i;
                    if depth <= 0 {
                        break;
                    }
                }
                for line in &mut lines[at..=end] {
                    line.clear();
                }
                at = end + 1;
                continue;
            }
        }
        at += 1;
    }
    let mut functions = Vec::new();
    for (start, line) in lines.iter().enumerate() {
        let mut sig = line.trim_start();
        for prefix in [
            "pub(crate) ",
            "pub(super) ",
            "pub ",
            "const ",
            "async ",
            "unsafe ",
        ] {
            sig = sig.strip_prefix(prefix).unwrap_or(sig);
        }
        let Some(rest) = sig.strip_prefix("fn ") else {
            continue;
        };
        let name: String = rest.chars().take_while(|c| is_ident(*c)).collect();
        if name.is_empty() {
            continue;
        }
        let command = lines[start.saturating_sub(4)..start]
            .iter()
            .any(|l| l.contains("#[tauri::command"));
        // Its body: from its first brace to the one that closes it.
        let mut depth = 0i64;
        let mut opened = false;
        let mut to = start;
        for (i, l) in lines.iter().enumerate().skip(start) {
            if !opened && depth == 0 && l.contains(';') && !l.contains('{') {
                break;
            }
            let opens = l.matches('{').count() as i64;
            depth += opens;
            depth -= l.matches('}').count() as i64;
            opened |= opens > 0;
            to = i;
            if opened && depth <= 0 {
                break;
            }
        }
        if opened {
            functions.push(Function {
                name,
                command,
                from: start,
                to,
            });
        }
    }
    (lines, functions)
}

/// What the guard reads of the crate: each write site, as `<file> <function>`, and each command
/// with the sites it reaches within two calls.
struct Read {
    sites: BTreeSet<String>,
    reached: BTreeMap<String, BTreeSet<String>>,
}

/// Reads `files`, each as (its path under `src`, its text).
fn read_crate(files: &[(String, String)]) -> Read {
    struct Known {
        file: String,
        name: String,
        command: bool,
        lines: Vec<String>,
    }
    let mut known: Vec<Known> = Vec::new();
    let mut sites = BTreeSet::new();
    for (file, text) in files {
        let (lines, functions) = read_file(text);
        for (at, line) in lines.iter().enumerate() {
            if !writes_a_lane_file(line) {
                continue;
            }
            // The innermost function holding the line.
            let holder = functions
                .iter()
                .filter(|f| f.from <= at && at <= f.to)
                .max_by_key(|f| f.from)
                .map_or("(none)", |f| f.name.as_str());
            sites.insert(format!("{file} {holder}"));
        }
        for f in &functions {
            known.push(Known {
                file: file.clone(),
                name: f.name.clone(),
                command: f.command,
                lines: lines[f.from..=f.to].to_vec(),
            });
        }
    }
    let module_of = |file: &str| -> String {
        file.trim_end_matches(".rs")
            .trim_end_matches("/mod")
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    // What one function calls that the crate defines: by `module::name(` to that module's file,
    // or by `name(` to its own file.
    let callees = |of: &Known| -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        for line in &of.lines[1..] {
            for (module, name) in qualified_calls(line) {
                let module = if module == "Self" || module == "self" {
                    module_of(&of.file)
                } else {
                    module.to_owned()
                };
                for (i, k) in known.iter().enumerate() {
                    if k.name == name && module_of(&k.file) == module {
                        out.insert(i);
                    }
                }
            }
            for name in plain_calls(line) {
                for (i, k) in known.iter().enumerate() {
                    if k.name == name && k.file == of.file {
                        out.insert(i);
                    }
                }
            }
        }
        out
    };
    let mut reached = BTreeMap::new();
    for (i, k) in known.iter().enumerate() {
        if !k.command {
            continue;
        }
        let first = callees(k);
        let mut all: BTreeSet<usize> = first.clone();
        all.insert(i);
        for j in first {
            all.extend(callees(&known[j]));
        }
        let at: BTreeSet<String> = all
            .into_iter()
            .map(|j| format!("{} {}", known[j].file, known[j].name))
            .filter(|site| sites.contains(site))
            .collect();
        if !at.is_empty() {
            reached.insert(k.name.clone(), at);
        }
    }
    Read { sites, reached }
}

/// A command's name as the window's bindings spell it.
fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in snake.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// What is wrong with `table` against what was `read`, one sentence each.
fn faults(table: &Value, read: &Read) -> Vec<String> {
    let mut faults = Vec::new();
    let sites = table["sites"].as_array().cloned().unwrap_or_default();
    let mut by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // The sites that write a lane's file: the others hold no command to its lane.
    let mut into_a_lane: BTreeSet<String> = BTreeSet::new();
    for site in &sites {
        let at = site["at"].as_str().unwrap_or_default().to_owned();
        let named: BTreeSet<String> = site["by"]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(|c| c.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let why = site["why"].as_str().is_some_and(|why| !why.is_empty());
        if (site["writes"].is_null() || named.is_empty()) && !why {
            faults.push(format!(
                "{at}: written by no window command, or into no lane's file, and no `why` says so"
            ));
        }
        if !site["writes"].is_null() {
            into_a_lane.insert(at.clone());
        }
        by.entry(at).or_default().extend(named);
    }
    for site in &read.sites {
        if !by.contains_key(site) {
            faults.push(format!(
                "{site} writes a lane's file and is not in the table"
            ));
        }
    }
    for at in by.keys() {
        if !at.starts_with("memory:") && !read.sites.contains(at) {
            faults.push(format!("{at} is in the table and writes no lane's file"));
        }
    }
    for (command, reaches) in &read.reached {
        for site in reaches {
            if into_a_lane.contains(site)
                && by
                    .get(site)
                    .is_some_and(|named| !named.contains(&camel(command)))
            {
                faults.push(format!(
                    "{} reaches {site}, which writes a lane's file, and is not in its `by`",
                    camel(command)
                ));
            }
        }
    }
    faults
}

/// The crate's own sources, by their path under `src`; its test files left out, a module's
/// `tests/` folder whole.
fn the_crates_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("the crate's sources") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, root, out);
                continue;
            }
            let file = path
                .strip_prefix(root)
                .expect("under the sources")
                .to_string_lossy()
                .into_owned();
            // Read under `src` alone: a checkout whose own path has a `tests` folder in it is
            // still read.
            let in_tests = file.split('/').any(|part| part == "tests");
            if !file.ends_with(".rs")
                || file.ends_with("_tests.rs")
                || file == "tests.rs"
                || file.ends_with("/tests.rs")
                || in_tests
            {
                continue;
            }
            out.push((file, std::fs::read_to_string(&path).expect("a source file")));
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    assert!(
        !out.is_empty(),
        "no source of the crate's was read under {}",
        root.display()
    );
    out.sort();
    out
}

fn the_table() -> Value {
    serde_json::from_str(include_str!("../../../../src/writeSites.json"))
        .expect("the table is JSON")
}

#[test]
fn every_write_of_a_lanes_file_and_every_command_that_reaches_one_is_in_the_window_s_table() {
    let read = read_crate(&the_crates_sources());
    let faults = faults(&the_table(), &read);
    assert!(
        faults.is_empty(),
        "app/src/writeSites.json does not hold what the crate writes:\n{}",
        faults.join("\n")
    );
}

/// A file of the crate, as a fixture.
fn a_file(path: &str, text: &str) -> (String, String) {
    (path.to_owned(), text.to_owned())
}

#[test]
fn a_new_command_that_writes_through_a_listed_helper_is_caught() {
    // The shape the guard exists for: a command added later that writes a lane's file through
    // a helper the table already lists, under a command it does not name.
    let files = [a_file(
        "settings.rs",
        r#"
pub(crate) fn remove_host(root: &Path, id: &str) -> Result<(), String> {
    match settings::hosts::remove(root, Which::Local, None, id) {
        Ok(_) => Ok(()),
        Err(why) => Err(why.to_string()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn reset_sandbox_hosts(planes: State<'_, Planes>, plane: PlaneId) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    for id in listed(&root) {
        remove_host(&root, &id)?;
    }
    Ok(())
}
"#,
    )];
    let table = serde_json::json!({
        "sites": [{ "at": "settings.rs remove_host", "writes": "x", "by": ["removeSandboxHost"] }]
    });
    let faults = faults(&table, &read_crate(&files));
    assert_eq!(
        faults,
        [
            "resetSandboxHosts reaches settings.rs remove_host, which writes a lane's file, and is \
          not in its `by`"
        ]
    );
}

#[test]
fn code_after_a_test_module_is_read_and_the_test_module_is_not() {
    let files = [a_file(
        "watchset.rs",
        r#"
pub fn first() {}

#[cfg(test)]
mod refuse {
    fn in_a_test() {
        machine::update(&config, |_| {});
        let s = "{ an unclosed brace in a string";
    }
}

pub fn forget(config: &Path) {
    let _ = machine::update(config, |store| store.windows.clear());
}
"#,
    )];
    let read = read_crate(&files);
    assert_eq!(
        read.sites.into_iter().collect::<Vec<_>>(),
        ["watchset.rs forget"]
    );
}

#[test]
fn a_new_function_of_the_sandboxs_local_file_counts_as_a_write_and_its_reads_do_not() {
    assert!(writes_a_lane_file(
        "    sandbox::local::wipe_every_grant(root)?;"
    ));
    assert!(!writes_a_lane_file(
        "    let seen = sandbox::local::hosts_changed(root);"
    ));
    // Another module whose name ends in `local` is not it.
    assert!(!writes_a_lane_file(
        "    renamelocal::finish(&local, &seams, waiting)"
    ));
}

#[test]
fn a_row_no_command_writes_says_why() {
    let table = serde_json::json!({
        "sites": [{ "at": "planes.rs open", "writes": "x", "by": [] }]
    });
    let read = Read {
        sites: ["planes.rs open".to_owned()].into_iter().collect(),
        reached: BTreeMap::new(),
    };
    assert_eq!(
        faults(&table, &read),
        [
            "planes.rs open: written by no window command, or into no lane's file, and no `why` \
          says so"
        ]
    );
}

#[test]
fn what_the_sandboxs_local_file_records_as_seen_is_a_write() {
    // `hosts_seen_by_you` and `presets_seen_by_you` acknowledge what the window wrote, which
    // writes this machine's sandbox file: a lane's read of it waits for them.
    assert!(writes_a_lane_file(
        "        purlis_core::sandbox::local::hosts_seen_by_you(root, self.hosts, written);"
    ));
    assert!(writes_a_lane_file(
        "        purlis_core::sandbox::local::presets_seen_by_you(root, self.presets, written);"
    ));
}

#[test]
fn a_call_inside_a_block_comment_or_a_raw_string_is_not_read() {
    let files = [a_file(
        "notes.rs",
        r###"
pub fn nothing_here() {
    /* an old way, kept for the record:
       machine::update(config, |store| store.windows.clear());
    */
    let example = r#"
        machine::update(config, |_| {});
    "#;
    let _ = example;
}

pub fn this_one(config: &Path) {
    let _ = machine::update(config, |store| store.windows.clear());
}
"###,
    )];
    assert_eq!(
        read_crate(&files).sites.into_iter().collect::<Vec<_>>(),
        ["notes.rs this_one"]
    );
}
