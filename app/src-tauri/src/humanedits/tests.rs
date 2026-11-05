// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::state::State;

use crate::host::Glance;

use super::state_of;

fn glance(state: State) -> Glance {
    Glance {
        state,
        asking: false,
        turns: 1,
        prompt: None,
        own_work: false,
        wakes_on_its_own_work: false,
    }
}

#[test]
fn a_chat_whose_turn_ended_is_idle_and_one_running_or_asking_is_named_with_its_state() {
    assert_eq!(state_of(&glance(State::Done)), None);
    assert_eq!(state_of(&glance(State::Failed)), None);
    assert_eq!(state_of(&glance(State::Waiting)), None);
    assert_eq!(
        state_of(&glance(State::Running)).as_deref(),
        Some("is running")
    );
    assert_eq!(
        state_of(&Glance {
            asking: true,
            ..glance(State::Waiting)
        })
        .as_deref(),
        Some("is asking you something")
    );
    assert_eq!(
        state_of(&Glance {
            own_work: true,
            ..glance(State::Waiting)
        })
        .as_deref(),
        Some("is waiting on work of its own")
    );
    assert_eq!(
        state_of(&glance(State::Unknown)).as_deref(),
        Some("has not been heard from yet")
    );
}
