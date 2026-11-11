// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dispatchrecord::{
    self, Asker, Changed, Ending, Mode, Opening, Outcome, Place, Report, Running, Worker,
};
use std::path::PathBuf;

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn steward() -> ChatRef {
    ChatRef {
        chat: 3,
        id: Some("01K6ASKER0000000000000000A".to_owned()),
        name: "steward 3".to_owned(),
        persona: Some("steward".to_owned()),
    }
}

/// Worker chat `n` of the fixture: one of 40, each given a ULID of its own.
fn worker(n: u32) -> ChatRef {
    ChatRef {
        chat: n,
        id: Some(ulid::Ulid::from_parts(1_700_000_000_000, u128::from(n)).to_string()),
        name: format!("devops {n}"),
        persona: Some("devops".to_owned()),
    }
}

fn a_task(chat: ChatRef) -> Opening {
    Opening {
        mode: Mode::Task,
        asker: Asker {
            chat: steward(),
            workspace: Some("alpha".to_owned()),
            by_person: false,
            session_record: None,
        },
        persona: Some("devops".to_owned()),
        worker: Worker {
            chat,
            harness: Some("claude".to_owned()),
            profile: None,
            session_record: None,
        },
        task: Some("check prod".to_owned()),
        place: Place::default(),
        brief: "# Check prod\nIs the rollout healthy?".to_owned(),
        report_owed: true,
    }
}

fn done(text: &str) -> Report {
    Report {
        outcome: Outcome::Done,
        text: text.to_owned(),
        changed: Changed::default(),
    }
}

/// How many dispatch records the history fixture holds.
const HISTORY: u32 = 3_000;

/// **A long history**: [`HISTORY`] records written straight to the store, oldest first, as
/// the app would have left them over months. One in a hundred still runs; the rest ended,
/// with each kind of end the sidebar tells apart. Spread over 40 worker chats, so each chat
/// has many dispatches and its newest is not its only one, and one in 37 is a task that never
/// started and was never a chat. The first half ended in August, the rest in October.
fn history(root: &Path) -> Vec<Record> {
    let (_scratch, elsewhere) = project();
    let template =
        dispatchrecord::open(&elsewhere, a_task(worker(1)), at("2026-08-01T00:00:00Z")).unwrap();
    std::fs::create_dir_all(dispatchrecord::dir(root)).unwrap();
    let start = at("2026-08-01T00:00:00Z").timestamp_millis();
    (0..HISTORY)
        .map(|n| {
            let mut record = template.clone();
            let minted = u64::try_from(start).unwrap() + u64::from(n) * 60_000;
            record.id = ulid::Ulid::from_parts(minted, u128::from(n)).to_string();
            record.worker.chat = worker(n % 40 + 1);
            if n % 100 != 99 {
                let month = if n < HISTORY / 2 { "08" } else { "10" };
                record.ended = Some(format!("2026-{month}-02T00:00:00+00:00"));
                record.report = Some(Report {
                    outcome: [Outcome::Done, Outcome::Blocked, Outcome::Failed][n as usize % 3],
                    ..done("checked")
                });
                if n % 7 == 0 {
                    record.ended_by = Some(dispatchrecord::EndedBy::Person);
                }
                if n % 37 == 0 {
                    record.did_not_start = true;
                    record.worker.chat.id = None;
                }
            }
            let path = dispatchrecord::dir(root).join(format!("{}.json", record.id));
            std::fs::write(path, serde_json::to_string_pretty(&record).unwrap()).unwrap();
            record
        })
        .collect()
}

fn ids<'a>(records: impl IntoIterator<Item = &'a Record>) -> Vec<String> {
    records
        .into_iter()
        .map(|record| record.id.clone())
        .collect()
}

/// What the full read answers: every record read, newest first.
struct Full(Vec<Record>);

impl Full {
    fn of(root: &Path) -> Self {
        Self(dispatchrecord::list(root))
    }

    fn running(&self) -> Vec<String> {
        ids(self.0.iter().filter(|record| record.running()))
    }

    fn ended(&self) -> Vec<String> {
        ids(self.0.iter().filter(|record| !record.running()))
    }

    fn latest_for(&self, chat: &ChatRef) -> Option<Record> {
        self.0
            .iter()
            .find(|record| {
                !dispatchrecord::never_a_chat(record)
                    && dispatchrecord::same_chat(&record.worker.chat, chat)
            })
            .cloned()
    }
}

/// Every answer the ledger gives is the full read's, for every worker chat of the fixture
/// and for a number that is nobody's.
fn same_as_the_full_read(root: &Path) {
    let full = Full::of(root);
    let ledger = ledger(root);
    assert_eq!(ids(ledger.running()), full.running(), "the running answer");
    assert_eq!(
        ledger.ended().map(str::to_owned).collect::<Vec<_>>(),
        full.ended(),
        "the ended answer"
    );
    for chat in (1..=41).map(worker) {
        assert_eq!(
            ledger.latest_for(root, &chat),
            full.latest_for(&chat),
            "the newest dispatch of {}",
            chat.name
        );
    }
}

fn summary(root: &Path) -> String {
    std::fs::read_to_string(dispatchrecord::dir(root).join(SUMMARY_NAME)).unwrap_or_default()
}

/// Writes `record` back as still running, behind the store's back: what a read of its file
/// would now find. Where an answer does not name it as running, its file was not read.
fn running_again_on_disk(root: &Path, record: &Record) {
    let mut back = record.clone();
    back.ended = None;
    back.report = None;
    let path = dispatchrecord::dir(root).join(format!("{}.json", record.id));
    std::fs::write(path, serde_json::to_string_pretty(&back).unwrap()).unwrap();
}

#[test]
fn over_thousands_of_dispatches_the_summary_answers_as_the_full_read_does() {
    let (_d, root) = project();
    history(&root);
    // The first read writes the summary, and every later one reads it.
    same_as_the_full_read(&root);
    assert!(!summary(&root).is_empty(), "the summary is written");
    same_as_the_full_read(&root);
}

#[test]
fn a_dispatch_the_summary_holds_as_ended_is_not_read_again() {
    let (_d, root) = project();
    let records = history(&root);
    let ended = records.iter().find(|record| !record.running()).unwrap();
    ledger(&root);

    running_again_on_disk(&root, ended);

    assert!(
        !ids(ledger(&root).running()).contains(&ended.id),
        "its file was read"
    );
}

#[test]
fn the_summary_holds_nothing_a_chat_wrote() {
    let (_d, root) = project();
    history(&root);
    ledger(&root);

    let text = summary(&root);
    assert!(!text.contains("Is the rollout healthy"), "no brief");
    assert!(!text.contains("checked"), "no report");
}

#[test]
fn deleting_the_summary_rebuilds_it_and_the_answers_stay_the_same() {
    let (_d, root) = project();
    history(&root);
    same_as_the_full_read(&root);
    let built = summary(&root);

    std::fs::remove_file(dispatchrecord::dir(&root).join(SUMMARY_NAME)).unwrap();

    same_as_the_full_read(&root);
    assert_eq!(summary(&root), built, "rebuilt as it was");
}

#[test]
fn a_summary_that_does_not_read_is_rebuilt_and_the_answers_stay_the_same() {
    let (_d, root) = project();
    history(&root);
    ledger(&root);
    std::fs::write(dispatchrecord::dir(&root).join(SUMMARY_NAME), "{ torn").unwrap();

    same_as_the_full_read(&root);
    assert!(summary(&root).starts_with('{') && summary(&root).contains("\"v\""));
}

#[test]
fn a_dispatch_that_ends_after_the_summary_was_written_is_read_and_summarised() {
    let (_d, root) = project();
    history(&root);
    let opened =
        dispatchrecord::open(&root, a_task(worker(5)), at("2026-11-01T00:00:00Z")).unwrap();
    assert!(ids(ledger(&root).running()).contains(&opened.id));
    assert!(
        !summary(&root).contains(&opened.id),
        "running is not summarised"
    );

    let ending = Ending {
        report: Some(done("healthy")),
        usage: None,
    };
    assert!(dispatchrecord::close(&root, &opened.id, ending, at("2026-11-01T00:05:00Z")).unwrap());

    same_as_the_full_read(&root);
    assert!(summary(&root).contains(&opened.id), "its end is summarised");
    assert_eq!(
        ledger(&root)
            .latest_for(&root, &worker(5))
            .map(|record| record.report),
        Some(Some(done("healthy"))),
        "the newest of its chat is read whole"
    );
}

#[test]
fn a_report_sent_again_after_its_end_is_what_the_newest_dispatch_of_its_chat_says() {
    let (_d, root) = project();
    history(&root);
    let opened =
        dispatchrecord::open(&root, a_task(worker(9)), at("2026-11-01T00:00:00Z")).unwrap();
    let ending = |text: &str| Ending {
        report: Some(Report {
            outcome: Outcome::Blocked,
            ..done(text)
        }),
        usage: None,
    };
    dispatchrecord::close(
        &root,
        &opened.id,
        ending("waiting"),
        at("2026-11-01T00:05:00Z"),
    )
    .unwrap();
    ledger(&root);

    assert!(
        dispatchrecord::reported_again(
            &root,
            &opened.id,
            ending("still"),
            at("2026-11-02T00:00:00Z")
        )
        .unwrap()
    );

    let newest = ledger(&root).latest_for(&root, &worker(9)).unwrap();
    assert_eq!(newest.report.unwrap().text, "still");
}

#[test]
fn pruning_after_30_days_keeps_the_summary_in_step() {
    let (_d, root) = project();
    let records = history(&root);
    ledger(&root);
    let (august, october): (Vec<&Record>, Vec<&Record>) = records
        .iter()
        .filter(|record| !record.running())
        .partition(|record| record.ended.as_deref().unwrap().starts_with("2026-08"));

    let swept = crate::retention::sweep(&root, at("2026-10-15T00:00:00Z").into(), &[]);

    assert_eq!(swept.dispatches, august.len());
    let text = summary(&root);
    assert!(
        august.iter().all(|record| !text.contains(&record.id)),
        "a pruned dispatch is gone from the summary"
    );
    assert!(october.iter().all(|record| text.contains(&record.id)));
    same_as_the_full_read(&root);
}

#[test]
fn the_roster_s_running_records_take_the_summary_s_word_for_what_ended() {
    // #5502: a launch's first look no longer reads every ended record.
    let (_d, root) = project();
    let records = history(&root);
    ledger(&root);
    let ended = records
        .iter()
        .find(|record| !record.running() && !dispatchrecord::never_a_chat(record))
        .unwrap();
    running_again_on_disk(&root, ended);

    let mut running = Running::default();
    let later = std::fs::metadata(dispatchrecord::dir(&root))
        .unwrap()
        .modified()
        .unwrap()
        + dispatchrecord::A_FOLDER_TIME_SETTLES;
    let named = ids(running.orphaned_at(&root, later, &steward(), |_| false));

    assert!(!named.contains(&ended.id), "its file was read");
    assert_eq!(
        named,
        Full::of(&root)
            .running()
            .into_iter()
            .filter(|id| *id != ended.id)
            .collect::<Vec<_>>()
    );
}

/// A chat that is not [`steward`]: the asker of most of [`a_busy_store`]'s ended tasks.
fn someone_else() -> ChatRef {
    ChatRef {
        chat: 4,
        id: Some("01K6ASKER0000000000000000B".to_owned()),
        name: "steward 4".to_owned(),
        persona: Some("steward".to_owned()),
    }
}

/// **A store a busy chat lists**: 50 records, oldest first. Ten still run, each asked by
/// [`steward`] of workers 1 to 10: the open rows of its list. Of the 40 that ended and
/// reported, five were asked by [`steward`] (workers 11 to 15) and the rest by someone else.
fn a_busy_store(root: &Path) -> Vec<Record> {
    let (_scratch, elsewhere) = project();
    let template =
        dispatchrecord::open(&elsewhere, a_task(worker(1)), at("2026-11-01T00:00:00Z")).unwrap();
    std::fs::create_dir_all(dispatchrecord::dir(root)).unwrap();
    let start = at("2026-11-01T00:00:00Z").timestamp_millis();
    (0..50u32)
        .map(|n| {
            let mut record = template.clone();
            let minted = u64::try_from(start).unwrap() + u64::from(n) * 60_000;
            record.id = ulid::Ulid::from_parts(minted, u128::from(n)).to_string();
            if n >= 40 {
                record.worker.chat = worker(n - 39);
            } else {
                record.worker.chat = worker(n % 20 + 11);
                record.ended = Some("2026-11-02T00:00:00+00:00".to_owned());
                record.report = Some(done("checked"));
                if n >= 5 {
                    record.asker.chat = someone_else();
                }
            }
            let path = dispatchrecord::dir(root).join(format!("{}.json", record.id));
            std::fs::write(path, serde_json::to_string_pretty(&record).unwrap()).unwrap();
            record
        })
        .collect()
}

#[test]
fn a_list_reads_each_running_record_once_and_of_the_ended_only_its_own() {
    // Felt speed round 2, H3: a chat's list read the store once per open row, and every
    // record in it for the finished rows. One reading answers every row now.
    let (_d, root) = project();
    let records = a_busy_store(&root);
    let open_rows: Vec<ChatRef> = (1..=10).map(worker).collect();
    ledger(&root);
    dispatchrecord::reads::taken();

    let ledger = ledger(&root);
    for row in &open_rows {
        assert!(
            ledger
                .latest_for(&root, row)
                .is_some_and(|record| record.running())
        );
    }
    let finished = ledger.finished_for(&root, &steward(), |worker| {
        open_rows
            .iter()
            .any(|row| dispatchrecord::same_chat(worker, row))
    });

    let reads = dispatchrecord::reads::taken();
    assert_eq!(
        ids(&finished),
        ids(records.iter().take(5)),
        "its own finished tasks, oldest first"
    );
    assert!(
        reads <= 10 + 5,
        "{reads} records read for a list of 10 open and 5 finished rows over 50 records"
    );
}
