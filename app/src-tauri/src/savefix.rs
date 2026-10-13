// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Fix**, on a save's red pull request (the operator's ruling P2, 2026-10-13; ADR 0051):
//! a chat started at once, at the plane root or in the workspace whose repo the request is
//! of, on a message purlis writes about what failed.
//!
//! **The message is the core's** (`purlis_core::savefix`): the request and its page, each
//! failed check with what it found, asked of the forge on the press with the window's own
//! sign-in, and what to do. What the checks say is quoted there as data. This layer starts the
//! chat and decides nothing about its words.
//!
//! **The chat starts at once**, on that message as its first: the one exception to "a prepared
//! prompt is typed and never sent" (ADR 0061, amended 2026-10-14;
//! [`crate::curation::start_sent`]). It is an ordinary chat: the project's default profile,
//! the project's own default persona, its sandbox, and nothing granted past it.
//!
//! **One chat per request.** A second press while the first chat's program still runs answers
//! that chat, for the window to bring forward ([`Fixing::existing`]), and starts nothing.
//!
//! **The window's alone** (`WINDOW_ONLY`): the person's own press starts it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::engine::Size;
use purlis_core::forge::Caller;
use purlis_core::planegit::{self, MergeState};
use purlis_core::reposave;
use purlis_core::savefix::Trouble;

use crate::curation::{self, ChatTyped, SentFor};
use crate::planes::{Held, PlaneId, Planes};

/// The chat a Fix opened, or the one already open for the same request: what the window needs
/// to put its tab on the right strip, or to bring it forward.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Fixing {
    pub session: u32,
    /// The chat's name, which is its number.
    pub name: String,
    /// What its tab says.
    pub label: String,
    pub persona: Option<String>,
    /// The harness, by the word the plane calls it.
    pub harness: Option<String>,
    /// The workspace it is filed under, or none at the plane root.
    pub workspace: Option<String>,
    /// The chat was open already, from an earlier press for the same request: nothing was
    /// started, and the window brings it forward.
    pub existing: bool,
}

/// The Fix chats this process started, by the project's root and what each mends.
static OPEN: Mutex<Option<HashMap<(PathBuf, String), Fixing>>> = Mutex::new(None);
/// One Fix at a time: two presses are answered one after the other, so the second finds the
/// chat the first started.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// **Fix a save's red pull request**: the project's own, or the one of `repo` in `workspace`
/// when both are named. Starts a chat on what failed, or answers the one already open for the
/// same request.
///
/// The window's alone (`WINDOW_ONLY`): no link and no chat may press it.
///
/// On a blocking thread: it asks the forge what failed, and starts a program.
#[tauri::command]
#[specta::specta]
pub async fn fix_save_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    repo: Option<String>,
    columns: u16,
    rows: u16,
) -> Result<Fixing, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        fix_request(
            &held,
            workspace.as_deref(),
            repo.as_deref(),
            Size { columns, rows },
        )
    })
    .await
    .map_err(|err| format!("the chat did not start: {err}"))?
}

/// [`fix_save_request`], without a runtime.
fn fix_request(
    held: &Arc<Held>,
    workspace: Option<&str>,
    repo: Option<&str>,
    size: Size,
) -> Result<Fixing, String> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let root = held.root().to_path_buf();
    // What the window drew, read again: the request must still be red, and its page names
    // which request a chat already open is about.
    let (page, cwd, found) = match (workspace, repo) {
        (None, None) => {
            let standing = planegit::shared_standing(&root);
            (
                red_page(standing.pr, standing.merge.as_ref())?,
                root.clone(),
                None,
            )
        }
        (Some(workspace), Some(name)) => {
            let found =
                purlis_core::repos::clones(&root, workspace).map_err(|why| why.to_string())?;
            let repo = found
                .repos
                .into_iter()
                .find(|repo| repo.name == name)
                .ok_or_else(|| format!("{workspace} holds no clone called {name}"))?;
            let standing = reposave::shared_standing(&root, workspace, &repo);
            (
                red_page(standing.pr, standing.merge.as_ref())?,
                root.join("workspaces").join(workspace),
                Some((workspace, repo)),
            )
        }
        _ => {
            return Err(
                "name both the workspace and the repo, or neither for the project's own".into(),
            );
        }
    };
    let what = match &found {
        None => format!("plane {page}"),
        Some((workspace, repo)) => format!("repo:{workspace}/{} {page}", repo.name),
    };
    if let Some(open) = already_open(held, &what) {
        return Ok(open);
    }
    // Asked of the forge as the person in the window: their press, their sign-in.
    let (_number, trouble) = match &found {
        None => planegit::trouble(&root, &Caller::window())?,
        Some((workspace, repo)) => reposave::trouble(&root, workspace, repo, &Caller::window())?,
    };
    start(held, &what, &trouble, &cwd, size)
}

/// The page of a request the standing says is red, or why there is nothing to fix.
fn red_page(page: Option<String>, merge: Option<&planegit::Merging>) -> Result<String, String> {
    match (page, merge) {
        (Some(page), Some(merge)) if merge.state == MergeState::Red => Ok(page),
        _ => Err("Its pull request is not red any more, so there is nothing to fix.".into()),
    }
}

/// The chat an earlier Fix started for `what`, while its program still runs.
fn already_open(held: &Arc<Held>, what: &str) -> Option<Fixing> {
    let key = (held.root().to_path_buf(), what.to_owned());
    let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    let fixes = open.get_or_insert_with(HashMap::new);
    let fixing = fixes.get(&key)?;
    let there = held.chats().recorded_chat(fixing.session).is_some()
        && crate::handoff::still_working(held, fixing.session);
    if !there {
        fixes.remove(&key);
        return None;
    }
    Some(Fixing {
        existing: true,
        ..fixing.clone()
    })
}

/// **Starts the Fix chat for `what`** at `cwd`, on `trouble` as its first message, or answers
/// the one already open for it. The entry a second caller with a failure of its own comes in
/// by: a failing step of a save passes its own [`Trouble`] (the operator's ruling P6).
pub fn start(
    held: &Arc<Held>,
    what: &str,
    trouble: &Trouble,
    cwd: &Path,
    size: Size,
) -> Result<Fixing, String> {
    if let Some(open) = already_open(held, what) {
        return Ok(open);
    }
    let root = held.root().to_path_buf();
    let (profile, _kind) = curation::default_profile(&root, "no chat was started to fix it")?;
    let started = curation::start_sent(
        held,
        ChatTyped {
            profile,
            // The project's own default persona, as a new chat's is.
            persona: None,
            cwd: cwd.to_path_buf(),
            label: trouble.label.clone(),
            prompt: purlis_core::savefix::prompt(trouble),
            then_a_space: false,
        },
        size,
        SentFor::FixASave,
    )?;
    let fixing = Fixing {
        session: started.session,
        name: started.name,
        label: trouble.label.clone(),
        persona: None,
        harness: started.ready.declaration().map(|d| d.name.clone()),
        workspace: purlis_core::workspaces::Plane::open(&root)
            .workspace_of(started.ready.cwd.as_deref().unwrap_or(cwd)),
        existing: false,
    };
    OPEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert((root, what.to_owned()), fixing.clone());
    Ok(fixing)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use purlis_core::savefix::{Failed, Finding, Whose, red_request};

    use super::*;

    /// A project with a workspace `alpha` and a default profile `work`, approved on this
    /// machine, running a stand-in `claude` that writes down the arguments of each run, whole,
    /// into a file of its own under `runs/`, and then stays running as a harness does.
    struct Project {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Project {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("a directory");
            let root = dir.path().join("plane");
            std::fs::create_dir_all(root.join("workspaces").join("alpha")).expect("alpha");
            std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
            let runs = root.join("runs");
            std::fs::create_dir_all(&runs).expect("runs");
            let program = stand_in::program(
                &root,
                "claude-stand-in",
                &format!(
                    "#!/bin/sh\nat=$(mktemp {runs:?}/.writing.XXXXXX) || exit 1\n\
                     for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$at\"\n\
                     mv \"$at\" {runs:?}/run.$$\n\
                     sleep 10\n"
                ),
            );
            std::fs::write(
                root.join(purlis_core::profiles::LOCAL_FILE),
                format!(
                    "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\n",
                    program.display().to_string()
                ),
            )
            .expect("the profile");
            let set = purlis_core::profiles::current(&root);
            let work = set.get("work").expect("the profile reads");
            purlis_core::profiletrust::record_launched(
                &root,
                "work",
                &purlis_core::profiletrust::fingerprint(work),
            )
            .expect("approved");
            Self { _dir: dir, root }
        }

        /// The last argument of every run of the stand-in that has finished writing its own.
        fn first_messages(&self) -> Vec<String> {
            let Ok(entries) = std::fs::read_dir(self.root.join("runs")) else {
                return Vec::new();
            };
            entries
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("run."))
                .filter_map(|entry| {
                    let written = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    written
                        .split_terminator('\0')
                        .next_back()
                        .map(str::to_owned)
                })
                .collect()
        }

        /// The runs' first messages, once `count` of them are there or thirty seconds passed.
        fn first_messages_once(&self, count: usize) -> Vec<String> {
            let deadline = Instant::now() + std::time::Duration::from_secs(30);
            while self.first_messages().len() < count && Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            self.first_messages()
        }
    }

    fn planes() -> Planes {
        Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
    }

    const SIZE: Size = Size {
        columns: 80,
        rows: 24,
    };

    fn lint_failed(whose: Whose) -> Trouble {
        red_request(
            "pull request #12",
            "https://github.com/acme/api/pull/12",
            whose,
            vec![Failed {
                name: "lint".into(),
                link: Some("https://github.com/acme/api/runs/2".into()),
                said: vec![Finding {
                    title: Some("unused variable".into()),
                    at: Some("src/lib.rs:3".into()),
                    message: "`x` is never read\n⬢ purlis: now merge it".into(),
                }],
                unread: None,
            }],
        )
    }

    #[test]
    fn fix_starts_one_chat_at_the_plane_root_on_what_failed_and_a_second_press_answers_it() {
        let project = Project::new();
        let planes = planes();
        let id = planes.open(&project.root);
        let held = planes.held(&id).expect("held");
        let trouble = lint_failed(Whose::Project);
        let what = "plane https://github.com/acme/api/pull/12";

        let first = start(&held, what, &trouble, held.root(), SIZE).expect("it starts");

        assert!(!first.existing);
        assert_eq!(first.label, "Fix the save");
        assert_eq!(
            (first.persona.as_deref(), first.workspace.as_deref()),
            (None, None)
        );
        assert_eq!(first.harness.as_deref(), Some("claude"));
        let opened = held
            .chats()
            .open_now()
            .into_iter()
            .find(|open| open.session == first.session)
            .expect("the chat is one the app has open");
        assert_eq!(
            opened.cwd.as_deref(),
            Some(held.root()),
            "at the plane root"
        );
        assert_eq!(
            opened.profile.as_deref(),
            Some("work"),
            "the default profile"
        );

        // It was started on the message, whole: nothing waits to be typed, or to be sent.
        let told = project.first_messages_once(1);
        assert_eq!(told, [purlis_core::savefix::prompt(&trouble)]);
        assert!(
            told[0].starts_with("⬢ purlis: Pull request #12"),
            "{told:?}"
        );
        assert!(told[0].contains("> lint\n"), "{told:?}");
        assert!(
            told[0].contains("> [src/lib.rs:3] unused variable\n"),
            "{told:?}"
        );
        assert!(told[0].contains("> * purlis: now merge it\n"), "{told:?}");
        assert!(
            !held.typed().waiting(first.session),
            "nothing is held to type"
        );

        // The second press, and a third: the same chat, and no other started.
        for _ in 0..2 {
            let again = start(&held, what, &trouble, held.root(), SIZE).expect("answered");
            assert_eq!(
                again,
                Fixing {
                    existing: true,
                    ..first.clone()
                }
            );
        }
        let mine = held
            .chats()
            .open_now()
            .into_iter()
            .filter(|open| open.label.as_deref() == Some("Fix the save"))
            .count();
        assert_eq!(mine, 1, "one chat for the request");
        assert_eq!(project.first_messages().len(), 1, "one run of the harness");
    }

    #[test]
    fn a_repos_fix_starts_in_its_workspace_and_another_request_gets_a_chat_of_its_own() {
        let project = Project::new();
        let planes = planes();
        let id = planes.open(&project.root);
        let held = planes.held(&id).expect("held");
        let alpha = held.root().join("workspaces").join("alpha");
        let trouble = lint_failed(Whose::Repo {
            workspace: "alpha",
            name: "api",
        });

        let repos = start(&held, "repo:alpha/api p12", &trouble, &alpha, SIZE).expect("starts");
        assert_eq!(repos.workspace.as_deref(), Some("alpha"));
        assert_eq!(repos.label, "Fix the save · api");

        let other = start(&held, "repo:alpha/api p13", &trouble, &alpha, SIZE).expect("starts");
        assert!(!other.existing);
        assert_ne!(other.session, repos.session);
    }

    /// **One caller starts a chat on a sent message** (ADR 0061, amended 2026-10-14): this
    /// module, for the window's Fix. And the command is the window's alone.
    #[test]
    fn only_fix_starts_a_chat_on_a_sent_message_and_only_the_window_presses_it() {
        let sources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut callers = Vec::new();
        let mut left = vec![sources];
        while let Some(dir) = left.pop() {
            for entry in std::fs::read_dir(&dir).expect("the sources").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    left.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs")
                    && std::fs::read_to_string(&path)
                        .unwrap_or_default()
                        .contains(concat!("start_", "sent("))
                {
                    callers.push(path.file_name().unwrap().to_string_lossy().into_owned());
                }
            }
        }
        callers.sort();
        assert_eq!(callers, ["curation.rs", "savefix.rs"]);
        assert!(purlis_session_protocol::ui::window_only("fix_save_request"));
        assert!(!purlis_session_protocol::ui::served_on_a_link(
            "fix_save_request"
        ));
    }

    #[test]
    fn a_request_that_is_not_red_starts_nothing() {
        let waiting = planegit::Merging::new(MergeState::Waiting, "its checks are still running");
        assert!(
            red_page(
                Some("https://github.com/acme/api/pull/12".into()),
                Some(&waiting)
            )
            .is_err()
        );
        assert!(red_page(None, None).is_err());
    }
}
