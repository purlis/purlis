// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge, a credential source and a recorded-repo list that stand in for the real ones, for the
//! tests of held forge acts (ADR 0128 D8). The forge keeps a **log of every act it performed**, so
//! a test checks what was done against what the ask showed, and that nothing was done before a
//! press.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use purlis_core::forge::Kind;
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::heldact::{
    Forked, HeldForge, Recorded, TagName, Trees, Unavailable, WorkflowCredential, World,
};
use purlis_core::mergehold::{Base, HeadSha, Repo};
use purlis_core::secrets::keyring::Secret;

/// A commit id made of one digit repeated: `sha('a')` is forty `a`s.
pub fn sha(digit: char) -> HeadSha {
    HeadSha::try_from(digit.to_string().repeat(40)).unwrap()
}

pub fn repo(path: &str) -> Repo {
    Repo::try_from(path).unwrap()
}

pub fn base(name: &str) -> Base {
    Base::try_from(name).unwrap()
}

pub fn tag(name: &str) -> TagName {
    TagName::try_from(name).unwrap()
}

std::thread_local! {
    static NONCES: std::cell::RefCell<HashMap<u64, String>> = std::cell::RefCell::default();
}

/// **The person presses ask `id`**, as the window does: a second press names the nonce the first
/// press was answered with.
pub fn press(
    held: &purlis_core::heldact::HeldActs,
    world: &World<'_>,
    id: u64,
    digest: &str,
) -> Result<purlis_core::heldact::Pressed, purlis_core::heldact::NotDone> {
    let nonce = NONCES.with(|n| n.borrow().get(&id).cloned());
    let pressed = held.press(world, id, digest, nonce.as_deref(), the_person());
    NONCES.with(|n| match &pressed {
        Ok(purlis_core::heldact::Pressed::AsksOnceMore { nonce, .. }) => {
            n.borrow_mut().insert(id, nonce.clone());
        }
        _ => {
            n.borrow_mut().remove(&id);
        }
    });
    pressed
}

/// The person, pressing in the window.
pub fn the_person() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).unwrap()
}

#[derive(Default)]
pub struct State {
    /// Revs the forge resolves: a branch's name, a tag's, or a commit's own id.
    pub revs: HashMap<String, HeadSha>,
    /// Every commit the forge has resolved a name to, which it still has, by its own id.
    pub known: HashSet<HeadSha>,
    pub default: Option<(Base, HeadSha)>,
    pub branches: HashMap<String, HeadSha>,
    /// The commits on the default branch.
    pub merged: HashSet<HeadSha>,
    pub tags: HashMap<String, HeadSha>,
    pub ci_config: Option<String>,
    pub may_fork: bool,
    pub source_id: String,
    pub account: String,
    /// Where the forge puts a fork, whatever was asked (a wrong place).
    pub fork_lands_in: Option<String>,
    /// Refuses every performed act, in these words.
    pub refuses: Option<String>,
    /// Refuses only the making of a release, in these words.
    pub fail_release: Option<String>,
    /// Every act performed, as one line each, in order.
    pub log: Vec<String>,
}

pub struct Fake {
    pub kind: Kind,
    pub state: Mutex<State>,
}

impl Fake {
    pub fn github() -> Fake {
        Fake::of(Kind::GitHub)
    }

    pub fn gitlab() -> Fake {
        Fake::of(Kind::GitLab)
    }

    fn of(kind: Kind) -> Fake {
        Fake {
            kind,
            state: Mutex::new(State {
                default: Some((base("main"), sha('1'))),
                may_fork: true,
                source_id: "id-1".to_owned(),
                account: "aaron".to_owned(),
                ..State::default()
            }),
        }
    }

    pub fn with<R>(&self, change: impl FnOnce(&mut State) -> R) -> R {
        change(&mut self.state.lock().unwrap())
    }

    /// What was performed.
    pub fn log(&self) -> Vec<String> {
        self.with(|s| s.log.clone())
    }
}

impl HeldForge for Fake {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn host(&self) -> String {
        "forge.example".to_owned()
    }

    fn rev_commit(&self, _: &Repo, rev: &str) -> Result<HeadSha, String> {
        self.with(|s| {
            if let Some(commit) = s.revs.get(rev).cloned() {
                s.known.insert(commit.clone());
                return Some(commit);
            }
            // A commit's own id resolves to itself where the forge still holds it: it was
            // resolved before, or a tag, a branch or the default branch is at it.
            let named = HeadSha::try_from(rev).ok()?;
            let held = s.known.contains(&named)
                || s.tags.values().any(|one| *one == named)
                || s.branches.values().any(|one| *one == named)
                || s.default.as_ref().is_some_and(|(_, tip)| *tip == named);
            held.then_some(named)
        })
        .ok_or_else(|| format!("no such commit {rev}"))
    }

    fn default_branch(&self, _: &Repo) -> Result<(Base, HeadSha), String> {
        self.with(|s| {
            let default = s.default.clone();
            // Every commit the forge has named stays on it, as a real forge keeps its objects.
            if let Some((_, tip)) = &default {
                s.known.insert(tip.clone());
            }
            default
        })
        .ok_or("none".to_owned())
    }

    fn branch_tip(&self, _: &Repo, branch: &Base) -> Result<Option<HeadSha>, String> {
        Ok(self.with(|s| s.branches.get(branch.as_str()).cloned()))
    }

    fn on_branch(&self, _: &Repo, _: &Base, commit: &HeadSha) -> Result<bool, String> {
        Ok(self.with(|s| s.merged.contains(commit)))
    }

    fn tag_at(&self, _: &Repo, name: &TagName) -> Result<Option<HeadSha>, String> {
        Ok(self.with(|s| s.tags.get(name.as_str()).cloned()))
    }

    fn ci_config_path(&self, _: &Repo) -> Result<Option<String>, String> {
        Ok(self.with(|s| s.ci_config.clone()))
    }

    fn source_id(&self, _: &Repo) -> Result<String, String> {
        Ok(self.with(|s| s.source_id.clone()))
    }

    fn account(&self) -> Result<String, String> {
        Ok(self.with(|s| s.account.clone()))
    }

    fn may_fork(&self, _: &Repo) -> Result<bool, String> {
        Ok(self.with(|s| s.may_fork))
    }

    fn create_tag(&self, repo: &Repo, name: &TagName, at: &HeadSha) -> Result<(), String> {
        self.with(|s| {
            if let Some(why) = &s.refuses {
                return Err(why.clone());
            }
            if s.tags.contains_key(name.as_str()) {
                return Err("the tag exists".to_owned());
            }
            s.tags.insert(name.as_str().to_owned(), at.clone());
            s.log.push(format!(
                "create_tag {} {} {}",
                repo.as_str(),
                name.as_str(),
                at.as_str()
            ));
            Ok(())
        })
    }

    fn create_release(
        &self,
        repo: &Repo,
        tag: &TagName,
        title: &str,
        notes: &str,
        draft: bool,
        prerelease: bool,
    ) -> Result<(), String> {
        self.with(|s| {
            if let Some(why) = &s.refuses {
                return Err(why.clone());
            }
            if let Some(why) = &s.fail_release {
                return Err(why.clone());
            }
            let at = s.tags.get(tag.as_str()).cloned().ok_or("no such tag")?;
            s.log.push(format!(
                "create_release {} {} at {} title={title:?} notes={notes:?} draft={draft} \
                 prerelease={prerelease}",
                repo.as_str(),
                tag.as_str(),
                at.as_str()
            ));
            Ok(())
        })
    }

    fn make_fork(&self, source: &Repo, source_id: &str) -> Result<Forked, String> {
        self.with(|s| {
            if let Some(why) = &s.refuses {
                return Err(why.clone());
            }
            let owner = s.fork_lands_in.clone().unwrap_or_else(|| s.account.clone());
            s.log
                .push(format!("make_fork {} {source_id}", source.as_str()));
            let name = source.as_str().rsplit('/').next().unwrap_or_default();
            Ok(Forked {
                path: format!("{owner}/{name}"),
                id: "fork-1".to_owned(),
            })
        })
    }

    fn push_tip(
        &self,
        repo: &Repo,
        branch: &Base,
        tip: &HeadSha,
        base: &HeadSha,
        new_branch: bool,
        workflow: Option<&Secret>,
    ) -> Result<(), String> {
        self.with(|s| {
            if let Some(why) = &s.refuses {
                return Err(why.clone());
            }
            s.log.push(format!(
                "push_tip {} {} {} base={} new_branch={} second_credential={}",
                repo.as_str(),
                branch.as_str(),
                tip.as_str(),
                base.as_str(),
                new_branch,
                workflow.is_some()
            ));
            Ok(())
        })
    }
}

/// Trees nobody has: for acts that read none.
pub struct NoTrees;

impl Trees for NoTrees {
    fn changed(&self, _: &HeadSha, _: &HeadSha) -> Result<Vec<String>, String> {
        Err("no clone".to_owned())
    }

    fn patch(&self, _: &HeadSha, _: &HeadSha, _: &[String]) -> Result<String, String> {
        Err("no clone".to_owned())
    }

    fn files(&self, _: &HeadSha) -> Result<Vec<String>, String> {
        Err("no clone".to_owned())
    }

    fn read(&self, _: &HeadSha, _: &str) -> Result<Option<String>, String> {
        Err("no clone".to_owned())
    }
}

/// The second credential, there or not, counting each time it is read.
pub struct Second {
    pub there: bool,
    pub reads: AtomicUsize,
}

impl Second {
    pub fn there() -> Second {
        Second {
            there: true,
            reads: AtomicUsize::new(0),
        }
    }

    pub fn missing() -> Second {
        Second {
            there: false,
            reads: AtomicUsize::new(0),
        }
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl WorkflowCredential for Second {
    fn available(&self, _: &str) -> Result<(), Unavailable> {
        if self.there {
            Ok(())
        } else {
            Err(Unavailable(
                "this account has not made the second sign-in".to_owned(),
            ))
        }
    }

    fn secret(&self, host: &str) -> Result<Secret, Unavailable> {
        self.available(host)?;
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(Secret::new("the-workflow-token".to_owned()))
    }
}

/// The repos the person recorded: `(id as a request names it, path)`.
pub struct Rec(pub Vec<(&'static str, &'static str)>);

impl Rec {
    pub fn acme() -> Rec {
        Rec(vec![("acme/app", "acme/app"), ("42", "acme/app")])
    }
}

impl Recorded for Rec {
    fn resolve(&self, _: Kind, id: &str) -> Option<Repo> {
        self.0
            .iter()
            .find(|(named, _)| *named == id)
            .map(|(_, path)| repo(path))
    }

    fn holds(&self, repo: &Repo) -> bool {
        self.0.iter().any(|(_, path)| *path == repo.as_str())
    }
}

/// A world over `forge`, `trees`, `second` and `rec`.
pub fn world<'a>(
    forge: &'a Fake,
    trees: &'a dyn Trees,
    second: &'a Second,
    rec: &'a Rec,
) -> World<'a> {
    World {
        forge,
        trees,
        workflow: second,
        recorded: rec,
    }
}
