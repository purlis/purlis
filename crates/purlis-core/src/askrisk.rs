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
//! - **A delete** ([`Class::Delete`]): what the ask's own command or tool plainly says deletes,
//!   and nothing guessed at: `rm`, `rmdir`, `unlink`, `git rm` (but `--cached`), `find …
//!   -delete`, `git branch -d`/`-D`, `git push --delete` or a `:branch` refspec, and an edit a
//!   harness's structured protocol names a delete (ACP's `delete` kind, [`delete_of`]).
//! - **A vault use** ([`Class::VaultUse`], [`vault_use`]): an ask whose Allow hands a stored
//!   credential from a purlis vault to a chat's program, a page or a pool's runner. The window's
//!   registry knows which asks those are; this says them.
//! - **A merge** ([`Class::Merge`], [`merge_of`]; Q239): a merge a chat's hook held, which the
//!   app performs on the person's press and which has no undo.
//!
//! A push that names no branch (`git push`, `git push origin`, `git push origin HEAD`) is
//! classed by the branch it would push, where the caller knows it ([`Pushes`]); a line that
//! moves to another folder first (`cd`, `--git-dir`) is not, since which branch is then is not
//! what the chat's folder says.
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
    Delete,
    VaultUse,
    Merge,
}

/// **What a push that names no branch would push** (#6178), as the chat's folder says: its
/// current branch, and the branch on the remote a push sends it to, where git knows one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pushes {
    /// The branch checked out.
    pub current: Option<String>,
    /// The branch on the remote a bare `git push` sends it to, where that is not its own name.
    pub to: Option<String>,
}

/// **Where a push that names no branch would land**, for the folder `-C` names (`""` for none):
/// the caller reads the chat's folder ([`pushes_from`]). A test names it.
pub type PushesIn<'a> = &'a dyn Fn(&str) -> Option<Pushes>;

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
        self.of_line(line, 0, None)
    }

    /// **The risk of running `line` in a chat's folder** (#6178): as [`Self::of_command`], and a
    /// push that names no branch is read by the branch `pushes` says it would push.
    pub fn of_command_in(&self, line: &str, pushes: PushesIn<'_>) -> Option<Risk> {
        self.of_line(line, 0, Some(pushes))
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

    /// Every command `line` runs, read for a protected push or a delete, then for a production
    /// host. `pushes` is dropped once the line moves to another folder.
    fn of_line(&self, line: &str, depth: usize, mut pushes: Option<PushesIn<'_>>) -> Option<Risk> {
        let segments = shellseg::segment_argv(line);
        for toks in &segments {
            let it = shellwrap::split_env_chdir(toks);
            let prog = base_lower(&it.prog);
            if matches!(prog.as_str(), "cd" | "pushd" | "popd") || !it.chdir.is_empty() {
                pushes = None;
            }
            let args = it.argv.get(1..).unwrap_or(&[]);
            if prog == "git"
                && let Some(risk) = self.of_git(args, pushes)
            {
                return Some(risk);
            }
            if let Some(risk) = deletes(&prog, args) {
                return Some(risk);
            }
            if depth < SCRIPT_DEPTH {
                for script in shellwrap::shell_scripts_of(&it.prog, &it.argv) {
                    if let Some(risk) = self.of_line(&script, depth + 1, pushes) {
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

    /// `git`'s arguments: a push to a protected branch, then a branch deleted.
    fn of_git(&self, args: &[String], mut pushes: Option<PushesIn<'_>>) -> Option<Risk> {
        /// `git`'s own options that take the next word as their value.
        const GIT_VALUED: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace"];
        /// `git push`'s options that take the next word as their value.
        const PUSH_VALUED: &[&str] = &["--repo", "-o", "--push-option", "--receive-pack", "--exec"];
        let mut at = 0;
        let mut folder = String::new();
        while let Some(word) = args.get(at) {
            if GIT_VALUED.contains(&word.as_str()) {
                match word.as_str() {
                    "-C" => folder = args.get(at + 1).cloned().unwrap_or_default(),
                    // Another repository than the folder's: its branch is not the folder's.
                    "--git-dir" | "--work-tree" => pushes = None,
                    _ => {}
                }
                at += 2;
            } else if word.starts_with("--git-dir=") || word.starts_with("--work-tree=") {
                pushes = None;
                at += 1;
            } else if word.starts_with('-') {
                at += 1;
            } else {
                break;
            }
        }
        let rest = args.get(at + 1..).unwrap_or(&[]);
        match args.get(at).map(String::as_str) {
            Some("push") => {}
            Some("branch") => return branch_deleted(rest),
            Some("rm") => return git_rm(rest),
            _ => return None,
        }
        let mut words = Vec::new();
        let mut every = false;
        let mut delete = false;
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
            } else if options && matches!(word.as_str(), "--delete" | "-d") {
                delete = true;
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
        let remote = words.first().copied();
        let refspecs = words.get(1..).unwrap_or(&[]);
        let named = refspecs.iter().find_map(|refspec| {
            let spec = refspec.trim_start_matches('+');
            let dst = spec.rsplit_once(':').map_or(spec, |(_, dst)| dst);
            let name = dst.strip_prefix("refs/heads/").unwrap_or(dst);
            (!name.starts_with("refs/") && self.protects(name)).then_some(name)
        });
        if let Some(branch) = named {
            return Some(Risk {
                class: Class::ProtectedPush,
                why: format!("Pushes to {branch}, a protected branch"),
                confirm: format!("Allow a push to {branch}?"),
            });
        }
        // **A push that names no branch** (#6178): `git push`, `git push origin`, or `HEAD`,
        // which pushes the current branch to one of the same name.
        if !every && !delete {
            let unnamed = refspecs.is_empty();
            let head = refspecs
                .iter()
                .any(|spec| spec.trim_start_matches('+') == "HEAD");
            if (unnamed || head)
                && let Some(read) = pushes
                && let Some(pushes) = read(&folder)
            {
                let lands = if unnamed {
                    pushes.to.or(pushes.current)
                } else {
                    pushes.current
                };
                if let Some(branch) = lands.filter(|branch| self.protects(branch)) {
                    return Some(Risk {
                        class: Class::ProtectedPush,
                        why: format!("Pushes this chat's branch to {branch}, a protected branch"),
                        confirm: format!("Allow a push to {branch}?"),
                    });
                }
            }
        }
        // **A branch deleted on the remote** (#6178): `--delete`, or a refspec with no source.
        let gone: Vec<&str> = if delete {
            refspecs.to_vec()
        } else {
            refspecs
                .iter()
                .filter_map(|spec| spec.trim_start_matches('+').strip_prefix(':'))
                .collect()
        };
        let first = gone.first()?;
        let name = first.strip_prefix("refs/heads/").unwrap_or(first);
        let on = remote.map_or_else(String::new, |remote| format!(" on {remote}"));
        Some(deleting(&format!("{name}{on}"), gone.len()))
    }

    /// Whether `branch` is protected: named, or matched by a pattern with `*`.
    fn protects(&self, branch: &str) -> bool {
        !branch.is_empty()
            && branch != "HEAD"
            && self.protected.iter().any(|one| matches(one, branch))
    }
}

/// **What an edit a harness names a delete deletes** (#6178): ACP's `delete` kind, which the
/// harness's own protocol says, on the file it names.
pub fn delete_of(path: &str) -> Risk {
    deleting(path, 1)
}

/// **A vault use** (#6178): an Allow that hands `what` from `vault` (or the person's, where the
/// ask names none) to `to`. Every name is the registry's, and is data.
pub fn vault_use(what: &str, vault: Option<&str>, to: &str) -> Risk {
    let from = vault.map_or_else(|| "your vault".to_owned(), |vault| format!("vault {vault}"));
    Risk {
        class: Class::VaultUse,
        why: format!("Hands {what} from {from} to {to}"),
        confirm: format!("Allow handing {what} to {to}?"),
    }
}

/// **A held merge** (Q239): request `number` of `repo` at `head`, into `base`, which the app
/// merges itself on the person's press. Both sentences name the short head (Q242), so the once
/// more press says which commit it merges. Every name is the key's, and is data.
pub fn merge_of(repo: &str, number: u64, head: &str, base: &str) -> Risk {
    let short = head.get(..7).unwrap_or(head);
    Risk {
        class: Class::Merge,
        why: format!("Merges request {number} of {repo} at {short} into {base}, with no undo"),
        confirm: format!("Merge request {number} of {repo} at {short} into {base}?"),
    }
}

/// **Where a push that names no branch from `folder` lands** (#6178): its current branch and,
/// where `push.default` is `upstream`, the branch it tracks, which a bare push sends it to;
/// under `simple` and `current` it lands on its own name. Read with git itself, never guessed;
/// nothing where `folder` is on no branch or git cannot say.
pub fn pushes_from(folder: &Path) -> Option<Pushes> {
    use crate::worktree::git;
    let said = |args: &[&str]| {
        git::run(folder, args, git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|ran| ran.line().trim().to_owned())
            .filter(|line| !line.is_empty())
    };
    let current = said(&["symbolic-ref", "--quiet", "HEAD"])?
        .strip_prefix("refs/heads/")?
        .to_owned();
    let to = said(&["config", "--get", "push.default"])
        .filter(|mode| matches!(mode.as_str(), "upstream" | "tracking"))
        .and_then(|_| said(&["config", "--get", &format!("branch.{current}.merge")]))
        .and_then(|merge| merge.strip_prefix("refs/heads/").map(str::to_owned));
    Some(Pushes {
        current: Some(current),
        to,
    })
}

/// A delete of `first`, and `more` in all.
fn deleting(first: &str, all: usize) -> Risk {
    let what = match all {
        0 | 1 => first.to_owned(),
        n => format!("{first} and {} more", n - 1),
    };
    Risk {
        class: Class::Delete,
        why: format!("Deletes {what}"),
        confirm: format!("Allow deleting {what}?"),
    }
}

/// The words of `args` that are not options, every word after `--` included.
fn operands(args: &[String]) -> Vec<&str> {
    let mut options = true;
    args.iter()
        .filter(|word| {
            if options && *word == "--" {
                options = false;
                return false;
            }
            !(options && word.starts_with('-') && word.len() > 1)
        })
        .map(String::as_str)
        .collect()
}

/// **What `prog` run with `args` deletes**, where its words plainly say so.
fn deletes(prog: &str, args: &[String]) -> Option<Risk> {
    match prog {
        "rm" | "rmdir" | "unlink" => {
            let gone = operands(args);
            Some(deleting(gone.first()?, gone.len()))
        }
        "find" if args.iter().any(|arg| arg == "-delete") => {
            let under = args
                .iter()
                .take_while(|arg| !arg.starts_with('-') && !arg.starts_with('('))
                .next()
                .map_or(".", String::as_str);
            Some(Risk {
                class: Class::Delete,
                why: format!("Deletes what find matches in {under}"),
                confirm: "Allow deleting what find matches?".to_owned(),
            })
        }
        _ => None,
    }
}

/// `git branch`'s arguments: a branch deleted (`-d`, `-D`, `--delete`, `-dr`).
fn branch_deleted(args: &[String]) -> Option<Risk> {
    let delete = args.iter().any(|arg| {
        arg == "--delete"
            || (arg.starts_with('-')
                && !arg.starts_with("--")
                && arg.chars().skip(1).any(|c| c == 'd' || c == 'D'))
    });
    if !delete {
        return None;
    }
    let gone = operands(args);
    let first = gone.first()?;
    Some(deleting(&format!("branch {first}"), gone.len()))
}

/// `git rm`'s arguments: the files it deletes, unless `--cached` keeps them.
fn git_rm(args: &[String]) -> Option<Risk> {
    if args.iter().any(|arg| arg == "--cached") {
        return None;
    }
    let gone = operands(args);
    Some(deleting(gone.first()?, gone.len()))
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
