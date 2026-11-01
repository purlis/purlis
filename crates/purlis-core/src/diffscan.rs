// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The scan of an agent's commit before it is made (SQ-16): every line the commit ADDS, in any
//! repository a chat commits to, asked [`crate::secretshape::leaks`] — the same scanner a plane
//! save and a repo save use, with the rule a diff of code needs: vendor credential shapes and
//! personal data, and not the credential-assignment rule that refuses `password: String`.
//!
//! **What is read is the index being committed**, from inside git's own `pre-commit` hook
//! ([`crate::githooks`]), so `git commit -a` and `git commit <paths>` — which stage into a
//! temporary index git names in `GIT_INDEX_FILE` — are scanned as what they commit.
//!
//! **Only added lines.** A value already in the history, or one this commit removes, is not
//! something this commit publishes. A file whose bytes are binary has no lines and is not read;
//! **a text file git only shows as binary** — a `binary` or `-diff` attribute, a diff driver that
//! says `binary`, a lowered `core.bigFileThreshold` — is read as the text it is (SQ-7,
//! [`shown_as_binary`]).
//!
//! **Each line as written, and as it reads** (#1304): a string can spell a character as an
//! escape (`\u0041` is `A`), so each added line is also asked with its escapes decoded
//! ([`crate::secretshape::escaped_leaks`]) — no parse, so a file of any name and one that
//! would not parse are read alike. What is found that way is masked, fingerprinted and
//! allowlisted by the value it spells, so an entry names a value however it is spelled. One
//! value read both ways is named once, by the value it spells, unless the allowlist lets one
//! reading through and not the other; and lines that end in a backslash are read joined to the
//! next, as a TOML `"""` string reads them (#1315) — an added line with the unchanged lines it
//! joins or that join it too, read from git's context lines and, past them, from the file as
//! it would be published (#1346).
//!
//! **What is shown is masked** ([`crate::secretshape::masked`]): the kind, where it is, and a
//! short head of the value. Never the value.
//!
//! # What is let through (SQ-17)
//!
//! [`checked`] is [`staged`] filtered through the allowlist ([`crate::scanallow`]): charter's
//! own entries, and the repository's file as it is at `HEAD`. It also says whether the commit
//! changes that file, which a chat's commit may not.
//!
//! # Before a push (SQ-7)
//!
//! [`pushed`] is the same scan over what a push would send: every commit git's `pre-push`
//! names that the remote does not already have, each commit's added lines, filtered through the
//! same allowlist. It catches a commit that never passed a chat's `pre-commit`: one the operator
//! made in an unarmed terminal, or one `cherry-pick`, `rebase` or `am` made (ADR 0074, V26b).
//! It reads what a merge's own resolution adds, and each commit's and annotated tag's message
//! (SQ-7). A save's own push is scanned the same way before it pushes
//! ([`crate::planegit::push_head`]).

use std::path::Path;

use crate::scanallow::{self, Allowlist, Entry, Seen};
use crate::secretshape;
use crate::worktree::git;

/// One thing a staged commit would publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The file, as git names it from the top of the work tree.
    pub path: String,
    /// The 1-based line in the new version of the file.
    pub line: usize,
    /// The rule that found it, by the id an allowlist entry names it with.
    pub rule: &'static str,
    /// What it looks like.
    pub kind: &'static str,
    /// The value, masked.
    pub masked: String,
    /// The value's fingerprint ([`scanallow::fingerprint`]), which an entry can name it by.
    pub fingerprint: String,
}

/// A staged commit, scanned and filtered through its allowlist.
#[derive(Debug, Clone, Default)]
pub struct Scan {
    /// What the commit is refused for.
    pub refused: Vec<Finding>,
    /// What an entry let through, and the entry.
    pub allowed: Vec<(Finding, Entry)>,
    /// Entries of the repository's allowlist that allow nothing, and why.
    pub problems: Vec<String>,
    /// Whether the commit changes the allowlist file itself, which only a commit made outside a
    /// chat may.
    pub changes_the_allowlist: bool,
    /// **Whether the commit stages a project file whose `[sandbox]`, `[chat_env]` or
    /// `[dispatch]` tables differ from that file in the working tree** (#1464), wherever the
    /// project sits in the repository. A chat's `pre-commit` refuses it, as a brokered write is
    /// refused ([`crate::brokered::runs_under_differs`]). **A staged or working file that cannot
    /// be read, or one staged as anything but a regular file, is refused too.** Committing the
    /// file as it stands in the working tree is not this. Like every arm of the hooks, a guard
    /// against mistakes (ADR 0074).
    pub stages_what_a_chat_runs_under: bool,
}

/// [`staged`], filtered through charter's own entries and the repository's allowlist as it is at
/// `HEAD` — never the working tree's or the index's, so an entry written and not yet committed
/// allows nothing ([`crate::scanallow`]).
pub fn checked(repo: &Path) -> Result<Scan, String> {
    let found = staged_read(repo)?;
    let at_head = |args: &[&str]| git::run_in_hook(repo, args, git::READ).ok();
    let changed = at_head(&[
        "-c",
        "core.quotePath=false",
        "diff",
        "--cached",
        "--no-renames",
        "--name-only",
        "-z",
    ])
    .filter(|run| run.code == Some(0))
    .ok_or("git could not name the files the commit changes")?;
    let mut scan = through_the_allowlist(repo, found);
    let names = String::from_utf8_lossy(&changed.out);
    scan.changes_the_allowlist = names.split('\0').any(scanallow::is_the_file);
    scan.stages_what_a_chat_runs_under = names
        .split('\0')
        .filter(|name| is_a_project_file(name))
        .any(|name| staged_runs_under_differs(repo, name));
    Ok(scan)
}

/// Where a finding in the message of the commit being made is said to be.
pub const MESSAGE: &str = "the commit message";

/// What `message`, the message a commit in `repo` is about to be made with, holds that looks
/// like a credential, filtered through the allowlist as [`checked`] filters (#1133). A chat's
/// own `commit-msg` and a brokered commit ask it; the staged diff is [`checked`]'s.
///
/// **Credentials only**, as at push ([`in_message`]), and **the whole message**, comments
/// included, as git may store it. git cuts at its scissors line only under `commit -v` or
/// `--cleanup=scissors`; under `--cleanup=verbatim` or `whitespace` what is below it is
/// committed, and a hook cannot tell which ran. So below that line one thing only is let
/// through: a credential the repository already holds, at `HEAD` or in the index, in a file the
/// commit changes ([`held_by_the_change`]). That is what the diff `commit -v` puts there shows,
/// and the staged diff is [`checked`]'s; anything else below the line is read.
pub fn message_checked(repo: &Path, message: &str) -> Scan {
    let mut found = in_message(MESSAGE, message);
    if !found.found.is_empty()
        && let Some(cut) = scissors_line(message, || comment_mark(repo))
        && found.found.iter().any(|finding| finding.line > cut)
    {
        let held = held_by_the_change(repo);
        found.drop_where(|finding| finding.line > cut && held.contains(&finding.fingerprint));
    }
    through_the_allowlist(repo, found)
}

/// The number of `message`'s first scissors line, as [`in_message`] numbers lines: git's
/// comment mark, a space, and git's cut line. The mark is one character that is neither a
/// letter, a digit nor a space, as `core.commentChar=auto` picks one, or the mark `configured`
/// names, of however many characters (`core.commentString`). `configured` is asked only when a
/// line with another mark in front of the cut is met.
fn scissors_line(message: &str, configured: impl FnOnce() -> Option<String>) -> Option<usize> {
    const CUT: &str = " ------------------------ >8 ------------------------";
    let mark_set = std::cell::OnceCell::new();
    let mut configured = Some(configured);
    message
        .lines()
        .position(|line| {
            line.strip_suffix(CUT).is_some_and(|mark| {
                let mut chars = mark.chars();
                let one = chars
                    .next()
                    .is_some_and(|c| !c.is_alphanumeric() && !c.is_whitespace())
                    && chars.next().is_none();
                one || (!mark.is_empty()
                    && mark_set
                        .get_or_init(|| configured.take().and_then(|ask| ask()))
                        .as_deref()
                        == Some(mark))
            })
        })
        .map(|at| at + 1)
}

/// The comment mark git is set to in `repo` (`core.commentChar` or `core.commentString`,
/// whichever it reads last), or `None` when it is unset, `auto`, or git cannot say: then only
/// a one-character mark cuts, and what is below any other is read.
fn comment_mark(repo: &Path) -> Option<String> {
    let run = git::run_in_hook(
        repo,
        &[
            "config",
            "--get-regexp",
            "-z",
            r"^core\.comment(char|string)$",
        ],
        git::READ,
    )
    .ok()
    .filter(|run| run.code == Some(0))?;
    let out = String::from_utf8(run.out).ok()?;
    let last = out.split('\0').rfind(|entry| !entry.is_empty())?;
    let (_, value) = last.split_once('\n')?;
    (!value.is_empty() && value != "auto").then(|| value.to_owned())
}

/// The fingerprints of every credential in the files the commit being made in `repo` changes,
/// as `HEAD` has them and as the index has them: what the diff `commit -v` shows can hold. Empty
/// when git cannot say, so nothing is let through.
fn held_by_the_change(repo: &Path) -> std::collections::HashSet<String> {
    let mut held = std::collections::HashSet::new();
    let Some(changed) = git::run_in_hook(
        repo,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--cached",
            "--no-renames",
            "--name-only",
            "-z",
        ],
        git::READ,
    )
    .ok()
    .filter(|run| run.code == Some(0)) else {
        return held;
    };
    let names = String::from_utf8_lossy(&changed.out).into_owned();
    for name in names.split('\0').filter(|name| !name.is_empty()) {
        for commit in ["HEAD", ""] {
            let Ok(text) = new_text_in(repo, commit, name) else {
                continue;
            };
            // A file past the project save's limit is not read for what it holds: it lets
            // nothing through.
            if text.len() as u64 > PROBE_MOST {
                continue;
            }
            for line in text.lines() {
                held.extend(
                    on_line(line)
                        .into_iter()
                        .map(|(_, value, _)| scanallow::fingerprint(&value)),
                );
            }
        }
    }
    held
}

/// Whether `name`, as git names a file from the top of the work tree, is the project's file or
/// its local settings beside it, under any name purlis reads them by, compared case-folded as
/// [`crate::brokered::guard`] compares them.
///
/// **Matched on the last part of the path**, as [`crate::brokered::guard`] matches a part: a
/// project may be a folder inside a larger repository, and git names its file from the top
/// of the work tree (`ops/purlis.toml`). A file of that name anywhere in the repository is
/// held to the same rule.
fn is_a_project_file(name: &str) -> bool {
    let last = name.rsplit('/').next().unwrap_or(name);
    [crate::names::PLANE_MANIFEST, crate::names::LOCAL_SETTINGS]
        .iter()
        .flat_map(|one| std::iter::once(one.write).chain(one.reads.iter().copied()))
        .any(|file| file.eq_ignore_ascii_case(last))
}

/// **Whether the staged `name` holds other `[sandbox]`, `[chat_env]` or `[dispatch]` tables
/// than the file in the working tree** of `repo`, or either cannot be read: [`Scan`]'s
/// [`Scan::stages_what_a_chat_runs_under`]. It fails closed: a staged blob git does not show,
/// a working file that is there and does not read, or a text that is not UTF-8 refuses. **So
/// does a file staged as anything but a regular file** (a link, a submodule, a conflict's
/// several entries), and a working file that is not one.
fn staged_runs_under_differs(repo: &Path, name: &str) -> bool {
    let asked = |args: &[&str]| {
        git::run_in_hook(repo, args, git::READ)
            .ok()
            .filter(|run| run.code == Some(0))
    };
    // What the index holds under the name: none where the commit deletes the file, else one
    // entry, of a regular file's mode.
    let Some(listed) = asked(&[
        "ls-files",
        "--stage",
        "-z",
        "--",
        &format!(":(literal){name}"),
    ]) else {
        return true;
    };
    let entries: Vec<&[u8]> = listed
        .out
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .collect();
    let staged: Option<Vec<u8>> = match entries.as_slice() {
        [] => None,
        [entry] if entry.starts_with(b"100644 ") || entry.starts_with(b"100755 ") => {
            match asked(&["show", &format!(":{name}")]) {
                Some(run) => Some(run.out),
                None => return true,
            }
        }
        _ => return true,
    };
    let path = repo.join(name);
    let working: Option<Vec<u8>> = match std::fs::symlink_metadata(&path) {
        Err(none) if none.kind() == std::io::ErrorKind::NotFound => None,
        Ok(there) if there.is_file() => match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(_) => return true,
        },
        _ => return true,
    };
    if staged == working {
        return false;
    }
    fn text(bytes: Option<&[u8]>) -> Result<Option<&str>, std::str::Utf8Error> {
        bytes.map(std::str::from_utf8).transpose()
    }
    match (text(staged.as_deref()), text(working.as_deref())) {
        (Ok(staged), Ok(working)) => crate::brokered::runs_under_differs(staged, working),
        _ => true,
    }
}

/// `found`, split into what is refused and what an entry lets through: charter's own entries
/// and the repository's allowlist as it is at `HEAD`.
fn through_the_allowlist(repo: &Path, found: Read) -> Scan {
    through(allowlist_at(repo, "HEAD").unwrap_or_default(), found)
}

/// The text of the repository's allowlist at `rev`, or `None` when `rev` has none or git could
/// not show it. The purlis name wins when `rev` has it (RN-2a, V93e); the old one is read only
/// when it does not.
fn allowlist_at(repo: &Path, rev: &str) -> Option<String> {
    scanallow::spellings().find_map(|file| {
        git::run_in_hook(repo, &["show", &format!("{rev}:{file}")], git::READ)
            .ok()
            .filter(|run| run.code == Some(0))
            .map(|run| String::from_utf8_lossy(&run.out).into_owned())
    })
}

/// How [`pushed`] shows a merge of two: as what its resolution adds to git merging its parents
/// again.
const REMERGED: &str = "--diff-merges=remerge";

/// How [`pushed`] shows a merge of three or more, which git does not merge again: against its
/// first parent.
const OCTOPUS: &str = "--diff-merges=first-parent";

/// The type git gives the object `oid` names (`commit`, `tag`, …). `Err` is git failing to say.
fn object_type(repo: &Path, oid: &str) -> Result<String, String> {
    let run = git::run_in_hook(repo, &["cat-file", "-t", oid], git::READ)
        .map_err(|gone| gone.to_string())?;
    match run.code {
        Some(0) => Ok(String::from_utf8_lossy(&run.out).trim().to_owned()),
        None => Err("git took too long to show what the push sends".to_owned()),
        Some(_) => Err(format!("git could not say what {oid} is")),
    }
}

/// The annotated tag `oid`, and each tag it names in turn down to what is not a tag: their
/// header lines, then their messages, each one after another, as git stores them. The header
/// is published with the tag as its message is (#1133). `Err` is git failing to show one.
fn tag_messages(repo: &Path, oid: &str) -> Result<(String, String), String> {
    let mut headers = String::new();
    let mut messages = String::new();
    let mut at = oid.to_owned();
    // A tag names a tag only so many times before it names what is tagged.
    for _ in 0..16 {
        if object_type(repo, &at)? != "tag" {
            break;
        }
        let run = git::run_in_hook(repo, &["cat-file", "tag", &at], git::READ)
            .map_err(|gone| gone.to_string())?;
        if run.code != Some(0) {
            return Err(format!("git could not show the tag {at}"));
        }
        let text = String::from_utf8_lossy(&run.out).into_owned();
        // The header, to the first blank line, then the message.
        let (header, message) = text.split_once("\n\n").unwrap_or((&text, ""));
        headers.push_str(header);
        headers.push('\n');
        messages.push_str(message);
        messages.push('\n');
        let Some(next) = header.lines().find_map(|line| line.strip_prefix("object ")) else {
            break;
        };
        at = next.trim().to_owned();
    }
    Ok((headers, messages))
}

/// One commit as `git log --pretty=raw` shows it: its name, its header lines as stored, and its
/// message with git's four-space indent taken off. `None` is a record that does not start as
/// git writes one.
fn raw_commit(record: &str) -> Option<(&str, &str, String)> {
    let rest = record.trim_start_matches('\n').strip_prefix("commit ")?;
    let (first, rest) = rest.split_once('\n').unwrap_or((rest, ""));
    let commit = first.split_whitespace().next()?;
    let (header, message) = rest.split_once("\n\n").unwrap_or((rest, ""));
    let message = message
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    Some((commit, header, message))
}

/// **The text of a Git LFS object a push would upload** (#4399), read as a commit's added lines
/// are and filtered through the allowlist at `HEAD`: the pointer a push sends says nothing of
/// what it names. `path` is where a pointer to it is.
pub(crate) fn in_large_file(repo: &Path, path: &str, text: &[u8]) -> Scan {
    let mut read = Read::default();
    let text = String::from_utf8_lossy(text);
    for (at, line) in text.lines().enumerate() {
        let found = on_line(line);
        let first = read.found.len();
        for (leak, value, twin) in &found {
            read.found.push(Finding {
                path: path.to_owned(),
                line: at + 1,
                rule: leak.rule,
                kind: leak.kind,
                masked: secretshape::masked(value),
                fingerprint: scanallow::fingerprint(value),
            });
            read.twin.push(twin.map(|other| first + other));
        }
    }
    through_the_allowlist(repo, read)
}

/// Whether `bytes` are text as git reads a file: no NUL in its first [`BINARY_PROBE`] bytes.
pub(crate) fn reads_as_text(bytes: &[u8]) -> bool {
    !bytes[..bytes.len().min(BINARY_PROBE)].contains(&0)
}

/// What a commit's or a tag's message, `where_` it is, holds that looks like a credential.
///
/// **Credentials only, never personal data**: a message names people as a matter of course —
/// `Co-Authored-By`, `Signed-off-by` — so an email address there is ordinary, as it is in a
/// manifest ([`scanallow`]'s own entry).
fn in_message(where_: &str, message: &str) -> Read {
    let mut read = Read::default();
    for (n, line) in message.lines().enumerate() {
        let all = on_line(line);
        // Where each kept finding was among them all, to number its twin among the kept: a
        // twin is of the same rule, so it is kept too.
        let kept: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, (leak, ..))| !secretshape::is_personal(leak.rule))
            .map(|(at, _)| at)
            .collect();
        let first = read.found.len();
        for (leak, value, twin) in all
            .into_iter()
            .filter(|(leak, ..)| !secretshape::is_personal(leak.rule))
        {
            read.found.push(Finding {
                path: where_.to_owned(),
                line: n + 1,
                rule: leak.rule,
                kind: leak.kind,
                masked: secretshape::masked(&value),
                fingerprint: scanallow::fingerprint(&value),
            });
            read.twin.push(
                twin.and_then(|other| kept.iter().position(|at| *at == other))
                    .map(|other| first + other),
            );
        }
    }
    read
}

/// What a diff adds that looks like a secret or personal data, with which of its findings are
/// one value read two ways.
#[derive(Debug, Default)]
struct Read {
    found: Vec<Finding>,
    /// `twin[i] = Some(j)`: finding `i` is the line as written of the value finding `j` reads
    /// through its escapes — the same rule, ending where it ends on the line, and one of the
    /// two values ending the other (a `\n` in front of an email reads as a longer address as
    /// written).
    twin: Vec<Option<usize>>,
    /// The files git showed as "Binary files … differ", with no lines: read again as text when
    /// their bytes are text ([`shown_as_binary`]).
    binary: Vec<ShownBinary>,
}

impl Read {
    /// This one without the findings `drop` names, its twins renumbered; a finding whose twin
    /// is dropped stands alone.
    fn drop_where(&mut self, drop: impl Fn(&Finding) -> bool) {
        let mut at_now = Vec::with_capacity(self.found.len());
        let mut kept = 0;
        for finding in &self.found {
            if drop(finding) {
                at_now.push(None);
            } else {
                at_now.push(Some(kept));
                kept += 1;
            }
        }
        let found = std::mem::take(&mut self.found);
        let twin = std::mem::take(&mut self.twin);
        for ((finding, twin), now) in found.into_iter().zip(twin).zip(&at_now) {
            if now.is_some() {
                self.found.push(finding);
                self.twin.push(twin.and_then(|other| at_now[other]));
            }
        }
    }

    /// `other`'s findings after this one's, its twins renumbered to stay its own.
    fn append(&mut self, other: Read) {
        let first = self.found.len();
        self.found.extend(other.found);
        self.twin
            .extend(other.twin.into_iter().map(|twin| twin.map(|at| first + at)));
        self.binary.extend(other.binary);
    }
}

/// A file a diff showed as "Binary files … differ": the commit it is in (empty for the index
/// being committed), its name, and the name it had where git found it renamed or copied.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShownBinary {
    commit: String,
    path: String,
    from: Option<String>,
}

/// `found` split through charter's own entries and the allowlist file `text`.
///
/// A value read two ways ([`Read::twin`]) is named once, by the value its escapes spell, when
/// the allowlist lets both readings through or neither (#1315). When it lets one through and
/// not the other, both are kept, so an entry for one spelling never answers for the other.
fn through(text: String, found: Read) -> Scan {
    let allowlist = Allowlist::of(Allowlist::parse(&text));
    let mut scan = Scan {
        problems: allowlist.problems.clone(),
        ..Scan::default()
    };
    let allowing = |finding: &Finding| {
        allowlist.allowing(Seen {
            rule: finding.rule,
            path: &finding.path,
            fingerprint: &finding.fingerprint,
        })
    };
    let entries: Vec<Option<&Entry>> = found.found.iter().map(allowing).collect();
    for (at, finding) in found.found.into_iter().enumerate() {
        let joined = found
            .twin
            .get(at)
            .copied()
            .flatten()
            .is_some_and(|other| entries[other].is_some() == entries[at].is_some());
        if joined {
            continue;
        }
        match entries[at] {
            Some(entry) => scan.allowed.push((finding, entry.clone())),
            None => scan.refused.push(finding),
        }
    }
    scan
}

/// How long git may take to show what a push sends. A push is a network operation already, and
/// a branch's first push to a new remote shows its whole history.
const SHOWING_A_PUSH: std::time::Duration = git::NETWORK;

/// git's object name for "nothing": a deleted ref on the local side, a new one on the remote's.
fn is_nothing(oid: &str) -> bool {
    oid.bytes().all(|b| b == b'0')
}

/// What a push from `repo` to `remote` (at `url`) would publish, from the lines git writes on
/// `pre-push`'s standard input (`updates`: `<local ref> <local oid> <remote ref> <remote oid>`),
/// filtered through the allowlist the remote already has.
///
/// The commits read are those reachable from what is pushed and from neither what the remote
/// has for those refs nor, when the push names a configured remote, any of its remote-tracking
/// refs: what the push sends. Each is read as its own diff, root commits included whatever
/// `log.showRoot` says, so a secret one commit adds and a later one removes is found: the history
/// still publishes it. **A merge is read for what its resolution adds** (SQ-7): its diff from git
/// merging its two parents again (`--diff-merges=remerge`), so a value no side's commit holds —
/// a conflict resolved by writing it, or a merge `commit-tree` made of any tree — is read, and
/// what either side added is read once, in that side's commit. A merge of three or more, which
/// git does not merge again, is read against its first parent. A text file git shows as binary
/// is read as text ([`shown_as_binary`]).
///
/// **Messages are read too**: each commit's, and each annotated tag's the push sends, with
/// every header line of each object (#1133), for credentials only ([`in_message`]), in UTF-8
/// whatever encoding the repository's config asks git to write messages in.
///
/// **What is pushed must be a commit.** A ref is peeled to the commit it names (an annotated tag
/// to its commit); a ref that names a blob or a tree, directly or through a tag, has no history
/// to read, so it is refused rather than sent unread.
///
/// **The allowlist is the remote's.** A pushed range that changes `.charter-scan-allow.toml`
/// sets [`Scan::changes_the_allowlist`], which the push hook refuses: only the operator pushes an
/// allowlist change. Otherwise findings go through the file as the remote has it for each ref
/// (the remote's commit, or, for a new ref, the pushed tip, which the range leaves unchanged);
/// refs whose files differ get charter's own entries alone.
///
/// `Err` is input git did not write, a ref that is not a commit, or git failing to answer, all
/// of which the caller refuses on.
pub fn pushed(repo: &Path, remote: &str, url: &str, updates: &str) -> Result<Scan, String> {
    let commit_of = |oid: &str| {
        git::run_in_hook(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{oid}^{{commit}}"),
            ],
            git::READ,
        )
        .ok()
        .filter(|run| run.code == Some(0))
        .map(|run| String::from_utf8_lossy(&run.out).trim().to_owned())
        .filter(|oid| !oid.is_empty())
    };
    let mut sends = Vec::new();
    let mut has = Vec::new();
    // The revision whose allowlist each update is filtered through.
    let mut allowlists = Vec::new();
    // The annotated tags the push sends, by the ref it names and the tag's own object.
    let mut tags: Vec<(String, String)> = Vec::new();
    for line in updates.lines().filter(|l| !l.trim().is_empty()) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [name, local, _, theirs] = fields[..] else {
            return Err(format!("a line git wrote could not be read: {line:?}"));
        };
        let is_oid = |oid: &str| !oid.is_empty() && oid.bytes().all(|b| b.is_ascii_hexdigit());
        if !is_oid(local) || !is_oid(theirs) {
            return Err(format!("a line git wrote could not be read: {line:?}"));
        }
        if is_nothing(local) {
            continue;
        }
        let Some(tip) = commit_of(local) else {
            return Err(format!(
                "{name} does not name a commit, so what it would publish has no history to \
                 scan; push without it"
            ));
        };
        // An annotated tag carries a message of its own, which the push publishes.
        if theirs != local && object_type(repo, local)?.as_str() == "tag" {
            tags.push((name.to_owned(), local.to_owned()));
        }
        let known = (!is_nothing(theirs)).then(|| commit_of(theirs)).flatten();
        if !is_nothing(theirs) {
            has.push(format!("^{theirs}"));
        }
        allowlists.push(known.unwrap_or_else(|| tip.clone()));
        sends.push(tip);
    }
    if sends.is_empty() {
        return Ok(Scan::default());
    }
    let mut range = sends;
    range.extend(has);
    // git hands a push to a URL with no remote configured the URL twice, and a URL is no
    // remote-tracking namespace.
    if remote != url {
        range.push("--not".to_owned());
        range.push(format!("--remotes={remote}"));
    }
    let ask = |head: &[&str]| {
        let mut args: Vec<&str> = head.to_vec();
        // The remote's side of a ref names a commit this clone may never have fetched.
        args.push("--ignore-missing");
        args.extend(range.iter().map(String::as_str));
        args.push("--");
        // Every name the allowlist goes by: a change to either is one to the allowlist.
        if head.first() == Some(&"rev-list") {
            for name in scanallow::spellings() {
                args.push(name);
            }
        }
        let run = git::run_in_hook(repo, &args, SHOWING_A_PUSH).map_err(|gone| gone.to_string())?;
        match run.code {
            Some(0) => Ok(String::from_utf8_lossy(&run.out).into_owned()),
            None => Err("git took too long to show what the push sends".to_owned()),
            Some(_) => Err(String::from_utf8_lossy(&run.err).trim().to_owned()),
        }
    };
    let changes_the_allowlist = !ask(&["rev-list", "--full-history"])?.trim().is_empty();
    // Which commit each diff is, so the lines around a change are read from its files.
    let marker = format!("--format={COMMIT_MARKER}%H");
    fn log<'a>(verb: &'a str, merges: &'a str, marker: &'a str, more: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec![
            "-c",
            "core.quotePath=false",
            "-c",
            AS_GIT_SHIPS,
            verb,
            "-p",
            // `log.showRoot=false` would otherwise show a root commit with no diff at all.
            "--root",
            merges,
            marker,
        ];
        args.extend(SHOWN);
        args.extend(more);
        args
    }
    let mut new_text = |commit: &str, path: &str| new_text_in(repo, commit, path);
    let mut read = Read::default();
    // A merge of two is read as what its own resolution adds: its diff from git merging its
    // two parents again, so neither side's lines, read in their own commits, are read twice.
    // git does not merge three or more again, so such a merge is read against its first
    // parent instead: all it brings in beside that one.
    for (merges, only) in [(REMERGED, None), (OCTOPUS, Some("--min-parents=3"))] {
        let mut args = log("log", merges, &marker, &[]);
        args.extend(only);
        let mut shown = in_diff(&ask(&args)?, &mut new_text)?;
        let binary = std::mem::take(&mut shown.binary);
        read.append(shown);
        let mut as_text = |commit: &str, paths: &[String]| {
            // `show`, not `log -1`: a walk limited to the paths could pass a merge by.
            let mut args = log("show", merges, &marker, &["--text", commit, "--"]);
            args.extend(paths.iter().map(String::as_str));
            let run =
                git::run_in_hook(repo, &args, SHOWING_A_PUSH).map_err(|gone| gone.to_string())?;
            match run.code {
                Some(0) => Ok(String::from_utf8_lossy(&run.out).into_owned()),
                None => Err("git took too long to show what the push sends".to_owned()),
                Some(_) => Err(String::from_utf8_lossy(&run.err).trim().to_owned()),
            }
        };
        read.append(shown_as_binary(repo, &binary, &mut as_text, &mut new_text)?);
    }
    // What each commit says, and each annotated tag pushed: a message is published with them,
    // and so is every header line of the object (#1133): a merged tag's whole text under
    // `mergetag`, or a header git does not know. Asked in UTF-8 whatever the repository's
    // config names, so a message is never written in an encoding this does not read.
    let said = ask(&["log", "-z", "--encoding=UTF-8", "--pretty=raw"])?;
    for one in said.split('\0').filter(|one| !one.trim().is_empty()) {
        let (commit, header, message) =
            raw_commit(one).ok_or_else(|| "git showed a commit this could not read".to_owned())?;
        let short = commit.get(..12).unwrap_or(commit);
        read.append(in_message(&format!("commit {short} (its header)"), header));
        read.append(in_message(
            &format!("commit {short} (its message)"),
            &message,
        ));
    }
    for (name, tag) in &tags {
        let (header, message) = tag_messages(repo, tag)?;
        read.append(in_message(&format!("tag {name} (its header)"), &header));
        read.append(in_message(&format!("tag {name} (its message)"), &message));
    }
    let texts: Vec<Option<String>> = allowlists
        .iter()
        .map(|rev| allowlist_at(repo, rev))
        .collect();
    let text = if texts.windows(2).all(|pair| pair[0] == pair[1]) {
        texts.into_iter().next().flatten().unwrap_or_default()
    } else {
        String::new()
    };
    let mut scan = through(text, read);
    scan.changes_the_allowlist = changes_the_allowlist;
    Ok(scan)
}

/// Everything the staged diff in `repo` adds that looks like a secret or personal data, in the
/// order git lists it — a value its escapes spell as well as one written as it is, so one value
/// can be here twice; [`checked`] names it once. `Err` is git failing to answer, which the
/// caller refuses on: a commit that could not be read is not a commit that was checked.
pub fn staged(repo: &Path) -> Result<Vec<Finding>, String> {
    staged_read(repo).map(|read| read.found)
}

/// [`staged`], with which findings are one value read two ways.
fn staged_read(repo: &Path) -> Result<Read, String> {
    let staged_diff = |more: &[&str]| -> Result<String, String> {
        let mut args = vec![
            "-c",
            "core.quotePath=false",
            "-c",
            AS_GIT_SHIPS,
            "diff",
            "--cached",
        ];
        args.extend(SHOWN);
        args.extend(more);
        let run = git::run_in_hook(repo, &args, git::READ).map_err(|gone| gone.to_string())?;
        match run.code {
            Some(0) => Ok(String::from_utf8_lossy(&run.out).into_owned()),
            None => Err("git took too long to show the staged diff".to_owned()),
            Some(_) => Err(String::from_utf8_lossy(&run.err).trim().to_owned()),
        }
    };
    let mut new_text = |commit: &str, path: &str| new_text_in(repo, commit, path);
    let mut read = in_diff(&staged_diff(&[])?, &mut new_text)?;
    let shown = std::mem::take(&mut read.binary);
    let mut as_text = |_: &str, paths: &[String]| {
        let mut more = vec!["--text", "--"];
        more.extend(paths.iter().map(String::as_str));
        staged_diff(&more)
    };
    read.append(shown_as_binary(repo, &shown, &mut as_text, &mut new_text)?);
    Ok(read)
}

/// Where the new version of a file in a diff is read from, for the unchanged lines a joined run
/// reaches past the diff's own context: `(commit, path)` to the file's text, with `commit` empty
/// for the index being committed. `Err` refuses: a line that could not be read is a line nobody
/// scanned.
type NewText<'a> = dyn FnMut(&str, &str) -> Result<String, String> + 'a;

/// The text of `path` as `commit` has it, or as the index being committed has it when `commit`
/// is empty: the blob's bytes, with no filter or text conversion. Stage 0 is named, so a path
/// that starts with a digit and a colon is read as the path it is.
fn new_text_in(repo: &Path, commit: &str, path: &str) -> Result<String, String> {
    let spec = if commit.is_empty() {
        format!(":0:{path}")
    } else {
        format!("{commit}:{path}")
    };
    let run = git::run_in_hook(repo, &["cat-file", "blob", &spec], git::READ)
        .map_err(|gone| gone.to_string())?;
    match run.code {
        Some(0) => Ok(String::from_utf8_lossy(&run.out).into_owned()),
        None => Err(format!("git took too long to show {path}")),
        Some(_) => Err(format!(
            "git could not show {path} as it would be published, to read the lines around what \
             changed"
        )),
    }
}

/// How every diff here is shown: no colour, no external diff or text conversion a setting could
/// choose, git's own prefixes, and [`CONTEXT`] lines around a change.
const SHOWN: [&str; 6] = [
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--src-prefix=a/",
    "--dst-prefix=b/",
    CONTEXT,
];

/// git's own size past which it shows a file as binary without looking, asked for by value
/// (SQ-7): a setting that lowered it would show every text file as binary, with no lines to
/// read.
const AS_GIT_SHIPS: &str = "core.bigFileThreshold=512m";

/// How far into a file git looks for a NUL before it calls the file binary (`xdiff-interface.c`'s
/// `FIRST_FEW_BYTES`), and so does [`shown_as_binary`].
const BINARY_PROBE: usize = 8000;

/// The largest file shown as binary whose bytes [`shown_as_binary`] reads to see whether they are
/// text: the plane save's own limit ([`crate::planegit::SCAN_LIMIT`]).
pub(crate) const PROBE_MOST: u64 = crate::planegit::SCAN_LIMIT as u64;

/// Reads a diff of `paths` in `commit` (empty for the index being committed) again, as text.
type AsText<'a> = dyn FnMut(&str, &[String]) -> Result<String, String> + 'a;

/// **What git showed as binary that is text** (SQ-7), read again as text: a file a `binary` or
/// `-diff` attribute, or a diff driver that says `binary`, has git show as "Binary files …
/// differ" with no lines, whatever it holds.
///
/// Only a file an attribute turned binary is looked at (`git check-attr diff`): with
/// [`AS_GIT_SHIPS`] asked for, any other file git shows as binary is one whose first
/// [`BINARY_PROBE`] bytes hold a NUL, as git reads it, and is not read here either. Of those an
/// attribute turned binary, the ones up to [`PROBE_MOST`] bytes are read, and those whose first
/// bytes hold no NUL — text — are shown again with `--text` (`as_text`) and read as any diff
/// is. So a real binary file stays unread, an image a `.gitattributes` calls binary included.
///
/// `Err` is git failing to answer, which the caller refuses on.
fn shown_as_binary(
    repo: &Path,
    shown: &[ShownBinary],
    as_text: &mut AsText,
    new_text: &mut NewText,
) -> Result<Read, String> {
    let mut read = Read::default();
    if shown.is_empty() {
        return Ok(read);
    }
    let asked_in = |dir: &Path, args: &[&str]| -> Result<Vec<u8>, String> {
        let run = git::run_in_hook(dir, args, git::READ).map_err(|gone| gone.to_string())?;
        match run.code {
            Some(0) => Ok(run.out),
            None => Err("git took too long to show what a commit holds".to_owned()),
            Some(_) => Err(String::from_utf8_lossy(&run.err).trim().to_owned()),
        }
    };
    let asked = |args: &[&str]| asked_in(repo, args);
    // git names each file from the top of the work tree, which `repo` may be inside of; the
    // attributes are asked for from there.
    let top = String::from_utf8_lossy(&asked(&["rev-parse", "--show-toplevel"])?)
        .trim()
        .to_owned();
    let top = if top.is_empty() {
        repo
    } else {
        Path::new(&top)
    };
    // Which files an attribute shows as binary: its `diff` unset, or set to a driver's name.
    let mut names: Vec<&str> = shown.iter().map(|one| one.path.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let mut turned = std::collections::HashSet::new();
    for chunk in names.chunks(256) {
        let mut args = vec!["check-attr", "-z", "diff", "--"];
        args.extend(chunk.iter().copied());
        let out = asked_in(top, &args)?;
        let fields: Vec<&[u8]> = out.split(|b| *b == 0).collect();
        for one in fields.chunks(3) {
            if let [path, _, value] = one
                && !matches!(*value, b"set" | b"unspecified")
            {
                turned.insert(String::from_utf8_lossy(path).into_owned());
            }
        }
    }
    // What is text, by commit, in the order git showed it.
    let mut again: Vec<(String, Vec<String>)> = Vec::new();
    for one in shown.iter().filter(|one| turned.contains(&one.path)) {
        let spec = if one.commit.is_empty() {
            format!(":0:{}", one.path)
        } else {
            format!("{}:{}", one.commit, one.path)
        };
        let size = asked(&["cat-file", "-s", &spec])?;
        let size: u64 = String::from_utf8_lossy(&size)
            .trim()
            .parse()
            .map_err(|_| format!("git could not say how large {} is", one.path))?;
        if size > PROBE_MOST {
            continue;
        }
        let bytes = asked(&["cat-file", "blob", &spec])?;
        if bytes.iter().take(BINARY_PROBE).any(|b| *b == 0) {
            continue;
        }
        let paths = match again.iter_mut().find(|(commit, _)| *commit == one.commit) {
            Some((_, paths)) => paths,
            None => {
                again.push((one.commit.clone(), Vec::new()));
                &mut again.last_mut().expect("just pushed").1
            }
        };
        paths.push(format!(":(top,literal){}", one.path));
        if let Some(from) = &one.from {
            paths.push(format!(":(top,literal){from}"));
        }
    }
    for (commit, paths) in again {
        let diff = as_text(&commit, &paths)?;
        let mut more = in_diff(&diff, new_text)?;
        // Shown with `--text`, nothing is binary any more; were it, it is not read twice.
        more.binary.clear();
        read.append(more);
    }
    Ok(read)
}

/// The marker [`pushed`] has git write in front of each commit's diff, so [`in_diff`] knows
/// which commit's files the unchanged lines around a change are read from.
const COMMIT_MARKER: &str = "purlis-commit ";

/// How many unchanged lines git shows around each change: git's own default, asked for by
/// number so a setting cannot change it. What a joined run reaches past them is read from the
/// file ([`NewText`]).
const CONTEXT: &str = "-U3";

/// What [`staged`] finds in a unified diff's text, or why the text could not be read — which the
/// caller refuses the commit on, since a file whose name could not be read is a file nobody
/// scanned.
///
/// Each added line as written and as it reads ([`on_line`]). Lines in a row that end in a
/// backslash are also read as one text (#1315): a TOML `"""` string joins such a line to the
/// next, so a credential split across them reads whole. **A run reaches across unchanged lines
/// too** (#1346): an unchanged line that joins the first added line of a change, and an
/// unchanged line an added line joins, are read with it — from the diff's context lines, and
/// past them from the file's new version (`new_text`), so the scan is still one pass. Only a
/// value that no single line shows, and that an added line holds part of or that a removal
/// joined, is taken from the joined text, named on the line it starts on. Each line is joined
/// into one text at most, what was seen is a set and the line a finding starts on is a binary
/// search, so the time grows with the diff and not with its square.
fn in_diff(diff: &str, new_text: &mut NewText) -> Result<Read, String> {
    let mut read = Read::default();
    let mut file = NewFile::default();
    let mut commit = String::new();
    let mut at = 0usize;
    let mut in_hunk = false;
    // Whether removed lines stand between the last new line and the next.
    let mut removed = false;
    let mut run = Joined::default();
    // The file the section git is showing names, from its `diff --git` line, and the names a
    // rename or a copy gives it: what a "Binary files … differ" line, which names none
    // exactly, is about.
    let mut section: Option<String> = None;
    let mut section_from: Option<String> = None;
    for line in diff.lines() {
        if let Some(next) = line.strip_prefix(COMMIT_MARKER) {
            file.end(&mut run, &mut read, at, &commit, new_text)?;
            commit = next.trim().to_owned();
            in_hunk = false;
            continue;
        }
        if !in_hunk {
            if let Some(header) = line.strip_prefix("diff --git ") {
                section = header_path(header);
                section_from = None;
            } else if let Some(to) = line
                .strip_prefix("rename to ")
                .or_else(|| line.strip_prefix("copy to "))
            {
                section = Some(named(to)?);
            } else if let Some(from) = line
                .strip_prefix("rename from ")
                .or_else(|| line.strip_prefix("copy from "))
            {
                section_from = Some(named(from)?);
            } else if line.starts_with("Binary files ") && line.ends_with(" differ") {
                // A deletion publishes nothing.
                if !line.ends_with(" and /dev/null differ") {
                    let Some(path) = section.clone() else {
                        return Err(format!(
                            "a file header git wrote could not be read: {line:?}"
                        ));
                    };
                    read.binary.push(ShownBinary {
                        commit: commit.clone(),
                        path,
                        from: section_from.clone(),
                    });
                }
                continue;
            }
        }
        let added = if in_hunk {
            line.strip_prefix('+')
        } else {
            None
        };
        let unchanged = if in_hunk {
            line.strip_prefix(' ')
        } else {
            None
        };
        if line.starts_with("diff --git ") {
            file.end(&mut run, &mut read, at, &commit, new_text)?;
            in_hunk = false;
        } else if !in_hunk && let Some(new) = line.strip_prefix("+++ ") {
            file.path = new_path(new)?;
        } else if let Some(hunk) = line.strip_prefix("@@ ") {
            let start = hunk
                .split_whitespace()
                .find_map(|part| part.strip_prefix('+'))
                .and_then(|new| new.split(',').next())
                .and_then(|start| start.parse().ok())
                .ok_or_else(|| format!("a hunk header git wrote could not be read: {line:?}"))?;
            // The unchanged lines between the last change and this one, while a run is open.
            if in_hunk {
                file.fill(&mut run, &mut read, at, start, &commit, new_text)?;
            }
            if !run.open {
                run.flush(&mut read);
            }
            in_hunk = true;
            removed = false;
            at = start;
            file.hunk_start = start;
            file.hunk.clear();
        } else if let Some(added) = added {
            let Some(path) = file.path.clone() else {
                return Err("git showed added lines with no file named for them".to_owned());
            };
            let found = on_line(added);
            let first = read.found.len();
            for (leak, value, twin) in &found {
                read.found.push(Finding {
                    path: path.clone(),
                    line: at,
                    rule: leak.rule,
                    kind: leak.kind,
                    masked: secretshape::masked(value),
                    fingerprint: scanallow::fingerprint(value),
                });
                read.twin.push(twin.map(|other| first + other));
            }
            let seen = found.into_iter().map(|(leak, value, _)| (leak.rule, value));
            file.hunk.push(added.to_owned());
            file.step(
                &mut run,
                &mut read,
                at,
                added,
                Part::Added,
                seen,
                &commit,
                new_text,
            )?;
            removed = false;
            at += 1;
        } else if let Some(unchanged) = unchanged {
            let part = if removed { Part::Seam } else { Part::Unchanged };
            file.hunk.push(unchanged.to_owned());
            file.step(
                &mut run,
                &mut read,
                at,
                unchanged,
                part,
                None.into_iter(),
                &commit,
                new_text,
            )?;
            // A removal joins the lines on either side of it, and blank lines read as nothing
            // in a join: the seam is the first line after it that is not blank.
            removed = removed && is_blank(unchanged);
            at += 1;
        } else if in_hunk && line.starts_with('-') {
            removed = true;
        }
    }
    file.end(&mut run, &mut read, at, &commit, new_text)?;
    Ok(read)
}

/// What [`in_diff`] keeps of the file it is reading: its name, the new lines of the hunk it is
/// in, and — read only when a run reaches past those — the file's whole new text.
#[derive(Default)]
struct NewFile {
    path: Option<String>,
    /// The new line the hunk starts on, and its new lines so far, from there.
    hunk_start: usize,
    hunk: Vec<String>,
    /// The file's new lines, once a run has needed one the diff does not show.
    lines: Option<Vec<String>>,
}

impl NewFile {
    /// The text of new line `n` (from 1), or `None` past either end of the file.
    fn line(
        &mut self,
        n: usize,
        commit: &str,
        new_text: &mut NewText,
    ) -> Result<Option<String>, String> {
        if n == 0 {
            return Ok(None);
        }
        if let Some(shown) = n
            .checked_sub(self.hunk_start)
            .and_then(|i| self.hunk.get(i))
        {
            return Ok(Some(shown.clone()));
        }
        let Some(path) = &self.path else {
            return Ok(None);
        };
        if self.lines.is_none() {
            self.lines = Some(new_text(commit, path)?.lines().map(str::to_owned).collect());
        }
        Ok(self
            .lines
            .as_ref()
            .and_then(|lines| lines.get(n - 1))
            .cloned())
    }

    /// Line `n`, a new line that is `part`, into `run`: joined to it while the run is open,
    /// and otherwise, for a line this change adds or a removal makes adjacent, after the
    /// unchanged lines before it that join it ([`NewFile::joining_before`]).
    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        run: &mut Joined,
        read: &mut Read,
        n: usize,
        text: &str,
        part: Part,
        seen: impl Iterator<Item = (&'static str, String)>,
        commit: &str,
        new_text: &mut NewText,
    ) -> Result<(), String> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        if !run.open {
            run.flush(read);
            if part == Part::Unchanged {
                return Ok(());
            }
            // A blank line adds no character to what a join reads, and a removal in front of
            // one moves the seam to the next line that is not ([`in_diff`]). Looking back only
            // from a line that is not blank keeps each look over lines no other look covers.
            let before = if is_blank(text) {
                Vec::new()
            } else {
                self.joining_before(n, commit, new_text)?
            };
            for (k, before) in before {
                run.push(&path, k, &before, Part::Unchanged, seen_alone(&before));
            }
        }
        match part {
            Part::Added => run.push(&path, n, text, part, seen),
            _ => run.push(&path, n, text, part, seen_alone(text)),
        }
        if !run.open {
            run.flush(read);
        }
        Ok(())
    }

    /// The unchanged lines before new line `n` that join it, in order: each that ends in a
    /// backslash, with the blank lines after it that the join swallows.
    fn joining_before(
        &mut self,
        n: usize,
        commit: &str,
        new_text: &mut NewText,
    ) -> Result<Vec<(usize, String)>, String> {
        let mut joining = Vec::new();
        let mut blanks = Vec::new();
        let mut k = n;
        while k > 1 {
            k -= 1;
            let Some(text) = self.line(k, commit, new_text)? else {
                break;
            };
            if is_blank(&text) {
                blanks.push((k, text));
            } else if secretshape::joins_the_next_line(&text) {
                joining.append(&mut blanks);
                joining.push((k, text));
            } else {
                break;
            }
        }
        joining.reverse();
        Ok(joining)
    }

    /// The unchanged lines from `from` up to `to` (not included), into `run` while it is open.
    fn fill(
        &mut self,
        run: &mut Joined,
        read: &mut Read,
        from: usize,
        to: usize,
        commit: &str,
        new_text: &mut NewText,
    ) -> Result<(), String> {
        let mut k = from;
        while run.open && k < to {
            let Some(text) = self.line(k, commit, new_text)? else {
                break;
            };
            self.step(
                run,
                read,
                k,
                &text,
                Part::Unchanged,
                None.into_iter(),
                commit,
                new_text,
            )?;
            k += 1;
        }
        Ok(())
    }

    /// The end of the file's diff: the unchanged lines after the last change, from `at`, into
    /// `run` while it is open; then what it found, and a fresh start for the next file.
    fn end(
        &mut self,
        run: &mut Joined,
        read: &mut Read,
        at: usize,
        commit: &str,
        new_text: &mut NewText,
    ) -> Result<(), String> {
        self.fill(run, read, at, usize::MAX, commit, new_text)?;
        run.flush(read);
        *self = NewFile::default();
        Ok(())
    }
}

/// Whether a line is blanks only: one a line-ending backslash swallows on its way to the next.
fn is_blank(line: &str) -> bool {
    line.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r'))
}

/// What a line shows on its own, as written and as it reads, by rule and value: so a value an
/// unchanged line already showed is not taken from a run it is joined into.
fn seen_alone(text: &str) -> std::vec::IntoIter<(&'static str, String)> {
    on_line(text)
        .into_iter()
        .map(|(leak, value, _)| (leak.rule, value))
        .collect::<Vec<_>>()
        .into_iter()
}

/// What `added` holds as written, then what it spells through its escapes, in the order they
/// stand on the line: each with its value, and — for one read as written — the index of the
/// finding its escapes make it (see [`Read::twin`]).
fn on_line(added: &str) -> Vec<(secretshape::Leak, String, Option<usize>)> {
    let mut found: Vec<(secretshape::Leak, String, bool)> = secretshape::leaks(added)
        .into_iter()
        .map(|leak| {
            let value = added[leak.span.clone()].to_owned();
            (leak, value, false)
        })
        .chain(
            secretshape::escaped_leaks(added)
                .into_iter()
                .map(|(leak, value)| (leak, value, true)),
        )
        .collect();
    found.sort_by_key(|(leak, ..)| leak.span.start);
    // One value read two ways ends where the line ends it either way: the escape is in front.
    // Looked up by where each ends, so a line of many findings is still read in one pass.
    let spelled: std::collections::HashMap<(&str, usize), usize> = found
        .iter()
        .enumerate()
        .filter(|(_, (.., escaped))| *escaped)
        .map(|(at, (leak, ..))| ((leak.rule, leak.span.end), at))
        .collect();
    let twin = |(leak, value, escaped): &(secretshape::Leak, String, bool)| {
        if *escaped {
            return None;
        }
        spelled
            .get(&(leak.rule, leak.span.end))
            .copied()
            .filter(|at| {
                let other = &found[*at].1;
                value.ends_with(other.as_str()) || other.ends_with(value.as_str())
            })
    };
    let twins: Vec<Option<usize>> = found.iter().map(twin).collect();
    found
        .into_iter()
        .zip(twins)
        .map(|((leak, value, _), twin)| (leak, value, twin))
        .collect()
}

/// What a line of a joined run is to the change being scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// A line the change adds.
    Added,
    /// An unchanged line that removed lines stood in front of: the change joins it to the line
    /// before.
    Seam,
    /// An unchanged line, with the line before it as it was.
    Unchanged,
}

/// Lines in a row, each but the last ending in a backslash (or blank, after one that does),
/// read as one text.
#[derive(Default)]
struct Joined {
    path: String,
    /// The file line each joined line is, where it starts in `text`, and what it is to the
    /// change.
    starts: Vec<(usize, usize, Part)>,
    text: String,
    /// What the lines showed one at a time, by rule and value: a set, so many lines of many
    /// values are still read in one pass.
    seen: std::collections::HashSet<(&'static str, String)>,
    /// Whether the last line pushed joins the next: one that ends in a backslash, or a blank
    /// line inside a run, which TOML's line-ending backslash swallows up to the next that is
    /// not.
    open: bool,
}

impl Joined {
    fn push(
        &mut self,
        path: &str,
        line: usize,
        text: &str,
        part: Part,
        seen: impl Iterator<Item = (&'static str, String)>,
    ) {
        if self.starts.is_empty() {
            self.path = path.to_owned();
        } else {
            self.text.push('\n');
        }
        self.starts.push((line, self.text.len(), part));
        self.text.push_str(text);
        self.seen.extend(seen);
        self.open =
            secretshape::joins_the_next_line(text) || (is_blank(text) && self.starts.len() > 1);
    }

    /// Whether `span` of the text is part of the change: it holds part of an added line, or it
    /// crosses into a line a removal joined to the one before.
    fn changed(&self, span: &std::ops::Range<usize>) -> bool {
        self.starts.iter().enumerate().any(|(i, (_, start, part))| {
            let end = self
                .starts
                .get(i + 1)
                .map_or(self.text.len(), |(_, next, _)| next - 1);
            match part {
                Part::Added => span.start < end.max(start + 1) && span.end > *start,
                Part::Seam => span.start < *start && span.end > *start,
                Part::Unchanged => false,
            }
        })
    }

    /// What only the joined text shows, and the change holds part of, into `read`; then start
    /// again.
    fn flush(&mut self, read: &mut Read) {
        let joined = std::mem::take(self);
        if joined.starts.len() < 2 {
            return;
        }
        for (leak, value) in secretshape::escaped_leaks(&joined.text) {
            if joined.seen.contains(&(leak.rule, value.clone())) || !joined.changed(&leak.span) {
                continue;
            }
            // The last line that starts at or before it: `starts` is in order.
            let after = joined
                .starts
                .partition_point(|(_, start, _)| *start <= leak.span.start);
            let line = joined.starts[after.saturating_sub(1)].0;
            read.found.push(Finding {
                path: joined.path.clone(),
                line,
                rule: leak.rule,
                kind: leak.kind,
                masked: secretshape::masked(&value),
                fingerprint: scanallow::fingerprint(&value),
            });
            read.twin.push(None);
        }
    }
}

/// The file a `+++ ` header names: `b/<path>`, or git's C-quoted `"b/<path>"` for a name
/// holding a quote, a backslash, a control character or (with `core.quotePath` off) nothing
/// else; `None` for `/dev/null`, a deletion. Anything else is an error.
///
/// git ends a name that holds a space with a tab (`diff.c`, so `patch` can find where the name
/// stops); that one tab is not part of the name.
fn new_path(header: &str) -> Result<Option<String>, String> {
    let header = header.strip_suffix('\t').unwrap_or(header);
    if header == "/dev/null" {
        return Ok(None);
    }
    let name = if header.starts_with('"') {
        unquoted(header)
            .ok_or_else(|| format!("a file name git quoted could not be read: {header:?}"))?
    } else {
        header.to_owned()
    };
    name.strip_prefix("b/")
        .map(|path| Some(path.to_owned()))
        .ok_or_else(|| format!("a file header git wrote could not be read: {header:?}"))
}

/// A name as git writes it after `rename to ` and its kin: as it is, or C-quoted.
fn named(name: &str) -> Result<String, String> {
    if name.starts_with('"') {
        unquoted(name).ok_or_else(|| format!("a file name git quoted could not be read: {name:?}"))
    } else {
        Ok(name.to_owned())
    }
}

/// The file a `diff --git a/<name> b/<name>` line names, where both sides name the same file
/// — every section but a rename's or a copy's, whose own lines name the file instead. `None`
/// where the two sides differ, or a side cannot be read.
fn header_path(header: &str) -> Option<String> {
    let (old, new) = if header.starts_with('"') {
        // The first quoted name ends at the first quote no backslash escapes.
        let bytes = header.as_bytes();
        let mut at = 1;
        while at < bytes.len() && bytes[at] != b'"' {
            at += if bytes[at] == b'\\' { 2 } else { 1 };
        }
        let old = unquoted(header.get(..=at)?)?;
        let new = header.get(at + 1..)?.strip_prefix(' ')?;
        (old, named(new).ok()?)
    } else {
        // `a/<name> b/<name>`: the one split where both halves name one file.
        let half = header.len().checked_sub(1)? / 2;
        let (old, new) = (header.get(..half)?, header.get(half + 1..)?);
        if header.as_bytes().get(half) != Some(&b' ') {
            return None;
        }
        (old.to_owned(), new.to_owned())
    };
    let old = old.strip_prefix("a/")?;
    let new = new.strip_prefix("b/")?;
    (old == new).then(|| new.to_owned())
}

/// git's C-quoted name, unquoted (`quote.c`'s `unquote_c_style`): `\"`, `\\`, `\a` `\b` `\t`
/// `\n` `\v` `\f` `\r`, and three octal digits for any other byte. `None` for anything that is
/// not exactly one such quoted string.
fn unquoted(quoted: &str) -> Option<String> {
    let inner = quoted.strip_prefix('"')?.strip_suffix('"')?;
    let mut bytes = Vec::with_capacity(inner.len());
    let mut rest = inner.as_bytes().iter().copied();
    while let Some(byte) = rest.next() {
        match byte {
            b'"' => return None,
            b'\\' => {
                let escaped = rest.next()?;
                bytes.push(match escaped {
                    b'"' => b'"',
                    b'\\' => b'\\',
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'v' => 0x0b,
                    b'f' => 0x0c,
                    b'r' => b'\r',
                    b'0'..=b'3' => {
                        let (two, three) = (rest.next()?, rest.next()?);
                        let digit = |d: u8| (b'0'..=b'7').contains(&d).then(|| d - b'0');
                        (escaped - b'0') * 64 + digit(two)? * 8 + digit(three)?
                    }
                    _ => return None,
                });
            }
            other => bytes.push(other),
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// What a chat's commit that changes the allowlist is refused with.
pub const ALLOWLIST_REFUSAL: &str = "purlis: commit refused — it changes .charter-scan-allow.toml, the \
     scan's allowlist, which only a commit made outside a chat may change. Take the file out of \
     the commit (`git restore --staged .charter-scan-allow.toml`); the operator reviews and \
     commits an entry. Do not use --no-verify; the operator has been told.\n";

/// What a chat's commit is refused with where it stages a change to what chats run under that
/// the project's file in the working tree does not hold (#1464).
pub const RUNS_UNDER_REFUSAL: &str = "purlis: commit refused — it stages a version of the \
     project's file (purlis.toml) whose [sandbox], [chat_env] or [dispatch] tables differ from \
     the file in the working tree, or one purlis cannot read as a regular file. A chat never \
     changes what chats run under or who may dispatch to whom. Take it out of the commit \
     (`git restore --staged <the file>`), or stage the file as it is. Do not use --no-verify; \
     the operator has been told.\n";

/// What a chat's push of a range that changes the allowlist is refused with.
pub const PUSH_ALLOWLIST_REFUSAL: &str = "purlis: push refused — it would publish a change to \
     .charter-scan-allow.toml, the scan's allowlist. The operator pushes an allowlist change \
     themselves. Do not use --no-verify; the operator has been told.\n";

/// The most findings a refusal lists; the rest are counted.
const LISTED: usize = 20;

/// What a refusal stops: a chat's commit, at `pre-commit` or `pre-merge-commit`, or its push,
/// at `pre-push`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    Commit,
    Push,
}

impl Stopped {
    fn word(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Push => "push",
        }
    }
}

/// What the commit or push is refused with, on standard error: every finding, masked.
pub fn refusal(stopped: Stopped, found: &[Finding]) -> String {
    let mut out = format!(
        "purlis: {} refused — it would publish what looks like a secret or personal data:\n",
        stopped.word()
    );
    for finding in found.iter().take(LISTED) {
        out.push_str(&format!("  {}\n", one_line(finding)));
    }
    if found.len() > LISTED {
        out.push_str(&format!("  … and {} more\n", found.len() - LISTED));
    }
    out.push_str(match stopped {
        Stopped::Commit => "Take it out of the change and commit again. ",
        Stopped::Push => {
            "It is in a commit the push would send, which the commit scan never saw. Take it out \
             of that commit's history before pushing (amend it, or rebase and edit the commit \
             that added it), and if it is a live credential, revoke it: removing it from the \
             files alone leaves it in history. "
        }
    });
    out.push_str(
        "A credential belongs in a vault (`purlis secret`), and personal data does not belong \
         in a repository. If a finding is not what it looks like, `purlis scan --explain` names \
         its rule and the entry that would let it through; tell the operator, who adds it to \
         .charter-scan-allow.toml in a commit of their own. Do not use --no-verify; the \
         operator has been told.\n",
    );
    out
}

/// What the app's needs-you item says about a refusal in `repo`: what was stopped, the
/// repository's name and the first finding, masked, with how many more there are.
pub fn summary(stopped: Stopped, repo: &Path, found: &[Finding]) -> String {
    let name = repo.file_name().map_or_else(
        || repo.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let first = found.first().map(one_line).unwrap_or_default();
    let what = stopped.word();
    match found.len() {
        0 | 1 => format!("{what} refused in {name}: {first}"),
        n => format!("{what} refused in {name}: {first} (and {} more)", n - 1),
    }
}

/// One finding as a line: where, what, and the value masked.
pub fn one_line(finding: &Finding) -> String {
    format!(
        "{}:{}  {}  {}",
        finding.path, finding.line, finding.kind, finding.masked
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testgit;

    /// A repository whose git directory sits beside its work tree, as a branch's does — and
    /// which a sandbox that keeps `.git/config` from being written can still make.
    fn repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let repo = base.join("work");
        let made = testgit::run(
            &base,
            &[
                "init",
                "-q",
                "-b",
                "main",
                "--separate-git-dir",
                &base.join("work.git").display().to_string(),
                &repo.display().to_string(),
            ],
        );
        assert!(made.ok(), "{}", made.err);
        testgit::run(&repo, &["config", "user.name", "t"]);
        testgit::run(&repo, &["config", "user.email", "t@example.invalid"]);
        (dir, repo)
    }

    fn key() -> String {
        ["ghp", "_0123456789abcdefABCDEF"].concat()
    }

    #[test]
    fn a_planted_key_and_email_in_the_staged_diff_are_found_masked_with_path_and_line() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("app.py"), "print('hi')\n").unwrap();
        testgit::run(&repo, &["add", "app.py"]);
        testgit::run(&repo, &["commit", "-q", "-m", "one"]);
        std::fs::write(
            repo.join("app.py"),
            format!(
                "print('hi')\nTOKEN = '{}'\nOWNER = 'ada@lovelace.dev'\n",
                key()
            ),
        )
        .unwrap();
        testgit::run(&repo, &["add", "app.py"]);

        let found = staged(&repo).unwrap();
        assert_eq!(
            found,
            vec![
                Finding {
                    path: "app.py".into(),
                    line: 2,
                    rule: "forge-token",
                    kind: "a token by its forge's prefix",
                    masked: "ghp_**********************".into(),
                    fingerprint: scanallow::fingerprint(&key()),
                },
                Finding {
                    path: "app.py".into(),
                    line: 3,
                    rule: "email",
                    kind: "an email address",
                    masked: "ad**************".into(),
                    fingerprint: scanallow::fingerprint("ada@lovelace.dev"),
                },
            ]
        );
        let said = refusal(Stopped::Commit, &found);
        assert!(
            !said.contains(&key()) && !said.contains("ada@lovelace.dev"),
            "{said}"
        );
        assert!(
            said.contains("app.py:2") && said.contains("app.py:3"),
            "{said}"
        );
    }

    #[test]
    fn only_added_lines_count_and_a_clean_diff_finds_nothing() {
        let (_dir, repo) = repo();
        // Already committed, and removed: neither is this commit publishing it.
        std::fs::write(repo.join("old.txt"), format!("{}\nkeep\n", key())).unwrap();
        std::fs::write(repo.join("owners"), "ada@lovelace.dev\n").unwrap();
        testgit::run(&repo, &["add", "."]);
        testgit::run(&repo, &["commit", "-q", "-m", "one"]);
        std::fs::write(repo.join("old.txt"), "keep\n").unwrap();
        std::fs::write(repo.join("owners"), "ada@lovelace.dev\nplain\n").unwrap();
        testgit::run(&repo, &["add", "."]);
        assert_eq!(staged(&repo).unwrap(), Vec::new());
    }

    #[test]
    fn a_new_file_and_a_binary_file_are_read_as_git_shows_them() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("new.env"), format!("\n\nX={}\n", key())).unwrap();
        std::fs::write(repo.join("blob.bin"), b"\0\0ada@lovelace.dev\0").unwrap();
        testgit::run(&repo, &["add", "."]);
        let found = staged(&repo).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|f| (f.path.as_str(), f.line))
                .collect::<Vec<_>>(),
            vec![("new.env", 3)]
        );
    }

    #[test]
    fn a_file_whose_name_git_quotes_is_scanned_under_its_own_name() {
        let (_dir, repo) = repo();
        for name in [
            "say \"hi\".txt",
            "tab\there.txt",
            "back\\slash.txt",
            "plain space.txt",
        ] {
            std::fs::write(repo.join(name), format!("{}\n", key())).unwrap();
        }
        testgit::run(&repo, &["add", "."]);

        let mut named: Vec<String> = staged(&repo).unwrap().into_iter().map(|f| f.path).collect();
        named.sort();

        assert_eq!(
            named,
            [
                "back\\slash.txt",
                "plain space.txt",
                "say \"hi\".txt",
                "tab\there.txt"
            ]
        );
    }

    /// [`in_diff`] of a diff that shows every line a run reaches: reading the file's new text
    /// refuses.
    fn in_diff_alone(diff: &str) -> Result<Read, String> {
        in_diff(diff, &mut |_, _| {
            Err("this diff was to be read alone".to_owned())
        })
    }

    /// A diff of `path` whose hunks are `(new start, lines)`, each line `+`, `-` or ` ` and its
    /// text.
    fn hunks(path: &str, hunks: &[(usize, &[(char, &str)])]) -> String {
        let mut diff = format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n");
        for (start, lines) in hunks {
            diff.push_str(&format!("@@ -{start} +{start} @@\n"));
            for (sign, text) in *lines {
                diff.push(*sign);
                diff.push_str(text);
                diff.push('\n');
            }
        }
        diff
    }

    /// #1346: an unchanged line that joins the first added line, and one the last added line
    /// joins, are read with the change — from the diff's context lines.
    #[test]
    fn a_credential_completed_across_an_unchanged_neighbour_is_found() {
        let token = crate::secretshape::escaped::token();
        let (head, tail) = (
            format!("note = \"\"\"{}\\", &token[..8]),
            format!("    {}\"\"\"", &token[8..]),
        );
        for lines in [
            [
                (' ', "[deploy]"),
                (' ', head.as_str()),
                ('+', tail.as_str()),
            ],
            [
                (' ', "[deploy]"),
                ('+', head.as_str()),
                (' ', tail.as_str()),
            ],
        ] {
            let found = in_diff_alone(&hunks("deploy/settings", &[(1, &lines)]))
                .unwrap()
                .found;
            assert_eq!(found.len(), 1, "{lines:?}: {found:?}");
            assert_eq!((found[0].line, found[0].rule), (2, "forge-token"));
            assert_eq!(found[0].fingerprint, scanallow::fingerprint(&token));
        }
        // A removal that makes the two halves neighbours publishes the joined value too.
        let seam = [(' ', head.as_str()), ('-', "old"), (' ', tail.as_str())];
        let found = in_diff_alone(&hunks("deploy/settings", &[(1, &seam)]))
            .unwrap()
            .found;
        assert_eq!(found.len(), 1, "{found:?}");
        // A removal in front of blank lines joins what is on either side of them.
        let blanks = [
            (' ', head.as_str()),
            ('-', "old"),
            (' ', ""),
            (' ', " \t"),
            (' ', tail.as_str()),
        ];
        let found = in_diff_alone(&hunks("deploy/settings", &[(1, &blanks)]))
            .unwrap()
            .found;
        assert_eq!(found.len(), 1, "{found:?}");
        // Both halves unchanged, and nothing between them removed: nothing this change
        // publishes.
        let before = [
            (' ', head.as_str()),
            (' ', tail.as_str()),
            ('+', "note = 1"),
        ];
        assert_eq!(
            in_diff_alone(&hunks("deploy/settings", &[(1, &before)]))
                .unwrap()
                .found,
            Vec::new()
        );
    }

    /// #1346: past the diff's context lines, the lines a run reaches are read from the file's
    /// new version, and only when a run reaches them.
    #[test]
    fn a_run_past_the_context_lines_reads_the_new_file_and_only_then() {
        let token = crate::secretshape::escaped::token();
        // Line 2 opens the run, lines 3-6 are blank, line 7 carries it on, line 8 is added:
        // the diff shows lines 5-8.
        let mid = format!("    {}\\", &token[4..10]);
        let tail = format!("{}\"\"\"", &token[10..]);
        let file = format!(
            "[deploy]\nnote = \"\"\"{}\\\n\n\n\n\n{mid}\n{tail}\nend = 1\n",
            &token[..4]
        );
        let lines = [
            (' ', ""),
            (' ', ""),
            (' ', mid.as_str()),
            ('+', tail.as_str()),
        ];
        let diff = hunks("deploy/settings", &[(5, &lines)]);
        let mut asked = Vec::new();
        let found = in_diff(&diff, &mut |commit, path| {
            asked.push((commit.to_owned(), path.to_owned()));
            Ok(file.clone())
        })
        .unwrap()
        .found;
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].line, found[0].rule), (2, "forge-token"));
        assert_eq!(asked, [(String::new(), "deploy/settings".to_owned())]);
        // A change no unchanged line joins asks for no file.
        let plain = hunks("a.txt", &[(4, &[(' ', "a"), ('+', "b"), (' ', "c")])]);
        let read = in_diff(&plain, &mut |_, _| panic!("read a file it did not need"));
        assert_eq!(read.unwrap().found, Vec::new());
        // A file that could not be read refuses rather than skips.
        let refused = in_diff(&diff, &mut |_, _| Err("gone".to_owned()));
        assert!(refused.is_err());
    }

    /// #1346: each look back over the lines before a change covers lines no other look does, so
    /// many changes among long runs of blank lines still read in one pass.
    #[test]
    fn many_changes_among_blank_lines_read_in_linear_time() {
        const GAP: usize = 64;
        crate::lineartime::reads_in_linear_time(1 << 10, |changes| {
            let mut file = String::new();
            let mut diff = String::from("diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n");
            for i in 0..changes {
                file.push_str(&"\n".repeat(GAP));
                let at = (i + 1) * (GAP + 1);
                file.push_str(&format!("line {i}\n"));
                diff.push_str(&format!("@@ -{at},0 +{at} @@\n+line {i}\n"));
            }
            let mut reads = 0;
            let found = in_diff(&diff, &mut |_, _| {
                reads += 1;
                Ok(file.clone())
            })
            .unwrap()
            .found;
            assert_eq!(found, Vec::new());
            assert_eq!(reads, 1);
        });
    }

    /// #1346: each commit of a push reads the lines around its changes from its own files.
    #[test]
    fn a_pushed_commits_neighbours_are_read_from_that_commit() {
        let token = crate::secretshape::escaped::token();
        let head = format!("note = \"\"\"{}\\", &token[..8]);
        let tail = format!("{}\"\"\"", &token[8..]);
        let diff = format!(
            "{COMMIT_MARKER}c0ffee\n\n{}{COMMIT_MARKER}beef\n\n{}",
            hunks("a.txt", &[(1, &[('+', "plain")])]),
            hunks("b.txt", &[(1, &[(' ', "one"), ('+', head.as_str())])]),
        );
        let mut asked = Vec::new();
        let found = in_diff(&diff, &mut |commit, path| {
            asked.push(format!("{commit}:{path}"));
            Ok(format!("one\n{head}\n{tail}\n"))
        })
        .unwrap()
        .found;
        assert_eq!(asked, ["beef:b.txt"]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].path.as_str(), found[0].line), ("b.txt", 2));
    }

    /// #1346, through real git: a commit that completes a credential an unchanged line began,
    /// past the context git shows, is refused; and so is a push of it.
    #[test]
    fn a_commit_and_a_push_completing_an_unchanged_lines_credential_are_refused() {
        let (_dir, repo) = repo();
        let token = crate::secretshape::escaped::token();
        let begun = format!(
            "[deploy]\nnote = \"\"\"{}\\\n\n\n\n\n\nend = 1\n",
            &token[..8]
        );
        std::fs::write(repo.join("settings"), &begun).unwrap();
        let base = commit_all(&repo, "one");
        let done = begun.replace("\n\nend = 1", &format!("\n{}\"\"\"\nend = 1", &token[8..]));
        std::fs::write(repo.join("settings"), &done).unwrap();
        testgit::run(&repo, &["add", "settings"]);

        let scan = checked(&repo).unwrap();
        assert_eq!(scan.refused.len(), 1, "{:?}", scan.refused);
        assert_eq!(
            (scan.refused[0].line, scan.refused[0].rule),
            (2, "forge-token")
        );
        assert_eq!(scan.refused[0].fingerprint, scanallow::fingerprint(&token));

        let tip = commit_all(&repo, "two");
        let pushed = pushed(&repo, "url", "url", &update(&tip, &base)).unwrap();
        assert_eq!(pushed.refused.len(), 1, "{:?}", pushed.refused);
    }

    /// A diff that adds `lines` to `path`, one hunk from line 1.
    fn adding(path: &str, lines: &[&str]) -> String {
        let mut diff = format!(
            "diff --git a/{path} b/{path}\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
            lines.len()
        );
        for line in lines {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
        diff
    }

    #[test]
    fn an_escape_in_front_of_an_email_names_it_once_by_the_value_it_spells() {
        // `\n` and `\0` are letters to the scan as written, so it reads a longer address than
        // the one the line spells; both are the same finding (#1315).
        // Built at run time, so this file carries no address the scan would stop.
        let address = ["ada", "@", "lovelace.dev"].concat();
        for line in [
            format!(r#""owner": "the deploy\n{address}""#),
            format!(r#""owner": "the deploy\0{address}""#),
        ] {
            let found = in_diff_alone(&adding("owners.json", &[&line])).unwrap();
            assert_eq!(found.found.len(), 2, "{line}: {:?}", found.found);
            let scan = through(String::new(), found);
            assert_eq!(scan.refused.len(), 1, "{line}: {:?}", scan.refused);
            assert_eq!(
                scan.refused[0].fingerprint,
                scanallow::fingerprint(&address),
                "{line}"
            );
        }
        // Only the spelling where the escape is: the same longer address elsewhere on the line
        // is its own finding.
        let two = format!(r#""a": "n{address}", "b": "\n{address}""#);
        let scan = through(
            String::new(),
            in_diff_alone(&adding("owners.json", &[&two])).unwrap(),
        );
        assert_eq!(scan.refused.len(), 2, "{:?}", scan.refused);
    }

    #[test]
    fn one_spelling_an_entry_lets_through_and_one_it_does_not_are_both_kept() {
        let finding = |value: &str| Finding {
            path: "fixture.txt".into(),
            line: 1,
            rule: "forge-token",
            kind: "a token by its forge's prefix",
            masked: secretshape::masked(value),
            fingerprint: scanallow::fingerprint(value),
        };
        let token = crate::secretshape::escaped::token();
        let longer = format!("{token}AB");
        let read = Read {
            found: vec![finding(&longer), finding(&token)],
            twin: vec![Some(1), None],
            binary: Vec::new(),
        };
        let allowing_the_shorter = format!(
            "[[allow]]\nfingerprint = \"{}\"\nreason = \"a revoked fixture\"\n",
            scanallow::fingerprint(&token)
        );
        let scan = through(allowing_the_shorter, read);
        assert_eq!(scan.refused.len(), 1, "{:?}", scan.refused);
        assert_eq!(scan.refused[0].fingerprint, scanallow::fingerprint(&longer));
        assert_eq!(scan.allowed.len(), 1);
    }

    #[test]
    fn a_credential_split_by_a_line_ending_backslash_is_read_across_the_added_lines() {
        let token = crate::secretshape::escaped::token();
        let diff = adding(
            "deploy/settings",
            &[
                "[deploy]",
                &format!("note = \"\"\"{}\\", &token[..8]),
                &format!("    {}\"\"\"", &token[8..]),
            ],
        );
        let found = in_diff_alone(&diff).unwrap().found;
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].line, found[0].rule), (2, "forge-token"));
        assert_eq!(found[0].fingerprint, scanallow::fingerprint(&token));
        // A line that does not end in one is not joined to the next.
        let apart = adding(
            "deploy/settings",
            &[&format!("{}\\ x", &token[..8]), &token[8..]],
        );
        assert_eq!(in_diff_alone(&apart).unwrap().found, Vec::new());
    }

    #[test]
    fn a_line_ending_backslash_with_blanks_after_it_joins_across_blank_lines() {
        // TOML's line-ending backslash swallows the blanks after it on its line and every
        // blank line up to the next that is not: the shape the train's CI found (#1315).
        let token = crate::secretshape::escaped::token();
        let first = format!("note = \"\"\"{}\\  ", &token[..8]);
        let last = format!("    {}\"\"\"", &token[8..]);
        let found = in_diff_alone(&adding("deploy", &[&first, "", " \t", &last, "note = 1"]))
            .unwrap()
            .found;
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].line, found[0].rule), (1, "forge-token"));
        assert_eq!(found[0].fingerprint, scanallow::fingerprint(&token));
        // A blank line with no join before it starts nothing.
        let apart = in_diff_alone(&adding("deploy", &["", &token[..8], &token[8..]]))
            .unwrap()
            .found;
        assert_eq!(apart, Vec::new());
    }

    #[test]
    fn many_escaped_addresses_and_many_joined_lines_read_in_linear_time() {
        crate::lineartime::reads_in_linear_time(1 << 10, |addresses| {
            // Every value its own, so no seen list is short-cut by a repeat.
            let line: String = (0..addresses)
                .map(|i| format!(r"\na{i}x@lovelace{i}.dev "))
                .collect();
            let scan = through(
                String::new(),
                in_diff_alone(&adding("a.txt", &[&line])).unwrap(),
            );
            assert_eq!(scan.refused.len(), addresses);
            let joined: Vec<String> = (0..2 * addresses)
                .map(|i| format!("b{i}x@lovelace{i}.dev a{i}\\"))
                .collect();
            let mut joined: Vec<&str> = joined.iter().map(String::as_str).collect();
            joined.push("end");
            let found = in_diff_alone(&adding("b.toml", &joined)).unwrap().found;
            assert!(found.len() >= 2 * addresses, "{}", found.len());
        });
    }

    #[test]
    fn a_header_that_cannot_be_read_refuses_rather_than_skips() {
        for diff in [
            "diff --git a/x b/x\n+++ \"b/unterminated\n@@ -0,0 +1 @@\n+x\n",
            "diff --git a/x b/x\n+++ elsewhere/x\n@@ -0,0 +1 @@\n+x\n",
            "diff --git a/x b/x\n+++ b/x\n@@ nonsense @@\n+x\n",
            "diff --git a/x b/x\n@@ -0,0 +1 @@\n+x\n",
        ] {
            assert!(in_diff_alone(diff).is_err(), "{diff:?}");
        }
        assert_eq!(
            unquoted(r#""b/a\"b\\c\td\303\251""#).as_deref(),
            Some("b/a\"b\\c\tdé")
        );
    }

    /// Commits `.charter-scan-allow.toml` with `text` at `HEAD`, as the operator would.
    fn allowlist_at_head(repo: &std::path::Path, text: &str) {
        std::fs::write(repo.join(scanallow::FILE), text).unwrap();
        testgit::run(repo, &["add", scanallow::FILE]);
        testgit::run(repo, &["commit", "-q", "-m", "allowlist"]);
    }

    const DOCS_EMAIL: &str = "[[allow]]\nrule = \"email\"\npaths = [\"docs/**\"]\nreason = \"the docs name their authors\"\n";

    #[test]
    fn an_allowlisted_fixture_passes_and_says_which_entry_let_it() {
        let (_dir, repo) = repo();
        allowlist_at_head(&repo, DOCS_EMAIL);
        std::fs::create_dir(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/intro.md"), "Written by ada@lovelace.dev\n").unwrap();
        testgit::run(&repo, &["add", "docs/intro.md"]);

        let scan = checked(&repo).unwrap();

        assert!(scan.refused.is_empty(), "{:?}", scan.refused);
        assert_eq!(scan.allowed.len(), 1);
        assert_eq!(scan.allowed[0].1.origin, scanallow::Origin::File(1));
        assert!(!scan.changes_the_allowlist);
    }

    #[test]
    fn an_entry_written_and_not_committed_allows_nothing() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("README.md"), "one\n").unwrap();
        testgit::run(&repo, &["add", "README.md"]);
        testgit::run(&repo, &["commit", "-q", "-m", "one"]);
        // In the working tree only: what an agent could write and not commit.
        std::fs::write(repo.join(scanallow::FILE), DOCS_EMAIL).unwrap();
        std::fs::create_dir(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/intro.md"), "ada@lovelace.dev\n").unwrap();
        testgit::run(&repo, &["add", "docs/intro.md"]);

        let scan = checked(&repo).unwrap();

        assert_eq!(scan.refused.len(), 1);
        // And staging it is the change a chat's commit may not make.
        testgit::run(&repo, &["add", scanallow::FILE]);
        assert!(checked(&repo).unwrap().changes_the_allowlist);
    }

    #[test]
    fn a_staged_project_file_whose_dispatch_or_sandbox_differs_from_the_working_one_is_flagged() {
        // #1464: a change to `[dispatch]` or `[sandbox]` put into the index without touching
        // the file in the working tree, which a sandboxed chat is denied writing.
        let (_dir, repo) = repo();
        let file = crate::names::PLANE_MANIFEST.write;
        let base = "schema = 1\n[dispatch.grants]\nsteward = [\"devops\"]\n";
        std::fs::write(repo.join(file), base).unwrap();
        testgit::run(&repo, &["add", file]);
        testgit::run(&repo, &["commit", "-q", "-m", "base"]);
        assert!(!checked(&repo).unwrap().stages_what_a_chat_runs_under);

        // Staged as the working tree holds it: a person's change, theirs to commit.
        let renamed = "schema = 1\nname = \"x\"\n[dispatch.grants]\nsteward = [\"devops\"]\n";
        std::fs::write(repo.join(file), renamed).unwrap();
        testgit::run(&repo, &["add", file]);
        assert!(!checked(&repo).unwrap().stages_what_a_chat_runs_under);
        let wider = "schema = 1\n[dispatch.grants]\nsteward = [\"devops\", \"*\"]\n";
        std::fs::write(repo.join(file), wider).unwrap();
        testgit::run(&repo, &["add", file]);
        assert!(!checked(&repo).unwrap().stages_what_a_chat_runs_under);

        // The working file put back, the wider grant left in the index.
        std::fs::write(repo.join(file), base).unwrap();
        assert!(checked(&repo).unwrap().stages_what_a_chat_runs_under);
        // A change outside those tables, staged alone, is not this.
        testgit::run(&repo, &["add", file]);
        std::fs::write(repo.join(file), renamed).unwrap();
        assert!(!checked(&repo).unwrap().stages_what_a_chat_runs_under);

        // The file deleted from the index while the working tree keeps it.
        std::fs::write(repo.join(file), base).unwrap();
        testgit::run(&repo, &["rm", "-q", "--cached", file]);
        assert!(checked(&repo).unwrap().stages_what_a_chat_runs_under);
    }

    #[test]
    fn a_project_in_a_folder_of_a_bigger_repository_is_held_to_the_same_rule() {
        // #1464 review: git names the file from the top of the work tree (`ops/purlis.toml`).
        let (_dir, repo) = repo();
        let file = format!("ops/{}", crate::names::PLANE_MANIFEST.write);
        std::fs::create_dir(repo.join("ops")).unwrap();
        let base = "schema = 1\n[dispatch.grants]\nsteward = [\"devops\"]\n";
        std::fs::write(repo.join(&file), base).unwrap();
        testgit::run(&repo, &["add", &file]);
        testgit::run(&repo, &["commit", "-q", "-m", "base"]);

        let wider = "schema = 1\n[dispatch.grants]\nsteward = [\"devops\", \"*\"]\n";
        std::fs::write(repo.join(&file), wider).unwrap();
        testgit::run(&repo, &["add", &file]);
        assert!(!checked(&repo).unwrap().stages_what_a_chat_runs_under);
        std::fs::write(repo.join(&file), base).unwrap();
        assert!(checked(&repo).unwrap().stages_what_a_chat_runs_under);
        // And under another case, as a case-insensitive disk opens it.
        assert!(is_a_project_file("ops/PURLIS.TOML"));
        assert!(is_a_project_file("a/b/charter.local.toml"));
        assert!(!is_a_project_file("ops/purlis.toml.bak"));
    }

    #[cfg(unix)]
    #[test]
    fn a_project_file_staged_as_a_link_is_flagged_whatever_it_points_at() {
        // #1464 review: the index entry's kind, not only its text.
        let (_dir, repo) = repo();
        let file = crate::names::PLANE_MANIFEST.write;
        std::fs::write(repo.join("other.toml"), "schema = 1\n").unwrap();
        std::fs::write(repo.join(file), "schema = 1\n").unwrap();
        testgit::run(&repo, &["add", "other.toml", file]);
        testgit::run(&repo, &["commit", "-q", "-m", "base"]);
        // Staged as a link, with a regular file of the same text left in the working tree.
        std::fs::remove_file(repo.join(file)).unwrap();
        std::os::unix::fs::symlink("other.toml", repo.join(file)).unwrap();
        testgit::run(&repo, &["add", file]);
        std::fs::remove_file(repo.join(file)).unwrap();
        std::fs::write(repo.join(file), "schema = 1\n").unwrap();
        assert!(checked(&repo).unwrap().stages_what_a_chat_runs_under);
    }

    #[test]
    fn an_author_in_a_manifest_is_let_through_with_no_allowlist_at_all() {
        let (_dir, repo) = repo();
        std::fs::write(
            repo.join("Cargo.toml"),
            "[package]\nauthors = [\"Ada <ada@lovelace.dev>\"]\n",
        )
        .unwrap();
        std::fs::write(
            repo.join("src.rs"),
            format!("// ada@lovelace.dev {}\n", key()),
        )
        .unwrap();
        testgit::run(&repo, &["add", "."]);

        let scan = checked(&repo).unwrap();

        assert_eq!(
            scan.allowed
                .iter()
                .map(|(f, _)| f.path.as_str())
                .collect::<Vec<_>>(),
            ["Cargo.toml"]
        );
        assert_eq!(
            scan.refused.iter().map(|f| f.rule).collect::<Vec<_>>(),
            ["email", "forge-token"]
        );
    }

    #[test]
    fn a_broken_allowlist_says_so_and_allows_nothing() {
        let (_dir, repo) = repo();
        allowlist_at_head(&repo, "[[allow]]\nrule = \"email\"\n");
        std::fs::write(repo.join("notes.md"), "ada@lovelace.dev\n").unwrap();
        testgit::run(&repo, &["add", "notes.md"]);

        let scan = checked(&repo).unwrap();

        assert_eq!(scan.refused.len(), 1);
        assert!(scan.problems[0].contains("reason"), "{:?}", scan.problems);
    }

    #[test]
    fn moving_the_allowlist_away_is_a_change_to_it() {
        let (_dir, repo) = repo();
        allowlist_at_head(&repo, DOCS_EMAIL);
        testgit::run(&repo, &["mv", scanallow::FILE, "elsewhere.toml"]);

        assert!(checked(&repo).unwrap().changes_the_allowlist);
    }

    // The rename window (RN-2a, V93e): the allowlist under either name.

    #[test]
    fn an_allowlist_under_the_purlis_name_is_read_and_wins_over_the_old_one() {
        let (_dir, repo) = repo();
        // The old file allows nothing in docs/; the purlis one does, and wins.
        std::fs::write(repo.join(".charter-scan-allow.toml"), "").unwrap();
        std::fs::write(repo.join(".purlis-scan-allow.toml"), DOCS_EMAIL).unwrap();
        testgit::run(&repo, &["add", "."]);
        testgit::run(&repo, &["commit", "-q", "-m", "allowlists"]);
        std::fs::create_dir(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/intro.md"), "Written by ada@lovelace.dev\n").unwrap();
        testgit::run(&repo, &["add", "docs/intro.md"]);

        let scan = checked(&repo).unwrap();

        assert!(scan.refused.is_empty(), "{:?}", scan.refused);
        assert_eq!(scan.allowed[0].1.origin, scanallow::Origin::File(1));
    }

    #[test]
    fn a_staged_allowlist_under_either_name_is_a_change_to_it() {
        for name in [".charter-scan-allow.toml", ".purlis-scan-allow.toml"] {
            let (_dir, repo) = repo();
            std::fs::write(repo.join("README.md"), "one\n").unwrap();
            testgit::run(&repo, &["add", "README.md"]);
            testgit::run(&repo, &["commit", "-q", "-m", "one"]);
            std::fs::write(repo.join(name), DOCS_EMAIL).unwrap();
            testgit::run(&repo, &["add", name]);

            assert!(checked(&repo).unwrap().changes_the_allowlist, "{name}");
        }
    }

    // `pushed` (SQ-7).

    const NOTHING: &str = "0000000000000000000000000000000000000000";

    fn commit_all(repo: &Path, message: &str) -> String {
        testgit::run(repo, &["add", "-A"]);
        testgit::run(repo, &["commit", "-q", "-m", message]);
        testgit::run(repo, &["rev-parse", "HEAD"])
            .out
            .trim()
            .to_owned()
    }

    fn update(local: &str, theirs: &str) -> String {
        format!("refs/heads/main {local} refs/heads/main {theirs}\n")
    }

    #[test]
    fn a_push_to_a_url_scans_what_the_remote_lacks_and_not_what_it_has() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("old.sh"), format!("T={}\n", key())).unwrap();
        let published = commit_all(&repo, "published long ago");
        std::fs::write(repo.join("owners"), "ada@lovelace.dev\n").unwrap();
        let tip = commit_all(&repo, "new");

        let url = "https://forge.invalid/o/r.git";
        let scan = pushed(&repo, url, url, &update(&tip, &published)).unwrap();

        assert_eq!(
            scan.refused
                .iter()
                .map(|f| (f.path.as_str(), f.rule))
                .collect::<Vec<_>>(),
            [("owners", "email")]
        );
    }

    #[test]
    fn a_new_branch_is_scanned_back_to_its_root_and_a_deletion_sends_nothing() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("old.sh"), format!("T={}\n", key())).unwrap();
        let tip = commit_all(&repo, "root");

        let url = "/elsewhere.git";
        assert_eq!(
            pushed(&repo, url, url, &update(&tip, NOTHING))
                .unwrap()
                .refused
                .len(),
            1
        );
        assert!(
            pushed(&repo, url, url, &update(NOTHING, &tip))
                .unwrap()
                .refused
                .is_empty()
        );
    }

    #[test]
    fn the_remotes_commit_this_clone_never_fetched_does_not_stop_the_scan() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("owners"), "ada@lovelace.dev\n").unwrap();
        let tip = commit_all(&repo, "new");
        let unknown = "1234567890abcdef1234567890abcdef12345678";

        let scan = pushed(&repo, "/r.git", "/r.git", &update(&tip, unknown)).unwrap();

        assert_eq!(scan.refused.len(), 1);
    }

    #[test]
    fn input_git_did_not_write_is_refused_not_waved_through() {
        let (_dir, repo) = repo();
        for updates in ["refs/heads/main abc", "a b c d e", "r not-an-oid r 0000"] {
            assert!(pushed(&repo, "o", "/o", updates).is_err(), "{updates}");
        }
        assert!(pushed(&repo, "o", "/o", "").unwrap().refused.is_empty());
    }

    #[test]
    fn a_push_refusal_says_the_value_is_in_history_and_never_shows_it() {
        let finding = Finding {
            path: "deploy.sh".into(),
            line: 1,
            rule: "forge-token",
            kind: "a token by its forge's prefix",
            masked: "ghp_****".into(),
            fingerprint: String::new(),
        };
        let said = refusal(Stopped::Push, std::slice::from_ref(&finding));
        assert!(said.starts_with("purlis: push refused"), "{said}");
        assert!(said.contains("history"), "{said}");
        assert_eq!(
            summary(Stopped::Push, Path::new("/x/app"), &[finding]),
            "push refused in app: deploy.sh:1  a token by its forge's prefix  ghp_****"
        );
    }

    #[test]
    fn a_new_branchs_root_commit_is_read_even_where_log_hides_root_diffs() {
        let (_dir, repo) = repo();
        testgit::run(&repo, &["config", "log.showRoot", "false"]);
        std::fs::write(repo.join("deploy.sh"), format!("T={}\n", key())).unwrap();
        let root = commit_all(&repo, "root");

        let scan = pushed(&repo, "/r.git", "/r.git", &update(&root, NOTHING)).unwrap();

        assert_eq!(scan.refused.len(), 1, "{scan:?}");
    }

    #[test]
    fn a_tag_that_names_a_blob_is_refused_and_an_annotated_tag_is_read_as_its_commit() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("loose"), format!("T={}\n", key())).unwrap();
        let blob = testgit::run(&repo, &["hash-object", "-w", "loose"])
            .out
            .trim()
            .to_owned();
        let tag = format!("refs/tags/x {blob} refs/tags/x {NOTHING}\n");
        let why = pushed(&repo, "/r.git", "/r.git", &tag).unwrap_err();
        assert!(
            why.contains("refs/tags/x") && why.contains("not name a commit"),
            "{why}"
        );

        std::fs::remove_file(repo.join("loose")).unwrap();
        std::fs::write(repo.join("owners"), "ada@lovelace.dev\n").unwrap();
        commit_all(&repo, "owners");
        testgit::run(&repo, &["tag", "-a", "-m", "v1", "v1"]);
        let annotated = testgit::run(&repo, &["rev-parse", "v1"])
            .out
            .trim()
            .to_owned();
        let tag = format!("refs/tags/v1 {annotated} refs/tags/v1 {NOTHING}\n");
        assert_eq!(
            pushed(&repo, "/r.git", "/r.git", &tag)
                .unwrap()
                .refused
                .len(),
            1
        );
    }

    #[test]
    fn a_pushed_range_that_changes_the_allowlist_says_so() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("README"), "x\n").unwrap();
        let published = commit_all(&repo, "published");
        allowlist_at_head(&repo, DOCS_EMAIL);
        let tip = testgit::run(&repo, &["rev-parse", "HEAD"])
            .out
            .trim()
            .to_owned();

        let scan = pushed(&repo, "/r.git", "/r.git", &update(&tip, &published)).unwrap();

        assert!(scan.changes_the_allowlist);
        assert!(
            !pushed(&repo, "/r.git", "/r.git", &update(&published, NOTHING))
                .unwrap()
                .changes_the_allowlist,
            "a range that does not touch the file"
        );
    }

    #[test]
    fn findings_go_through_the_allowlist_the_remote_has_not_the_checkouts() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("README"), "x\n").unwrap();
        let published = commit_all(&repo, "published");
        // The remote's commit has no allowlist; the branch pushed carries a docs email.
        std::fs::create_dir(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/intro.md"), "By ada@lovelace.dev\n").unwrap();
        let tip = commit_all(&repo, "docs");
        // The checkout is elsewhere, with an allowlist that would let it through.
        testgit::run(&repo, &["checkout", "-q", "-b", "elsewhere", &published]);
        allowlist_at_head(&repo, DOCS_EMAIL);

        let scan = pushed(&repo, "/r.git", "/r.git", &update(&tip, &published)).unwrap();

        assert_eq!(scan.refused.len(), 1, "{scan:?}");
        assert!(scan.allowed.is_empty());
    }

    #[test]
    fn an_allowlist_the_remote_already_has_lets_a_finding_through() {
        let (_dir, repo) = repo();
        allowlist_at_head(&repo, DOCS_EMAIL);
        let published = testgit::run(&repo, &["rev-parse", "HEAD"])
            .out
            .trim()
            .to_owned();
        std::fs::create_dir(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/intro.md"), "By ada@lovelace.dev\n").unwrap();
        let tip = commit_all(&repo, "docs");

        let scan = pushed(&repo, "/r.git", "/r.git", &update(&tip, &published)).unwrap();

        assert!(scan.refused.is_empty(), "{scan:?}");
        assert_eq!(scan.allowed.len(), 1);
    }

    /// The value an escaped shape spells: the AWS key or the forge token it carries.
    fn spelled(kind: &str) -> String {
        if kind == "AWS access key" {
            ["AKIA", "IOSFODNN7EXAMPLE"].concat()
        } else {
            crate::secretshape::escaped::token()
        }
    }

    #[test]
    fn a_credential_spelled_with_escapes_on_an_added_line_is_found_by_the_value_it_spells() {
        // Every bare-credential spelling of #1295, NO-7 and #1304, one added line at a time:
        // the escapes decoded with no parse, in a file of any name, parsed or not.
        for shape in crate::secretshape::escaped::shapes()
            .into_iter()
            .filter(|shape| shape.token)
        {
            let (_dir, repo) = repo();
            let file = repo.join(shape.path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, &shape.text).unwrap();
            testgit::run(&repo, &["add", "."]);

            let found = staged(&repo).unwrap();
            let value = spelled(shape.kind);
            assert_eq!(found.len(), 1, "{}: {found:?}", shape.text);
            let one = &found[0];
            assert_eq!(
                (one.path.as_str(), one.kind),
                (shape.path, shape.kind),
                "{}",
                shape.text
            );
            assert_eq!(
                one.fingerprint,
                scanallow::fingerprint(&value),
                "{}",
                shape.text
            );
            assert_eq!(one.masked, secretshape::masked(&value), "{}", shape.text);
            let line = shape.text.lines().nth(one.line - 1).unwrap();
            assert!(line.contains('\\'), "{}: line {}", shape.text, one.line);
            let said = refusal(Stopped::Commit, &found);
            assert!(!said.contains(&value[4..]), "{said}");
        }
    }

    #[test]
    fn a_push_of_a_credential_spelled_with_escapes_is_refused() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("README.md"), "one\n").unwrap();
        let published = commit_all(&repo, "one");
        let tail = &crate::secretshape::escaped::token()[1..];
        std::fs::write(
            repo.join("events.jsonl"),
            format!("{{\"n\": \"\\u0067{tail}\"}}\n"),
        )
        .unwrap();
        let tip = commit_all(&repo, "new");

        let url = "https://forge.invalid/o/r.git";
        let scan = pushed(&repo, url, url, &update(&tip, &published)).unwrap();

        assert_eq!(
            scan.refused
                .iter()
                .map(|f| (f.path.as_str(), f.rule))
                .collect::<Vec<_>>(),
            [("events.jsonl", "forge-token")]
        );
    }

    #[test]
    fn an_entry_naming_a_value_lets_it_through_however_it_is_spelled() {
        let (_dir, repo) = repo();
        let value = crate::secretshape::escaped::token();
        allowlist_at_head(
            &repo,
            &format!(
                "[[allow]]\nfingerprint = \"{}\"\nreason = \"a revoked fixture\"\n",
                scanallow::fingerprint(&value)
            ),
        );
        std::fs::write(
            repo.join("fixture.json"),
            format!("{{\"t\": \"\\u0067{}\"}}\n", &value[1..]),
        )
        .unwrap();
        testgit::run(&repo, &["add", "fixture.json"]);

        let scan = checked(&repo).unwrap();

        assert!(scan.refused.is_empty(), "{:?}", scan.refused);
        assert_eq!(scan.allowed.len(), 1);
    }

    #[test]
    fn a_file_with_escapes_and_no_credential_finds_nothing() {
        for (path, text) in crate::secretshape::escaped::clean() {
            let (_dir, repo) = repo();
            let file = repo.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, &text).unwrap();
            testgit::run(&repo, &["add", "."]);
            assert_eq!(staged(&repo).unwrap(), Vec::new(), "{text}");
        }
    }

    /// An address, built where it is used so no source line holds one.
    fn address() -> String {
        ["ada", "@lovelace.dev"].concat()
    }

    /// The rule and file of each finding refused.
    fn refused(scan: &Scan) -> Vec<(&str, &str)> {
        scan.refused
            .iter()
            .map(|f| (f.path.as_str(), f.rule))
            .collect()
    }

    /// SQ-7: a text file an attribute, a diff driver or a lowered threshold has git show as
    /// "Binary files … differ" is read as the text it is, at commit and at push.
    #[test]
    fn a_text_file_git_only_shows_as_binary_is_read_as_text() {
        let (_dir, repo) = repo();
        std::fs::write(
            repo.join(".gitattributes"),
            "*.cfg binary\n*.ini -diff\n*.conf diff=opaque\n",
        )
        .unwrap();
        testgit::run(&repo, &["config", "diff.opaque.binary", "true"]);
        let published = commit_all(&repo, "attributes");
        for name in ["a.cfg", "b.ini", "c.conf"] {
            std::fs::write(repo.join(name), format!("x\nT={}\n", key())).unwrap();
        }
        testgit::run(&repo, &["add", "-A"]);

        let mut found: Vec<(String, usize)> = staged(&repo)
            .unwrap()
            .into_iter()
            .map(|f| (f.path, f.line))
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                ("a.cfg".to_owned(), 2),
                ("b.ini".to_owned(), 2),
                ("c.conf".to_owned(), 2)
            ]
        );

        let tip = commit_all(&repo, "three files");
        let scan = pushed(&repo, "/r.git", "/r.git", &update(&tip, &published)).unwrap();
        assert_eq!(scan.refused.len(), 3, "{:?}", scan.refused);

        // A threshold lowered so far that every file is "big" shows nothing as text either.
        testgit::run(&repo, &["config", "core.bigFileThreshold", "1"]);
        std::fs::write(repo.join("plain.txt"), format!("T={}\n", key())).unwrap();
        testgit::run(&repo, &["add", "plain.txt"]);
        assert_eq!(staged(&repo).unwrap().len(), 1);
    }

    /// SQ-7 must not refuse what was never a secret: a real binary file — one an attribute
    /// calls binary, and one git finds binary by its bytes — a deleted binary file, an LFS
    /// pointer and a submodule are committed and pushed as before.
    #[test]
    fn real_binaries_lfs_pointers_submodules_and_deletions_are_still_let_through() {
        let (_dir, repo) = repo();
        std::fs::write(
            repo.join(".gitattributes"),
            "*.png binary\n*.psd filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        // What a binary file can hold among its bytes: an address, a key's shape.
        let binary = |n: u8| {
            let mut bytes = vec![0x89, b'P', b'N', b'G', 0, 0, n];
            bytes.extend(format!("{} {}", address(), key()).as_bytes());
            bytes.push(0);
            bytes
        };
        std::fs::write(repo.join("gone.png"), binary(1)).unwrap();
        let published = commit_all(&repo, "a picture");
        std::fs::remove_file(repo.join("gone.png")).unwrap();
        std::fs::write(repo.join("logo.png"), binary(2)).unwrap();
        std::fs::write(repo.join("raw.bin"), binary(3)).unwrap();
        std::fs::write(
            repo.join("art.psd"),
            format!(
                "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 1048576\n",
                "4d7a2146".repeat(8)
            ),
        )
        .unwrap();
        testgit::run(&repo, &["add", "-A"]);
        testgit::run(
            &repo,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{published},vendored"),
            ],
        );

        assert_eq!(staged(&repo).unwrap(), Vec::new());
        let tip = commit_all(&repo, "pictures, a pointer and a submodule");
        let scan = pushed(&repo, "/r.git", "/r.git", &update(&tip, &published)).unwrap();
        assert_eq!(refused(&scan), []);
    }

    /// A commit made with `git commit-tree`, `parents` its parents, of `tree`.
    fn commit_tree(repo: &Path, tree: &str, parents: &[&str], message: &str) -> String {
        let mut args = vec!["commit-tree", tree, "-m", message];
        for parent in parents {
            args.extend(["-p", parent]);
        }
        testgit::run(repo, &args).out.trim().to_owned()
    }

    /// SQ-7: a merge whose own tree adds a value neither parent holds — a resolution, or a
    /// merge `commit-tree` made of any tree, two parents or more — is read at push. A merge
    /// that adds nothing of its own reads nothing again of what its sides published.
    #[test]
    fn what_only_a_merge_adds_is_read_at_push_and_its_sides_are_not_read_twice() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("f"), "a\nb\nc\n").unwrap();
        let base = commit_all(&repo, "base");
        testgit::run(&repo, &["switch", "-q", "-c", "side"]);
        std::fs::write(repo.join("owners"), format!("{}\n", address())).unwrap();
        let side = commit_all(&repo, "side");
        testgit::run(&repo, &["switch", "-q", "main"]);
        std::fs::write(repo.join("g"), "g\n").unwrap();
        let ours = commit_all(&repo, "ours");
        // The side is published: the remote's tracking ref holds it.
        testgit::run(&repo, &["update-ref", "refs/remotes/origin/side", &side]);

        // An honest merge: nothing of its own.
        testgit::run(&repo, &["merge", "-q", "--no-edit", "side"]);
        let honest = testgit::run(&repo, &["rev-parse", "HEAD"])
            .out
            .trim()
            .to_owned();
        let scan = pushed(&repo, "origin", "/r.git", &update(&honest, &ours)).unwrap();
        assert_eq!(refused(&scan), []);

        // The same parents, and a tree with a credential no parent has.
        std::fs::write(repo.join("evil.sh"), format!("T={}\n", key())).unwrap();
        testgit::run(&repo, &["add", "evil.sh"]);
        let tree = testgit::run(&repo, &["write-tree"]).out.trim().to_owned();
        let evil = commit_tree(&repo, &tree, &[&ours, &side], "merge");
        let scan = pushed(&repo, "origin", "/r.git", &update(&evil, &ours)).unwrap();
        assert_eq!(refused(&scan), [("evil.sh", "forge-token")]);

        // Three parents: git does not merge them again; the merge is read against its first.
        testgit::run(&repo, &["switch", "-q", "-c", "third", &base]);
        std::fs::write(repo.join("h"), "h\n").unwrap();
        let third = commit_all(&repo, "third");
        testgit::run(&repo, &["update-ref", "refs/remotes/origin/third", &third]);
        let octopus = commit_tree(&repo, &tree, &[&ours, &side, &third], "octopus");
        let scan = pushed(&repo, "origin", "/r.git", &update(&octopus, &ours)).unwrap();
        assert!(
            refused(&scan).contains(&("evil.sh", "forge-token")),
            "{:?}",
            scan.refused
        );
    }

    /// SQ-7: a commit's message and an annotated tag's are published with them, and read for
    /// credentials. An address in a message — a trailer's — is not refused.
    #[test]
    fn a_credential_in_a_commit_or_tag_message_is_refused_at_push_and_a_trailer_is_not() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("f"), "one\n").unwrap();
        let published = commit_all(&repo, "one");
        std::fs::write(repo.join("f"), "two\n").unwrap();
        let ordinary = commit_all(
            &repo,
            &format!(
                "Two\n\nCo-Authored-By: Ada <{0}>\nSigned-off-by: Ada <{0}>",
                address()
            ),
        );
        let scan = pushed(&repo, "/r.git", "/r.git", &update(&ordinary, &published)).unwrap();
        assert_eq!(refused(&scan), []);

        std::fs::write(repo.join("f"), "three\n").unwrap();
        let told = commit_all(&repo, &format!("Three\n\nuse {} to push", key()));
        let scan = pushed(&repo, "/r.git", "/r.git", &update(&told, &ordinary)).unwrap();
        let short = &told[..12];
        assert_eq!(
            refused(&scan),
            [(
                format!("commit {short} (its message)").as_str(),
                "forge-token"
            )]
        );
        assert_eq!(scan.refused[0].line, 3);
        let said = refusal(Stopped::Push, &scan.refused);
        assert!(!said.contains(&key()), "{said}");

        testgit::run(
            &repo,
            &[
                "tag",
                "-a",
                "-m",
                &format!("v1\n\n{}", key()),
                "v1",
                &ordinary,
            ],
        );
        let tag = testgit::run(&repo, &["rev-parse", "v1"])
            .out
            .trim()
            .to_owned();
        let update = format!("refs/tags/v1 {tag} refs/tags/v1 {NOTHING}\n");
        let scan = pushed(&repo, "/r.git", "/r.git", &update).unwrap();
        assert!(
            refused(&scan).contains(&("tag refs/tags/v1 (its message)", "forge-token")),
            "{:?}",
            scan.refused
        );
    }

    /// An object written as `text`, of git's type `kind`, as `hash-object` stores it.
    fn object(repo: &Path, kind: &str, text: &str) -> String {
        let file = repo.parent().unwrap().join(format!("{kind}.object"));
        std::fs::write(&file, text).unwrap();
        let made = testgit::run(
            repo,
            &["hash-object", "-t", kind, "-w", &file.display().to_string()],
        );
        assert!(made.ok(), "{}", made.err);
        made.out.trim().to_owned()
    }

    /// #1133: a commit object publishes every line it holds, not only its message: a header
    /// git keeps for itself (a merged tag's whole text, under `mergetag`) or one it does not
    /// know. An annotated tag's own header lines are published with it too. Each is read for
    /// credentials, as the message is; the people a header names are not refused.
    #[test]
    fn a_credential_in_a_commits_or_tags_header_lines_is_refused_at_push() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("f"), "one\n").unwrap();
        let published = commit_all(&repo, "one");
        let tree = testgit::run(&repo, &["rev-parse", "HEAD^{tree}"])
            .out
            .trim()
            .to_owned();
        let who = format!("Ada <{}> 1 +0000", address());
        let header = |extra: &str| {
            format!(
                "tree {tree}\nparent {published}\nauthor {who}\ncommitter {who}\n{extra}\n\nplain\n"
            )
        };
        // The people a commit's header names are not a finding.
        let plain = object(&repo, "commit", &header("x-note nothing here"));
        let scan = pushed(&repo, "/r.git", "/r.git", &update(&plain, &published)).unwrap();
        assert_eq!(refused(&scan), []);

        for extra in [
            format!("x-note {}", key()),
            format!(
                "mergetag object {published}\n type commit\n tag v0\n tagger {who}\n \n told {}",
                key()
            ),
        ] {
            let told = object(&repo, "commit", &header(&extra));
            let scan = pushed(&repo, "/r.git", "/r.git", &update(&told, &published)).unwrap();
            let short = &told[..12];
            assert_eq!(
                refused(&scan),
                [(
                    format!("commit {short} (its header)").as_str(),
                    "forge-token"
                )],
                "{extra}"
            );
            assert!(!refusal(Stopped::Push, &scan.refused).contains(&key()));
        }

        let tag = object(
            &repo,
            "tag",
            &format!(
                "object {published}\ntype commit\ntag v2\ntagger {who}\nx-note {}\n\nv2\n",
                key()
            ),
        );
        let update = format!("refs/tags/v2 {tag} refs/tags/v2 {NOTHING}\n");
        let scan = pushed(&repo, "/r.git", "/r.git", &update).unwrap();
        assert_eq!(
            refused(&scan),
            [("tag refs/tags/v2 (its header)", "forge-token")],
        );
    }

    /// #1133: what git writes a message in is the repository's config to choose
    /// (`i18n.logOutputEncoding`), and a message written in an encoding the scan does not read
    /// would be read as nothing. The push scan asks for UTF-8 whatever the config says.
    #[test]
    fn a_messages_credential_is_found_whatever_encoding_the_repository_asks_git_to_write_in() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("f"), "one\n").unwrap();
        let published = commit_all(&repo, "one");
        std::fs::write(repo.join("f"), "two\n").unwrap();
        let told = commit_all(&repo, &format!("Two\n\nuse {} to push", key()));
        testgit::run(&repo, &["config", "i18n.logOutputEncoding", "UTF-16"]);
        let scan = pushed(&repo, "/r.git", "/r.git", &update(&told, &published)).unwrap();
        let short = &told[..12];
        assert_eq!(
            refused(&scan),
            [(
                format!("commit {short} (its message)").as_str(),
                "forge-token"
            )]
        );
    }

    /// #1133: the message a chat's own commit is about to be made with is read at commit, for
    /// credentials only, as a push reads it, and the staged diff is the commit scan's.
    #[test]
    fn a_credential_in_the_message_being_committed_is_refused_and_a_trailer_is_not() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("f"), "one\n").unwrap();
        commit_all(&repo, "one");

        let plain = format!("Two\n\nCo-Authored-By: Ada <{}>\n", address());
        assert_eq!(refused(&message_checked(&repo, &plain)), []);

        let told = format!("Two\n\nuse {} to push\n", key());
        let scan = message_checked(&repo, &told);
        assert_eq!(refused(&scan), [(MESSAGE, "forge-token")]);
        assert_eq!(scan.refused[0].line, 3);
        assert!(!refusal(Stopped::Commit, &scan.refused).contains(&key()));

        // A line that only looks like the scissors, with text of the message's own in front,
        // cuts nothing.
        let written = format!(
            "Two\nsee ------------------------ >8 ------------------------\n{}\n",
            key()
        );
        assert_eq!(
            refused(&message_checked(&repo, &written)),
            [(MESSAGE, "forge-token")]
        );
    }

    /// What git keeps below a scissors line is committed wherever nothing cuts there
    /// (`--cleanup=verbatim` or `whitespace`, no `-v`), and the hook cannot tell which, so it is
    /// read. Below the line, a credential the repository already holds in a file the commit
    /// changes is not new: that is the diff `commit -v` shows there, which git cuts.
    #[test]
    fn a_credential_below_the_scissors_line_is_refused_unless_the_commit_already_holds_it() {
        let (_dir, repo) = repo();
        let cut = "# ------------------------ >8 ------------------------";
        std::fs::write(repo.join("f"), format!("T={}\n", key())).unwrap();
        commit_all(&repo, "one");

        let other = ["ghp", "_9876543210fedcbaFEDCBA"].concat();
        for below in [
            format!("{other}\n"),
            format!("diff --git a/f b/f\n-T={other}\n"),
            format!("+T={other}\n"),
        ] {
            let message = format!("Two\n{cut}\n{below}");
            let scan = message_checked(&repo, &message);
            assert_eq!(refused(&scan), [(MESSAGE, "forge-token")], "{message}");
            assert_eq!(scan.refused[0].line, below.lines().count() + 2);
        }
        // Nothing staged: what HEAD holds is in no file the commit changes, so it is read.
        let unchanged = format!("Two\n{cut}\n T={}\n", key());
        assert_eq!(
            refused(&message_checked(&repo, &unchanged)),
            [(MESSAGE, "forge-token")]
        );

        // `commit -v` removing the credential: the diff below the line shows it, and HEAD
        // holds it in the file the commit changes.
        std::fs::write(repo.join("f"), "one\n").unwrap();
        testgit::run(&repo, &["add", "f"]);
        let verbose = format!(
            "Remove it\n{cut}\n# Do not modify or remove the line above.\n# Everything below \
             it will be ignored.\ndiff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-T={}\n+one\n",
            key()
        );
        assert_eq!(refused(&message_checked(&repo, &verbose)), []);
        // Above the line the same value is the message's own, and read as ever.
        let above = format!("Remove T={}\n{cut}\n-T={}\n", key(), key());
        let scan = message_checked(&repo, &above);
        assert_eq!(refused(&scan), [(MESSAGE, "forge-token")]);
        assert_eq!(scan.refused[0].line, 1);
    }

    /// git's comment mark may be several characters (`core.commentString`, or `core.commentChar`
    /// set to more than one), and its scissors line is that mark, a space and the cut: a
    /// `commit -v` removing a credential under one is not refused for it. A mark git is not set
    /// to cuts nothing, a credential HEAD does not hold is read below the cut as ever, and the
    /// one a commit stages is the staged diff's to refuse.
    #[test]
    fn a_scissors_line_under_a_comment_mark_of_several_characters_cuts() {
        for key_name in ["core.commentString", "core.commentChar"] {
            let (_dir, repo) = repo();
            std::fs::write(repo.join("f"), format!("T={}\n", key())).unwrap();
            commit_all(&repo, "one");
            let set = testgit::run(&repo, &["config", key_name, "//"]);
            assert!(set.ok(), "{}", set.err);
            std::fs::write(repo.join("f"), "one\n").unwrap();
            testgit::run(&repo, &["add", "f"]);
            let verbose = |mark: &str, removed: &str| {
                format!(
                    "Remove it\n{mark} ------------------------ >8 ------------------------\n\
                     {mark} Do not modify or remove the line above.\n{mark} Everything below \
                     it will be ignored.\ndiff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n\
                     -T={removed}\n+one\n"
                )
            };
            assert_eq!(
                refused(&message_checked(&repo, &verbose("//", &key()))),
                [],
                "{key_name}"
            );
            for mark in ["%%", "///"] {
                let scan = message_checked(&repo, &verbose(mark, &key()));
                assert_eq!(
                    refused(&scan),
                    [(MESSAGE, "forge-token")],
                    "{key_name} {mark}"
                );
            }
            let other = ["ghp", "_9876543210fedcbaFEDCBA"].concat();
            assert_eq!(
                refused(&message_checked(&repo, &verbose("//", &other))),
                [(MESSAGE, "forge-token")],
                "{key_name}"
            );
            std::fs::write(repo.join("f"), format!("T={other}\n")).unwrap();
            testgit::run(&repo, &["add", "f"]);
            let staged = checked(&repo).unwrap();
            assert_eq!(refused(&staged), [("f", "forge-token")], "{key_name}");
        }
    }

    /// `commit -v -v` puts the unstaged diff below the cut too. A credential only the working
    /// tree holds is in no commit and not in the index, and a message that carries it below a
    /// line the hook cannot tell git cuts is refused: a false refusal that fails closed, kept,
    /// since letting it through would let a message carry a credential nothing else does.
    #[test]
    fn a_credential_only_the_working_tree_holds_is_refused_below_the_cut() {
        let (_dir, repo) = repo();
        std::fs::write(repo.join("f"), "one\n").unwrap();
        commit_all(&repo, "one");
        std::fs::write(repo.join("f"), "two\n").unwrap();
        testgit::run(&repo, &["add", "f"]);
        std::fs::write(repo.join("f"), format!("two\nT={}\n", key())).unwrap();
        let message = format!(
            "Two\n# ------------------------ >8 ------------------------\n\
             diff --git c/f i/f\n--- c/f\n+++ i/f\n@@ -1 +1 @@\n-one\n+two\n\
             --------------------------------------------------\nChanges not staged for commit:\n\
             diff --git i/f w/f\n--- i/f\n+++ w/f\n@@ -1 +1,2 @@\n two\n+T={}\n",
            key()
        );
        assert_eq!(
            refused(&message_checked(&repo, &message)),
            [(MESSAGE, "forge-token")]
        );
    }

    #[test]
    fn a_sections_file_is_read_from_its_diff_line_where_both_sides_name_it() {
        assert_eq!(header_path("a/x y b/x y").as_deref(), Some("x y"));
        assert_eq!(header_path("a/x b/y"), None);
        assert_eq!(
            header_path(r#""a/t\there" "b/t\there""#).as_deref(),
            Some("t\there")
        );
        assert_eq!(header_path("a/ b"), None);
    }

    #[test]
    fn a_binary_line_in_a_section_whose_file_cannot_be_named_refuses() {
        let diff = "diff --git a/x b/y\nindex 1..2 100644\nBinary files a/x and b/y differ\n";
        assert!(in_diff_alone(diff).is_err());
        let deleted = "diff --git a/x b/y\nBinary files a/x and /dev/null differ\n";
        assert!(in_diff_alone(deleted).is_ok());
    }
}
