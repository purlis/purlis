// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::engine::Size;

use super::Opening;

#[test]
fn a_default_opening_is_a_shell_with_nothing_set() {
    // #2453: what fixtures build from. A real start sets every field.
    let opening = Opening::default();
    assert_eq!(opening.program, None, "no program is the operator's shell");
    assert!(opening.args.is_empty() && opening.cwd.is_none());
    assert_eq!(opening.size, Size::MIN);
    assert!(opening.env.is_empty() && opening.env_strip.is_empty());
    assert!(opening.declared_env.is_empty() && opening.env_pass.is_empty());
    assert!(opening.harness.is_none() && opening.git_hooks.is_none());
    assert!(!opening.operator_shell);
    assert!(opening.handed.is_empty());
    assert!(opening.confinement.is_none() && opening.run.is_none());
}

#[test]
fn a_chat_waiting_on_its_own_work_is_in_its_turn_and_one_with_nothing_in_flight_is_not() {
    // #3887: the board says `waiting` for a turn that ended on monitors or shells of its own,
    // and a restart, a move or a save made then would cut them off.
    use purlis_core::state::State;
    let glance = |state, own_work| super::Glance {
        state,
        asking: false,
        turns: 1,
        prompt: None,
        own_work,
    };
    assert!(glance(State::Running, false).in_its_turn());
    assert!(glance(State::Waiting, true).in_its_turn());
    assert!(!glance(State::Waiting, false).in_its_turn());
    assert!(!glance(State::Unknown, false).in_its_turn());
    assert!(!glance(State::Done, false).in_its_turn());
}
