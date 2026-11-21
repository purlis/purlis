// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of the held acts: what the window is shown, the two presses, and Don't, against a
//! forge that stands in for the person's and records what it was asked to do.

use std::sync::{Arc, Mutex};

use purlis_core::forge::Kind;
use purlis_core::forge::proxy::ChatId;
use purlis_core::heldact::{
    Asked, Forked, HeldContext, HeldForge, OwnedWorld, Recorded, Requested, TagName, Trees,
    Unavailable, WorkflowCredential,
};
use purlis_core::mergehold::{Base, HeadSha, Repo, Request};
use purlis_core::secrets::keyring::Secret;

use super::{HeldActState, HeldPress};

fn sha(c: char) -> HeadSha {
    HeadSha::try_from(c.to_string().repeat(40)).unwrap()
}

#[derive(Default)]
struct Forge {
    log: Mutex<Vec<String>>,
}

impl HeldForge for Forge {
    fn kind(&self) -> Kind {
        Kind::GitHub
    }
    fn host(&self) -> String {
        "github.com".to_owned()
    }
    fn rev_commit(&self, _: &Repo, _: &str) -> Result<HeadSha, String> {
        Ok(sha('2'))
    }
    fn default_branch(&self, _: &Repo) -> Result<(Base, HeadSha), String> {
        Ok((Base::try_from("main").unwrap(), sha('1')))
    }
    fn branch_tip(&self, _: &Repo, _: &Base) -> Result<Option<HeadSha>, String> {
        Ok(None)
    }
    fn on_branch(&self, _: &Repo, _: &Base, _: &HeadSha) -> Result<bool, String> {
        Ok(false)
    }
    fn tag_at(&self, _: &Repo, name: &TagName) -> Result<Option<HeadSha>, String> {
        let made = self
            .log
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains(name.as_str()));
        Ok(made.then(|| sha('2')))
    }
    fn ci_config_path(&self, _: &Repo) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn source_id(&self, _: &Repo) -> Result<String, String> {
        Ok("id-1".to_owned())
    }
    fn account(&self) -> Result<String, String> {
        Ok("aaron".to_owned())
    }
    fn may_fork(&self, _: &Repo) -> Result<bool, String> {
        Ok(true)
    }
    fn create_tag(&self, repo: &Repo, name: &TagName, at: &HeadSha) -> Result<(), String> {
        self.log.lock().unwrap().push(format!(
            "tag {} {} {}",
            repo.as_str(),
            name.as_str(),
            at.as_str()
        ));
        Ok(())
    }
    fn create_release(
        &self,
        _: &Repo,
        _: &TagName,
        _: &str,
        _: &str,
        _: bool,
        _: bool,
    ) -> Result<(), String> {
        Ok(())
    }
    fn make_fork(&self, source: &Repo, _: &str) -> Result<Forked, String> {
        self.log
            .lock()
            .unwrap()
            .push(format!("fork {}", source.as_str()));
        Ok(Forked {
            path: "aaron/app".to_owned(),
            id: "9".to_owned(),
        })
    }
    fn push_tip(
        &self,
        _: &Repo,
        _: &Base,
        _: &HeadSha,
        _: &HeadSha,
        _: bool,
        _: Option<&Secret>,
    ) -> Result<(), String> {
        Ok(())
    }
}

struct NoTrees;
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

struct NoSecond;
impl WorkflowCredential for NoSecond {
    fn available(&self, _: &str) -> Result<(), Unavailable> {
        Err(Unavailable("not made".to_owned()))
    }
    fn secret(&self, _: &str) -> Result<Secret, Unavailable> {
        Err(Unavailable("not made".to_owned()))
    }
}

struct Rec;
impl Recorded for Rec {
    fn resolve(&self, _: Kind, id: &str) -> Option<Repo> {
        Repo::try_from(id).ok()
    }
    fn holds(&self, _: &Repo) -> bool {
        true
    }
}

struct Context {
    forge: Arc<Forge>,
}

impl HeldContext for Context {
    fn session(&self, _: &ChatId) -> Option<u32> {
        Some(4)
    }
    fn attended(&self) -> bool {
        true
    }
    fn now(&self) -> u64 {
        50
    }
    fn world(&self, _: u32, _: &Repo) -> Result<OwnedWorld, String> {
        Ok(OwnedWorld {
            forge: self.forge.clone(),
            trees: Arc::new(NoTrees),
            workflow: Arc::new(NoSecond),
            recorded: Arc::new(Rec),
        })
    }
    fn merge(&self, _: &ChatId, _: &Request) -> String {
        String::new()
    }
}

fn state() -> (HeldActState, Arc<Forge>) {
    let state = HeldActState::default();
    let forge = Arc::new(Forge::default());
    state.set_context(Arc::new(Context {
        forge: forge.clone(),
    }));
    (state, forge)
}

fn ask_for_a_tag(state: &HeldActState) {
    let sink = state.handoff().unwrap();
    use purlis_core::forge::proxy::HeldSink;
    let said = sink.ask(
        &ChatId::new("chat-4"),
        &Requested::Tag {
            repo: Repo::try_from("acme/app").unwrap(),
            name: TagName::try_from("v1".to_owned()).unwrap(),
            rev: "feature".to_owned(),
        },
    );
    assert!(said.contains("Ask 1: tag v1 in acme/app."), "{said}");
}

#[test]
fn a_waiting_tag_is_listed_with_every_field_its_digest_covers() {
    let (state, _) = state();
    ask_for_a_tag(&state);
    let waiting = state.waiting();
    assert_eq!(waiting.len(), 1);
    let one = &waiting[0];
    assert_eq!((one.ask, one.session, one.kind.as_str()), (1, 4, "tag"));
    let rows: Vec<(&str, &str)> = one
        .rows
        .iter()
        .map(|r| (r.label.as_str(), r.value.as_str()))
        .collect();
    assert!(rows.contains(&("Tag", "v1")), "{rows:?}");
    assert!(rows.contains(&("Commit", sha('2').as_str())), "{rows:?}");
    assert!(
        rows.iter()
            .any(|(l, v)| *l == "On the default branch" && v.starts_with("No")),
        "{rows:?}"
    );
    assert!(!one.digest.is_empty());
}

#[test]
fn two_presses_make_exactly_the_tag_shown_and_the_first_makes_nothing() {
    let (state, forge) = state();
    ask_for_a_tag(&state);
    let digest = state.waiting()[0].digest.clone();
    let HeldPress::AsksOnceMore { shown, nonce } = state.press(1, &digest, None).unwrap() else {
        panic!("the first press asks once more");
    };
    assert_eq!(shown.digest, digest);
    assert!(forge.log.lock().unwrap().is_empty());
    // A double click of the first press performs nothing either.
    assert!(matches!(
        state.press(1, &digest, None),
        Ok(HeldPress::AsksOnceMore { .. })
    ));
    assert!(forge.log.lock().unwrap().is_empty());
    let HeldPress::Done { said } = state.press(1, &digest, Some(&nonce)).unwrap() else {
        panic!("the second press performs");
    };
    assert!(
        said.starts_with("Made the tag v1 at 222222222222"),
        "{said}"
    );
    assert_eq!(
        forge.log.lock().unwrap().as_slice(),
        [format!("tag acme/app v1 {}", sha('2').as_str())]
    );
    assert!(state.waiting().is_empty());
}

#[test]
fn a_press_of_a_row_that_is_not_the_one_drawn_does_nothing() {
    let (state, forge) = state();
    ask_for_a_tag(&state);
    let said = state.press(1, "not-the-digest", None).unwrap_err();
    assert!(said.contains("no longer shows what you pressed"), "{said}");
    assert!(forge.log.lock().unwrap().is_empty());
}

#[test]
fn a_second_press_naming_another_nonce_does_nothing() {
    let (state, forge) = state();
    ask_for_a_tag(&state);
    let digest = state.waiting()[0].digest.clone();
    state.press(1, &digest, None).unwrap();
    assert!(
        state
            .press(1, &digest, Some("a-nonce-nobody-gave"))
            .is_err()
    );
    assert!(forge.log.lock().unwrap().is_empty());
}

#[test]
fn dont_puts_the_ask_away_and_makes_nothing() {
    let (state, forge) = state();
    ask_for_a_tag(&state);
    assert_eq!(state.dont(1), Ok(true));
    assert_eq!(state.dont(1), Ok(false));
    assert!(state.waiting().is_empty());
    assert!(forge.log.lock().unwrap().is_empty());
}

#[test]
fn a_press_without_the_projects_context_says_so_and_does_nothing() {
    let (state, _) = state();
    ask_for_a_tag(&state);
    let fresh = HeldActState::default();
    let said = fresh.press(1, "x", None).unwrap_err();
    assert!(
        said.contains("not waiting") || said.contains("not turned on"),
        "{said}"
    );
}

#[test]
fn a_closed_chat_leaves_none_of_its_asks() {
    let (state, _) = state();
    ask_for_a_tag(&state);
    state.forget(4);
    assert!(state.waiting().is_empty());
    let _ = Asked::TooMany;
}
