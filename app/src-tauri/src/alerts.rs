// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Inbox's alerts' one question: what is wrong in every project this process holds (#1695:
//! the Alerts drawer's rows are Notices in the Inbox now).
//!
//! **Cross-project by construction.** An alert is about a plane — its pin, its front door,
//! its workspaces' layout, its root — and not about the workspace or the chat on screen, so
//! the reading is the window's and asks about every plane at once. The command takes no plane
//! for exactly that reason: a reading that could be asked about one project could be wired to
//! the one in front and quietly stop being about the rest.
//!
//! The deciding is `purlis_core::alerts`, the port of charter's `_alerts`, which also draws
//! the terminal status line's rows — so the Inbox and `purlis statusline` cannot disagree
//! about whether a plane has anything to say.

use std::path::Path;

use purlis_core::alerts;

use crate::planes::PlaneId;

/// One alert, as the Inbox draws it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub(crate) struct AlertRow {
    /// `warn` or `bad` — charter's two accents above plain text. `bad` is the one that loses
    /// work if it is left: a nested plane, a memory commit that reached no remote.
    severity: String,
    /// What it is about, in charter's word for it: `charter`, `front door`, `reinit`,
    /// `nested plane`, `plane root`.
    subject: String,
    /// What is wrong.
    detail: String,
    /// **The workspaces it is about, by name**, where it is about some (#3650): the `reinit`
    /// alert's, every workspace behind the current layout, so the title bar hangs it from each
    /// one's chip. Empty for an alert about the project as a whole.
    workspaces: Vec<String>,
    /// **What fixes it, in the window** (NO-6, #1238): the Inbox draws it as the Notice's button,
    /// in place of the command the terminal status line names.
    way: AlertWay,
}

/// **An alert's way out**, decided by the core's kind of alert ([`way_out`]) and never by its
/// words: the Inbox turns each into one button.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum AlertWay {
    /// A Settings group of the project the alert is about, by its address (SE-22) — and the
    /// setting in it that mends the alert, by the id the Settings builders give it
    /// (`settings/project.ts`), so the link lands on that control (#1289). `null` when the
    /// group as a whole is the way out.
    Settings {
        group: String,
        setting: Option<String>,
    },
    /// A fix of the doctor's registry, by the id `charter doctor --fix` takes (FX-1).
    Fix { id: String },
    /// Another project to open: the one whose `workspaces/` this one is nested in.
    OpenProject { path: String },
    /// The project's Saving view, where a save is resolved.
    Saving,
    /// **A save's red request** (`save-request-red`; the operator's rulings P1 and P2,
    /// 2026-10-13): the Inbox offers what the Saving view offers for it, Fix and Merge as it
    /// stands, so it is seen and answered without opening that view. `workspace` and `repo`
    /// name a workspace repo's request, and are both null for the project's own.
    SaveRequest {
        workspace: Option<String>,
        repo: Option<String>,
        /// Whose it is, as a sentence names it: `this project`, or the repo's name.
        name: String,
        /// `pull request` or `merge request`.
        request: String,
        /// The request's page.
        pr: String,
        /// The branch it goes into, where purlis can tell.
        target: Option<String>,
        /// Whether purlis saves it by itself. A repo's fix reaches its request by the repo's
        /// next save, so where this is off the Notice says a save is needed.
        autosave: bool,
        merge: Box<crate::saving::SaveMerge>,
    },
}

/// The way out of one of the core's alerts.
fn way_out(alert: &alerts::Alert) -> AlertWay {
    use purlis_core::doctor::fix::FixId;
    match alert {
        // The version lock and the default persona's picker are both in Project › General —
        // where the doctor's `version lock` and `front door` rows link too — and the link lands
        // on the control itself (#1289).
        alerts::Alert::PinBesideDev { .. } | alerts::Alert::PinDrift { .. } => {
            general_at(&["charter", "version"])
        }
        alerts::Alert::FrontDoor { .. } => general_at(&["persona", "default"]),
        alerts::Alert::Reinit { .. } => AlertWay::Fix {
            id: FixId::WorkspaceReinit.id().to_owned(),
        },
        alerts::Alert::NestedPlane { outer, .. } => AlertWay::OpenProject {
            path: outer.display().to_string(),
        },
        alerts::Alert::PlaneRoot { .. } => AlertWay::Saving,
    }
}

/// **The workspaces an alert is about** (#3650): the stale ones of `reinit`, none for the rest.
fn about(alert: &alerts::Alert) -> Vec<String> {
    match alert {
        alerts::Alert::Reinit { stale } => stale.clone(),
        _ => Vec::new(),
    }
}

/// Project › General, at the setting whose key in `charter.toml` is `key`: the id the Settings
/// builders give a Shared setting, its group's address then its dotted key (`driver.ts`,
/// `fileSetting`).
fn general_at(key: &[&str]) -> AlertWay {
    let group = purlis_core::doctor::SettingsGroup::General.id();
    AlertWay::Settings {
        group: group.to_owned(),
        setting: Some(format!("{group}.{}", key.join("."))),
    }
}

/// One open project's alerts.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub(crate) struct PlaneAlerts {
    plane: PlaneId,
    alerts: Vec<AlertRow>,
    /// Why charter stopped looking before the last alert, or null when it looked at every one.
    /// **The alerts above stand, and their number is not this project's number of alerts** —
    /// a count drawn from a reading that stopped would be smaller than the truth with nothing
    /// on it saying so.
    stopped: Option<String>,
}

/// One project's reading. The app flags no workspace's layout of its own, so every stale
/// workspace counts; and it stands in the plane it opened, so a plane nested inside another's
/// `workspaces/` is said to be.
pub(crate) fn of(plane: PlaneId, root: &Path) -> PlaneAlerts {
    let (alerts, stopped) = rows(root);
    PlaneAlerts {
        plane,
        alerts,
        stopped,
    }
}

/// The core's alert as the Inbox shows it beside the title bar's save indicator
/// (charter-app#332): a plane root's uncommitted and unpushed findings are what the indicator
/// already says, so the Inbox keeps only what it does not — a detached HEAD, a branch that is
/// not the default — and drops the row when nothing is left. The terminal status line, which has
/// no indicator, still gets the whole row from the core.
fn beside_the_indicator(alert: &alerts::Alert) -> Option<alerts::Alert> {
    match alert {
        alerts::Alert::PlaneRoot {
            name,
            detached,
            off,
            ..
        } => (*detached || off.is_some()).then(|| alerts::Alert::PlaneRoot {
            name: name.clone(),
            dirty: false,
            detached: *detached,
            off: off.clone(),
            memory: None,
        }),
        other => Some(other.clone()),
    }
}

/// How long a save may stay blocked before the Inbox says so (ADR 0051). A secret the scan
/// caught is said at once: that save never goes through without somebody.
const BLOCKED_FOR: f64 = 600.0;

/// The Inbox's alert for a plane whose save is blocked, once it has been for [`BLOCKED_FOR`] —
/// or at once for a secret. The app's alone, not the core's: the terminal status line reads the
/// core's alerts, and the Saving view is where this one is resolved.
fn save_blocked(root: &Path, now: f64) -> Option<AlertRow> {
    use purlis_core::planegit;
    let standing = planegit::shared_standing(root);
    // A secret is said at once: that save never goes through without somebody. And said with
    // the file and line it is in, so the Notice names what to mend.
    let secret = standing.blocked.as_deref()? == planegit::SECRET_REFUSED;
    let why = standing.blocked_said()?;
    // Otherwise from when the cause began, read from the cause itself: the push record for a
    // conflict or a stranded push, the journal line that recorded a branch mismatch. No cause
    // with a time, no alert — never a clock that restarts on every look.
    let since = if !standing.conflicts.is_empty() || planegit::unlanded(root).is_some() {
        planegit::push_record(root).and_then(|r| r["at"].as_f64())
    } else {
        planegit::journal(root)
            .last()
            .filter(|line| line["outcome"] == "blocked")
            .and_then(|line| line["at"].as_f64())
    };
    if !secret && since.is_none_or(|since| now - since < BLOCKED_FOR) {
        return None;
    }
    Some(AlertRow {
        severity: "bad".to_owned(),
        subject: "save".to_owned(),
        detail: format!("the plane's save is blocked: {why}"),
        workspaces: Vec::new(),
        way: AlertWay::Saving,
    })
}

/// What a red request's alert is about: its subject in the Inbox, and the kind the docs name
/// it by.
pub(crate) const SAVE_REQUEST_RED: &str = "save-request-red";

/// **The Inbox's alert for a save's request that is red** (the operator's ruling P1): there
/// from the look that found a failed check until the request merges or its checks pass,
/// because it is what the standing says and nothing kept beside it. A request that waits, is
/// queued, cannot merge or has merged is none: only red is trouble the person is told of here.
fn request_red(
    whose: (Option<&str>, Option<&str>),
    request: &str,
    pr: Option<&str>,
    target: Option<&str>,
    autosave: bool,
    merge: Option<&crate::saving::SaveMerge>,
) -> Option<AlertRow> {
    let (merge, pr) = (merge.filter(|merge| merge.state == "red")?, pr?);
    let (workspace, repo) = whose;
    let name = repo.unwrap_or("this project").to_owned();
    let numbered = match &merge.number {
        Some(number) => format!("{request} {number}"),
        None => request.to_owned(),
    };
    Some(AlertRow {
        severity: "bad".to_owned(),
        subject: SAVE_REQUEST_RED.to_owned(),
        detail: format!(
            "the {numbered} of {name} is red, so purlis did not merge it: {}",
            merge.why
        ),
        workspaces: Vec::new(),
        way: AlertWay::SaveRequest {
            workspace: workspace.map(str::to_owned),
            repo: repo.map(str::to_owned),
            name,
            request: request.to_owned(),
            pr: pr.to_owned(),
            target: target.map(str::to_owned),
            autosave,
            merge: Box::new(merge.clone()),
        },
    })
}

/// Every red request of the project at `root`: its own, then each workspace repo's. Read from
/// the shared standings, never the network; a project no table of which says `pr-merge` asks
/// about no repo at all.
fn requests_red(root: &Path) -> Vec<AlertRow> {
    use purlis_core::planesave::{Mode, Settings};
    let settings = Settings::read(root);
    let mut rows = Vec::new();
    if settings.plane.mode.value == Some(Mode::PrMerge) {
        let saving = crate::saving::saving_of(root);
        rows.extend(request_red(
            (None, None),
            &saving.request,
            saving.pr.as_deref(),
            Some(&saving.branch),
            purlis_core::autosave::on(&settings.plane),
            saving.merge.as_ref(),
        ));
    }
    let merging = settings
        .repo_tables()
        .into_iter()
        .any(|name| settings.repo(&name).mode.value == Mode::PrMerge);
    if !merging {
        return rows;
    }
    let Ok(workspaces) = purlis_core::workspaces::Plane::open(root).workspaces() else {
        return rows;
    };
    for workspace in workspaces {
        for repo in crate::saving::repos_saving(root, &workspace).unwrap_or_default() {
            rows.extend(request_red(
                (Some(&workspace), Some(&repo.name)),
                &repo.request,
                repo.pr.as_deref(),
                repo.target.as_deref(),
                repo.autosave,
                repo.merge.as_ref(),
            ));
        }
    }
    rows
}

/// Seconds since the epoch, as the journal writes them.
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// The rows, and why the reading stopped where it did.
fn rows(root: &Path) -> (Vec<AlertRow>, Option<String>) {
    let reading = alerts::read(&alerts::Asking {
        root,
        active: None,
        standing: root,
        // The app holds the plane's shared standing (FD-11): the root's dirt is read from it.
        shared: true,
    });
    (
        reading
            .alerts
            .iter()
            .filter_map(beside_the_indicator)
            .map(|alert| {
                let shown = alert.shown();
                AlertRow {
                    severity: match shown.severity {
                        alerts::Severity::Warn => "warn",
                        alerts::Severity::Bad => "bad",
                    }
                    .to_owned(),
                    subject: shown.subject,
                    detail: shown.detail,
                    workspaces: about(&alert),
                    way: way_out(&alert),
                }
            })
            .chain(save_blocked(root, now()))
            .chain(requests_red(root))
            .collect(),
        reading.stopped,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inbox_gets_charters_words_for_each_alert() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n\n[persona]\ndefault = \"ghost\"\n",
        )
        .unwrap();
        let (alerts, stopped) = rows(&root);
        assert_eq!(stopped, None);
        assert_eq!(alerts.len(), 1, "{alerts:?}");
        assert_eq!(alerts[0].severity, "warn");
        assert_eq!(alerts[0].subject, "front door");
        assert_eq!(alerts[0].detail, "ghost — no such persona");
        assert_eq!(
            alerts[0].way,
            AlertWay::Settings {
                group: "project.general".to_owned(),
                setting: Some("project.general.persona.default".to_owned()),
            }
        );
    }

    fn merge(state: &str) -> crate::saving::SaveMerge {
        crate::saving::SaveMerge {
            state: state.to_owned(),
            why: "1 check failed: lint".to_owned(),
            checks: vec![crate::saving::FailedCheck {
                name: "lint".to_owned(),
                url: None,
            }],
            needs: None,
            number: Some("#12".to_owned()),
            shown: "red - lint".to_owned(),
        }
    }

    #[test]
    fn a_red_request_is_an_inbox_alert_with_its_two_answers_until_it_merges_or_goes_green() {
        const PR: &str = "https://github.com/acme/api/pull/12";
        let red = merge("red");
        let row = request_red(
            (None, None),
            "pull request",
            Some(PR),
            Some("main"),
            true,
            Some(&red),
        )
        .expect("a red request is said");
        assert_eq!(
            (row.severity.as_str(), row.subject.as_str()),
            ("bad", "save-request-red")
        );
        assert_eq!(
            row.detail,
            "the pull request #12 of this project is red, so purlis did not merge it: 1 check \
             failed: lint"
        );
        assert_eq!(
            row.way,
            AlertWay::SaveRequest {
                workspace: None,
                repo: None,
                name: "this project".to_owned(),
                request: "pull request".to_owned(),
                pr: PR.to_owned(),
                target: Some("main".to_owned()),
                autosave: true,
                merge: Box::new(red.clone()),
            }
        );
        // A repo's names the repo, and where its answers go.
        let repos = request_red(
            (Some("alpha"), Some("api")),
            "merge request",
            Some(PR),
            None,
            false,
            Some(&red),
        )
        .expect("said");
        assert!(
            repos
                .detail
                .starts_with("the merge request #12 of api is red"),
            "{}",
            repos.detail
        );
        assert!(matches!(
            &repos.way,
            AlertWay::SaveRequest { workspace: Some(ws), repo: Some(repo), .. }
                if ws == "alpha" && repo == "api"
        ));
        // It goes when the request merges or its checks pass, and waiting is no trouble.
        for state in ["waiting", "queued", "cannot-merge", "merged"] {
            let not_red = merge(state);
            assert!(
                request_red(
                    (None, None),
                    "pull request",
                    Some(PR),
                    None,
                    true,
                    Some(&not_red)
                )
                .is_none(),
                "{state}"
            );
        }
        assert!(request_red((None, None), "pull request", Some(PR), None, true, None).is_none());
        assert!(request_red((None, None), "pull request", None, None, true, Some(&red)).is_none());
    }

    #[test]
    fn a_project_whose_saves_merge_nothing_is_asked_about_no_request() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("charter.toml"),
            "[plane]\nmode = \"commit\"\n",
        )
        .unwrap();
        assert!(requests_red(dir.path()).is_empty());
    }

    /// **Each row carries its way out** (NO-6, #1238): where the window mends it, by the core's
    /// own kind of alert rather than by its words.
    #[test]
    fn each_kind_of_alert_has_its_way_out() {
        // On the version lock's own control (#1289).
        let general = AlertWay::Settings {
            group: "project.general".to_owned(),
            setting: Some("project.general.charter.version".to_owned()),
        };
        let way = |alert: alerts::Alert| way_out(&alert);
        assert_eq!(
            way(alerts::Alert::PinBesideDev {
                pinned: "1.0.0".into()
            }),
            general
        );
        assert_eq!(
            way(alerts::Alert::PinDrift {
                running: "2.0.0".into(),
                pinned: "1.0.0".into()
            }),
            general
        );
        // The same fix id `purlis doctor --fix` takes, so the Inbox and the CLI do one thing.
        assert_eq!(
            way(alerts::Alert::Reinit {
                stale: vec!["ide".into()]
            }),
            AlertWay::Fix {
                id: "workspace-reinit".to_owned()
            }
        );
        assert_eq!(
            way(alerts::Alert::NestedPlane {
                inner: "/home/dev/outer/workspaces/inner".into(),
                outer: "/home/dev/outer".into(),
            }),
            AlertWay::OpenProject {
                path: "/home/dev/outer".to_owned()
            }
        );
        assert_eq!(
            way(alerts::Alert::PlaneRoot {
                name: "plane".into(),
                dirty: false,
                detached: true,
                off: None,
                memory: None,
            }),
            AlertWay::Saving
        );
    }

    #[test]
    fn a_layout_alert_names_the_workspaces_behind_it_and_no_other_alert_names_any() {
        // Each one's chip hangs the notice (#3650): by name, in the core's order.
        assert_eq!(
            about(&alerts::Alert::Reinit {
                stale: vec!["docs".into(), "ide".into()]
            }),
            vec!["docs".to_owned(), "ide".to_owned()]
        );
        assert!(
            about(&alerts::Alert::FrontDoor {
                declared: "ghost".into()
            })
            .is_empty()
        );
    }

    #[test]
    fn a_reading_that_stopped_says_why_and_keeps_what_it_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("charter.toml"), "schema = 1\n[[[ not toml\n").unwrap();
        let (alerts, stopped) = rows(&root);
        assert!(stopped.is_some(), "{alerts:?}");
        assert!(alerts.is_empty());
    }

    #[test]
    fn the_inbox_leaves_to_the_save_indicator_what_it_already_says_about_the_plane_root() {
        let dirty = alerts::Alert::PlaneRoot {
            name: "plane".into(),
            dirty: true,
            detached: false,
            off: None,
            memory: Some(alerts::Memory::NotPushed),
        };
        assert_eq!(beside_the_indicator(&dirty), None);

        let off = alerts::Alert::PlaneRoot {
            name: "plane".into(),
            dirty: true,
            detached: false,
            off: Some(("work".into(), "main".into())),
            memory: Some(alerts::Memory::NotPushed),
        };
        assert_eq!(
            beside_the_indicator(&off),
            Some(alerts::Alert::PlaneRoot {
                name: "plane".into(),
                dirty: false,
                detached: false,
                off: Some(("work".into(), "main".into())),
                memory: None,
            })
        );
    }

    fn blocked_plane(detail: &str, at: f64) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("charter.toml"), "[plane]\nmode = \"push\"\n").unwrap();
        for args in [
            &["init", "-q"][..],
            &["add", "-A"],
            &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "one"],
        ] {
            let mut command = std::process::Command::new("git");
            command
                .arg("-C")
                .arg(root)
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
            purlis_core::forklock::output(&mut command).expect("git runs");
        }
        let journal = purlis_core::planegit::journal_path(root);
        std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
        std::fs::write(
            journal,
            format!(
                "{}\n",
                serde_json::json!({"at": at, "outcome": "blocked", "detail": detail})
            ),
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_save_blocked_by_a_secret_is_said_at_once() {
        let dir = blocked_plane(purlis_core::planegit::SECRET_REFUSED, now());
        let memory = dir.path().join("personas/steward/memory");
        std::fs::create_dir_all(&memory).unwrap();
        std::fs::write(
            memory.join("m.md"),
            "token: ghp_0123456789abcdefghijklmnopqrstuvwxyz\n",
        )
        .unwrap();
        let row = save_blocked(dir.path(), now()).expect("said at once");
        assert_eq!(
            (row.severity.as_str(), row.subject.as_str()),
            ("bad", "save")
        );
        assert_eq!(row.way, AlertWay::Saving);
        assert!(row.detail.contains("secret-shaped"), "{row:?}");
        // And it names the file and the line, so the person knows what to mend.
        assert!(
            row.detail.ends_with("in personas/…/m.md, line 1"),
            "{row:?}"
        );
    }

    #[test]
    fn any_other_block_is_said_after_ten_minutes_and_not_before() {
        let at = now();
        // The plane on another branch than it saves into, recorded by the save that stopped.
        let dir = blocked_plane("this plane is on main, and [plane] branch is trunk", at);
        std::fs::write(
            dir.path().join("charter.toml"),
            "[plane]\nmode = \"push\"\nbranch = \"trunk\"\n",
        )
        .unwrap();
        assert!(save_blocked(dir.path(), at + 60.0).is_none());
        assert!(save_blocked(dir.path(), at + 601.0).is_some());
    }
}
