// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

use jiff::{SignedDuration, Timestamp};

use super::*;

fn at(minutes: i64) -> Timestamp {
    Timestamp::from_second(1_764_230_400).unwrap() + SignedDuration::from_mins(minutes)
}

/// A switch whose filter changes are written down, in order.
fn switch() -> (Switch, Arc<Mutex<Vec<bool>>>) {
    let applied = Arc::new(Mutex::new(Vec::new()));
    let told = Arc::clone(&applied);
    (
        Switch::new(Box::new(move |on| told.lock().unwrap().push(on))),
        applied,
    )
}

#[test]
fn it_is_off_until_it_is_turned_on() {
    let (switch, applied) = switch();
    assert_eq!(switch.at(at(0)), Detailed { until: None });
    assert!(applied.lock().unwrap().is_empty());
}

#[test]
fn turned_on_it_raises_the_steps_to_debug_for_one_hour() {
    let (switch, applied) = switch();
    let now = switch.turn_on(at(0));
    assert_eq!(now.until, Some(at(60)));
    assert_eq!(*applied.lock().unwrap(), [true]);
    assert_eq!(switch.at(at(59)).until, Some(at(60)));
    assert_eq!(*applied.lock().unwrap(), [true]);
}

#[test]
fn it_turns_itself_off_once_the_hour_is_up() {
    let (switch, applied) = switch();
    switch.turn_on(at(0));
    assert_eq!(switch.at(at(60)), Detailed { until: None });
    assert_eq!(*applied.lock().unwrap(), [true, false]);
    // Asked again, it is off, and the filter is not changed again.
    assert_eq!(switch.at(at(61)), Detailed { until: None });
    assert_eq!(*applied.lock().unwrap(), [true, false]);
}

#[test]
fn turned_on_again_it_lasts_an_hour_from_then() {
    let (switch, _) = switch();
    switch.turn_on(at(0));
    assert_eq!(switch.turn_on(at(30)).until, Some(at(90)));
    assert_eq!(switch.at(at(75)).until, Some(at(90)));
}

#[test]
fn turned_off_it_lowers_them_at_once() {
    let (switch, applied) = switch();
    switch.turn_on(at(0));
    assert_eq!(switch.turn_off(), Detailed { until: None });
    assert_eq!(*applied.lock().unwrap(), [true, false]);
    // Off already: nothing to change.
    switch.turn_off();
    assert_eq!(*applied.lock().unwrap(), [true, false]);
}

#[test]
fn the_filter_lets_the_steps_debug_lines_through_only_while_it_is_on() {
    use tracing_subscriber::layer::SubscriberExt;
    let said = Arc::new(Mutex::new(Vec::<String>::new()));
    struct Lines(Arc<Mutex<Vec<String>>>);
    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Lines {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            self.0.lock().unwrap().push(format!(
                "{} {}",
                event.metadata().level(),
                event.metadata().target()
            ));
        }
    }
    for (on, step_debug) in [(false, false), (true, true)] {
        said.lock().unwrap().clear();
        let subscriber = tracing_subscriber::registry()
            .with(kept(on))
            .with(Lines(Arc::clone(&said)));
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: "purlis::step", "a docker call");
            tracing::debug!(target: "purlis_core::other", "core detail");
            tracing::info!(target: "purlis::step", "a step");
            tracing::debug!(target: "hyper", "a library's detail");
        });
        let said = said.lock().unwrap().clone();
        assert_eq!(
            said.contains(&"DEBUG purlis::step".to_owned()),
            step_debug,
            "{on}: {said:?}"
        );
        assert!(said.contains(&"INFO purlis::step".to_owned()), "{said:?}");
        // Only the steps: nothing else of purlis's, and no library's, at debug.
        assert!(
            !said
                .iter()
                .any(|l| l.contains("other") || l.contains("hyper")),
            "{said:?}"
        );
    }
}
