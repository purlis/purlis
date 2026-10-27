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
//! `refs/remotes/<pool>/*` as this machine last fetched them (#2046): nothing crosses the link.

use std::collections::BTreeMap;
use std::path::Path;

use crate::planes::{PlaneId, Planes};

/// One runner of a project, as the away summary reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerAway {
    /// The pool whose runner it is.
    pub pool: String,
    /// Of the chats asked about, those that run on it, by their number in the plane.
    pub chats: Vec<u32>,
    /// Its branches, as this machine last fetched them, in the workspaces that run on it.
    pub branches: Vec<BranchOnARunner>,
}

/// One branch of a runner, as this machine last fetched it (#2046).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BranchOnARunner {
    pub workspace: String,
    /// The clone's folder name in the workspace.
    pub repo: String,
    pub branch: String,
    /// The commits it has that this machine's branch of that name does not, or what this
    /// checkout is on where there is none.
    pub ahead: u32,
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
    let on_pools: Vec<(u32, String)> = {
        let routed = held.chats().sessions();
        sessions
            .iter()
            .filter_map(|session| routed.pool_of(*session).map(|pool| (*session, pool)))
            .collect()
    };
    let root = held.root().to_path_buf();
    let config = held.config().map(Path::to_path_buf);
    tauri::async_runtime::spawn_blocking(move || {
        let workspaces = config
            .map(|config| purlis_core::pools::workspaces_on_pools(&config, &root))
            .unwrap_or_default();
        runners_of(&on_pools, &workspaces, |workspace, pool| {
            branches_of(&root, workspace, pool)
        })
    })
    .await
    .map_err(|why| format!("the runners could not be read: {why}"))
}

/// The runners `on_pools` (each chat with its pool) and `workspaces` (each pool with the
/// workspaces that run on it) name, by pool, with each one's branches as `branches` reads them.
fn runners_of(
    on_pools: &[(u32, String)],
    workspaces: &BTreeMap<String, Vec<String>>,
    branches: impl Fn(&str, &str) -> Vec<BranchOnARunner>,
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
        })
        .collect()
}

/// The branches of `pool`'s runner each clone of `workspace` keeps, in the order of the clones'
/// folders.
fn branches_of(root: &Path, workspace: &str, pool: &str) -> Vec<BranchOnARunner> {
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
        .flat_map(|repo| {
            purlis_core::codelink::on_a_runner(&folder.join(&repo), pool)
                .into_iter()
                .map(move |branch| BranchOnARunner {
                    workspace: workspace.to_owned(),
                    repo: repo.clone(),
                    branch: branch.branch,
                    ahead: branch.ahead,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(workspace: &str, name: &str) -> BranchOnARunner {
        BranchOnARunner {
            workspace: workspace.into(),
            repo: "app".into(),
            branch: name.into(),
            ahead: 1,
        }
    }

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
            vec![branch(workspace, &format!("{pool}-work"))]
        });

        assert_eq!(
            runners,
            vec![
                RunnerAway {
                    pool: "build".into(),
                    chats: vec![5],
                    branches: vec![],
                },
                RunnerAway {
                    pool: "idle".into(),
                    chats: vec![],
                    branches: vec![branch("beta", "idle-work")],
                },
                RunnerAway {
                    pool: "qa".into(),
                    chats: vec![3, 2],
                    branches: vec![branch("alpha", "qa-work")],
                },
            ]
        );
    }

    #[test]
    fn a_project_with_no_chat_on_a_runner_and_no_pool_has_no_runner() {
        assert!(runners_of(&[], &BTreeMap::new(), |_, _| unreachable!()).is_empty());
    }
}
