// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where to listen for what an agent does to a branch, so its change markers follow (FM-4,
//! #1107): the branch's folder, the folders in it that git knows, and whether what moved is
//! anything git would mark.
//!
//! **A first write anywhere in the branch has to be heard**, in a folder nobody opened and that
//! held no change yet. Where the platform watches a whole tree as one (FSEvents on macOS,
//! `ReadDirectoryChangesW` on Windows), the app watches the branch's folder recursively and asks
//! [`Root::matters`] whether what moved is worth reading the status again for: what git ignores
//! (a build's output) and git's own folder are not. Where a recursive watch is one per folder
//! (inotify), the app watches [`Root::folders`] instead: the folders holding a file git tracks
//! or does not ignore, so `node_modules/` and `target/` cost nothing.
//!
//! **Read as the status is** (D-88f, D-88h): by gitoxide, in the bounded child
//! ([`super::Reader`]), starting no program. The app asks these by itself as an agent writes.
//!
//! **The app holds the resolved folder; the window never names one.** A [`Root`] is found from a
//! branch named the window's way, with the status read's checks.
//!
//! **A build's churn starts no reader** (#874, L8). Each burst in a branch asked the child
//! whether its paths matter, about four a second per branch under a build writing `target/`.
//! [`Root::matters`] learns once, by the same child ([`Ask::IgnoredTops`]), which top-level
//! folders git ignores whole and that hold nothing git tracks, and answers a path under one
//! without asking. What it learnt is forgotten when a `.gitignore`, the clone's index or its
//! exclude file moves, and trusted for [`IGNORED_TRUSTED_FOR`] at most, for what the watch does
//! not hear: the user's own excludes. A worktree's index is kept in its clone's git folder, so
//! the clone's watch hears it and forgets what was learnt for the clone's worktrees.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::reader::{Answer, Ask, Reader};
use super::status::open;
use super::{Branch, Refused};

/// The most folders [`Root::folders`] answers with, the shallowest first: a branch of tens of
/// thousands of folders is listened to near its top.
pub const KNOWN: usize = 4096;

/// The most moved paths [`Root::matters`] looks at in one batch. Past it, the batch matters:
/// a burst that large is read again rather than sorted.
pub const ASKED: usize = 256;

/// How long [`Root::matters`] trusts the top-level folders it learnt git ignores whole (#874,
/// L8), for the changes to them the watch does not hear. A build's churn asks again at most
/// once in this long.
pub const IGNORED_TRUSTED_FOR: Duration = Duration::from_secs(30);

/// The top-level folders of each branch's folder that git ignores whole and that hold nothing
/// git tracks, by the branch's folder, with when they were asked for (#874, L8); and when each
/// branch's were last forgotten, so an answer asked for before that is not kept.
#[derive(Debug, Default)]
struct KnownIgnored {
    by_root: HashMap<PathBuf, (Instant, HashSet<String>)>,
    forgotten: HashMap<PathBuf, Instant>,
    /// Folders whose every branch was forgotten at once ([`Self::forget_under`]), and when.
    forgotten_under: HashMap<PathBuf, Instant>,
}

impl KnownIgnored {
    /// `root`'s tops, while they are trusted at `now`.
    fn of(&self, root: &Path, now: Instant) -> Option<HashSet<String>> {
        let (asked, tops) = self.by_root.get(root)?;
        (now.saturating_duration_since(*asked) < IGNORED_TRUSTED_FOR).then(|| tops.clone())
    }

    /// `root`'s tops, as asked for at `asked`: not kept when they were forgotten since.
    fn keep(&mut self, root: &Path, tops: HashSet<String>, asked: Instant) {
        let fresh = |at: &Instant| asked.saturating_duration_since(*at) < IGNORED_TRUSTED_FOR;
        // Old entries go, so the branches let go of do not pile up.
        self.by_root.retain(|_, (at, _)| fresh(at));
        self.forgotten.retain(|_, at| fresh(at) || *at >= asked);
        self.forgotten_under
            .retain(|_, at| fresh(at) || *at >= asked);
        if self.forgotten.get(root).is_some_and(|at| *at >= asked)
            || self
                .forgotten_under
                .iter()
                .any(|(folder, at)| root.starts_with(folder) && *at >= asked)
        {
            return;
        }
        self.by_root.insert(root.to_path_buf(), (asked, tops));
    }

    /// `root`'s tops may have changed.
    fn forget(&mut self, root: &Path) {
        self.by_root.remove(root);
        self.forgotten.insert(root.to_path_buf(), Instant::now());
    }

    /// The tops of every branch whose folder is in `folder` may have changed: a clone's
    /// worktrees, when one's index moved in the clone's git folder.
    fn forget_under(&mut self, folder: &Path) {
        self.by_root.retain(|root, _| !root.starts_with(folder));
        self.forgotten_under
            .insert(folder.to_path_buf(), Instant::now());
    }
}

/// What every [`Root::matters`] of this process has learnt.
static KNOWN_IGNORED: LazyLock<Mutex<KnownIgnored>> = LazyLock::new(Mutex::default);

fn known_ignored() -> std::sync::MutexGuard<'static, KnownIgnored> {
    KNOWN_IGNORED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether `relative`, a path inside a branch's folder, is one of `tops` or under one.
fn under_known_ignored(relative: &Path, tops: &HashSet<String>) -> bool {
    match relative.components().next() {
        Some(Component::Normal(first)) => first.to_str().is_some_and(|first| tops.contains(first)),
        _ => false,
    }
}

/// Whether `relative` moving may change which top-level folders git ignores, or which hold a
/// file it tracks: a `.gitignore`, but not one inside a folder of `tops` (git ignores a whole
/// folder, and nothing under it can bring a path back), the clone's index, or its exclude file.
fn forgets_known_ignored(relative: &Path, tops: &HashSet<String>) -> bool {
    if relative == Path::new(".git/index") || relative == Path::new(".git/info/exclude") {
        return true;
    }
    relative
        .file_name()
        .is_some_and(|name| name == ".gitignore")
        && !under_known_ignored(relative, tops)
}

/// Whether `relative`, a path inside a clone's folder, is the index of one of its worktrees
/// (`.git/worktrees/<name>/index`): kept in the clone's git folder, so heard by the clone's
/// watch and not by the worktree's own (#874).
fn moves_a_worktrees_index(relative: &Path) -> bool {
    let steps: Vec<Component<'_>> = relative.components().collect();
    matches!(
        steps.as_slice(),
        [
            Component::Normal(git),
            Component::Normal(worktrees),
            Component::Normal(_),
            Component::Normal(index),
        ] if *git == ".git" && *worktrees == "worktrees" && *index == "index"
    )
}

/// A branch's folder, resolved, with the names it was found by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    base: PathBuf,
    refs: Vec<PathBuf>,
    plane: PathBuf,
    ws: String,
    repo: String,
    piece: Option<String>,
}

/// What the reader's child answers [`root`] with: the branch's folder, and the files in its
/// clone's git folder whose change moves the branch's last commit ([`Root::refs`]).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Rooted {
    base: PathBuf,
    refs: Vec<PathBuf>,
}

/// The branch's folder, resolved as the status read resolves it, by `reader`.
pub fn root(reader: &Reader, plane: &Path, branch: Branch<'_>) -> Result<Root, Refused> {
    let Rooted { base, refs } = match reader.ask(plane, branch, Ask::Root)? {
        Answer::Root(rooted) => rooted,
        _ => return Err(Refused::Read("the reader answered something else".into())),
    };
    Ok(Root {
        base,
        refs,
        plane: plane.to_path_buf(),
        ws: branch.ws.to_string(),
        repo: branch.repo.to_string(),
        piece: branch.piece.map(str::to_string),
    })
}

impl Root {
    /// The branch's folder.
    pub fn path(&self) -> &Path {
        &self.base
    }

    /// The project it was found in.
    pub(super) fn plane(&self) -> &Path {
        &self.plane
    }

    /// The files in the clone's git folder whose change moves what the branch has checked out
    /// (#1152): its `HEAD`, the branch's own ref, and the clone's `packed-refs`. A commit that
    /// writes no file in the branch's folder — `git commit --amend --no-edit`, a commit of
    /// what was staged — moves only these. Empty when the reader found none it would vouch for.
    pub fn refs(&self) -> &[PathBuf] {
        &self.refs
    }

    /// The same branch, resolved again by `reader`: after a checkout in its folder, [`refs`]
    /// names the branch checked out now (#1152).
    ///
    /// [`refs`]: Root::refs
    pub fn again(&self, reader: &Reader) -> Result<Root, Refused> {
        root(reader, &self.plane, self.branch())
    }

    /// The branch it was found by, named as the window names it.
    pub(super) fn branch(&self) -> Branch<'_> {
        Branch {
            ws: &self.ws,
            repo: &self.repo,
            piece: self.piece.as_deref(),
        }
    }

    /// The branch's own folder and every folder in it holding a file git tracks or does not
    /// ignore, the shallowest first, at most [`KNOWN`], read by `reader`.
    pub fn folders(&self, reader: &Reader) -> Result<Vec<PathBuf>, Refused> {
        match reader.ask(&self.plane, self.branch(), Ask::Folders)? {
            Answer::Folders(folders) => Ok(folders
                .into_iter()
                .filter(|folder| folder.starts_with(&self.base))
                .collect()),
            _ => Err(Refused::Read("the reader answered something else".into())),
        }
    }

    /// Whether any of `moved` is something the branch's status could show: inside the
    /// branch's folder, not git's own, and not ignored by git. Past [`ASKED`] paths, or when
    /// `reader` does not answer in time, it is. A path under a top-level folder git ignores
    /// whole is answered without asking `reader`, once those folders are learnt (#874, L8).
    pub fn matters(&self, reader: &Reader, moved: &[PathBuf]) -> bool {
        let mut known = known_ignored().of(&self.base, Instant::now());
        let none = HashSet::new();
        let forgets = moved.iter().any(|path| {
            path.strip_prefix(&self.base).is_ok_and(|relative| {
                forgets_known_ignored(relative, known.as_ref().unwrap_or(&none))
            })
        });
        if forgets {
            known_ignored().forget(&self.base);
            known = None;
        }
        // A worktree's index moves in its clone's git folder, which only the clone's watch
        // hears: what was learnt for the clone's worktrees is forgotten now, not when it
        // expires (#874). They are in the workspace's worktrees folder, beside the clone.
        if self.piece.is_none()
            && moved.iter().any(|path| {
                path.strip_prefix(&self.base)
                    .is_ok_and(moves_a_worktrees_index)
            })
            && let Some(workspace) = self.base.parent()
        {
            known_ignored()
                .forget_under(&workspace.join(crate::worktree::DIR_NAME).join(&self.repo));
        }
        let mut asked: Vec<PathBuf> = Vec::new();
        for path in moved {
            let Ok(relative) = path.strip_prefix(&self.base) else {
                continue;
            };
            if relative.components().any(|step| step.as_os_str() == ".git") {
                continue;
            }
            if relative.as_os_str().is_empty() {
                // The branch's folder itself: something in it changed.
                return true;
            }
            if relative
                .components()
                .all(|step| matches!(step, Component::Normal(_)))
            {
                asked.push(relative.to_path_buf());
            }
        }
        if asked.is_empty() {
            return false;
        }
        // Only a path below the top can be under an ignored folder: a write at the top asks
        // nothing more than it did.
        let nested = asked.iter().any(|path| path.components().nth(1).is_some());
        let tops = match known {
            Some(tops) => Some(tops),
            None if nested => self.learn_ignored(reader),
            None => None,
        };
        if let Some(tops) = tops {
            asked.retain(|path| !under_known_ignored(path, &tops));
            if asked.is_empty() {
                return false;
            }
        }
        if asked.len() > ASKED {
            return true;
        }
        match reader.ask(&self.plane, self.branch(), Ask::Matters(asked)) {
            Ok(Answer::Matters(matters)) => matters,
            _ => true,
        }
    }

    /// The top-level folders git ignores whole, asked of `reader` and kept for the next
    /// [`Root::matters`]. None when it did not answer: every path is asked, as before.
    fn learn_ignored(&self, reader: &Reader) -> Option<HashSet<String>> {
        let asked = Instant::now();
        match reader.ask(&self.plane, self.branch(), Ask::IgnoredTops) {
            Ok(Answer::IgnoredTops(tops)) => {
                let tops: HashSet<String> = tops.into_iter().collect();
                known_ignored().keep(&self.base, tops.clone(), asked);
                Some(tops)
            }
            _ => None,
        }
    }
}

/// [`Root`]'s top-level folders git ignores whole and that hold nothing git tracks, here, in
/// this process: the bounded child's half of [`Ask::IgnoredTops`] (#874, L8). A folder itself,
/// never a link to one, and not git's own; at most [`KNOWN`] entries of the folder are looked
/// at. Leaving one out only means its paths are asked.
pub(super) fn ignored_tops_here(plane: &Path, branch: Branch<'_>) -> Result<Vec<String>, Refused> {
    let unreadable = |why: String| Refused::Unreadable {
        what: format!("the files of {}", branch.called()),
        why,
    };
    let opened = open(plane, branch)?;
    let repo = opened.repo;
    let index = repo
        .index_or_empty()
        .map_err(|e| unreadable(e.to_string()))?;
    let mut excludes = repo
        .excludes(
            &index,
            None,
            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
        )
        .map_err(|e| unreadable(e.to_string()))?;
    let entries = std::fs::read_dir(&opened.base).map_err(|e| unreadable(e.to_string()))?;
    let mut tops = Vec::new();
    for entry in entries.take(KNOWN).flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        // Not followed: a link to a folder is not the branch's folder.
        if name == ".git" || !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let tracked = index.entry_by_path(name.as_str().into()).is_some()
            || index
                .prefixed_entries(format!("{name}/").as_str().into())
                .is_some_and(|below| !below.is_empty());
        if tracked {
            continue;
        }
        let ignored = excludes
            .at_path(Path::new(&name), Some(gix::index::entry::Mode::DIR))
            .map_err(|e| unreadable(e.to_string()))?
            .is_excluded();
        if ignored {
            tops.push(name);
        }
    }
    Ok(tops)
}

/// [`root`], here, in this process: the bounded child's half.
pub(super) fn root_here(plane: &Path, branch: Branch<'_>) -> Result<Rooted, Refused> {
    let opened = open(plane, branch)?;
    let refs = refs_of(&opened.repo);
    Ok(Rooted {
        base: opened.base,
        refs,
    })
}

/// [`Root::refs`] of an opened branch, resolved. Only files inside the clone's own git folder:
/// the branch's git folder must be the clone's, or one of its worktrees' inside it, and the
/// branch's ref a plain `refs/heads/` name; otherwise that file is left out rather than
/// watched wherever the branch's `.git` file points.
fn refs_of(repo: &gix::Repository) -> Vec<PathBuf> {
    let (Ok(common), Ok(own)) = (
        std::fs::canonicalize(repo.common_dir()),
        std::fs::canonicalize(repo.git_dir()),
    ) else {
        return Vec::new();
    };
    if own != common && !own.starts_with(common.join("worktrees")) {
        return Vec::new();
    }
    let mut refs = vec![own.join("HEAD"), common.join("packed-refs")];
    if let Ok(Some(name)) = repo.head_name()
        && let Ok(name) = std::str::from_utf8(name.as_bstr())
        && let Some(branch) = name.strip_prefix("refs/heads/")
        && !branch.is_empty()
        && Path::new(name)
            .components()
            .all(|step| matches!(step, Component::Normal(_)))
    {
        refs.push(common.join(name));
    }
    refs
}

/// [`Root::folders`], here, in this process: the bounded child's half.
pub(super) fn folders_here(plane: &Path, branch: Branch<'_>) -> Result<Vec<PathBuf>, Refused> {
    let unreadable = |why: String| Refused::Unreadable {
        what: format!("the folders of {}", branch.called()),
        why,
    };
    let opened = open(plane, branch)?;
    let repo = opened.repo;
    let index = repo
        .index_or_empty()
        .map_err(|e| unreadable(e.to_string()))?;
    let options = repo
        .dirwalk_options()
        .map_err(|e| unreadable(e.to_string()))?
        .emit_tracked(true)
        .emit_untracked(gix::dir::walk::EmissionMode::Matching)
        .emit_ignored(None)
        .recurse_repositories(false);
    let walk = repo
        .dirwalk_iter(
            index,
            None::<gix::bstr::BString>,
            Default::default(),
            options,
        )
        .map_err(|e| unreadable(e.to_string()))?;
    let mut folders: BTreeSet<(usize, String)> = BTreeSet::new();
    folders.insert((0, String::new()));
    for item in walk {
        let item = item.map_err(|e| unreadable(e.to_string()))?;
        let Ok(file) = std::str::from_utf8(&item.entry.rela_path) else {
            continue;
        };
        let mut above = Path::new(file).parent();
        while let Some(folder) = above {
            let spelled = folder.to_string_lossy().into_owned();
            if spelled.is_empty() {
                break;
            }
            let depth = folder.components().count();
            if !folders.insert((depth, spelled)) {
                break;
            }
            above = folder.parent();
        }
    }
    Ok(folders
        .into_iter()
        .take(KNOWN)
        .map(|(_, folder)| opened.base.join(folder))
        .collect())
}

/// [`Root::matters`] for paths relative to the branch's folder, here, in this process: the
/// bounded child's half.
pub(super) fn matters_here(
    plane: &Path,
    branch: Branch<'_>,
    relative: &[PathBuf],
) -> Result<bool, Refused> {
    let unreadable = |why: String| Refused::Unreadable {
        what: format!("the files of {}", branch.called()),
        why,
    };
    let repo = open(plane, branch)?.repo;
    let index = repo
        .index_or_empty()
        .map_err(|e| unreadable(e.to_string()))?;
    let mut excludes = repo
        .excludes(
            &index,
            None,
            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
        )
        .map_err(|e| unreadable(e.to_string()))?;
    Ok(relative.iter().take(ASKED).any(|path| {
        let plain = path
            .components()
            .all(|step| matches!(step, Component::Normal(_)));
        if !plain {
            return true;
        }
        // A file git tracks matters whatever the ignore rules say (`git add -f`): its edit is
        // one `status` shows. So does a folder holding one (#874): a rename of an ignored
        // folder is told by the folder's name alone, and moves every tracked file in it.
        let spelled: Option<Vec<&str>> = path
            .components()
            .map(|step| step.as_os_str().to_str())
            .collect();
        let tracked = spelled.is_some_and(|steps| {
            let spelled = steps.join("/");
            index.entry_by_path(spelled.as_str().into()).is_some()
                || index
                    .prefixed_entries(format!("{spelled}/").as_str().into())
                    .is_some_and(|below| !below.is_empty())
        });
        tracked
            || excludes
                .at_path(path, None)
                .map_or(true, |platform| !platform.is_excluded())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn tops(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    /// #874 (L8): a path under a top-level folder git ignores whole is known not to matter
    /// without asking; the folder itself too; nothing else is.
    #[test]
    fn a_path_under_a_known_ignored_top_is_known_not_to_matter() {
        let known = tops(&["target", "node_modules"]);
        assert!(under_known_ignored(Path::new("target/debug/out"), &known));
        assert!(under_known_ignored(Path::new("target"), &known));
        assert!(under_known_ignored(
            Path::new("node_modules/x/y.js"),
            &known
        ));
        assert!(!under_known_ignored(Path::new("targets/x"), &known));
        assert!(!under_known_ignored(Path::new("src/target/x"), &known));
        assert!(!under_known_ignored(Path::new("README.md"), &known));
    }

    /// #874 (L8): what may change which folders git ignores, or which hold a tracked file,
    /// forgets them: a `.gitignore` (but one inside a folder git ignores whole, which cannot
    /// bring anything back), the clone's index, and its own exclude file.
    #[test]
    fn an_ignore_file_or_the_index_moving_forgets_the_known_ignored_tops() {
        let known = tops(&["target"]);
        for forgets in [
            ".gitignore",
            "src/.gitignore",
            ".git/index",
            ".git/info/exclude",
        ] {
            assert!(
                forgets_known_ignored(Path::new(forgets), &known),
                "{forgets}"
            );
        }
        for keeps in [
            "target/.gitignore",
            "target/debug/.gitignore",
            ".git/objects/ab/cdef",
            ".git/index.lock",
            "src/main.rs",
            "src/gitignore",
        ] {
            assert!(!forgets_known_ignored(Path::new(keeps), &known), "{keeps}");
        }
    }

    /// #874: a worktree's index is kept in its clone's git folder, which the clone's watch
    /// hears, so its move forgets what was learnt for the clone's worktrees at once; the lock
    /// beside it, and the rest of the worktree's git folder, do not.
    #[test]
    fn a_worktrees_index_moving_in_its_clone_is_told_apart() {
        for forgets in [".git/worktrees/fix/index", ".git/worktrees/a b/index"] {
            assert!(moves_a_worktrees_index(Path::new(forgets)), "{forgets}");
        }
        for keeps in [
            ".git/index",
            ".git/worktrees/fix/index.lock",
            ".git/worktrees/fix/HEAD",
            ".git/worktrees/index",
            "src/.git/worktrees/fix/index",
            ".git/worktrees/fix/sub/index",
        ] {
            assert!(!moves_a_worktrees_index(Path::new(keeps)), "{keeps}");
        }
    }

    /// #874: forgetting a clone's worktrees forgets every root under their folder, and nothing
    /// else, and an answer asked for before it is not kept, as [`KnownIgnored::forget`]'s.
    #[test]
    fn a_clones_worktrees_are_forgotten_together_and_alone() {
        let start = Instant::now();
        let mut known = KnownIgnored::default();
        let clone = Path::new("/p/workspaces/w/svc");
        let fix = Path::new("/p/workspaces/w/.worktrees/svc/fix");
        let feat = Path::new("/p/workspaces/w/.worktrees/svc/feat");
        let other = Path::new("/p/workspaces/w/.worktrees/svc-old/x");
        for root in [clone, fix, feat, other] {
            known.keep(root, tops(&["target"]), start);
        }
        let asked = Instant::now();

        known.forget_under(Path::new("/p/workspaces/w/.worktrees/svc"));

        assert_eq!(known.of(fix, asked), None);
        assert_eq!(known.of(feat, asked), None);
        assert_eq!(known.of(clone, asked), Some(tops(&["target"])));
        assert_eq!(known.of(other, asked), Some(tops(&["target"])));
        known.keep(fix, tops(&["target"]), asked);
        assert_eq!(
            known.of(fix, asked),
            None,
            "asked for before it was forgotten"
        );
    }

    /// #874 (L8): the tops are trusted for [`IGNORED_TRUSTED_FOR`] at most, for what the watch
    /// does not hear (the user's own excludes; a worktree's index where its clone is not
    /// watched), and a root's are forgotten on their own.
    #[test]
    fn the_known_ignored_tops_are_trusted_for_a_while_and_per_root() {
        let start = Instant::now();
        let mut known = KnownIgnored::default();
        let a = Path::new("/w/a");
        let b = Path::new("/w/b");
        assert_eq!(known.of(a, start), None);

        known.keep(a, tops(&["target"]), start);
        known.keep(b, tops(&["dist"]), start);
        assert_eq!(
            known.of(a, start + Duration::from_secs(1)),
            Some(tops(&["target"]))
        );

        known.forget(a);
        assert_eq!(known.of(a, start + Duration::from_secs(1)), None);
        assert_eq!(
            known.of(b, start + Duration::from_secs(1)),
            Some(tops(&["dist"]))
        );

        assert_eq!(known.of(b, start + IGNORED_TRUSTED_FOR), None);

        // An answer asked for before its root was forgotten is not kept.
        let asked = Instant::now();
        known.forget(b);
        known.keep(b, tops(&["dist"]), asked);
        assert_eq!(known.of(b, asked), None);
    }
}
