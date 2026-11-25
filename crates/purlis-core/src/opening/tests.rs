// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_phase_is_named_with_its_milliseconds_in_the_order_it_ran() {
    let timed = timed(|| {
        phase("first", || {
            std::thread::sleep(std::time::Duration::from_millis(20))
        });
        phase("second", || ());
    });

    let names: Vec<&str> = timed.phases.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["first", "second"]);
    assert!(timed.phases[0].ms >= 20, "{:?}", timed.phases);
    assert!(timed.ms >= timed.phases[0].ms);
}

#[test]
fn a_phase_with_no_opening_open_only_runs() {
    assert_eq!(phase("alone", || 7), 7);
    assert!(
        timed(|| ()).phases.is_empty(),
        "it leaked into the next opening"
    );
}

#[test]
fn an_opening_inside_an_opening_keeps_its_own_phases() {
    let outer = timed(|| {
        phase("outer", || ());
        let inner = timed(|| phase("inner", || ()));
        assert_eq!(inner.phases.len(), 1);
        phase("outer again", || ());
    });

    let names: Vec<&str> = outer.phases.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["outer", "outer again"]);
}
