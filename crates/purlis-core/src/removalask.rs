// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat asks the person to remove a workspace** (qw356).
//!
//! Removing a workspace deletes its memory, its todos and its clones: what other chats and the
//! person rely on. So a chat whose sandbox holds the removal never makes it, and until this
//! was written that refusal was where it ended, even in the chat purlis's own Safe remove
//! opens. Here the refusal becomes a question: `purlis workspace remove <name>` hands the name
//! to the app ([`crate::hookwire::Ask::Removal`]), the app holds it as a question on the asking
//! chat's tab, and **the person's press in the window is the only thing that removes
//! anything**, through the window's own remove and its guard ([`crate::wscmd::remove`]).
//!
//! **A request is a name, and the name is looked up** ([`named`]): it is a workspace of this
//! project as the project's own listing has it, or nothing is asked. Every word the question
//! says is purlis's; the name is drawn as data.
//!
//! The chat is told what the person decided on its next turn
//! ([`crate::dispatchtalk::RemovalWord`]), on the road the person's other answers take.

use std::path::Path;

/// The workspace `name` names in the project at `root`, as the project's own listing spells
/// it, or the sentence that says why nothing was asked: a name that cannot be a workspace's,
/// or no workspace of this project.
pub fn named(root: &Path, name: &str) -> Result<String, String> {
    if !crate::contain::workspace_name_ok(name) {
        return Err(no_such_said(name));
    }
    crate::workspaces::Plane::open(root)
        .workspaces()
        .ok()
        .and_then(|every| every.into_iter().find(|one| one == name))
        .ok_or_else(|| no_such_said(name))
}

/// What the asking chat's command says once the person has been asked. It replaces "ask the
/// person to remove it from the window": the asking is done, and the chat waits.
pub fn asked_said(workspace: &str) -> String {
    format!(
        "removing a workspace is the person's to decide, so nothing was removed: purlis has \
         asked the person, in its window, whether to remove '{}'. Wait for their answer. purlis \
         tells this chat what they decided on its next turn, so do not ask again and do not \
         remove it another way",
        crate::shown::short(workspace)
    )
}

/// What the asking chat's command says of a name that is no workspace of this project.
pub fn no_such_said(name: &str) -> String {
    format!(
        "this project has no workspace '{}', so the person was not asked and nothing was \
         removed",
        crate::shown::short(name)
    )
}

/// What the asking chat's command says when it already has as many questions waiting as one
/// chat may: this one is not asked, and none that waits is put away for it.
pub fn too_many_said(name: &str) -> String {
    format!(
        "purlis is already asking the person about other workspaces this chat asked to have \
         removed, so the person was not asked about '{}' and nothing was removed. Wait for \
         their answers; purlis tells this chat each on its next turn",
        crate::shown::short(name)
    )
}

/// What a workspace holds that a removal deletes, as far as it is cheap to count: read from
/// the workspace's own stores, never from anything a chat said.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Holds {
    /// Its memories, archived ones left out.
    pub memories: usize,
    /// Its open todos.
    pub todos: usize,
}

/// What workspace `workspace` of the project at `root` holds now. A store that cannot be read
/// counts as none: the question still says what a removal deletes, in words.
pub fn holds(root: &Path, workspace: &str) -> Holds {
    let Ok(ws) = crate::workspaces::Plane::open(root).workspace(workspace) else {
        return Holds::default();
    };
    Holds {
        memories: ws.memories().map_or(0, |every| every.len()),
        todos: ws.todos().map_or(0, |every| every.len()),
    }
}

/// **What one removal would lose, as its row lists it** (T1.1-67, Q304 Q2): read from the thing
/// itself, never from anything a chat said, and read again at the press. Two listings of the
/// same thing are equal only if nothing it holds moved between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// One line for each thing that goes, in words.
    pub lines: Vec<String>,
    /// Whether the lines are everything that goes. A listing git or the store would not give
    /// whole is not exact, and its row offers Keep only: nothing is removed on a list that may
    /// leave something out.
    pub exact: bool,
    /// **A digest of everything that goes, whole**: every entry of the thing, not the few the
    /// lines name, with the size and the time last written of each file it lists. Two listings
    /// are the same only if the digest is, so an entry swapped past the lines' eighth, or an edit
    /// to a file already listed, moves it. Empty where the listing is not exact.
    pub digest: String,
}

/// How many of a long list a row names before it counts the rest.
const NAMED: usize = 8;

/// The most entries one digest reads below the paths a listing names. A listing that holds more
/// is not exact: past them a change would not be seen.
const MOST_STAMPED: usize = 100_000;

/// **What the things a listing names hold, as one digest**: their text, and for each path its
/// kind, its size and when it was last written, below a folder too, read without following a
/// link. A path that is gone is stamped as gone, and one that cannot be read, or too many
/// entries, leave the digest `whole == false`.
struct Stamp {
    hash: sha2::Sha256,
    left: usize,
    whole: bool,
}

impl Stamp {
    fn new() -> Self {
        use sha2::Digest as _;
        Self {
            hash: sha2::Sha256::new(),
            left: MOST_STAMPED,
            whole: true,
        }
    }

    /// Adds `text`, length first, so two texts never run into one.
    fn text(&mut self, text: &str) {
        use sha2::Digest as _;
        self.hash.update((text.len() as u64).to_le_bytes());
        self.hash.update(text.as_bytes());
    }

    /// Adds what `path` holds, and what is below it.
    fn path(&mut self, path: &Path) {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.text("gone");
                return;
            }
            Err(_) => {
                self.whole = false;
                return;
            }
        };
        if self.left == 0 {
            self.whole = false;
            return;
        }
        self.left -= 1;
        let written = meta
            .modified()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos());
        let kind = if meta.is_dir() {
            "dir"
        } else if meta.is_symlink() {
            "link"
        } else {
            "file"
        };
        self.text(&format!("{kind}|{}|{written}", meta.len()));
        if meta.is_symlink() {
            match std::fs::read_link(path) {
                Ok(to) => self.text(&to.to_string_lossy()),
                Err(_) => self.whole = false,
            }
        } else if meta.is_dir() {
            let Ok(entries) = std::fs::read_dir(path) else {
                self.whole = false;
                return;
            };
            let mut names: Vec<std::ffi::OsString> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect();
            names.sort();
            for name in names {
                self.text(&name.to_string_lossy());
                self.path(&path.join(&name));
            }
        }
    }

    /// The path a `git status --porcelain` line names, below `folder`: its status and a rename's
    /// old half dropped, a quoted name read as it is. `false` where the name is one git quoted
    /// with an escape, which cannot be read back here.
    fn porcelain(&mut self, folder: &Path, line: &str) {
        let named = line.get(3..).unwrap_or(line);
        let named = named.rsplit(" -> ").next().unwrap_or(named);
        let named = named
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
            .unwrap_or(named);
        if named.contains('\\') {
            self.whole = false;
            return;
        }
        self.path(&folder.join(named.trim_end_matches('/')));
    }

    /// The digest, or `None` where it did not read everything.
    fn finish(self) -> Option<String> {
        use sha2::Digest as _;
        self.whole.then(|| {
            self.hash
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        })
    }
}

impl Listing {
    /// The listing of `lines`, whole where `exact` and the `stamp` read everything.
    fn from(mut lines: Vec<String>, mut exact: bool, stamp: Stamp) -> Self {
        let digest = stamp.finish();
        if exact && digest.is_none() {
            lines.push(
                "more than purlis could read in full, so it cannot tell whether it changes"
                    .to_owned(),
            );
        }
        exact &= digest.is_some();
        Listing {
            lines,
            exact,
            digest: digest.filter(|_| exact).unwrap_or_default(),
        }
    }
}

/// `lines`, the first [`NAMED`] each prefixed by `what`, and a count of the rest.
fn named_lines(what: &str, lines: impl IntoIterator<Item = String>) -> Vec<String> {
    let all: Vec<String> = lines.into_iter().collect();
    let mut out: Vec<String> = all
        .iter()
        .take(NAMED)
        .map(|line| format!("{what}: {line}"))
        .collect();
    if all.len() > NAMED {
        out.push(format!("and {} more {what}", all.len() - NAMED));
    }
    out
}

/// What removing persona `name` of the project at `root` deletes: its definition, and the
/// memory and curation actions kept in its folder, one line each as the folder holds them; the
/// generated sub-agent `.claude/agents/<name>.md` that goes with it; and the dispatch grants on
/// this machine that name it, which are in force for no chat once it is gone. `Err` with the
/// sentence that says why nothing is asked: a name that cannot be a persona's, or no persona of
/// this project.
pub fn persona_listing(root: &Path, name: &str) -> Result<Listing, String> {
    let no_such = || {
        format!(
            "this project has no persona '{}', so the person was not asked and nothing was \
             removed",
            crate::shown::short(name)
        )
    };
    if crate::personas::shape_refusal(name).is_some()
        || !crate::personas::def_path(root, name).exists()
    {
        return Err(no_such());
    }
    let definition = crate::personas::def_path(root, name);
    let dir = root.join("personas").join(name);
    let mut lines = Vec::new();
    let mut exact = true;
    let mut stamp = Stamp::new();
    if definition.file_name().is_some_and(|f| f == "persona.md") {
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                let mut names: Vec<String> = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                for entry in names {
                    lines.push(if entry == "memory" {
                        let count = crate::personas::memory_count(root, name);
                        format!("personas/{name}/memory: {count} memories")
                    } else {
                        format!("personas/{name}/{entry}")
                    });
                }
                // Everything below the folder, so a memory added or a file edited moves it.
                stamp.path(&dir);
            }
            Err(_) => exact = false,
        }
    } else {
        lines.push(format!("personas/{name}.md"));
        stamp.path(&definition);
    }
    // The sub-agent persona remove takes with it, when purlis wrote it.
    let agent = crate::personaverbs::retired::agents_dir(root).join(format!("{name}.md"));
    if crate::personaverbs::retired::is_generated(root, &agent, name) {
        lines.push(format!(".claude/agents/{name}.md"));
        stamp.text("agent");
        stamp.path(&agent);
    }
    if crate::sandbox::local::dispatch_granted_names(root)
        .iter()
        .any(|one| one == name)
    {
        lines.push(
            "the dispatch grants on this machine that name it, which are in force for no chat \
             once it is gone"
                .to_owned(),
        );
        stamp.text("grants");
    }
    if lines.is_empty() {
        exact = false;
    }
    Ok(Listing::from(lines, exact, stamp))
}

/// **What removing a branch folder would lose, and why a plain removal is not enough**: the
/// two flags [`crate::hookwire::Removing::BranchFolder`] carries, and the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    /// Uncommitted changes, commits no other ref reaches, or git's own state of the folder go.
    pub force: bool,
    /// Files git ignores go.
    pub ignored: bool,
    pub listing: Listing,
}

/// What removing `piece` of `repo` in workspace `ws` of the plane at `plane` loses, read now,
/// with git run in the folder's checked clone and folder only
/// ([`crate::gitbroker::in_a_checked_folder`]), as a chat's own removal reads them. `Err` says
/// why nothing can be asked: the folder is not there, or git would not be read.
pub fn folder_loss(plane: &Path, ws: &str, repo: &str, piece: &str) -> Result<Loss, String> {
    let held = crate::worktree::git::isolation().unwrap_or_default();
    crate::gitbroker::in_a_checked_folder(plane, ws, repo, piece, &held, || {
        read_loss(plane, ws, repo, piece)
    })
    .map_err(|not_run| match not_run {
        crate::gitbroker::NotRun::Repo(why) => why,
        crate::gitbroker::NotRun::Folder(why) => why.to_string(),
    })?
}

fn read_loss(plane: &Path, ws: &str, repo: &str, piece: &str) -> Result<Loss, String> {
    let risk = match crate::worktree::standing::at_risk(plane, ws, repo, piece) {
        Ok(Some(risk)) => risk,
        Ok(None) => {
            return Err(format!(
                "{repo} · {piece} has no folder to remove, so the person was not asked"
            ));
        }
        Err(refusal) => return Err(refusal.to_string()),
    };
    let Ok(folder) = crate::worktree::path_for(plane, ws, repo, piece) else {
        return Err(format!(
            "{repo} · {piece} has no folder to remove, so the person was not asked"
        ));
    };
    let mut lines = Vec::new();
    let mut exact = true;
    let mut force = false;
    let mut stamp = Stamp::new();
    match &risk.changes {
        Some(changes) if !changes.is_empty() => {
            force = true;
            stamp.text("changes");
            for change in changes {
                stamp.text(change);
                stamp.porcelain(&folder, change);
            }
            lines.extend(named_lines("uncommitted", changes.iter().cloned()));
        }
        Some(_) => {}
        None => {
            force = true;
            exact = false;
            lines.push("uncommitted changes git would not list".to_owned());
        }
    }
    match risk.unmerged {
        Some(0) => {}
        Some(count) => {
            force = true;
            stamp.text(&format!("commits {count}"));
            for commit in &risk.commits {
                stamp.text(commit);
            }
            lines.extend(named_lines(
                "commit no other branch reaches",
                risk.commits.iter().cloned(),
            ));
            // The commits git named are fewer than it counted: the rest are not listed.
            let named = u32::try_from(risk.commits.len()).unwrap_or(u32::MAX);
            if count > named {
                exact = false;
                lines.push(format!(
                    "{} more commits no other branch reaches, which git did not name",
                    count - named
                ));
            }
        }
        None => {
            force = true;
            exact = false;
            lines.push("commits no other branch reaches, which git would not count".to_owned());
        }
    }
    match &risk.own {
        Some(own) if !own.is_empty() => {
            force = true;
            let said = own.said();
            stamp.text("own");
            for line in &said {
                stamp.text(line);
            }
            lines.extend(said);
        }
        Some(_) => {}
        None => {
            force = true;
            exact = false;
            lines.push("git's own state of this folder, which git would not read".to_owned());
        }
    }
    let mut ignored = false;
    match crate::piececmd::ignored_kept(plane, ws, repo, piece) {
        Some(Ok(kept)) if !kept.is_empty() => {
            ignored = true;
            stamp.text("ignored");
            for path in &kept {
                stamp.text(path);
                stamp.path(&folder.join(path.trim_end_matches('/')));
            }
            lines.extend(named_lines("ignored file", kept));
        }
        Some(Ok(_)) | None => {}
        Some(Err(_)) => {
            ignored = true;
            exact = false;
            lines.push("files git ignores, which git would not list".to_owned());
        }
    }
    Ok(Loss {
        force,
        ignored,
        listing: Listing::from(lines, exact, stamp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a project");
        std::fs::create_dir_all(dir.path().join("workspaces/alpha")).expect("alpha");
        dir
    }

    #[test]
    fn only_a_workspace_of_this_project_is_asked_about() {
        let dir = a_project();
        assert_eq!(named(dir.path(), "alpha"), Ok("alpha".to_owned()));
        // Not there, another spelling, a path, a control character: nothing is asked.
        for not_one in [
            "beta",
            "Alpha",
            "../alpha",
            "alpha/..",
            ".",
            "",
            "alpha\nbeta",
        ] {
            let why = named(dir.path(), not_one).expect_err(not_one);
            assert!(why.contains("the person was not asked"), "{why}");
            assert!(!why.contains('\n'), "{why:?}");
        }
    }

    #[test]
    fn what_the_command_says_is_that_the_person_was_asked_and_to_wait() {
        let said = asked_said("alpha");
        assert!(said.contains("nothing was removed"), "{said}");
        assert!(said.contains("asked the person, in its window"), "{said}");
        assert!(said.contains("Wait for their answer"), "{said}");
        // Never the old way round, which a sandboxed chat's person had to do by hand.
        assert!(!said.contains("terminal"), "{said}");
    }

    #[test]
    fn a_chat_with_its_few_questions_waiting_is_told_this_one_was_not_asked() {
        let said = too_many_said("alpha");
        assert!(
            said.contains("the person was not asked about 'alpha'"),
            "{said}"
        );
        assert!(said.contains("nothing was removed"), "{said}");
    }

    #[test]
    fn what_a_workspace_holds_is_counted_from_its_own_stores() {
        let dir = a_project();
        assert_eq!(holds(dir.path(), "alpha"), Holds::default());
        let ws = crate::workspaces::Plane::open(dir.path())
            .workspace("alpha")
            .expect("alpha");
        let now = crate::wallclock::day(2026, 10, 13)
            .map(|day| day.at(9, 0, 0, 0))
            .expect("a time");
        ws.add_todo("Ship it", now).expect("a todo");
        ws.remember("Deploys go through the pipeline", now)
            .expect("a memory");
        assert_eq!(
            holds(dir.path(), "alpha"),
            Holds {
                memories: 1,
                todos: 1
            }
        );
        assert_eq!(holds(dir.path(), "no-such"), Holds::default());
    }
}
