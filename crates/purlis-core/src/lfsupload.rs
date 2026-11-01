// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A push never leaves a pointer nobody can download** (#4399). Purlis's git runs with hooks
//! off, so git-lfs's `pre-push` never runs. Before a push of a repository purlis saves or a
//! branch folder's push, [`before_push`] finds the Git LFS pointers in the commits going out and
//! uploads their objects. Where it cannot, the push does not happen, and the sentence says why.
//!
//! **A repository with no pointer going out meets none of this.** It reads what the push itself
//! is about to walk, and never starts git-lfs.
//!
//! **What the upload can reach, and with what** (`crates/purlis-core/docs/git-policy.md`):
//!
//! - **Only the LFS address of the URL the push itself goes to**, derived as git-lfs derives it
//!   and given on the command line ([`PINNED`]), which beats a committed `.lfsconfig` and the
//!   repository's own config. A chat can write the first. Over HTTPS only.
//! - **Only the helper the push has**, on the same one-credential rule ([`git::spawn_network`]).
//! - **The server may then name where the bytes go**, as a forge names its storage. That is the
//!   forge's answer, not a chat's, and the helper is asked about that host like any other.
//! - **Only objects this push needs, whose bytes hash to their pointer**, copied into a store of
//!   this call's own first ([`Store`]), under the machine's folder, which no chat can write. A
//!   chat can write `.git/lfs`, and this process runs outside its sandbox: a link at any level
//!   there, a second name for a file, or a file swapped after it was checked, reaches nothing.
//! - **No program.** A repository whose config names one git-lfs would start is refused.

use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::cloning::{self, Followed};
use crate::worktree::git;

/// One push, as [`before_push`] needs it.
pub(crate) struct Push<'a> {
    /// The folder the push runs in.
    pub dir: &'a Path,
    /// The helper the push has, the only one the upload has.
    pub helper: Option<&'a str>,
    /// What the push will be given, from its verb on, which says where it goes.
    pub args: &'a [&'a str],
    /// The commit going out.
    pub tip: &'a str,
    /// The remote whose tracking refs say what is there already.
    pub remote: &'a str,
}

/// The most a pointer can be, as git-lfs reads one.
const POINTER_MOST: u64 = 1024;
/// Below the least a pointer can be, in any spelling git-lfs reads.
const POINTER_LEAST: u64 = 100;
/// How many objects one `git lfs push` names, so that a command line always fits.
const NAMED_AT_ONCE: usize = 1000;
/// The deadline for reading what a push sends, which can be a whole history at a first push.
const READING: Duration = cloning::STALL;

/// A pointer in a commit going out.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pointer {
    oid: String,
    size: u64,
    /// A path it is at, for a sentence to name.
    path: String,
}

/// A stand-in for the forge in a test: a remote on this machine has no LFS address of its own.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct StandIn {
    pub endpoint: String,
    pub helper: Option<String>,
    pub stall: Duration,
}

#[cfg(test)]
thread_local! {
    pub(crate) static STAND_IN: std::cell::RefCell<Option<StandIn>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn stand_in() -> Option<StandIn> {
    STAND_IN.with(|held| held.borrow().clone())
}

/// Uploads every object the push needs, or says in one sentence why the push must not happen.
pub(crate) fn before_push(push: &Push) -> Result<(), String> {
    let pointers = going_out(push.dir, push.tip, push.remote)?;
    if pointers.is_empty() {
        return Ok(());
    }
    let endpoints = endpoints_of(push)?;
    // First, and before git-lfs is asked anything: `git lfs` can itself be a repository's alias.
    if git::lfs_would_start_a_program(push.dir).map_err(|e| e.to_string())? {
        return Err(format!(
            "This push carries {}, and this repository's git config names a program or setting \
             git-lfs would use, so nothing was pushed. Remove it from .git/config, or push from \
             your own terminal",
            carried(pointers.len())
        ));
    }
    let lfs = git::run(push.dir, &["lfs", "version"], git::READ).map_err(|e| e.to_string())?;
    if !lfs.ok() {
        return Err(format!(
            "This push carries {} and git-lfs is not on this machine, so nothing was pushed. \
             Install git-lfs and push again",
            carried(pointers.len())
        ));
    }
    let store = Store::of(push.dir, &pointers)?;
    let oids: Vec<&str> = pointers.iter().map(|one| one.oid.as_str()).collect();
    for endpoint in &endpoints {
        upload(push, endpoint, &store, &oids)?;
    }
    Ok(())
}

fn carried(count: usize) -> String {
    if count == 1 {
        "a Git LFS file".to_owned()
    } else {
        format!("{count} Git LFS files")
    }
}

/// The pointers in what the tip has that the remote's tracking refs do not, whatever the
/// attributes say: `pre-push` reads them the same way. Each object once.
fn going_out(dir: &Path, tip: &str, remote: &str) -> Result<Vec<Pointer>, String> {
    let failed = |why: String| format!("purlis could not read what this push sends: {why}");
    crate::hookwire::alive::beat();
    // `--ignore-missing` below is for the tracking refs: a tip that resolves to nothing refuses.
    let commit = format!("{tip}^{{commit}}");
    let resolved = git::run(
        dir,
        &["rev-parse", "--verify", "--quiet", commit.as_str()],
        git::READ,
    )
    .map_err(|e| failed(e.to_string()))?;
    if !resolved.ok() {
        return Err(failed(format!(
            "{} names no commit",
            crate::shown::line(tip)
        )));
    }
    // Read one at a time, rather than `--not --remotes`: one broken ref must not refuse a push.
    // A ref that says nothing only makes the walk longer.
    let tracking = format!("refs/remotes/{remote}/");
    let there = git::run(
        dir,
        &["for-each-ref", "--format=^%(objectname)", tracking.as_str()],
        git::READ,
    )
    .map_err(|e| failed(e.to_string()))?;
    let there = if there.ok() { there.out } else { String::new() };
    let listed = git::run_with_input(
        dir,
        &[
            "-c",
            "core.quotePath=false",
            "-c",
            "core.useReplaceRefs=false",
            "rev-list",
            "--objects",
            "--filter=blob:limit=1025",
            "--ignore-missing",
            "--stdin",
            tip,
        ],
        there.into_bytes(),
        READING,
    )
    .map_err(|e| failed(e.to_string()))?;
    if listed.code != Some(0) {
        return Err(failed(
            String::from_utf8_lossy(&listed.err).trim().to_owned(),
        ));
    }
    let listed_out = String::from_utf8_lossy(&listed.out);
    let named: Vec<(&str, &str)> = listed_out
        .lines()
        .filter_map(|line| line.split_once(' '))
        .collect();
    if named.is_empty() {
        return Ok(Vec::new());
    }
    let ask = |args: &[&str], input: Vec<u8>| -> Result<Vec<u8>, String> {
        crate::hookwire::alive::beat();
        let run =
            git::run_with_input(dir, args, input, READING).map_err(|e| failed(e.to_string()))?;
        if run.code == Some(0) {
            Ok(run.out)
        } else {
            Err(failed(String::from_utf8_lossy(&run.err).trim().to_owned()))
        }
    };
    let input = named
        .iter()
        .flat_map(|(id, _)| format!("{id}\n").into_bytes())
        .collect();
    let checked = ask(
        &[
            "-c",
            "core.useReplaceRefs=false",
            "cat-file",
            "--batch-check",
        ],
        input,
    )?;
    let sized: Vec<&(&str, &str)> = String::from_utf8_lossy(&checked)
        .lines()
        .zip(&named)
        .filter(|(line, _)| {
            let mut words = line.split(' ').skip(1);
            words.next() == Some("blob")
                && words
                    .next()
                    .and_then(|size| size.parse::<u64>().ok())
                    .is_some_and(|size| (POINTER_LEAST..=POINTER_MOST).contains(&size))
        })
        .map(|(_, one)| one)
        .collect();
    if sized.is_empty() {
        return Ok(Vec::new());
    }
    let input = sized
        .iter()
        .flat_map(|(id, _)| format!("{id}\n").into_bytes())
        .collect();
    let blobs = ask(
        &["-c", "core.useReplaceRefs=false", "cat-file", "--batch"],
        input,
    )?;
    let mut rest = blobs.as_slice();
    let mut pointers: Vec<Pointer> = Vec::new();
    for (_, path) in sized {
        let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
            return Err(failed("git's answer ended early".to_owned()));
        };
        let head = String::from_utf8_lossy(&rest[..end]);
        let size: usize = head
            .rsplit(' ')
            .next()
            .and_then(|size| size.parse().ok())
            .ok_or_else(|| failed("git's answer did not read as it should".to_owned()))?;
        let Some(body) = rest.get(end + 1..end + 1 + size) else {
            return Err(failed("git's answer ended early".to_owned()));
        };
        match pointer_in(body) {
            Blob::Other => {}
            Blob::Extended => {
                return Err(format!(
                    "{} is a Git LFS file that uses an extension, which purlis cannot upload, so \
                     nothing was pushed. Push from your own terminal",
                    crate::shown::line(path)
                ));
            }
            Blob::Pointer(oid, size) => {
                if !pointers.iter().any(|one| one.oid == oid) {
                    pointers.push(Pointer {
                        oid,
                        size,
                        path: (*path).to_owned(),
                    });
                }
            }
        }
        rest = rest.get(end + 2 + size..).unwrap_or_default();
    }
    Ok(pointers)
}

/// What a blob is, read as git-lfs reads a pointer: lines of a key and a value, blank lines and
/// a carriage return let by, the version first.
#[derive(Debug, PartialEq, Eq)]
enum Blob {
    Other,
    Pointer(String, u64),
    /// A pointer that names extensions, which purlis does not upload.
    Extended,
}

fn pointer_in(blob: &[u8]) -> Blob {
    let Ok(text) = std::str::from_utf8(blob) else {
        return Blob::Other;
    };
    let mut lines = text
        .trim()
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| !line.is_empty());
    if !matches!(
        lines.next(),
        Some(
            "version https://git-lfs.github.com/spec/v1"
                | "version https://hawser.github.com/spec/v1"
                | "version http://git-media.io/v/2"
        )
    ) {
        return Blob::Other;
    }
    let (mut oid, mut size, mut extended) = (None, None, false);
    for line in lines {
        match line.split_once(' ') {
            Some(("oid", value)) if oid.is_none() => oid = value.strip_prefix("sha256:"),
            Some(("size", value)) if size.is_none() => size = Some(value),
            Some((key, _)) if key.starts_with("ext-") => extended = true,
            _ => return Blob::Other,
        }
    }
    let (Some(oid), Some(size)) = (oid, size) else {
        return Blob::Other;
    };
    if oid.len() != 64
        || !oid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Blob::Other;
    }
    if !size.bytes().all(|b| b.is_ascii_digit()) {
        return Blob::Other;
    }
    let Ok(size) = size.parse() else {
        return Blob::Other;
    };
    if extended {
        Blob::Extended
    } else {
        Blob::Pointer(oid.to_owned(), size)
    }
}

/// Where each object goes: the LFS address of every URL the push goes to.
fn endpoints_of(push: &Push) -> Result<Vec<String>, String> {
    #[cfg(test)]
    if let Some(stand_in) = stand_in() {
        return Ok(vec![stand_in.endpoint]);
    }
    let urls = git::push_destinations(push.dir, push.args);
    if urls.is_empty() {
        return Err(
            "This push carries Git LFS files and purlis could not tell where it goes, so nothing \
             was pushed. Push from your own terminal"
                .to_owned(),
        );
    }
    urls.iter().map(|url| endpoint_of(url)).collect()
}

/// The address git-lfs derives from a remote's URL, with no user or password in it, or the
/// sentence that refuses a remote that is not plain HTTPS.
fn endpoint_of(url: &str) -> Result<String, String> {
    let refused = || {
        format!(
            "This push carries Git LFS files, and purlis uploads them only over HTTPS, which {} \
             is not, so nothing was pushed. Push from your own terminal",
            crate::shown::line(&without_user(url))
        )
    };
    let Some(rest) = url
        .get(.."https://".len())
        .filter(|scheme| scheme.eq_ignore_ascii_case("https://"))
        .map(|_| &url["https://".len()..])
    else {
        return Err(refused());
    };
    if rest.is_empty()
        || rest
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '?' || c == '#')
    {
        return Err(refused());
    }
    let (place, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = place.rsplit('@').next().unwrap_or(place);
    if host.is_empty() {
        return Err(refused());
    }
    let path = path.trim_end_matches('/');
    let repo = if path.ends_with(".git") {
        path.to_owned()
    } else {
        format!("{path}.git")
    };
    Ok(format!("https://{host}/{repo}/info/lfs"))
}

/// `url` with any user and password taken out, for a sentence.
fn without_user(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    let (place, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = place.rsplit('@').next().unwrap_or(place);
    format!("{scheme}://{host}/{path}")
}

/// **The objects this push sends, in a folder of this call's own.** Each is read once, not
/// following a link, and hashed as it is copied, so what git-lfs reads is what was checked.
struct Store {
    held: tempfile::TempDir,
}

impl Store {
    fn of(dir: &Path, pointers: &[Pointer]) -> Result<Self, String> {
        let could_not = |why: std::io::Error| {
            format!("purlis could not set the Git LFS files aside to upload them: {why}")
        };
        let theirs = store_of(dir)?;
        let aside = aside().ok_or_else(|| {
            "purlis could not find its own folder to set the Git LFS files aside in, so nothing \
             was pushed"
                .to_owned()
        })?;
        std::fs::create_dir_all(&aside).map_err(could_not)?;
        clear_what_a_crash_left(&aside);
        let held = tempfile::Builder::new()
            .prefix("upload-")
            .tempdir_in(&aside)
            .map_err(could_not)?;
        // A sparse file a chat planted would be written out whole here.
        let needed: u64 = pointers
            .iter()
            .map(|one| one.size)
            .fold(0, u64::saturating_add);
        let free = rustix::fs::statvfs(held.path())
            .map(|at| at.f_bavail.saturating_mul(at.f_frsize))
            .map_err(|why| could_not(why.into()))?;
        if needed >= free {
            return Err(format!(
                "This push's Git LFS files need more free space than this machine has to set \
                 them aside for upload, so nothing was pushed. Free {} and push again",
                crate::shown::line(&format!("{needed} bytes"))
            ));
        }
        let store = Self { held };
        let mut unread = Vec::new();
        for one in pointers {
            let to = object_path(&store.held.path().join("objects"), &one.oid);
            if let Some(text) = copied(&theirs, &to, one, &could_not)? {
                unread.push((one.path.clone(), text));
            }
        }
        for (path, text) in &unread {
            let scan = crate::diffscan::in_large_file(dir, path, text);
            if !scan.refused.is_empty() {
                return Err(crate::diffscan::refusal(
                    crate::diffscan::Stopped::Push,
                    &scan.refused,
                )
                .trim()
                .to_owned());
            }
        }
        Ok(store)
    }

    fn path(&self) -> &Path {
        self.held.path()
    }
}

/// Where this call sets objects aside, in the machine's own folder, which no chat can write
/// (`docs/plane-format.md`).
const ASIDE: &str = "lfs-upload";

/// What a call older than this left behind can only be a crash's.
const A_CRASH_LEFT_IT: Duration = Duration::from_secs(7 * 24 * 60 * 60);

fn aside() -> Option<PathBuf> {
    // A test's build is fenced from this machine's own folder.
    #[cfg(test)]
    if stand_in().is_some() {
        return Some(std::env::temp_dir().join(ASIDE));
    }
    crate::machine::config_root().map(|config| crate::machine::dir(&config).join(ASIDE))
}

fn clear_what_a_crash_left(aside: &Path) {
    let Ok(entries) = std::fs::read_dir(aside) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|found| found.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age > A_CRASH_LEFT_IT);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Where to find the objects: a folder that is trusted as it stands, and the one in it that a
/// chat can write, opened like everything below it.
struct Theirs {
    root: PathBuf,
    inside: Vec<String>,
}

/// The folder git-lfs keeps objects under here: yours where your own config moved it (a
/// repository's own is refused before this), else the clone's. Its own `objects` is below it.
fn store_of(dir: &Path) -> Result<Theirs, String> {
    let lost = || "purlis could not find this clone's Git LFS files".to_owned();
    let ask = |args: &[&str]| git::run(dir, args, git::READ).map_err(|e| e.to_string());
    let common = ask(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    if !common.ok() {
        return Err(lost());
    }
    let common = PathBuf::from(common.line().trim());
    let moved = ask(&["config", "--get", "lfs.storage"])?;
    let moved = moved.line().trim();
    Ok(if moved.is_empty() {
        Theirs {
            root: common,
            inside: vec!["lfs".to_owned()],
        }
    } else if Path::new(moved).is_absolute() {
        Theirs {
            root: PathBuf::from(moved),
            inside: Vec::new(),
        }
    } else {
        // Relative to the git directory, which a chat can write in: opened like what is below.
        let inside: Vec<String> = moved
            .split('/')
            .filter(|name| !name.is_empty() && *name != ".")
            .map(str::to_owned)
            .collect();
        if inside.iter().any(|name| name == "..") {
            return Err(lost());
        }
        Theirs {
            root: common,
            inside,
        }
    })
}

/// The object `oid` opened through `store`, one folder at a time, following no link in any of
/// them and never waiting on a pipe: `None` where it is not there.
fn opened(store: &Theirs, oid: &str) -> std::io::Result<Option<std::fs::File>> {
    use rustix::fs::{CWD, Mode, OFlags, openat};
    let folder = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let mut at = openat(CWD, &store.root, folder, Mode::empty())?;
    let held = folder | OFlags::NOFOLLOW;
    let below = ["objects", &oid[..2], &oid[2..4]];
    for name in store.inside.iter().map(String::as_str).chain(below) {
        at = match openat(&at, name, held, Mode::empty()) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(why) => return Err(why.into()),
        };
    }
    let file = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    match openat(&at, oid, file, Mode::empty()) {
        Ok(found) => Ok(Some(std::fs::File::from(found))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(why) => Err(why.into()),
    }
}

/// `<root>/aa/bb/<oid>`, as git-lfs lays its objects out.
fn object_path(root: &Path, oid: &str) -> PathBuf {
    root.join(&oid[..2]).join(&oid[2..4]).join(oid)
}

/// One object copied from `from` to `to`, or the sentence that refuses the push. Its text, where
/// it is text the scan of a push would read, for [`crate::diffscan::in_large_file`].
fn copied(
    from: &Theirs,
    to: &Path,
    one: &Pointer,
    could_not: &dyn Fn(std::io::Error) -> String,
) -> Result<Option<Vec<u8>>, String> {
    let short = &one.oid[..12];
    let name = crate::shown::line(&one.path);
    let missing = || {
        format!(
            "{name} is a Git LFS file this push needs (object {short}), and its object is not in \
             this clone, so nothing was pushed. Run `git lfs fetch --all` in your own terminal, \
             then push again"
        )
    };
    let wrong = || {
        format!(
            "{name} is a Git LFS file this push needs (object {short}), and the copy in this \
             clone is not what its pointer names, so nothing was pushed. Remove that copy from \
             .git/lfs/objects, run `git lfs fetch --all` in your own terminal, then push again"
        )
    };
    let mut source = match opened(from, &one.oid) {
        Ok(Some(source)) => source,
        Ok(None) => return Err(missing()),
        Err(_) => return Err(wrong()),
    };
    let found = source.metadata().map_err(could_not)?;
    // More than one name can be a name for something outside the store.
    if !found.is_file() || found.len() != one.size || found.nlink() != 1 {
        return Err(wrong());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(could_not)?;
    }
    let mut copy = std::fs::File::create(to).map_err(could_not)?;
    let mut head = Vec::new();
    let keep = usize::try_from(one.size)
        .ok()
        .filter(|size| *size as u64 <= crate::diffscan::PROBE_MOST);
    let (read, oid) = capped_copy(&mut source, &mut copy, one.size, keep.is_some(), &mut head)
        .map_err(could_not)?;
    copy.sync_all().map_err(could_not)?;
    if read != one.size || oid != one.oid {
        return Err(wrong());
    }
    let is_text = keep.is_some() && crate::diffscan::reads_as_text(&head);
    Ok(is_text.then_some(head))
}

/// `source` copied to `copy`, read no further than one byte past `size`: a file a chat grows
/// after it was measured stops here. How much was read, and its hash; its bytes in `head` where
/// `keeping`.
fn capped_copy(
    source: &mut impl Read,
    copy: &mut impl Write,
    size: u64,
    keeping: bool,
    head: &mut Vec<u8>,
) -> std::io::Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut capped = source.take(size.saturating_add(1));
    let mut chunk = vec![0; 256 * 1024];
    let mut read: u64 = 0;
    loop {
        let got = capped.read(&mut chunk)?;
        if got == 0 {
            break;
        }
        read += got as u64;
        crate::hookwire::alive::beat();
        if read > size {
            break;
        }
        hash.update(&chunk[..got]);
        copy.write_all(&chunk[..got])?;
        if keeping {
            head.extend_from_slice(&chunk[..got]);
        }
    }
    Ok((read, crate::extension::hex(&hash.finalize())))
}

/// **What the upload is held to**, each after the repository's own config and `.lfsconfig`, so
/// each beats them. `lfs.url` and `lfs.pushurl` are the only keys git-lfs reads ahead of a
/// remote's own address ([`upload`] adds their value).
const PINNED: [&str; 3] = [
    "lfs.allowincompletepush=false",
    "lfs.standalonetransferagent=",
    "lfs.transfer.enablehrefrewrite=false",
];

/// `oids` sent to `endpoint`, followed to their end under the stall limit, listed in the network
/// log as git's own push is.
fn upload(push: &Push, endpoint: &str, store: &Store, oids: &[&str]) -> Result<(), String> {
    let config: Vec<String> = PINNED
        .iter()
        .map(|one| (*one).to_owned())
        .chain([
            format!("lfs.url={endpoint}"),
            format!("lfs.pushurl={endpoint}"),
            format!("lfs.storage={}", store.path().display()),
        ])
        .collect();
    let pass = vec![("GIT_LFS_FORCE_PROGRESS".to_owned(), "1".into())];
    #[cfg(test)]
    let stood_in = stand_in();
    #[cfg(test)]
    let helper = stood_in
        .as_ref()
        .map_or(push.helper, |one| one.helper.as_deref());
    #[cfg(not(test))]
    let helper = push.helper;
    #[cfg(test)]
    let stall = stood_in.as_ref().map_or(cloning::STALL, |one| one.stall);
    #[cfg(not(test))]
    let stall = cloning::STALL;
    let started = Instant::now();
    let never = AtomicBool::new(false);
    let mut ok = true;
    let outcome = (|| {
        for named in oids.chunks(NAMED_AT_ONCE) {
            let mut args = vec!["lfs", "push", "--object-id", endpoint];
            args.extend_from_slice(named);
            let child = match git::spawn_network(push.dir, helper, &config, pass.clone(), &args) {
                Ok(Ok(child)) => child,
                Ok(Err(why)) => return Err(not_uploaded(endpoint, &why)),
                Err(gone) => return Err(not_uploaded(endpoint, &gone.to_string())),
            };
            match cloning::follow(child, &never, Some(stall), &mut |_| {}) {
                Ok(Followed::Ended { ok: true, .. }) => {}
                Ok(Followed::Ended { said, .. }) => {
                    let last = said
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .map_or("git-lfs said nothing", String::as_str);
                    return Err(not_uploaded(endpoint, last));
                }
                Ok(Followed::Stalled { .. }) => {
                    return Err(format!(
                        "Git LFS sent nothing to {} for {} minutes, so it was stopped and nothing \
                         was pushed. Push again when the network is back",
                        host_of(endpoint),
                        stall.as_secs() / 60
                    ));
                }
                Ok(Followed::CalledOff) => {
                    return Err("The Git LFS upload was stopped, so nothing was pushed".to_owned());
                }
                Err(why) => return Err(not_uploaded(endpoint, &why.to_string())),
            }
        }
        Ok(())
    })();
    ok &= outcome.is_ok();
    crate::netlog::git_call("LFS-PUSH", endpoint, ok, started.elapsed());
    outcome
}

fn host_of(endpoint: &str) -> String {
    crate::shown::line(&crate::forge::host_of(endpoint))
}

/// `said` with each address's query taken out: a storage address can be signed in its query.
fn without_queries(said: &str) -> String {
    said.split(' ')
        .map(|word| {
            if word.contains("://") {
                word.split('?').next().unwrap_or(word)
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn not_uploaded(endpoint: &str, said: &str) -> String {
    format!(
        "Git LFS could not upload to {}, so nothing was pushed: {}. Check your access to that \
         repository's large files, then push again",
        host_of(endpoint),
        crate::shown::line(&without_queries(said))
    )
}

#[cfg(test)]
pub(crate) mod testserver;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testgit;

    /// A repository with one commit and a folder remote, which this module refuses if it ever
    /// looks at where a push goes.
    fn repo_with(file: &str, text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let repo = top.join("repo");
        std::fs::create_dir(&repo).unwrap();
        let ran = |args: &[&str]| assert!(testgit::run(&repo, args).ok(), "{args:?}");
        ran(&["init", "-q", "-b", "main", "."]);
        ran(&["config", "user.name", "T"]);
        ran(&["config", "user.email", "t@example.invalid"]);
        ran(&["remote", "add", "origin", "/nowhere/here.git"]);
        std::fs::write(repo.join(file), text).unwrap();
        ran(&["add", "-A"]);
        ran(&["commit", "-q", "-m", "one"]);
        (dir, repo)
    }

    fn asked<'a>(repo: &'a Path, args: &'a [&'a str]) -> Push<'a> {
        Push {
            dir: repo,
            helper: None,
            args,
            tip: "HEAD",
            remote: "origin",
        }
    }

    #[test]
    fn a_repository_with_no_pointer_going_out_never_reaches_git_lfs_or_its_remote() {
        let (_dir, repo) = repo_with("plain.txt", "not a pointer\n");
        assert_eq!(
            before_push(&asked(&repo, &["push", "origin", "main"])),
            Ok(())
        );
    }

    #[test]
    fn a_pointer_for_a_remote_that_is_not_https_refuses_the_push() {
        let (_dir, repo) = repo_with("large.bin", &pointer(OID, "12"));
        let said = before_push(&asked(&repo, &["push", "origin", "main"])).unwrap_err();
        assert!(said.contains("only over HTTPS"), "{said}");
    }

    #[test]
    fn pointers_already_on_the_remote_are_not_going_out() {
        let (_dir, repo) = repo_with("large.bin", &pointer(OID, "12"));
        assert_eq!(going_out(&repo, "HEAD", "origin").unwrap().len(), 1);
        assert!(testgit::run(&repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]).ok());
        assert_eq!(going_out(&repo, "HEAD", "origin").unwrap(), Vec::new());
    }

    #[test]
    fn a_tip_that_names_no_commit_refuses() {
        let (_dir, repo) = repo_with("plain.txt", "text\n");
        assert!(going_out(&repo, "refs/heads/nothing", "origin").is_err());
        std::fs::write(repo.join(".git/refs/heads/broken"), "garbage\n").unwrap();
        assert!(going_out(&repo, "refs/heads/broken", "origin").is_err());
    }

    #[test]
    fn a_broken_tracking_ref_refuses_nothing_and_a_good_one_still_counts() {
        let (_dir, repo) = repo_with("large.bin", &pointer(OID, "12"));
        let refs = repo.join(".git/refs/remotes/origin");
        std::fs::create_dir_all(&refs).unwrap();
        std::fs::write(refs.join("broken"), "not a ref\n").unwrap();
        assert_eq!(going_out(&repo, "HEAD", "origin").unwrap().len(), 1);
        assert!(testgit::run(&repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]).ok());
        assert_eq!(going_out(&repo, "HEAD", "origin").unwrap(), Vec::new());
    }

    #[test]
    fn a_pointer_going_out_is_named_by_its_path_whatever_the_attributes_say_and_once() {
        let (_dir, repo) = repo_with("large one.bin", &pointer(OID, "12"));
        std::fs::write(repo.join("again.bin"), pointer(OID, "12")).unwrap();
        assert!(testgit::run(&repo, &["add", "-A"]).ok());
        assert!(testgit::run(&repo, &["commit", "-q", "-m", "two"]).ok());
        let found = going_out(&repo, "HEAD", "origin").unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].oid, OID);
        assert_eq!(found[0].size, 12);
    }

    const OID: &str = "4d7a214614ab2935c943f9e0ff69d22eadbb8f3a7c4a1f5e0dbe7e4bc1f2a3d4";

    fn pointer(oid: &str, size: &str) -> String {
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {size}\n")
    }

    #[test]
    fn a_blob_is_a_pointer_only_where_it_reads_exactly_as_one() {
        assert_eq!(
            pointer_in(pointer(OID, "12").as_bytes()),
            Blob::Pointer(OID.to_owned(), 12)
        );
        for same in [
            pointer(OID, "12").trim_end().to_owned(),
            pointer(OID, "12").replace('\n', "\r\n"),
            pointer(OID, "12").replace('\n', "\n\n"),
            pointer(OID, "12").replace("git-lfs.github", "hawser.github"),
            pointer(OID, "12").replace(
                "https://git-lfs.github.com/spec/v1",
                "http://git-media.io/v/2",
            ),
            format!(" \n{}\n ", pointer(OID, "12")),
        ] {
            assert_eq!(
                pointer_in(same.as_bytes()),
                Blob::Pointer(OID.to_owned(), 12),
                "{same}"
            );
        }
        let extended = pointer(OID, "12").replace("oid", &format!("ext-0-x sha256:{OID}\noid"));
        assert_eq!(pointer_in(extended.as_bytes()), Blob::Extended);
        for not in [
            pointer(&OID[1..], "12"),
            pointer(&OID.to_uppercase(), "12"),
            pointer(OID, "x"),
            format!("{}extra\n", pointer(OID, "12")),
            format!("#{}", pointer(OID, "12")),
            "plain text\n".to_owned(),
        ] {
            assert_eq!(pointer_in(not.as_bytes()), Blob::Other, "{not}");
        }
    }

    #[test]
    fn the_least_a_pointer_can_be_is_above_what_the_size_filter_drops() {
        let least = pointer(OID, "1").replace("git-lfs.github", "hawser.github");
        assert!(least.len() as u64 > POINTER_LEAST, "{}", least.len());
    }

    #[test]
    fn an_object_is_opened_through_no_link_at_any_level_of_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let (common, kept) = (top.join("git"), top.join("kept"));
        let folder = kept.join("objects").join(&OID[..2]).join(&OID[2..4]);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(OID), "bytes").unwrap();
        std::fs::create_dir(&common).unwrap();
        let theirs = Theirs {
            root: common.clone(),
            inside: vec!["lfs".to_owned()],
        };
        assert!(opened(&theirs, OID).unwrap().is_none());
        std::os::unix::fs::symlink(&kept, common.join("lfs")).unwrap();
        assert!(opened(&theirs, OID).is_err(), "through a link at lfs");
        std::fs::remove_file(common.join("lfs")).unwrap();
        std::fs::rename(&kept, common.join("lfs")).unwrap();
        assert!(opened(&theirs, OID).unwrap().is_some());
    }

    #[test]
    fn only_the_exact_wired_helper_leaves_a_repository_fit_to_upload() {
        let (_dir, repo) = repo_with("plain.txt", "text\n");
        let says = |key: &str, value: &str| {
            assert!(testgit::run(&repo, &["config", key, value]).ok());
            let said = git::lfs_would_start_a_program(&repo).unwrap();
            assert!(testgit::run(&repo, &["config", "--unset-all", key]).ok());
            said
        };
        assert!(!git::lfs_would_start_a_program(&repo).unwrap());
        assert!(!says("credential.helper", "!gh auth git-credential"));
        for (key, value) in [
            ("credential.helper", "!gh auth git-credential --x"),
            (
                "credential.https://f.example.helper",
                "!gh auth git-credential",
            ),
            ("alias.lfs", "!x"),
            ("alias.lfs.command", "!x"),
            ("lfs.storage", "/x"),
            ("lfs.customTransfer.x.path", "x"),
            ("xlfs.customtransfer.x.path", "x"),
            ("lfs.https://f.example/.standalonetransferagent", "x"),
            ("http.extraheader", "x: y"),
            ("http.https://f.example/.extraheader", "x: y"),
            ("core.sshcommand", "x"),
        ] {
            assert!(says(key, value), "{key}");
        }
    }

    #[test]
    fn a_file_grown_past_its_size_is_read_no_further() {
        let mut grown = std::io::repeat(0);
        let (mut copy, mut head) = (Vec::new(), Vec::new());
        let (read, _) = capped_copy(&mut grown, &mut copy, 12, true, &mut head).unwrap();
        assert_eq!(read, 13);
        assert!(copy.len() <= 12, "{}", copy.len());
        assert!(head.len() <= 12, "{}", head.len());
    }

    #[test]
    fn an_address_in_what_git_lfs_says_loses_its_query() {
        assert_eq!(
            without_queries("PUT https://s.example/o?sig=secret: 500"),
            "PUT https://s.example/o 500"
        );
    }

    #[test]
    fn the_lfs_address_comes_from_the_push_url_with_no_user_in_it() {
        for (url, address) in [
            (
                "https://f.example/a/b.git",
                "https://f.example/a/b.git/info/lfs",
            ),
            (
                "https://f.example/a/b",
                "https://f.example/a/b.git/info/lfs",
            ),
            (
                "https://f.example/a/b/",
                "https://f.example/a/b.git/info/lfs",
            ),
            (
                "https://u:t@f.example:8443/a/b",
                "https://f.example:8443/a/b.git/info/lfs",
            ),
            (
                "HTTPS://f.example/a/b.git",
                "https://f.example/a/b.git/info/lfs",
            ),
        ] {
            assert_eq!(endpoint_of(url).as_deref(), Ok(address), "{url}");
        }
    }

    #[test]
    fn only_a_plain_https_remote_takes_an_upload_and_the_refusal_shows_no_password() {
        for url in [
            "http://f.example/a/b",
            "ssh://git@f.example/a/b",
            "git@f.example:a/b",
            "file:///a/b.git",
            "/a/b.git",
            "https://f.example/a?x=1",
            "https://f.example/a#b",
            "https://f.example/a b",
            "https://",
            "http://u:secret@f.example/a",
        ] {
            let said = endpoint_of(url).expect_err(url);
            assert!(said.contains("only over HTTPS"), "{said}");
            assert!(!said.contains("secret"), "{said}");
        }
    }
}
