// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A page subscribes to a catalogue query it declared, on its one port** (ADR 0122 §7,
//! #5970), and purlis pushes it a new answer when the plane moves what it reads.
//!
//! The page posts `{ id, subscribe: "<id>@<N>", params }` on `purlis.port`; purlis's script
//! carries it as `/.purlis/port/<load>/subscribe/<id>?<{query, params}>`, and `{ id,
//! unsubscribe: true }` as `/.purlis/port/<load>/unsubscribe/<id>` ([`said`]). Each answer
//! comes back with the subscription's `id`, as a call's does, the first at once and each new
//! one when it is pushed.
//!
//! - **Admitted as a call is** ([`calls::admitted`]): the pin and this machine's approval read
//!   again, only as declared, and a plane-scoped one only with its grant; otherwise the page is
//!   answered the error once and nothing is subscribed.
//! - **Pushed on the plane watch's change kinds** ([`moved`], [`live::moves`]): a todo of the
//!   workspace moves `purlis.todo_list`, and a change the watch could not place moves every
//!   feed.
//! - **Coalesced, one run in flight, rate-limited** ([`live::Hub`]): one feed per plane,
//!   workspace, query and parameters, whichever pages hold it, run at most once per
//!   [`live::MIN_INTERVAL`].
//! - **Each push is gated again for each page** ([`hand`]): its load still the open one and its
//!   pin and approval still running its script, or the subscription is dropped (with
//!   `not-approved` when the approval went).
//! - **A reload revokes them with the port** ([`Artifacts::unsubscribe_label`]), as the tab
//!   closing and the window going do.

use std::time::Instant;

use purlis_core::artifact::live::{self, Do};
use purlis_core::catalogue::{Answer, ErrorKind, Query};
use purlis_core::planechange::Change;
use serde_json::Value;
use tauri::{Manager, Runtime};

use super::{Artifacts, Ask, calls, delivery, still_runs};
use crate::planes::{PlaneId, Planes};

/// What a page said on its port about a subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Said {
    Subscribe,
    Unsubscribe,
}

/// One feed: the query as the page declared it (`<id>@<N>`), its parameters, and where it
/// reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    plane: PlaneId,
    workspace: String,
    query: String,
    /// The parameters as JSON, `{}` for none, so `null` and `{}` are one feed.
    params: String,
}

impl Key {
    fn of(plane: PlaneId, workspace: String, query: String, params: &Value) -> Self {
        let params = match params {
            Value::Null => "{}".to_owned(),
            params => params.to_string(),
        };
        Self {
            plane,
            workspace,
            query,
            params,
        }
    }

    /// Whether it is an extension's query (#5971), which time moves, not the plane.
    fn of_an_extension(&self) -> bool {
        super::queried::is_extension(&self.query)
    }

    fn catalogued(&self) -> Option<&'static Query> {
        let (id, _) = self.query.rsplit_once('@')?;
        purlis_core::catalogue::find(id).map(|(query, _)| query)
    }
}

/// One subscriber: the webview, its load, and the page's id for the subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Who {
    label: String,
    load: String,
    id: u64,
}

/// Every subscription of this run, and the instant a wake is already set for.
#[derive(Default)]
pub(crate) struct Live {
    hub: live::Hub<Key, Who>,
    waking: Option<Instant>,
}

/// **What `said` after a load's word says about a subscription**, with the query string after
/// `?`: `subscribe/<id>?<{query, params}>` or `unsubscribe/<id>`; nothing when it is neither.
pub(super) fn said(
    label: &str,
    load: &str,
    said: &str,
    query: Option<&str>,
) -> Option<Option<Ask>> {
    let (live, id) = match said.strip_prefix("subscribe/") {
        Some(id) => (Said::Subscribe, id),
        None => (Said::Unsubscribe, said.strip_prefix("unsubscribe/")?),
    };
    let ask = || {
        let id = super::page_id(id)?;
        let (name, params) = match live {
            Said::Subscribe => calls::asked(query?)?,
            Said::Unsubscribe => (String::new(), Value::Null),
        };
        Some(Ask {
            label: label.to_owned(),
            load: load.to_owned(),
            id,
            name,
            params: Some(params),
            live: Some(live),
        })
    };
    Some(ask())
}

impl Artifacts {
    fn live(&self) -> std::sync::MutexGuard<'_, Live> {
        self.live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// **Drops every subscription of webview `label`**: its page loads again, its tab closed,
    /// or its window went.
    pub(crate) fn unsubscribe_label(&self, label: &str) {
        self.live().hub.unsubscribe(|who| who.label == label);
    }

    /// How many feeds this run holds.
    #[cfg(test)]
    fn feeds(&self) -> usize {
        self.live().hub.feeds()
    }
}

/// **A page's subscribe or unsubscribe**, `ask`, off the scheme's thread.
pub(super) fn asked<R: Runtime>(app: tauri::AppHandle<R>, ask: Ask) {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(artifacts) = app.try_state::<Artifacts>() else {
            return;
        };
        let who = Who {
            label: ask.label.clone(),
            load: ask.load.clone(),
            id: ask.id,
        };
        if ask.live != Some(Said::Subscribe) {
            artifacts.live().hub.unsubscribe(|w| *w == who);
            return;
        }
        let Some((of, snapshot, declared)) = artifacts.for_ask(&ask) else {
            return;
        };
        let params = ask.params.clone().unwrap_or(Value::Null);
        let config = purlis_core::machine::config_root();
        // An extension's query is asked again each `fresh_seconds` while subscribed (#5971).
        let admitted = if super::queried::is_extension(&ask.name) {
            super::queried::admitted(
                &app,
                config.as_deref(),
                &of,
                &snapshot,
                declared.as_ref(),
                &ask.name,
                &params,
            )
            .map(|(_, every)| Some(every))
        } else {
            calls::admitted(
                config.as_deref(),
                &of,
                &snapshot,
                declared.as_ref(),
                &ask.name,
                &params,
            )
            .map(|_| None)
        };
        let every = match admitted {
            Ok(every) => every,
            Err(kind) => {
                let answer = Answer::error(kind, chrono::Utc::now());
                if let Some(webview) = app.get_webview(&ask.label) {
                    let _ = webview.eval(delivery(&ask, &answer));
                }
                return;
            }
        };
        let key = Key::of(of.plane, of.workspace, ask.name.clone(), &params);
        let todo = artifacts
            .live()
            .hub
            .subscribe_every(key, who, Instant::now(), every);
        perform(&app, todo);
    });
}

/// **The plane `plane` moved `changes`** (`None`: not known): each feed of it they may have
/// moved is asked again, now or when it is due.
pub(crate) fn moved<R: Runtime>(
    app: &tauri::AppHandle<R>,
    plane: &PlaneId,
    changes: Option<&[Change]>,
) {
    let Some(artifacts) = app.try_state::<Artifacts>() else {
        return;
    };
    let todo = artifacts.live().hub.moved(Instant::now(), |key| {
        key.plane == *plane
            && !key.of_an_extension()
            && key
                .catalogued()
                .is_none_or(|query| live::moves(query, &key.workspace, changes))
    });
    perform(app, todo);
}

/// Whether feed `key` is a `purlis.chats` feed of project `plane`, and of one of `workspaces`
/// where they are known.
fn of_chats(key: &Key, plane: &PlaneId, workspaces: Option<&[String]>) -> bool {
    key.plane == *plane
        && workspaces.is_none_or(|named| named.contains(&key.workspace))
        && key
            .catalogued()
            .is_some_and(|query| query.name == purlis_core::chattools::CHATS)
}

/// What the hub does when chats of `workspaces` of `plane` moved (`None`: any of its
/// workspaces).
fn chats_todo(
    live: &mut Live,
    plane: &PlaneId,
    workspaces: Option<&[String]>,
    now: Instant,
) -> Vec<Do<Key, Who>> {
    live.hub.moved(now, |key| of_chats(key, plane, workspaces))
}

/// **Chat `session` of project `plane` moved on its board** (#6235): each `purlis.chats` feed
/// of a workspace the move may change ([`crate::chatlisting::moved_in`]) is asked again, now
/// or when it is due. The chats are the app's record, which no plane change moves. Nothing is
/// looked up where no page reads the project's chats, and the lookup is made off the thread
/// that told of the move, which may hold the chats' record.
pub(crate) fn chats_moved<R: Runtime>(app: &tauri::AppHandle<R>, plane: &PlaneId, session: u32) {
    let Some(artifacts) = app.try_state::<Artifacts>() else {
        return;
    };
    if !artifacts.live().hub.any(|key| of_chats(key, plane, None)) {
        return;
    }
    let (app, plane) = (app.clone(), plane.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let workspaces = app
            .try_state::<Planes>()
            .and_then(|planes| planes.held(&plane).ok())
            .and_then(|held| crate::chatlisting::moved_in(&held, session));
        let Some(artifacts) = app.try_state::<Artifacts>() else {
            return;
        };
        let todo = chats_todo(
            &mut artifacts.live(),
            &plane,
            workspaces.as_deref(),
            Instant::now(),
        );
        perform(&app, todo);
    });
}

/// Does what the hub said.
fn perform<R: Runtime>(app: &tauri::AppHandle<R>, todo: Vec<Do<Key, Who>>) {
    for step in todo {
        match step {
            Do::Run(key) => run(app.clone(), key),
            Do::Hand(who, answer) => hand(app, &who, &answer),
            Do::WakeAt(at) => wake_at(app.clone(), at),
        }
    }
}

/// **Runs feed `key`'s query** off the calling thread, and hands its answer on.
fn run<R: Runtime>(app: tauri::AppHandle<R>, key: Key) {
    tauri::async_runtime::spawn_blocking(move || {
        let answer = answer_of(&app, &key);
        let Some(artifacts) = app.try_state::<Artifacts>() else {
            return;
        };
        let todo = artifacts.live().hub.finished(&key, answer, Instant::now());
        perform(&app, todo);
    });
}

/// What feed `key`'s query answers now, of its plane as it is held.
fn answer_of<R: Runtime>(app: &tauri::AppHandle<R>, key: &Key) -> Answer {
    let params: Value = serde_json::from_str(&key.params).unwrap_or(Value::Null);
    if key.of_an_extension() {
        let id = key.query.rsplit_once('@').map_or("", |(id, _)| id);
        return super::queried::ask(app, &key.plane, &key.workspace, id, &params);
    }
    let at = chrono::Utc::now();
    let root = app
        .try_state::<Planes>()
        .and_then(|planes| planes.held(&key.plane).ok())
        .map(|held| held.root().to_path_buf());
    let (Some(root), Some(query)) = (root, key.catalogued()) else {
        return Answer::error(ErrorKind::Failed, at);
    };
    if query.name == purlis_core::chattools::CHATS {
        return super::calls::chats_answer(app, &key.plane, &key.workspace, &params);
    }
    purlis_core::catalogue::answer::ask(
        &root,
        &key.workspace,
        query,
        &params,
        at,
        chrono::Local::now().naive_local(),
    )
}

/// **Hands `answer` to subscriber `who`**, gated again: its load still the open one, and its
/// pin and approval still running its script; otherwise its subscription is dropped.
fn hand<R: Runtime>(app: &tauri::AppHandle<R>, who: &Who, answer: &Answer) {
    let Some(artifacts) = app.try_state::<Artifacts>() else {
        return;
    };
    let ask = Ask {
        label: who.label.clone(),
        load: who.load.clone(),
        id: who.id,
        name: String::new(),
        params: None,
        live: Some(Said::Subscribe),
    };
    let Some((of, snapshot, _)) = artifacts.for_ask(&ask) else {
        artifacts.live().hub.unsubscribe(|w| w == who);
        return;
    };
    let Some(webview) = app.get_webview(&who.label) else {
        artifacts.live().hub.unsubscribe(|w| w == who);
        return;
    };
    let config = purlis_core::machine::config_root();
    if still_runs(config.as_deref(), &of, &snapshot) {
        let _ = webview.eval(delivery(&ask, answer));
    } else {
        artifacts.live().hub.unsubscribe(|w| w == who);
        let refused = Answer::error(ErrorKind::NotApproved, chrono::Utc::now());
        let _ = webview.eval(delivery(&ask, &refused));
    }
}

/// **Ticks the hub at `at`**, unless a wake at or before it is already set.
fn wake_at<R: Runtime>(app: tauri::AppHandle<R>, at: Instant) {
    let Some(artifacts) = app.try_state::<Artifacts>() else {
        return;
    };
    {
        let mut live = artifacts.live();
        if live.waking.is_some_and(|set| set <= at) {
            return;
        }
        live.waking = Some(at);
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(at.saturating_duration_since(Instant::now())).await;
        let Some(artifacts) = app.try_state::<Artifacts>() else {
            return;
        };
        let todo = {
            let mut live = artifacts.live();
            if live.waking == Some(at) {
                live.waking = None;
            }
            live.hub.tick(Instant::now())
        };
        perform(&app, todo);
    });
}

#[cfg(all(test, not(windows)))]
mod tests {
    use serde_json::json;

    use super::*;

    const LOAD: &str = "0123456789abcdef0123456789abcdef";

    fn encoded(v: &Value) -> String {
        percent_encoding::utf8_percent_encode(&v.to_string(), percent_encoding::NON_ALPHANUMERIC)
            .to_string()
    }

    #[test]
    fn a_page_subscribes_and_unsubscribes_by_the_id_it_chose_on_its_port() {
        let q = encoded(&json!({ "query": "purlis.todo_list@1", "params": { "done": true } }));
        let ask = said("artifact-1", LOAD, "subscribe/4", Some(&q))
            .flatten()
            .unwrap();
        assert_eq!(
            (ask.id, ask.name.as_str(), ask.params, ask.live),
            (
                4,
                "purlis.todo_list@1",
                Some(json!({ "done": true })),
                Some(Said::Subscribe)
            )
        );
        let gone = said("artifact-1", LOAD, "unsubscribe/4", None)
            .flatten()
            .unwrap();
        assert_eq!((gone.id, gone.live), (4, Some(Said::Unsubscribe)));
        // Not a subscription: left to the port's other words.
        assert!(said("artifact-1", LOAD, "query/4", Some(&q)).is_none());
        // A subscription said wrong is nothing.
        for (path, query) in [
            ("subscribe/x", Some(q.as_str())),
            ("subscribe/", Some(q.as_str())),
            ("subscribe/4", None),
            ("subscribe/4", Some("%ff")),
            ("unsubscribe/-1", None),
        ] {
            assert_eq!(said("artifact-1", LOAD, path, query), Some(None), "{path}");
        }
    }

    #[test]
    fn one_query_with_no_parameters_or_null_ones_is_one_feed() {
        let plane: PlaneId = serde_json::from_str("\"plane\"").unwrap();
        let key = |params: Value| {
            Key::of(
                plane.clone(),
                "alpha".into(),
                "purlis.todo_list@1".into(),
                &params,
            )
        };
        assert_eq!(key(Value::Null), key(json!({})));
        assert_ne!(key(json!({})), key(json!({ "done": true })));
        assert_eq!(
            key(Value::Null).catalogued().map(Query::id).as_deref(),
            Some("purlis.todo_list")
        );
    }

    #[test]
    fn a_reload_revokes_the_page_s_subscriptions_with_its_port_and_no_other_page_s() {
        let artifacts = Artifacts::default();
        let plane: PlaneId = serde_json::from_str("\"plane\"").unwrap();
        let key = Key::of(
            plane,
            "alpha".into(),
            "purlis.todo_list@1".into(),
            &json!({}),
        );
        let who = |label: &str| Who {
            label: label.into(),
            load: LOAD.into(),
            id: 1,
        };
        let now = Instant::now();
        let done = Key {
            params: "{\"done\":true}".into(),
            ..key.clone()
        };
        for (key, label) in [
            (&key, "artifact-1"),
            (&key, "artifact-2"),
            (&done, "artifact-3"),
        ] {
            artifacts.live().hub.subscribe(key.clone(), who(label), now);
        }
        assert_eq!(artifacts.feeds(), 2, "two pages of one query are one feed");

        artifacts.reloaded("artifact-1");
        assert_eq!(artifacts.feeds(), 2, "artifact-1 shared its feed");
        artifacts.reloaded("artifact-3");
        assert_eq!(artifacts.feeds(), 1, "artifact-2's is kept");
        artifacts.reloaded("artifact-2");
        assert_eq!(artifacts.feeds(), 0);
    }

    #[test]
    fn a_move_of_chats_asks_again_only_the_chats_feeds_of_the_workspaces_it_may_change() {
        // #6235: a board's move does not read every chats feed of every workspace again.
        let mut live = Live::default();
        let plane: PlaneId = serde_json::from_str("\"plane\"").unwrap();
        let other: PlaneId = serde_json::from_str("\"other\"").unwrap();
        let key = |plane: &PlaneId, workspace: &str, query: &str| {
            Key::of(plane.clone(), workspace.into(), query.into(), &json!({}))
        };
        let alpha = key(&plane, "alpha", "purlis.chats@1");
        let beta = key(&plane, "beta", "purlis.chats@1");
        let todos = key(&plane, "alpha", "purlis.todo_list@1");
        let elsewhere = key(&other, "alpha", "purlis.chats@1");
        let t = Instant::now();
        for (n, key) in [&alpha, &beta, &todos, &elsewhere].into_iter().enumerate() {
            let who = Who {
                label: format!("artifact-{n}"),
                load: LOAD.into(),
                id: 1,
            };
            live.hub.subscribe(key.clone(), who, t);
            live.hub
                .finished(key, Answer::data(json!({}), chrono::Utc::now()), t);
        }
        let later = t + live::MIN_INTERVAL;
        let runs = |todo: Vec<Do<Key, Who>>| -> Vec<Key> {
            todo.into_iter()
                .filter_map(|d| match d {
                    Do::Run(key) => Some(key),
                    _ => None,
                })
                .collect()
        };
        let named = ["alpha".to_owned()];
        assert_eq!(
            runs(chats_todo(&mut live, &plane, Some(&named), later)),
            [alpha.clone()]
        );
        for key in [&alpha] {
            live.hub
                .finished(key, Answer::data(json!({}), chrono::Utc::now()), later);
        }
        // A chat that closed: every chats feed of its project, and no other feed.
        let mut every = runs(chats_todo(&mut live, &plane, None, later + live::MIN_INTERVAL));
        every.sort_by(|a, b| a.workspace.cmp(&b.workspace));
        assert_eq!(every, [alpha, beta]);
        assert!(live.hub.any(|key| *key == todos));
    }
}
