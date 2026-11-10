// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a yes to an ask risks, from the project's own facts** (ADR 0121 decision 8, #6011;
//! takes over #1046's protected push and production host).
//!
//! Three classes, each read from what the ask already carries and what the project already
//! knows, never from what a chat says of itself:
//!
//! - **A push to a protected branch** ([`Class::ProtectedPush`]): a `git push` whose refspec
//!   names one, or that pushes every branch. A branch is protected where it is a repo's default
//!   branch in `inventory/repos.json`, or matches `[risk] protected_branches` in `charter.toml`
//!   (a name, or a pattern with `*`: `release/*`). A push that names no branch pushes the
//!   chat's current one, which the ask does not say, so it is not classed.
//! - **A host that looks like production** ([`Class::ProductionHost`]): `prod` or `production`
//!   as a label of the host (`api.prod.example.com`, `prod-db.internal`), or a host that
//!   `[risk] production_hosts` names, or a subdomain of one. Read from a sandbox's host, and from
//!   every URL a command line or a tool's input holds.
//! - **A write outside the project** ([`Class::OutsideProject`]): a sandbox's folder, or an
//!   edit's file, that is not under the project's root. Read on the path as written (`~` is the
//!   home folder, `..` is taken back), never by following links on disk.
//!
//! A high-risk ask is shown with its [`Risk::why`], never grouped, never offered as a rule, and
//! its Allow asks once more ([`Risk::confirm`]) before it acts. Pure but for [`Facts::read`].

use std::path::{Component, Path, PathBuf};

use crate::shellseg;
use crate::shellwrap::{self, base_lower};

/// The table of `charter.toml` the project's own risk facts are in.
pub const TABLE: &str = "risk";
/// The key in [`TABLE`] naming more protected branches.
pub const PROTECTED_BRANCHES: &str = "protected_branches";
/// The key in [`TABLE`] naming the project's production hosts.
pub const PRODUCTION_HOSTS: &str = "production_hosts";

/// How many shells inside shells (`sh -c '…'`) a line is read through.
const SCRIPT_DEPTH: usize = 4;

/// Which risk an ask carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    ProtectedPush,
    ProductionHost,
    OutsideProject,
}

/// **An ask's risk, as its row says it.** Both lines are purlis's words around the ask's own
/// branch, host or path, which are data.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Risk {
    pub class: Class,
    /// What the row says of it: "Pushes to main, a protected branch".
    pub why: String,
    /// What its Allow asks before it acts: "Allow a push to main?".
    pub confirm: String,
}

/// **What the project knows that an ask's risk is read against.**
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The project's root, as given and as resolved on disk, where they differ.
    roots: Vec<PathBuf>,
    /// The home folder `~` stands for.
    home: Option<PathBuf>,
    /// Protected branches: names, or patterns with `*`.
    protected: Vec<String>,
    /// Hosts the project marks as production, lowercase.
    production: Vec<String>,
}

impl Facts {
    /// The facts of the project at `root`: its repos' default branches, its `[risk]` table, and
    /// where it is. A file that cannot be read adds nothing.
    pub fn read(root: &Path) -> Self {
        let mut protected: Vec<String> = crate::inventory::load(root, "")
            .map(|doc| crate::inventory::listed(&doc))
            .unwrap_or_default()
            .iter()
            .filter_map(|repo| repo.get("default_branch")?.as_str().map(str::to_owned))
            .filter(|branch| !branch.is_empty())
            .collect();
        let top = std::fs::read_to_string(root.join(crate::profiles::COMMITTED_FILE))
            .ok()
            .and_then(|text| text.parse::<toml::Table>().ok());
        let table = top.as_ref().and_then(|top| top.get(TABLE)?.as_table());
        protected.extend(strings(table, PROTECTED_BRANCHES));
        let production = strings(table, PRODUCTION_HOSTS);
        Self::new(root, crate::profiles::home(), protected, production)
    }

    /// The facts as given: what [`Self::read`] gathers, and what a test names.
    pub fn new(
        root: &Path,
        home: Option<PathBuf>,
        protected: impl IntoIterator<Item = String>,
        production: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut roots = vec![lexical(root)];
        if let Ok(real) = root.canonicalize()
            && !roots.contains(&real)
        {
            roots.push(real);
        }
        let mut protected: Vec<String> = protected.into_iter().collect();
        protected.sort();
        protected.dedup();
        Self {
            roots,
            home,
            protected,
            production: production
                .into_iter()
                .map(|host| host.trim().trim_end_matches('.').to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect(),
        }
    }

    /// **The risk of running `line`**: a push to a protected branch, then a production host
    /// any URL in it reaches.
    pub fn of_command(&self, line: &str) -> Option<Risk> {
        self.of_line(line, 0)
    }

    /// **The risk of a tool's call**, from its input: a production host any URL in it reaches.
    pub fn of_tool(&self, input: &str) -> Option<Risk> {
        let value: serde_json::Value = serde_json::from_str(input).ok()?;
        let mut found = None;
        each_string(&value, &mut |text| {
            if found.is_none() {
                found = self.of_words(std::iter::once(text));
            }
        });
        found
    }

    /// **The risk of reaching `host`** (a sandbox's host, with or without its port).
    pub fn of_host(&self, host: &str) -> Option<Risk> {
        let host = bare_host(host);
        if host.is_empty() {
            return None;
        }
        let why = if self.production.iter().any(|named| within(&host, named)) {
            format!("Reaches {host}, a production host of this project")
        } else if host
            .split(['.', '-'])
            .any(|label| label == "prod" || label == "production")
        {
            format!("Reaches {host}, a host that looks like production")
        } else {
            return None;
        };
        Some(Risk {
            class: Class::ProductionHost,
            why,
            confirm: format!("Allow reaching {host}?"),
        })
    }

    /// **The risk of writing at `path`** (a sandbox's folder, an edit's file). A relative path
    /// is the chat's own folder's, which is in the project.
    pub fn of_write(&self, path: &str) -> Option<Risk> {
        let at = match path.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                self.home.as_ref()?.join(rest.trim_start_matches('/'))
            }
            _ => PathBuf::from(path),
        };
        if !at.is_absolute() {
            return None;
        }
        let at = lexical(&at);
        if self.roots.iter().any(|root| at.starts_with(root)) {
            return None;
        }
        Some(Risk {
            class: Class::OutsideProject,
            why: format!("Writes outside the project, in {path}"),
            confirm: "Allow a write outside the project?".to_owned(),
        })
    }

    /// Every command `line` runs, read for a protected push, then for a production host.
    fn of_line(&self, line: &str, depth: usize) -> Option<Risk> {
        let segments = shellseg::segment_argv(line);
        for toks in &segments {
            let it = shellwrap::split_env_chdir(toks);
            if base_lower(&it.prog) == "git"
                && let Some(risk) = self.of_git(it.argv.get(1..).unwrap_or(&[]))
            {
                return Some(risk);
            }
            if depth < SCRIPT_DEPTH {
                for script in shellwrap::shell_scripts_of(&it.prog, &it.argv) {
                    if let Some(risk) = self.of_line(&script, depth + 1) {
                        return Some(risk);
                    }
                }
            }
        }
        self.of_words(segments.iter().flatten().map(String::as_str))
    }

    /// The first production host a URL among `words` reaches.
    fn of_words<'a>(&self, words: impl Iterator<Item = &'a str>) -> Option<Risk> {
        words
            .filter_map(url_host)
            .find_map(|host| self.of_host(&host))
    }

    /// `git`'s arguments: a push to a protected branch.
    fn of_git(&self, args: &[String]) -> Option<Risk> {
        /// `git`'s own options that take the next word as their value.
        const GIT_VALUED: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace"];
        /// `git push`'s options that take the next word as their value.
        const PUSH_VALUED: &[&str] = &["--repo", "-o", "--push-option", "--receive-pack", "--exec"];
        let mut at = 0;
        while let Some(word) = args.get(at) {
            if GIT_VALUED.contains(&word.as_str()) {
                at += 2;
            } else if word.starts_with('-') {
                at += 1;
            } else {
                break;
            }
        }
        if args.get(at).map(String::as_str) != Some("push") {
            return None;
        }
        let rest = &args[at + 1..];
        let mut words = Vec::new();
        let mut every = false;
        let mut skip = false;
        let mut options = true;
        for word in rest {
            if std::mem::take(&mut skip) {
                continue;
            }
            if options && word == "--" {
                options = false;
            } else if options && PUSH_VALUED.contains(&word.as_str()) {
                skip = true;
            } else if options && matches!(word.as_str(), "--all" | "--mirror" | "--branches") {
                every = true;
            } else if !(options && word.starts_with('-')) {
                words.push(word.as_str());
            }
        }
        if every && !self.protected.is_empty() {
            return Some(Risk {
                class: Class::ProtectedPush,
                why: "Pushes every branch, protected ones included".to_owned(),
                confirm: "Allow a push of every branch?".to_owned(),
            });
        }
        // The first word is the remote; each after it is a refspec.
        let branch = words.iter().skip(1).find_map(|refspec| {
            let spec = refspec.trim_start_matches('+');
            let dst = spec.rsplit_once(':').map_or(spec, |(_, dst)| dst);
            let name = dst.strip_prefix("refs/heads/").unwrap_or(dst);
            (!name.starts_with("refs/") && self.protects(name)).then_some(name)
        })?;
        Some(Risk {
            class: Class::ProtectedPush,
            why: format!("Pushes to {branch}, a protected branch"),
            confirm: format!("Allow a push to {branch}?"),
        })
    }

    /// Whether `branch` is protected: named, or matched by a pattern with `*`.
    fn protects(&self, branch: &str) -> bool {
        !branch.is_empty()
            && branch != "HEAD"
            && self.protected.iter().any(|one| matches(one, branch))
    }
}

/// The strings of `table`'s list at `key`; anything else in it is ignored.
fn strings(table: Option<&toml::Table>, key: &str) -> Vec<String> {
    table
        .and_then(|table| table.get(key)?.as_array())
        .into_iter()
        .flatten()
        .filter_map(|one| one.as_str().map(str::to_owned))
        .collect()
}

/// Whether `pattern` (a name, `*` standing for any run of characters) matches `name` whole.
fn matches(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut left) = name.strip_prefix(first) else {
        return false;
    };
    let rest: Vec<&str> = parts.collect();
    let Some((last, middle)) = rest.split_last() else {
        return left.is_empty();
    };
    for part in middle {
        match left.find(part) {
            Some(at) => left = &left[at + part.len()..],
            None => return false,
        }
    }
    left.len() >= last.len() && left.ends_with(last)
}

/// Whether `host` is `named` or one of its subdomains.
fn within(host: &str, named: &str) -> bool {
    host == named
        || host
            .strip_suffix(named)
            .is_some_and(|sub| sub.ends_with('.'))
}

/// `host` without its port, brackets or trailing dot, lowercase.
fn bare_host(host: &str) -> String {
    let host = host.trim();
    let host = if let Some(inner) = host.strip_prefix('[') {
        inner.split(']').next().unwrap_or_default()
    } else if host.matches(':').count() == 1 {
        host.split(':').next().unwrap_or_default()
    } else {
        host
    };
    host.trim_end_matches('.').to_ascii_lowercase()
}

/// The host of the URL `word` holds (`https://api.prod.example.com/v1`), if it holds one.
fn url_host(word: &str) -> Option<String> {
    let (_, after) = word.split_once("://")?;
    let authority = after.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = bare_host(host);
    (!host.is_empty()).then_some(host)
}

/// Each string in `value`, however deep.
fn each_string(value: &serde_json::Value, found: &mut impl FnMut(&str)) {
    match value {
        serde_json::Value::String(text) => found(text),
        serde_json::Value::Array(all) => all.iter().for_each(|one| each_string(one, found)),
        serde_json::Value::Object(all) => all.values().for_each(|one| each_string(one, found)),
        _ => {}
    }
}

/// `path` with `.` dropped and `..` taken back, on its words alone.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests;
