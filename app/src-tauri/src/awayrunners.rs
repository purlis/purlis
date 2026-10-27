// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the window's away summary reads of each runner** (#2051, spec #2041 story 15): for each
//! pool a project's chats run on, which of the chats the window has open run there, and the
//! branches its runner has that this machine fetched and does not have yet, ready to push.
//!
//! What finished there and what waits on the person is what the window already counts
//! (`awayCounts.ts`); this says only where each chat runs, so the summary can say it runner by
//! runner (`awayRunners.ts`). A chat's pool is where the window started it and keeps it, so a
//! chat moved since (#2048) is said where it runs now. The branches are read from each clone's
//! `refs/remotes/<pool>/*` as this machine last fetched them (#2046).
//!
//! **And what the runner itself recorded, beside this machine's** (#3393): each run of this
//! project's chats that ended on the runner, as the runner recorded it, labelled with the
//! runner's device and the time it was written, read over the link from the window's own cursor
//! (`crate::runnerlog`) and never copied onto this machine. **Only a chat the window routed to
//! that runner is said**, by how it ended in one of three words: a runner says nothing of a chat
//! it does not run. A runner whose link is down is said to be, never shown as one where nothing
//! happened.

use std::collections::BTreeMap;
use std::path::Path;

use crate::panels::RunnerBranch;
use crate::planes::{PlaneId, Planes};
use crate::runnerlog::{RunEnded, RunnerLogNow};

/// One runner of a project, as the away summary reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerAway {
    /// The pool whose runner it is.
    pub pool: String,
    /// Of the chats asked about, those that run on it, by their number in the plane.
    pub chats: Vec<u32>,
    /// Its branches, as this machine last fetched them, by the clone of a workspace that runs on
    /// it.
    pub branches: Vec<BranchesOfAClone>,
    /// How the window reads what it records now (#3393).
    pub log: RunnerLogRead,
    /// The runs of this project's chats that ended there, as it recorded them, oldest first:
    /// what the window read since it linked.
    pub ended: Vec<RunEndedOnARunner>,
}

/// One clone's branches on a runner, as this machine last fetched them (#2046).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BranchesOfAClone {
    pub workspace: String,
    /// The clone's folder name in the workspace.
    pub repo: String,
    pub branches: Vec<RunnerBranch>,
}

/// How the window reads what a runner records (#3393).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RunnerLogRead {
    /// Over the link, which is up.
    Read,
    /// Over the link, which is down: what happened there since is not known yet.
    LinkDown,
    /// Some of it was not kept for the window: what it says is not all that happened.
    Partly,
    /// Not at all: no link to it is open, or the runner is from a purlis before 1.41.
    NotRead,
}

/// One run of a chat that ended on a runner, as the runner recorded it (#3393).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunEndedOnARunner {
    /// The chat, by its number in the plane.
    pub chat: u32,
    /// How it ended.
    pub ended: RunEnded,
    /// When the runner wrote it, in seconds since 1970: the window says it in the person's time
    /// zone.
    pub at: u32,
    /// The device that wrote it: the runner's.
    pub device: String,
}

/// **Each runner of `plane`'s chats, for the away summary** (#2051): of `sessions`, the chats
/// that run on each pool's runner, and the branches each runner has, as this machine last
/// fetched them. A pool none of them runs on and no workspace names is not said.
#[tauri::command]
#[specta::specta]
pub async fn runners_away(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    sessions: Vec<u32>,
) -> Result<Vec<RunnerAway>, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let config = held.config().map(Path::to_path_buf);
    // This project's chats, by the id its record keeps for each (ADR 0066).
    let reading = root.clone();
    let numbers: BTreeMap<String, u32> = tauri::async_runtime::spawn_blocking(move || {
        purlis_core::reopen::read_or_refusal(&reading)
            .map(|record| {
                record
                    .chats
                    .into_iter()
                    .filter_map(|chat| Some((chat.identity.id?, chat.number?)))
                    .collect()
            })
            .unwrap_or_default()
    })
    .await
    .map_err(|why| format!("the project's chats could not be read: {why}"))?;
    let (on_pools, logs, routed) = {
        let routes = held.chats().sessions();
        let on_pools: Vec<(u32, String)> = sessions
            .iter()
            .filter_map(|session| routes.pool_of(*session).map(|pool| (*session, pool)))
            .collect();
        // Each chat of the record, with the pool the window routed it to.
        let routed: Routed = numbers
            .into_iter()
            .filter_map(|(id, number)| Some((id, (number, routes.pool_of(number)?))))
            .collect();
        (on_pools, routes.runner_logs(), routed)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let workspaces = config
            .map(|config| purlis_core::pools::workspaces_on_pools(&config, &root))
            .unwrap_or_default();
        let logs: BTreeMap<String, RunnerLogNow> = logs.into_iter().collect();
        runners_of(&on_pools, &workspaces, |workspace, pool| {
            branches_of(&root, workspace, pool)
        })
        .into_iter()
        .map(|runner| {
            let log = logs.get(&runner.pool);
            with_its_log(runner, log, &routed)
        })
        .collect()
    })
    .await
    .map_err(|why| format!("the runners could not be read: {why}"))
}

/// Each chat of a project, by its id, with its number and the pool the window routed it to.
type Routed = BTreeMap<String, (u32, String)>;

/// `runner`, with what the window read of what it recorded, `log`, where it read any: each run
/// of a chat `routed` sends to that runner that ended there.
fn with_its_log(mut runner: RunnerAway, log: Option<&RunnerLogNow>, routed: &Routed) -> RunnerAway {
    let Some(log) = log else {
        return runner;
    };
    // A link that is down is said so, whether or not the runner said where its log ends.
    runner.log = if log.down {
        RunnerLogRead::LinkDown
    } else if log.device.is_none() {
        return runner;
    } else if log.missed {
        RunnerLogRead::Partly
    } else {
        RunnerLogRead::Read
    };
    runner.ended = log
        .lines
        .iter()
        .filter_map(|line| {
            let (chat, pool) = routed.get(&line.chat)?;
            (*pool == runner.pool).then_some(())?;
            Some(RunEndedOnARunner {
                chat: *chat,
                ended: line.ended,
                at: u32::try_from(purlis_core::eventlog::written_at_ms(&line.ulid)? / 1000).ok()?,
                device: line.device.clone(),
            })
        })
        .collect();
    runner
}

/// The runners `on_pools` (each chat with its pool) and `workspaces` (each pool with the
/// workspaces that run on it) name, by pool, with each one's branches as `branches` reads them.
fn runners_of(
    on_pools: &[(u32, String)],
    workspaces: &BTreeMap<String, Vec<String>>,
    branches: impl Fn(&str, &str) -> Vec<BranchesOfAClone>,
) -> Vec<RunnerAway> {
    let mut by_pool: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for (session, pool) in on_pools {
        by_pool.entry(pool.clone()).or_default().push(*session);
    }
    for pool in workspaces.keys() {
        by_pool.entry(pool.clone()).or_default();
    }
    by_pool
        .into_iter()
        .map(|(pool, chats)| RunnerAway {
            branches: workspaces
                .get(&pool)
                .into_iter()
                .flatten()
                .flat_map(|workspace| branches(workspace, &pool))
                .collect(),
            pool,
            chats,
            log: RunnerLogRead::NotRead,
            ended: Vec::new(),
        })
        .collect()
}

/// The branches of `pool`'s runner each clone of `workspace` keeps, in the order of the clones'
/// folders: a clone with none is left out.
fn branches_of(root: &Path, workspace: &str, pool: &str) -> Vec<BranchesOfAClone> {
    let folder = root.join("workspaces").join(workspace);
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut repos: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    repos.sort();
    repos
        .into_iter()
        .filter_map(|repo| {
            let branches: Vec<RunnerBranch> =
                purlis_core::codelink::on_a_runner(&folder.join(&repo), pool)
                    .into_iter()
                    .map(|branch| RunnerBranch {
                        pool: pool.to_owned(),
                        branch: branch.branch,
                        ahead: branch.ahead,
                    })
                    .collect();
            (!branches.is_empty()).then(|| BranchesOfAClone {
                workspace: workspace.to_owned(),
                repo,
                branches,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::runnerlog::RunEndLine;

    use super::*;

    fn clone_of(workspace: &str, pool: &str) -> BranchesOfAClone {
        BranchesOfAClone {
            workspace: workspace.into(),
            repo: "app".into(),
            branches: vec![RunnerBranch {
                pool: pool.into(),
                branch: format!("{pool}-work"),
                ahead: 1,
            }],
        }
    }

    // Seen red: with the pools' workspaces left out of `by_pool`, "idle" is not said.
    #[test]
    fn each_pool_says_its_chats_and_the_branches_of_the_workspaces_on_it() {
        let on_pools = vec![
            (3, "qa".to_owned()),
            (5, "build".to_owned()),
            (2, "qa".to_owned()),
        ];
        let workspaces = BTreeMap::from([
            ("qa".to_owned(), vec!["alpha".to_owned()]),
            ("idle".to_owned(), vec!["beta".to_owned()]),
        ]);

        let runners = runners_of(&on_pools, &workspaces, |workspace, pool| {
            vec![clone_of(workspace, pool)]
        });

        assert_eq!(
            runners,
            vec![
                not_read("build", vec![5], vec![]),
                not_read("idle", vec![], vec![clone_of("beta", "idle")]),
                not_read("qa", vec![3, 2], vec![clone_of("alpha", "qa")]),
            ]
        );
    }

    fn not_read(pool: &str, chats: Vec<u32>, branches: Vec<BranchesOfAClone>) -> RunnerAway {
        RunnerAway {
            pool: pool.into(),
            chats,
            branches,
            log: RunnerLogRead::NotRead,
            ended: vec![],
        }
    }

    const RUNNER: &str = "01JB0RUNNER0000000000000000";
    const CHAT: &str = "01K6W0RKER000000000000000B";
    const HERE: &str = "01K6HERE00000000000000000C";
    // 2026-10-27T03:00:00Z, as a ULID's time part.
    const WRITTEN: &str = "01K8HFQ7K00000000000000000";

    fn line(chat: &str, ended: RunEnded) -> RunEndLine {
        RunEndLine {
            chat: chat.into(),
            ended,
            ulid: WRITTEN.into(),
            device: RUNNER.into(),
        }
    }

    // Seen red: with the pool test taken out, the line of chat 7 (this machine's) is said too.
    #[test]
    fn a_runner_says_each_run_of_a_chat_routed_to_it_that_ended_there_and_no_other() {
        let log = RunnerLogNow {
            device: Some(RUNNER.into()),
            lines: vec![
                line(CHAT, RunEnded::Failed),
                line(HERE, RunEnded::Completed),
                line("01K6ANOTHERPROJECT00000000", RunEnded::Completed),
            ],
            missed: false,
            down: false,
        };
        let routed = Routed::from([
            (CHAT.to_owned(), (4, "qa".to_owned())),
            (HERE.to_owned(), (7, "build".to_owned())),
        ]);

        let runner = with_its_log(not_read("qa", vec![4], vec![]), Some(&log), &routed);

        assert_eq!(runner.log, RunnerLogRead::Read);
        assert_eq!(
            runner.ended,
            vec![RunEndedOnARunner {
                chat: 4,
                ended: RunEnded::Failed,
                at: u32::try_from(purlis_core::eventlog::written_at_ms(WRITTEN).unwrap() / 1000)
                    .unwrap(),
                device: RUNNER.into(),
            }]
        );
    }

    // Seen red: with the link's state read only once the runner said its device, the first
    // case reads `not_read`.
    #[test]
    fn a_runner_whose_link_is_down_is_said_so_and_one_never_read_is_not_read() {
        let routed = Routed::new();
        let down_before_it_said = RunnerLogNow {
            down: true,
            ..Default::default()
        };
        assert_eq!(
            with_its_log(
                not_read("qa", vec![], vec![]),
                Some(&down_before_it_said),
                &routed
            )
            .log,
            RunnerLogRead::LinkDown
        );
        let old_runner = RunnerLogNow::default();
        assert_eq!(
            with_its_log(not_read("qa", vec![], vec![]), Some(&old_runner), &routed).log,
            RunnerLogRead::NotRead
        );
        assert_eq!(
            with_its_log(not_read("qa", vec![], vec![]), None, &routed).log,
            RunnerLogRead::NotRead
        );
    }

    #[test]
    fn a_project_with_no_chat_on_a_runner_and_no_pool_has_no_runner() {
        assert!(runners_of(&[], &BTreeMap::new(), |_, _| unreachable!()).is_empty());
    }
}
