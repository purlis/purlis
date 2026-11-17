// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::cmp::Ordering;
use std::time::Duration;

use super::{FIELDS, HARNESS_BASH_TIMEOUT, PAIRS, Side, Timings};

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

/// The panic message `run` ends with.
fn refusal(run: impl FnOnce() + std::panic::UnwindSafe) -> String {
    let err = std::panic::catch_unwind(run).expect_err("refused");
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

/// The field names `Debug` prints for a value, in order: every field the type has.
fn field_names(timings: &Timings) -> Vec<String> {
    let debug = format!("{timings:?}");
    let inner = debug
        .trim_start_matches("Timings")
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}');
    inner
        .split(", ")
        .filter_map(|part| {
            part.split_once(": ")
                .map(|(name, _)| name.trim().to_owned())
        })
        .filter(|name| name != "sealed")
        .collect()
}

#[test]
fn production_timings_are_exactly_the_shipped_constants() {
    let t = Timings::production();
    let shipped: [(&str, Option<Duration>, &str); 10] = [
        (
            "hold",
            Some(secs(60)),
            "N-6: a held connection waits about a minute",
        ),
        (
            "group",
            Some(secs(2)),
            "N-10: new hosts within 2 s are one ask",
        ),
        (
            "quiet_hold",
            Some(secs(15 * 60)),
            "ADR 0121 decision 13, #6007: held open while Quiet, 15 min at most (Q300)",
        ),
        (
            "throttle_window",
            Some(secs(60)),
            "#1338, Q289: the same block is heard once a minute",
        ),
        (
            "patience",
            Some(secs(30)),
            "egress: a client sends its request, a host answers, within 30 s",
        ),
        (
            "teller_patience",
            Some(secs(2)),
            "#1353: a stopping proxy waits 2 s for its teller",
        ),
        (
            "head",
            Some(secs(30)),
            "egress LIMITS: a request's head within 30 s",
        ),
        (
            "idle",
            Some(secs(600)),
            "egress LIMITS: a connection idle 600 s is closed",
        ),
        (
            "tally_window",
            Some(secs(60)),
            "#1699: a tally line stands for a minute",
        ),
        (
            "tally_tick",
            Some(secs(5)),
            "#1699: the tally timer ticks every 5 s",
        ),
    ];
    for (field, value, set_by) in shipped {
        assert_eq!(t.duration(field), value, "{field} is what {set_by} set");
    }
    assert_eq!(
        t.throttle_per_chat, 10,
        "throttle_per_chat is what #1338 set: at most 10 blocks a minute per chat"
    );
    // And every field is one of those: a new one is pinned here too.
    let mut pinned: Vec<&str> = shipped.iter().map(|(field, _, _)| *field).collect();
    pinned.push("throttle_per_chat");
    let mut names = field_names(&t);
    names.sort();
    pinned.sort_unstable();
    assert_eq!(names, pinned, "every field of Timings is pinned here");
}

#[test]
fn scaled_timings_keep_every_declared_pairs_production_relation() {
    let half = Timings::scaled(0.5);
    assert_eq!(half.group, secs(1));
    assert_eq!(half.teller_patience, secs(1));
    assert_eq!(half.hold, secs(30));
    for pair in PAIRS.iter().filter(|pair| !pair.production_only) {
        let at = |t: &Timings| t.side(pair.left).cmp(&t.side(pair.right));
        assert_eq!(
            at(&half),
            at(&Timings::production()),
            "{} compares as at production",
            pair.name
        );
    }
    let why = refusal(|| {
        let _ = Timings::scaled(0.25);
    });
    assert!(
        why.contains("group") && why.contains("the hook sender's wait"),
        "scaled(0.25) is refused on group against the 500 ms wait: {why}"
    );
}

#[test]
fn scaled_keeps_the_throttle_hold_tie() {
    let pair = PAIRS
        .iter()
        .find(|pair| {
            pair.left == Side::Field("throttle_window") && pair.right == Side::Field("hold")
        })
        .expect("throttle_window against hold is declared");
    assert!(!pair.production_only, "the tie is kept in a scaled run");
    let production = Timings::production();
    assert_eq!(
        production.throttle_window, production.hold,
        "a tie at production"
    );
    for factor in [0.5, 0.6, 0.75, 1.0] {
        let t = Timings::scaled(factor);
        assert_eq!(t.throttle_window, t.hold, "still a tie at ×{factor}");
    }
}

#[test]
fn every_timings_field_names_its_interacting_pairs() {
    let declared: Vec<&str> = FIELDS.iter().map(|one| one.field).collect();
    let mut sorted = declared.clone();
    sorted.sort_unstable();
    let mut names = field_names(&Timings::production());
    names.sort();
    assert_eq!(sorted, names, "every field of Timings is declared once");
    for one in &FIELDS {
        assert!(
            !one.pairs.is_empty() || one.alone.is_some_and(|why| !why.is_empty()),
            "{} names its pairs, or why it is in none",
            one.field
        );
        for name in one.pairs {
            let pair = PAIRS
                .iter()
                .find(|pair| pair.name == *name)
                .unwrap_or_else(|| panic!("{}'s pair `{name}` is declared", one.field));
            assert!(
                [pair.left, pair.right].contains(&Side::Field(one.field)),
                "`{name}` is a pair of {}",
                one.field
            );
        }
    }
    for pair in &PAIRS {
        for side in [pair.left, pair.right] {
            if let Side::Field(field) = side {
                let one = FIELDS
                    .iter()
                    .find(|one| one.field == field)
                    .unwrap_or_else(|| panic!("`{}` names a field, {field}", pair.name));
                assert!(
                    one.pairs.contains(&pair.name),
                    "{field} names `{}`, a pair it is in",
                    pair.name
                );
            }
        }
    }
}

#[test]
fn the_supervisor_window_tie_is_production_only() {
    let window = purlis_linux_sandbox::refusal::WINDOW;
    let production = Timings::production();
    for field in ["throttle_window", "hold"] {
        let pair = PAIRS
            .iter()
            .find(|pair| {
                pair.left == Side::Field(field)
                    && matches!(pair.right, Side::Fixed(_, at) if at == window)
            })
            .unwrap_or_else(|| panic!("{field} against the supervisor's WINDOW is declared"));
        assert!(pair.production_only, "{} is production only", pair.name);
        assert_eq!(
            production.side(pair.left).cmp(&production.side(pair.right)),
            Ordering::Equal,
            "{} is a tie at production",
            pair.name
        );
        // Exempt: a scaled run breaks the tie and is not refused for it.
        let half = Timings::scaled(0.5);
        assert_eq!(
            half.side(pair.left).cmp(&half.side(pair.right)),
            Ordering::Less
        );
    }
    let hook = PAIRS
        .iter()
        .find(|pair| {
            pair.left == Side::Field("hold")
                && matches!(pair.right, Side::Fixed(_, at) if at == crate::harness::hooked::HOOK_TIMEOUT)
        })
        .expect("hold against HOOK_TIMEOUT is declared");
    assert!(
        hook.production_only,
        "hold against HOOK_TIMEOUT is production only"
    );
}

#[test]
fn quiet_hold_scales_by_its_own_factor_and_stays_above_hold_and_the_bash_timeout() {
    assert_eq!(HARNESS_BASH_TIMEOUT, secs(120), "the unscaled Bash timeout");
    for pair in [
        "quiet_hold > hold",
        "quiet_hold > the harness's Bash timeout",
    ] {
        let declared = PAIRS
            .iter()
            .find(|one| one.name == pair)
            .unwrap_or_else(|| panic!("`{pair}` is declared"));
        assert!(!declared.production_only, "`{pair}` is checked when scaled");
    }
    // Its own factor: the rest stays where it was.
    let base = Timings::scaled(0.5);
    let quiet = base.with_quiet_hold_scaled(0.2);
    assert_eq!(quiet.quiet_hold, secs(180), "15 min × 0.2");
    assert_eq!(quiet.hold, base.hold, "the hold is not moved");
    assert!(quiet.quiet_hold > quiet.hold);
    assert!(quiet.quiet_hold > HARNESS_BASH_TIMEOUT);
    // Below the Bash timeout, it is refused.
    let why = refusal(move || {
        let _ = base.with_quiet_hold_scaled(0.1);
    });
    assert!(
        why.contains("quiet_hold > the harness's Bash timeout"),
        "90 s is refused against the Bash timeout: {why}"
    );
}
