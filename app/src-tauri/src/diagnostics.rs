// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What Copy diagnostics asks the core** (#4774): the facts about this machine and this app's
//! chats that the window cannot read itself. The window puts the report together, beside the
//! version it already asked for and each harness's installed version, and puts it on the
//! clipboard; this only answers.
//!
//! Nothing here is a secret or a path: the system's name and release, and two counts.

use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::ramclass::RamClass;
use purlis_core::state::State;
use purlis_core::state::run::{Reason, RunState};

use crate::planes::{Held, Planes};

/// How far reading the machine's memory has come.
#[derive(Debug, Clone, Copy)]
enum Reading {
    /// Nothing has asked yet.
    NotAsked,
    /// A read is running on a thread of its own.
    UnderWay,
    /// The memory was read, as this class.
    Read(RamClass),
    /// The last read could not say: the next ask reads again.
    Unread,
}

/// What reads the machine's class: [`RamClass::read`], or a test's stand-in for it.
type Reader = Arc<dyn Fn() -> Option<RamClass> + Send + Sync>;

/// **This device's RAM class** (ADR 0082 §3, SC-4f #7635), read from the operating system on a
/// thread of its own and stored nowhere. An ask never waits on that read: while it runs the
/// answer is [`DeviceClass::Reading`], and a read that could not say is not kept, so the next
/// ask reads again. A scenario test may stand a class in for it ([`Self::stand_in`]), which
/// changes the class and the target it gives and nothing else.
pub struct Device {
    reading: Arc<Mutex<Reading>>,
    stand_in: Mutex<Option<RamClass>>,
    read: Reader,
    /// Told once the class is known, read or stood in: the gate of automatic hibernation
    /// reads a target only then (SC-4g, #7636).
    known: Arc<Mutex<Option<Known>>>,
}

/// What is told once the device's class is known.
type Known = Arc<dyn Fn() + Send + Sync>;

impl Default for Device {
    fn default() -> Self {
        Self::reading_with(Arc::new(RamClass::read))
    }
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("reading", &self.reading)
            .field("stand_in", &self.stand_in)
            .finish_non_exhaustive()
    }
}

impl Device {
    fn reading_with(read: Reader) -> Self {
        Self {
            reading: Arc::new(Mutex::new(Reading::NotAsked)),
            stand_in: Mutex::new(None),
            read,
            known: Arc::new(Mutex::new(None)),
        }
    }

    /// Tells `known` each time the class becomes known: a read that says, or a stand-in.
    pub fn telling_known(&self, known: Known) {
        *self.known.lock().unwrap_or_else(PoisonError::into_inner) = Some(known);
    }

    fn tell_known(known: &Mutex<Option<Known>>) {
        let known = known.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(known) = known {
            known();
        }
    }

    /// The class now, without waiting: the stand-in where one is set, else what the machine
    /// was read as. An ask that finds nothing read starts a read, and says so.
    pub fn class(&self) -> DeviceClass {
        if let Some(stand_in) = *self.stand_in.lock().unwrap_or_else(PoisonError::into_inner) {
            return DeviceClass::of(stand_in);
        }
        let mut reading = self.reading.lock().unwrap_or_else(PoisonError::into_inner);
        let said = match *reading {
            Reading::Read(class) => return DeviceClass::of(class),
            Reading::UnderWay => return DeviceClass::Reading,
            Reading::NotAsked => DeviceClass::Reading,
            Reading::Unread => DeviceClass::NotKnown,
        };
        *reading = Reading::UnderWay;
        let (read, into) = (Arc::clone(&self.read), Arc::clone(&self.reading));
        let known = Arc::clone(&self.known);
        std::thread::spawn(move || {
            let got = read();
            *into.lock().unwrap_or_else(PoisonError::into_inner) =
                got.map_or(Reading::Unread, Reading::Read);
            if got.is_some() {
                Self::tell_known(&known);
            }
        });
        said
    }

    /// Starts reading the machine's memory, so the first ask finds it read.
    pub fn begin_reading(&self) {
        self.class();
    }

    /// Puts the class of `gigabytes` of memory in place of the machine's, or with `None` takes
    /// it away.
    #[cfg(any(feature = "e2e", test))]
    pub fn stand_in(&self, gigabytes: Option<u64>) {
        *self.stand_in.lock().unwrap_or_else(PoisonError::into_inner) =
            gigabytes.map(|gigabytes| RamClass::of_gigabytes(Some(gigabytes)));
        Self::tell_known(&self.known);
    }
}

/// What Diagnostics says of the device's RAM class (ADR 0082 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeviceClass {
    /// The machine's memory is still being read.
    Reading,
    /// The machine's memory could not be read: no class is known, and so no target.
    NotKnown,
    /// The class, and the hot target it gives: a budget, never a cap.
    Known {
        /// The class in GB (8, 16, 32 or 64), or `None` under 8 GB.
        class_gb: Option<u32>,
        /// How many hot chats the device targets.
        hot_target: u32,
    },
}

impl DeviceClass {
    fn of(class: RamClass) -> Self {
        Self::Known {
            class_gb: class
                .gigabytes()
                .and_then(|gigabytes| u32::try_from(gigabytes).ok()),
            hot_target: class.hot_target(),
        }
    }
}

/// What the report says about where purlis runs and how many chats it holds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DiagnosticsHere {
    /// The system, as [`purlis_core::adopt::system`] says it: `Darwin 27.2.0, aarch64`.
    pub os: String,
    /// The chats open in every project this app holds, a shell tab not among them.
    pub open: u32,
    /// The open chats whose run has a process (the glossary's **hot chat**): `starting`,
    /// `working`, `input-required` or `paused`, on this device. A queued or hibernated run is
    /// open, not hot, and so is a runner's or a pool's chat, whose program runs elsewhere.
    pub hot: u32,
    /// The device's RAM class and its hot target.
    pub device: DeviceClass,
}

/// The run state an open chat's board state stands for, as far as being hot goes: a state hook
/// not yet heard from is a run still `starting`.
fn run_of(state: State) -> RunState {
    match state {
        State::Unknown => RunState::Starting,
        State::Running => RunState::Working,
        State::Waiting => RunState::InputRequired(Reason::TurnEnded),
        State::Done => RunState::Completed,
        State::Failed => RunState::Failed,
    }
}

/// The open and hot chats among `runs`, one current run for each open chat with whether its
/// program runs on this device: only one that does can be hot.
fn counted_where(runs: impl IntoIterator<Item = (RunState, bool)>) -> (u32, u32) {
    runs.into_iter().fold((0, 0), |(open, hot), (run, here)| {
        (open + 1, hot + u32::from(here && run.has_a_process()))
    })
}

/// The open and hot chats among `runs`, each run on this device.
#[cfg(test)]
fn counted(runs: impl IntoIterator<Item = RunState>) -> (u32, u32) {
    counted_where(runs.into_iter().map(|run| (run, true)))
}

/// Copy diagnostics' answer: the system, the open and hot chats of every project held, and the
/// device's class. Read from memory only, so it costs nothing on the window's thread.
#[tauri::command]
#[specta::specta]
pub fn diagnostics_here(
    planes: tauri::State<'_, Planes>,
    device: tauri::State<'_, Device>,
) -> DiagnosticsHere {
    let (open, hot) = planes
        .every_held()
        .iter()
        .map(|held| counted_in(held))
        .fold((0, 0), |(open, hot), (more, hotter)| {
            (open + more, hot + hotter)
        });
    here(purlis_core::adopt::system(), (open, hot), device.class())
}

/// **Chat `session`'s current run**: `None` for a shell tab; `hibernated` for one hibernated;
/// else what its board state stands for.
fn current_run(held: &Held, session: u32) -> Option<RunState> {
    held.chats().harness(session)?;
    if held.hibernated().of(session).is_some() {
        return Some(RunState::Hibernated);
    }
    Some(run_of(held.board().glance(session).state))
}

/// Whether chat `session`'s program runs on this device: not a runner's or a pool's chat,
/// whose program does not use this device's memory.
fn runs_here(held: &Held, session: u32) -> bool {
    let chats = held.chats();
    chats.runner_of(session).is_none() && chats.pool_of(session).is_none()
}

/// **The open and hot chats of `held`**: every chat with a harness is open, wherever it runs;
/// only one whose program runs on this device ([`runs_here`]) can be hot.
pub(crate) fn counted_in(held: &Held) -> (u32, u32) {
    counted_where(
        held.chats()
            .sessions_open()
            .into_iter()
            .filter_map(|session| {
                let run = current_run(held, session)?;
                Some((run, runs_here(held, session)))
            }),
    )
}

/// **How many of `held`'s chats are hot on this device**: one count for Diagnostics and the gate
/// of automatic hibernation (SC-4g, #7636), so both agree on "over the target". A runner's or a
/// pool's chat, a hibernated one and a shell tab are not counted.
pub fn hot_in(held: &Held) -> u32 {
    counted_in(held).1
}

/// The answer, out of the system's name, the open and hot chats, and the device's class.
fn here(os: String, (open, hot): (u32, u32), device: DeviceClass) -> DiagnosticsHere {
    DiagnosticsHere {
        os,
        open,
        hot,
        device,
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
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::*;

    /// A class of `gigabytes` GB, as Diagnostics says it.
    fn known(gigabytes: u64) -> DeviceClass {
        DeviceClass::of(RamClass::of_gigabytes(Some(gigabytes)))
    }

    /// What `device` says once it has stopped reading.
    fn once_read(device: &Device) -> DeviceClass {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let said = device.class();
            if said != DeviceClass::Reading || Instant::now() > deadline {
                return said;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A device whose reads give each of `reads` in turn, and `None` after them.
    fn reading_in_turn(reads: Vec<Option<RamClass>>) -> Device {
        let reads = Mutex::new(reads.into_iter());
        Device::reading_with(Arc::new(move || {
            reads
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .next()
                .flatten()
        }))
    }

    #[test]
    fn a_chat_whose_program_ended_is_open_and_not_hot() {
        let states = [
            State::Unknown,
            State::Running,
            State::Waiting,
            State::Done,
            State::Failed,
        ];
        assert_eq!(counted(states.map(run_of)), (5, 3));
    }

    #[test]
    fn a_chat_that_is_starting_counts_as_hot() {
        // `State::Unknown` is a run from `starting` until a hook reports: a program is there.
        assert_eq!(counted([run_of(State::Unknown)]), (1, 1));
    }

    #[test]
    fn a_queued_run_is_open_and_not_hot() {
        assert_eq!(counted([RunState::Queued]), (1, 0));
    }

    #[test]
    fn hot_is_the_glossary_s_four_states_and_no_other() {
        let runs = [
            RunState::Queued,
            RunState::Starting,
            RunState::Working,
            RunState::InputRequired(Reason::Asked),
            RunState::Paused,
            RunState::Hibernated,
            RunState::Completed,
            RunState::Failed,
            RunState::Stopped,
        ];
        assert_eq!(counted(runs), (9, 4));
    }

    #[test]
    fn the_answer_carries_the_class_the_target_and_the_count() {
        let answer = here(
            "Darwin".into(),
            counted([RunState::Working, RunState::Completed]),
            known(16),
        );
        assert_eq!(
            (answer.open, answer.hot, answer.device),
            (
                2,
                1,
                DeviceClass::Known {
                    class_gb: Some(16),
                    hot_target: 15
                }
            )
        );
    }

    #[test]
    fn a_device_under_8_gb_has_no_class_and_targets_one_chat() {
        assert_eq!(
            known(4),
            DeviceClass::Known {
                class_gb: None,
                hot_target: 1
            }
        );
    }

    #[test]
    fn a_memory_that_could_not_be_read_is_no_class_known_and_is_read_again() {
        let device = reading_in_turn(vec![None, Some(RamClass::of_gigabytes(Some(16)))]);
        assert_eq!(once_read(&device), DeviceClass::NotKnown);
        assert_eq!(once_read(&device), known(16));
    }

    #[test]
    fn an_ask_while_the_memory_is_read_answers_reading_without_waiting() {
        let (release, held) = mpsc::channel::<()>();
        let held = Mutex::new(held);
        let device = Arc::new(Device::reading_with(Arc::new(move || {
            let _ = held.lock().unwrap_or_else(PoisonError::into_inner).recv();
            Some(RamClass::of_gigabytes(Some(32)))
        })));
        // The first ask starts the read, and a second while it runs: neither may wait on it.
        let (told, answer) = mpsc::channel();
        let asking = Arc::clone(&device);
        std::thread::spawn(move || {
            asking.begin_reading();
            told.send(asking.class())
        });
        assert_eq!(
            answer.recv_timeout(Duration::from_secs(5)),
            Ok(DeviceClass::Reading),
            "the ask waited on the read"
        );
        release.send(()).expect("the read is waiting");
        assert_eq!(once_read(&device), known(32));
    }

    #[test]
    fn a_stand_in_replaces_the_machine_s_class_until_it_is_taken_away() {
        let device = reading_in_turn(vec![Some(RamClass::of_gigabytes(Some(16)))]);
        let machine = once_read(&device);
        assert_eq!(machine, known(16));
        device.stand_in(Some(64));
        assert_eq!(device.class(), known(64));
        device.stand_in(Some(8));
        assert_eq!(device.class(), known(8));
        device.stand_in(None);
        assert_eq!(device.class(), machine);
    }

    #[test]
    fn no_chats_count_nothing() {
        assert_eq!(counted([]), (0, 0));
    }
}
