// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What Copy diagnostics asks the core** (#4774): the facts about this machine and this app's
//! chats that the window cannot read itself. The window puts the report together, beside the
//! version it already asked for and each harness's installed version, and puts it on the
//! clipboard; this only answers.
//!
//! Nothing here is a secret or a path: the system's name and release, and two counts.

use std::sync::{Mutex, OnceLock, PoisonError};

use purlis_core::ramclass::RamClass;
use purlis_core::state::State;

use crate::planes::Planes;

/// **This device's RAM class**, read from the operating system once (ADR 0082 §3, SC-4f #7635),
/// and stored nowhere. A scenario test may stand a class in for it ([`Self::stand_in`]), which
/// changes the class and the target it gives and nothing else.
#[derive(Debug, Default)]
pub struct Device {
    read: OnceLock<RamClass>,
    stand_in: Mutex<Option<RamClass>>,
}

impl Device {
    /// The class now: the stand-in where one is set, else what the machine reads as.
    pub fn class(&self) -> RamClass {
        let stand_in = *self.stand_in.lock().unwrap_or_else(PoisonError::into_inner);
        stand_in.unwrap_or_else(|| *self.read.get_or_init(RamClass::read))
    }

    /// Reads the machine's memory now, so the first ask does not wait on it.
    pub fn read_now(&self) {
        self.read.get_or_init(RamClass::read);
    }

    /// Puts the class of `gigabytes` of memory in place of the machine's, or with `None` takes
    /// it away.
    #[cfg(any(feature = "e2e", test))]
    pub fn stand_in(&self, gigabytes: Option<u64>) {
        *self.stand_in.lock().unwrap_or_else(PoisonError::into_inner) =
            gigabytes.map(|gigabytes| RamClass::of_gigabytes(Some(gigabytes)));
    }
}

/// What the report says about where purlis runs and how many chats it holds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DiagnosticsHere {
    /// The system, as [`purlis_core::adopt::system`] says it: `Darwin 27.2.0, aarch64`.
    pub os: String,
    /// The chats open in every project this app holds, a shell tab not among them.
    pub open: u32,
    /// The open chats whose run has a program (the glossary's **hot chat**): any not done or
    /// failed.
    pub hot: u32,
    /// The device's RAM class in GB (8, 16, 32 or 64), or `None` under 8 GB (ADR 0082 §3).
    pub ram_class_gb: Option<u32>,
    /// How many hot chats the device targets: a budget, never a cap.
    pub hot_target: u32,
}

/// The open and hot chats among chats in `states`, one state for each open chat.
fn counted(states: impl IntoIterator<Item = State>) -> (u32, u32) {
    states.into_iter().fold((0, 0), |(open, hot), state| {
        let has_a_program = !matches!(state, State::Done | State::Failed);
        (open + 1, hot + u32::from(has_a_program))
    })
}

/// Copy diagnostics' answer: the system, and the open and hot chats of every project held.
/// Read from memory only, so it costs nothing on the window's thread.
#[tauri::command]
#[specta::specta]
pub fn diagnostics_here(
    planes: tauri::State<'_, Planes>,
    device: tauri::State<'_, Device>,
) -> DiagnosticsHere {
    let states = planes.every_held().into_iter().flat_map(|held| {
        held.chats()
            .open_now()
            .into_iter()
            .filter(|open| open.harness.is_some())
            .map(|open| held.board().glance(open.session).state)
            .collect::<Vec<_>>()
    });
    here(purlis_core::adopt::system(), states, device.class())
}

/// The answer, out of the system's name, each open chat's state and the device's class.
fn here(os: String, states: impl IntoIterator<Item = State>, class: RamClass) -> DiagnosticsHere {
    let (open, hot) = counted(states);
    DiagnosticsHere {
        os,
        open,
        hot,
        ram_class_gb: class.gigabytes().and_then(|gigabytes| u32::try_from(gigabytes).ok()),
        hot_target: class.hot_target(),
    }
}

/// **A class stood in for the machine's** (the scenario tests: CI's runner will not give the top
/// class). The `e2e` build only; it sets the class and the target and nothing else.
#[cfg(feature = "e2e")]
#[tauri::command]
#[specta::specta]
pub fn ram_class_stand_in(device: tauri::State<'_, Device>, gigabytes: Option<u32>) {
    device.stand_in(gigabytes.map(u64::from));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_whose_program_ended_is_open_and_not_hot() {
        let states = [
            State::Unknown,
            State::Running,
            State::Waiting,
            State::Done,
            State::Failed,
        ];
        assert_eq!(counted(states), (5, 3));
    }

    #[test]
    fn a_chat_that_is_starting_counts_as_hot() {
        // `State::Unknown` is a run from `starting` until a hook reports: a program is there.
        assert_eq!(counted([State::Unknown]), (1, 1));
    }

    #[test]
    fn the_answer_carries_the_class_the_target_and_the_count() {
        let class = RamClass::of_gigabytes(Some(16));
        let answer = here("Darwin".into(), [State::Running, State::Done], class);
        assert_eq!(
            (answer.open, answer.hot, answer.ram_class_gb, answer.hot_target),
            (2, 1, Some(16), 15)
        );
    }

    #[test]
    fn a_device_under_8_gb_has_no_class_and_targets_one_chat() {
        let answer = here("Darwin".into(), [], RamClass::of_gigabytes(Some(4)));
        assert_eq!((answer.ram_class_gb, answer.hot_target), (None, 1));
    }

    #[test]
    fn a_stand_in_replaces_the_machine_s_class_until_it_is_taken_away() {
        let device = Device::default();
        let machine = device.class();
        device.stand_in(Some(64));
        assert_eq!((device.class().gigabytes(), device.class().hot_target()), (Some(64), 50));
        device.stand_in(Some(8));
        assert_eq!((device.class().gigabytes(), device.class().hot_target()), (Some(8), 5));
        device.stand_in(None);
        assert_eq!(device.class(), machine);
    }

    #[test]
    fn no_chats_count_nothing() {
        assert_eq!(counted([]), (0, 0));
    }
}
