// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A page subscribes to a catalogue query it declared, and purlis pushes it new answers**
//! (ADR 0122 §7, #5970).
//!
//! - **On core events** ([`moves`]): a query is asked again when the plane's watch reports a
//!   change of a kind its answer is read from (a todo of its workspace for `purlis.todo_list`),
//!   or a change it could not place. Nothing is polled.
//! - **Coalesced** ([`Hub`]): every subscription to one query with one set of parameters, of
//!   one workspace, whichever pages hold it, is one feed: one run answers them all, and a page
//!   that subscribes to a feed already answered is handed that answer without a run.
//! - **One run in flight, and at most one run started per [`MIN_INTERVAL`]** per feed: a change
//!   heard while a run is in flight, or sooner than that after the last run started, asks
//!   again once, when the run is done and the interval has passed. A burst of changes is one
//!   run, and the page is never left on an answer older than the last change.
//! - **An extension's query is asked again once its `fresh_seconds` has passed**, while someone
//!   subscribes ([`Hub::subscribe_every`], #5971): extensions are never long-running and push
//!   nothing (ADR 0053), so its freshness is what keeps a page on it live. **So is a core
//!   query with a `fresh_seconds`** (`purlis.forge_milestones`, #6233), which reads what no
//!   change of the plane moves. Such a feed is asked at most once per freshness by its key,
//!   however its subscribers come and go: one left with no one is kept until its freshness
//!   passes, its last run and its answer with it, so subscribing again starts nothing early.
//!
//! The hub holds no clock and starts nothing itself: it says what to do ([`Do`]), and its
//! caller runs the query, hands the answers and wakes it, so it is tested with the instants it
//! is given.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use crate::catalogue::{Answer, Query, Scope};
use crate::planechange::{Change, Kind};

/// **The rate limit**: the least time between two runs of one feed's query. A second matches
/// the plane watch's longest burst, so a page is never more than about a second behind its
/// plane while the plane is written without pause.
pub const MIN_INTERVAL: Duration = Duration::from_secs(1);

/// The change kinds query `name`'s answer is read from, or `None` when the catalogue names a
/// query this does not know, which any change in its scope moves.
fn read_from(name: &str) -> Option<&'static [Kind]> {
    match name {
        "todo_list" => Some(&[Kind::Todos]),
        "memory_search" => Some(&[Kind::Memory]),
        "session_record_list" | "session_record_read" => Some(&[Kind::Sessions]),
        // `workspaces/<ws>/changes/` is a plain folder of the workspace to the watch.
        "change_status" => Some(&[Kind::Workspace]),
        // Read from the forge, which no change of the plane moves: it is asked again on its
        // `fresh_seconds` instead (#6233).
        crate::catalogue::milestones::NAME => Some(&[]),
        // The app's own record, not the plane: the app moves it as its chats move (#6235).
        "chats" => Some(&[]),
        _ => None,
    }
}

/// **Whether `changes` may have moved what `query` answers in workspace `workspace`**: a change
/// of a kind it reads, in that workspace (anywhere, for a plane-scoped one); or a change the
/// watch could not place (`None`), which moves everything.
pub fn moves(query: &Query, workspace: &str, changes: Option<&[Change]>) -> bool {
    let Some(changes) = changes else {
        return true;
    };
    let kinds = read_from(query.name);
    changes.iter().any(|change| {
        let in_scope = match query.scope {
            Scope::Workspace => change.workspace.as_deref() == Some(workspace),
            Scope::Plane => true,
        };
        in_scope && kinds.is_none_or(|kinds| kinds.contains(&change.kind))
    })
}

/// What the hub's caller does next.
#[derive(Debug, Clone, PartialEq)]
pub enum Do<K, W> {
    /// Run feed `K`'s query, and tell the hub its answer ([`Hub::finished`]).
    Run(K),
    /// Hand subscriber `W` this answer.
    Hand(W, Answer),
    /// Call [`Hub::tick`] at this instant: a feed waits out its interval.
    WakeAt(Instant),
}

/// One query with its parameters, and who subscribes to it.
struct Feed<W> {
    who: Vec<W>,
    running: bool,
    /// When its last run started.
    started: Option<Instant>,
    /// Whether something moved it since its last run started.
    stale: bool,
    /// Its last answer.
    last: Option<Answer>,
    /// How long an answer stays good, after which it is asked again: an extension query's
    /// `fresh_seconds` (#5971). `None` for a core query, which only a change moves.
    every: Option<Duration>,
}

/// **Every feed with a subscriber**, by key `K` (the query, its parameters, and where it reads),
/// each subscriber a `W`.
pub struct Hub<K, W> {
    feeds: HashMap<K, Feed<W>>,
}

impl<K, W> Default for Hub<K, W> {
    fn default() -> Self {
        Self {
            feeds: HashMap::new(),
        }
    }
}

impl<K: Clone + Eq + Hash, W: Clone + PartialEq> Hub<K, W> {
    /// **`who` subscribes to feed `key`**, in place of anything it subscribed to before: handed
    /// the feed's last answer at once when it has one, and its query run when it is due.
    pub fn subscribe(&mut self, key: K, who: W, now: Instant) -> Vec<Do<K, W>> {
        self.subscribe_every(key, who, now, None)
    }

    /// [`Self::subscribe`], to a feed asked again each `every` while it has a subscriber (an
    /// extension query's `fresh_seconds`, #5971).
    pub fn subscribe_every(
        &mut self,
        key: K,
        who: W,
        now: Instant,
        every: Option<Duration>,
    ) -> Vec<Do<K, W>> {
        self.unsubscribe(|w| *w == who);
        self.forget_spent(now);
        let feed = self.feeds.entry(key.clone()).or_insert_with(|| Feed {
            who: Vec::new(),
            running: false,
            started: None,
            stale: true,
            last: None,
            every,
        });
        feed.who.push(who.clone());
        let mut out = Vec::new();
        if let Some(last) = &feed.last {
            out.push(Do::Hand(who, last.clone()));
        }
        out.extend(self.due(&key, now));
        out
    }

    /// **Every subscriber `gone` names is dropped**, and each feed left with none with it: an
    /// answer of a run still in flight for it is then handed to no one. A feed with a
    /// freshness is kept, with no one, until it has passed ([`Self::forget_spent`]): its runs
    /// are counted by its key, not by how long someone held it, so a page that lets go and
    /// subscribes again, however often, starts no run sooner than its freshness allows (#6233).
    pub fn unsubscribe(&mut self, gone: impl Fn(&W) -> bool) {
        for feed in self.feeds.values_mut() {
            feed.who.retain(|w| !gone(w));
        }
        self.feeds
            .retain(|_, feed| !feed.who.is_empty() || feed.every.is_some());
    }

    /// Forgets each feed with no one whose run is done and whose freshness has passed at
    /// `now`: a subscription after it is asked again anyway.
    fn forget_spent(&mut self, now: Instant) {
        self.feeds.retain(|_, feed| {
            !feed.who.is_empty()
                || feed.running
                || feed
                    .every
                    .zip(feed.started)
                    .is_some_and(|(every, at)| now < at + every)
        });
    }

    /// **The plane moved what each feed `moved` names may answer**: each is asked again, now or
    /// once it is due.
    pub fn moved(&mut self, now: Instant, moved: impl Fn(&K) -> bool) -> Vec<Do<K, W>> {
        self.forget_spent(now);
        let keys: Vec<K> = self.feeds.keys().filter(|k| moved(k)).cloned().collect();
        let mut out = Vec::new();
        for key in keys {
            if let Some(feed) = self.feeds.get_mut(&key) {
                feed.stale = true;
            }
            out.extend(self.due(&key, now));
        }
        out
    }

    /// **Feed `key`'s run answered `answer`**: handed to every subscriber it has now, and run
    /// again when it is due if something moved it meanwhile. An answer that says what the
    /// feed's last one said, whenever it was read, is handed to no one (#6235): every page of
    /// the feed already holds it. A feed with a freshness hands each one on, since its time is
    /// what is fresh.
    pub fn finished(&mut self, key: &K, answer: Answer, now: Instant) -> Vec<Do<K, W>> {
        let Some(feed) = self.feeds.get_mut(key) else {
            return Vec::new();
        };
        feed.running = false;
        let unchanged = feed.every.is_none()
            && feed
                .last
                .as_ref()
                .is_some_and(|last| last.outcome == answer.outcome && last.cursor == answer.cursor);
        let mut out: Vec<Do<K, W>> = if unchanged {
            Vec::new()
        } else {
            feed.who
                .iter()
                .map(|w| Do::Hand(w.clone(), answer.clone()))
                .collect()
        };
        feed.last = Some(answer);
        out.extend(self.due(key, now));
        out
    }

    /// **The instant a [`Do::WakeAt`] named came**: every feed whose interval has passed runs.
    pub fn tick(&mut self, now: Instant) -> Vec<Do<K, W>> {
        self.forget_spent(now);
        let keys: Vec<K> = self.feeds.keys().cloned().collect();
        keys.iter().flat_map(|key| self.due(key, now)).collect()
    }

    /// Whether any feed is one `which` names: a look that runs nothing.
    pub fn any(&self, which: impl Fn(&K) -> bool) -> bool {
        self.feeds.keys().any(which)
    }

    /// How many feeds there are: one per query, parameters and place subscribed to.
    pub fn feeds(&self) -> usize {
        self.feeds.values().filter(|f| !f.who.is_empty()).count()
    }

    /// How many feeds the hub holds, those kept with no one until their freshness passes
    /// among them.
    pub fn kept(&self) -> usize {
        self.feeds.len()
    }

    /// Feed `key`'s run, when it is moved or its answer is past its freshness, not running, and
    /// its interval has passed; when it has not, the instant it will have.
    fn due(&mut self, key: &K, now: Instant) -> Vec<Do<K, W>> {
        let Some(feed) = self.feeds.get_mut(key) else {
            return Vec::new();
        };
        // A feed with no one is kept only to count its runs by: nobody is there to ask for.
        if feed.running || feed.who.is_empty() {
            return Vec::new();
        }
        let next_fresh = feed.every.zip(feed.started).map(|(every, at)| at + every);
        if next_fresh.is_some_and(|next| next <= now) {
            feed.stale = true;
        }
        if !feed.stale {
            return next_fresh.map(Do::WakeAt).into_iter().collect();
        }
        if let Some(next) = feed.started.map(|at| at + MIN_INTERVAL)
            && next > now
        {
            return vec![Do::WakeAt(next)];
        }
        feed.stale = false;
        feed.running = true;
        feed.started = Some(now);
        vec![Do::Run(key.clone())]
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::catalogue::{CATALOGUE, ErrorKind};

    type Hub = super::Hub<&'static str, (&'static str, u64)>;

    fn answer(n: u64) -> Answer {
        Answer::data(json!({ "n": n }), chrono::DateTime::UNIX_EPOCH)
    }

    fn runs(done: &[Do<&'static str, (&'static str, u64)>]) -> Vec<&'static str> {
        done.iter()
            .filter_map(|d| match d {
                Do::Run(key) => Some(*key),
                _ => None,
            })
            .collect()
    }

    fn handed(done: &[Do<&'static str, (&'static str, u64)>]) -> Vec<(&'static str, u64)> {
        done.iter()
            .filter_map(|d| match d {
                Do::Hand(who, _) => Some(*who),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn subscriptions_to_one_query_and_parameters_from_several_pages_are_one_run() {
        let t = Instant::now();
        let mut hub = Hub::default();
        assert_eq!(runs(&hub.subscribe("todos", ("page-a", 1), t)), ["todos"]);
        // A second page, while the first run is in flight: no second run.
        assert!(hub.subscribe("todos", ("page-b", 7), t).is_empty());
        assert_eq!(hub.feeds(), 1);
        let done = hub.finished(&"todos", answer(1), t);
        assert_eq!(handed(&done), [("page-a", 1), ("page-b", 7)]);
        assert!(runs(&done).is_empty());
        // A third, once it is answered: handed that answer, and nothing runs.
        let late = hub.subscribe("todos", ("page-c", 2), t + Duration::from_millis(10));
        assert_eq!(late, [Do::Hand(("page-c", 2), answer(1))]);
        // Other parameters are a feed of their own.
        assert_eq!(
            runs(&hub.subscribe("todos+done", ("page-a", 3), t)),
            ["todos+done"]
        );
        assert_eq!(hub.feeds(), 2);
    }

    #[test]
    fn a_change_pushes_a_new_answer_to_every_subscriber_of_the_feeds_it_moved() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        hub.subscribe("memory", ("page-b", 1), t);
        hub.finished(&"todos", answer(1), t);
        hub.finished(&"memory", answer(1), t);
        let later = t + MIN_INTERVAL;
        assert_eq!(runs(&hub.moved(later, |k| *k == "todos")), ["todos"]);
        assert_eq!(
            hub.finished(&"todos", answer(2), later),
            [Do::Hand(("page-a", 1), answer(2))]
        );
        // Nothing moved: nothing runs.
        assert!(hub.moved(later, |_| false).is_empty());
        assert!(hub.tick(later + MIN_INTERVAL).is_empty());
    }

    #[test]
    fn a_feed_runs_at_most_once_per_interval_and_a_burst_of_changes_is_one_run_after_it() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        hub.finished(&"todos", answer(1), t + Duration::from_millis(50));
        // Ten changes within the interval of the first run: none runs, each says when.
        for n in 1..=10 {
            let at = t + Duration::from_millis(80 * n);
            assert_eq!(
                hub.moved(at, |_| true),
                [Do::WakeAt(t + MIN_INTERVAL)],
                "change {n}"
            );
        }
        // Not yet.
        assert_eq!(
            hub.tick(t + MIN_INTERVAL - Duration::from_millis(1)),
            [Do::WakeAt(t + MIN_INTERVAL)]
        );
        // Then exactly one.
        assert_eq!(runs(&hub.tick(t + MIN_INTERVAL)), ["todos"]);
        assert!(hub.tick(t + MIN_INTERVAL).is_empty());
    }

    #[test]
    fn a_change_heard_while_a_run_is_in_flight_asks_again_once_it_is_done_and_due() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        // In flight: the change is kept, and no second run starts.
        assert!(
            hub.moved(t + Duration::from_millis(10), |_| true)
                .is_empty()
        );
        assert!(
            hub.moved(t + Duration::from_millis(20), |_| true)
                .is_empty()
        );
        // Done before the interval: handed, then asked again when it is due.
        let done = hub.finished(&"todos", answer(1), t + Duration::from_millis(30));
        assert_eq!(handed(&done), [("page-a", 1)]);
        assert!(done.contains(&Do::WakeAt(t + MIN_INTERVAL)));
        assert_eq!(runs(&hub.tick(t + MIN_INTERVAL)), ["todos"]);
        // Done after the interval: asked again at once.
        hub.moved(t + MIN_INTERVAL + Duration::from_millis(1), |_| true);
        let slow = hub.finished(&"todos", answer(2), t + MIN_INTERVAL * 3);
        assert_eq!(runs(&slow), ["todos"]);
    }

    #[test]
    fn a_subscriber_dropped_is_handed_nothing_and_a_feed_with_none_left_is_gone() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        hub.subscribe("todos", ("page-b", 1), t);
        hub.unsubscribe(|(page, _)| *page == "page-a");
        assert_eq!(
            handed(&hub.finished(&"todos", answer(1), t)),
            [("page-b", 1)]
        );
        hub.unsubscribe(|_| true);
        assert_eq!(hub.feeds(), 0);
        // A run still in flight for it answers no one, and nothing is asked again.
        assert!(hub.finished(&"todos", answer(2), t).is_empty());
        assert!(hub.moved(t + MIN_INTERVAL, |_| true).is_empty());
    }

    #[test]
    fn a_page_subscribing_again_with_one_id_holds_only_its_newest_subscription() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        hub.subscribe("memory", ("page-a", 1), t);
        assert_eq!(
            hub.feeds(),
            1,
            "the todos feed went with its only subscriber"
        );
        assert!(hub.finished(&"todos", answer(1), t).is_empty());
    }

    fn change(kind: Kind, workspace: Option<&str>) -> Change {
        Change {
            kind,
            workspace: workspace.map(str::to_owned),
            persona: None,
            path: String::new(),
        }
    }

    fn query(name: &str) -> &'static Query {
        CATALOGUE.iter().find(|q| q.name == name).unwrap()
    }

    #[test]
    fn a_query_is_moved_by_a_change_of_what_it_reads_in_its_own_workspace_only() {
        let todos = query("todo_list");
        let todo_here = [change(Kind::Todos, Some("alpha"))];
        assert!(moves(todos, "alpha", Some(&todo_here)));
        assert!(
            !moves(todos, "beta", Some(&todo_here)),
            "another workspace's"
        );
        assert!(!moves(
            todos,
            "alpha",
            Some(&[change(Kind::Memory, Some("alpha"))])
        ));
        assert!(
            !moves(todos, "alpha", Some(&[change(Kind::Git, None)])),
            "a save"
        );
        assert!(!moves(todos, "alpha", Some(&[])));
        // A change the watch could not place moves everything.
        assert!(moves(todos, "alpha", None));

        let memory = query("memory_search");
        assert!(moves(
            memory,
            "alpha",
            Some(&[change(Kind::Memory, Some("alpha"))])
        ));
        // A persona's memory is not the workspace's.
        assert!(!moves(memory, "alpha", Some(&[change(Kind::Memory, None)])));
        assert!(moves(
            query("session_record_list"),
            "alpha",
            Some(&[change(Kind::Sessions, Some("alpha"))])
        ));
        assert!(moves(
            query("change_status"),
            "alpha",
            Some(&[change(Kind::Workspace, Some("alpha"))])
        ));
    }

    #[test]
    fn the_chats_are_moved_by_the_app_s_chats_and_by_no_change_of_the_plane() {
        let chats = query("chats");
        for kind in [
            Kind::Todos,
            Kind::Memory,
            Kind::Sessions,
            Kind::Workspace,
            Kind::Git,
        ] {
            assert!(!moves(chats, "alpha", Some(&[change(kind, Some("alpha"))])));
        }
        // A change the watch could not place still moves it, as it moves everything.
        assert!(moves(chats, "alpha", None));
    }

    #[test]
    fn every_catalogue_query_names_the_change_kinds_its_answer_is_read_from() {
        for query in &CATALOGUE {
            assert!(
                read_from(query.name).is_some(),
                "{} names no change kind, so any change in its scope asks it again",
                query.id()
            );
        }
    }

    #[test]
    fn a_feed_with_a_freshness_is_asked_again_once_it_has_passed_while_someone_subscribes() {
        let t = Instant::now();
        let every = Duration::from_secs(60);
        let mut hub = Hub::default();
        assert_eq!(
            runs(&hub.subscribe_every("slack", ("page-a", 1), t, Some(every))),
            ["slack"]
        );
        let done = hub.finished(&"slack", answer(1), t + Duration::from_secs(2));
        assert_eq!(handed(&done), [("page-a", 1)]);
        assert!(
            done.contains(&Do::WakeAt(t + every)),
            "it says when it is next due: {done:?}"
        );
        // Not before.
        assert_eq!(
            hub.tick(t + every - Duration::from_secs(1)),
            [Do::WakeAt(t + every)]
        );
        assert_eq!(runs(&hub.tick(t + every)), ["slack"]);
        assert!(runs(&hub.tick(t + every)).is_empty(), "one run in flight");
        hub.finished(&"slack", answer(2), t + every);
        // Nobody left: nothing is asked again.
        hub.unsubscribe(|_| true);
        assert!(hub.tick(t + every * 3).is_empty());
    }

    #[test]
    fn a_feed_with_no_freshness_is_never_asked_again_by_time_alone() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        let done = hub.finished(&"todos", answer(1), t);
        assert!(!done.iter().any(|d| matches!(d, Do::WakeAt(_))), "{done:?}");
        assert!(hub.tick(t + Duration::from_secs(3600)).is_empty());
    }

    #[test]
    fn a_run_that_answers_what_the_feed_last_answered_hands_nothing_on() {
        // #6235: a board's move asks every chats feed again; most answer as they did.
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("chats", ("page-a", 1), t);
        assert_eq!(
            handed(&hub.finished(&"chats", answer(1), t)),
            [("page-a", 1)]
        );
        let later = t + MIN_INTERVAL;
        assert_eq!(runs(&hub.moved(later, |_| true)), ["chats"]);
        let same = Answer::data(json!({ "n": 1 }), chrono::Utc::now());
        assert!(handed(&hub.finished(&"chats", same, later)).is_empty());
        // A page that subscribes now is handed it, and a new answer goes to every page.
        let late = hub.subscribe("chats", ("page-b", 1), later);
        assert_eq!(handed(&late), [("page-b", 1)]);
        let again = later + MIN_INTERVAL;
        assert_eq!(runs(&hub.moved(again, |_| true)), ["chats"]);
        assert_eq!(
            handed(&hub.finished(&"chats", answer(2), again)),
            [("page-a", 1), ("page-b", 1)]
        );
    }

    #[test]
    fn a_feed_with_a_freshness_hands_on_each_answer_it_is_asked_again_for() {
        let t = Instant::now();
        let every = Duration::from_secs(60);
        let mut hub = Hub::default();
        hub.subscribe_every("slack", ("page-a", 1), t, Some(every));
        hub.finished(&"slack", answer(1), t);
        assert_eq!(runs(&hub.tick(t + every)), ["slack"]);
        let fresh = Answer::data(json!({ "n": 1 }), chrono::Utc::now());
        assert_eq!(
            handed(&hub.finished(&"slack", fresh, t + every)),
            [("page-a", 1)],
            "its answer's time is what is fresh"
        );
    }

    #[test]
    fn a_page_that_subscribes_again_and_again_asks_at_most_once_per_freshness() {
        // #6233: re-subscribing must not reset the feed, or a page could hammer GitHub.
        let t = Instant::now();
        let every = Duration::from_secs(60);
        let mut hub = Hub::default();
        let mut started = 0;
        let mut handed_on = 0;
        for n in 0..50u64 {
            let at = t + Duration::from_millis(n * 100);
            // The same subscriber, and a fresh id of the same page, alike.
            for who in [("page-a", 1), ("page-a", n + 2)] {
                let done = hub.subscribe_every("milestones", who, at, Some(every));
                started += runs(&done).len();
                handed_on += handed(&done).len();
            }
            // The page lets go of its last id each round: the feed is left with no one.
            hub.unsubscribe(|w| *w == ("page-a", n + 2));
            hub.unsubscribe(|w| *w == ("page-a", 1));
            if n == 0 {
                hub.finished(&"milestones", answer(1), at);
            }
        }
        assert_eq!(
            started, 1,
            "one run for every subscription within the freshness"
        );
        assert!(
            handed_on >= 98,
            "each later subscription is handed the last answer"
        );
        // Once the freshness has passed it is asked again, once.
        let later = t + every + Duration::from_secs(1);
        assert_eq!(
            runs(&hub.subscribe_every("milestones", ("page-a", 1), later, Some(every))),
            ["milestones"]
        );
        assert!(
            runs(&hub.subscribe_every("milestones", ("page-a", 1), later, Some(every))).is_empty()
        );
    }

    #[test]
    fn a_feed_left_with_no_one_while_its_run_is_in_flight_does_not_start_another() {
        let t = Instant::now();
        let every = Duration::from_secs(60);
        let mut hub = Hub::default();
        assert_eq!(
            runs(&hub.subscribe_every("milestones", ("page-a", 1), t, Some(every))),
            ["milestones"]
        );
        hub.unsubscribe(|_| true);
        assert_eq!(hub.feeds(), 0, "no feed has a subscriber");
        let back = t + Duration::from_secs(2);
        assert!(
            runs(&hub.subscribe_every("milestones", ("page-a", 1), back, Some(every))).is_empty()
        );
        // Its answer reaches whoever subscribes by then.
        assert_eq!(
            handed(&hub.finished(&"milestones", answer(1), back)),
            [("page-a", 1)]
        );
    }

    #[test]
    fn a_feed_with_no_one_is_forgotten_once_its_freshness_has_passed() {
        let t = Instant::now();
        let every = Duration::from_secs(60);
        let mut hub = Hub::default();
        hub.subscribe_every("milestones", ("page-a", 1), t, Some(every));
        hub.finished(&"milestones", answer(1), t);
        hub.unsubscribe(|_| true);
        assert!(hub.tick(t + every).is_empty(), "nobody is there to ask for");
        assert_eq!(hub.kept(), 0, "nothing is kept past its freshness");
        // A core feed with no freshness is dropped with its last subscriber, as before.
        hub.subscribe("todos", ("page-a", 1), t);
        hub.unsubscribe(|_| true);
        assert_eq!(hub.kept(), 0);
    }

    #[test]
    fn an_answer_that_is_an_error_is_handed_as_it_is() {
        let t = Instant::now();
        let mut hub = Hub::default();
        hub.subscribe("todos", ("page-a", 1), t);
        let failed = Answer::error(ErrorKind::Failed, chrono::DateTime::UNIX_EPOCH);
        assert_eq!(
            hub.finished(&"todos", failed.clone(), t),
            [Do::Hand(("page-a", 1), failed)]
        );
    }
}
