// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Regenerate, pressed in an artifact's bar** (ADR 0122 §6 and its amendment of 2026-11-09,
//! #5962): the pin's command run in the session host, never in this process.
//!
//! - **What it would run, and whether it asks** ([`artifact_regenerate_about`]): the code its
//!   command resolves to, that code's fingerprint, and where it stands with this person.
//! - **The person's approval** ([`artifact_regenerate_approve`]) is of the fingerprint they were
//!   shown: read again here, and refused when the code moved since, so a press never approves
//!   code nobody saw.
//! - **The run** ([`artifact_regenerate`]) reads the pin, resolves its command and asks the
//!   approval again right before it starts, so only the approved fingerprint ever runs. The
//!   window composes the line (the command wrapped in a sandbox with no network that writes
//!   only a scratch folder, [`purlis_core::artifact::regenerate::launch`]) and the session host
//!   starts it as a shell of the window's own. Only its exit status is waited for, at most
//!   [`regenerate::MOST_SECONDS`], after which it is stopped. A clean exit has its declared
//!   files placed whole ([`regenerate::place`]); anything else leaves the data as it was and
//!   says when it was last good, with the tail of the run's output, masked.
//!
//! Every command here is the window's alone (`WINDOW_ONLY`): no link serves it and no chat
//! runs it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use purlis_core::artifact::regenerate::{self, Resolved, Scratch, Standing};
use purlis_core::artifact::{self, data};
use purlis_session_protocol::session::Exit;
#[cfg(unix)]
use purlis_session_protocol::session::{Answer, Command, Launch, ProgramKind, StartProgram};
use serde::Serialize;

#[cfg(unix)]
use crate::hostedsessions::Ending;
use crate::planes::{PlaneId, Planes};

/// How long the window waits for the session host's link before it says it cannot run.
#[cfg(unix)]
const LINK_WITHIN: Duration = Duration::from_secs(10);

/// What an artifact's bar says of its Regenerate before anything runs.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactRegenerate {
    /// Whether the pin names a command at all; none draws no Regenerate.
    pub has_command: bool,
    /// Why it cannot run here, when it cannot: the sentence the bar shows.
    pub refused: Option<String>,
    /// `approved`, `new`, `changed` or `pin-changed`, when it resolved.
    pub standing: Option<String>,
    /// The folder of code a yes approves, as the prompt names it.
    pub code: Option<String>,
    /// That code's fingerprint, handed back with the yes.
    pub fingerprint: Option<String>,
    /// When the data in place was last good, UTC, as the bar shows it.
    pub last_good: Option<String>,
}

/// How a press of Regenerate ended.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactRegenerated {
    /// The data files written, when it succeeded.
    pub written: Vec<String>,
    /// What went wrong and that the data before it stands, with its date, when it failed.
    pub failed: Option<String>,
    /// The tail of the run's output, secrets masked, when it failed and said anything.
    pub output: Option<String>,
}

/// One pin's Regenerate, read from disk: the plane, the workspace, the pin, and its folder.
struct Read {
    root: PathBuf,
    workspace: PathBuf,
    pin: artifact::Pin,
    folder: PathBuf,
}

fn read(planes: &Planes, plane: &PlaneId, workspace: &str, folder: &str) -> Result<Read, String> {
    let root = planes.held(plane)?.root().to_path_buf();
    let at = purlis_core::wscmd::workspace_dir(&root, workspace)
        .ok_or_else(|| format!("'{workspace}' is not a workspace purlis knows"))?;
    let pin = artifact::pinned(&at, folder)?;
    let real = artifact::resolve(&at, folder)?;
    Ok(Read {
        root,
        workspace: at,
        pin,
        folder: real,
    })
}

/// What the page of artifact folder `folder` declares it reads; nothing when it declares none.
fn declared(folder: &Path) -> Result<data::Declared, String> {
    let page = purlis_core::contain::read_text_no_link(folder, &folder.join(artifact::PAGE))
        .map_err(|_| format!("the artifact has no readable {}", artifact::PAGE))?;
    match data::declared(&page) {
        Ok(Some(declared)) if !declared.data.is_empty() => Ok(declared),
        Ok(_) => Err("its page declares no data files, so Regenerate has nothing to write".into()),
        Err(why) => Err(format!("its page's declaration is refused: {why}")),
    }
}

fn said_at(at: chrono::DateTime<chrono::Utc>) -> String {
    at.format("%Y-%m-%d %H:%M UTC").to_string()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("purlis could not regenerate the artifact: {e}"))?
}

/// **What Regenerate would run for artifact `folder`, and whether a press asks first.**
#[tauri::command]
#[specta::specta]
pub async fn artifact_regenerate_about(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<ArtifactRegenerate, String> {
    let read = read(&planes, &plane, &workspace, &folder)?;
    let config = purlis_core::machine::config_root();
    blocking(move || {
        let has_command = read.pin.command.is_some();
        let last_good = declared(&read.folder)
            .ok()
            .and_then(|d| regenerate::last_good(&read.folder, &d))
            .map(said_at);
        let mut about = ArtifactRegenerate {
            has_command,
            refused: None,
            standing: None,
            code: None,
            fingerprint: None,
            last_good,
        };
        if !has_command {
            return Ok(about);
        }
        match regenerate::resolve(&read.root, &read.workspace, &read.pin) {
            Err(why) => about.refused = Some(why),
            Ok(resolved) => {
                let standing = regenerate::standing(
                    config.as_deref(),
                    &read.workspace,
                    &read.pin.folder,
                    &read.pin.id,
                    &resolved,
                );
                about.standing = Some(standing.as_str().to_owned());
                about.code = Some(resolved.code.display().to_string());
                about.fingerprint = Some(resolved.fingerprint);
            }
        }
        Ok(about)
    })
    .await
}

/// **The person's yes to the code Regenerate runs**, pressed in the bar's prompt: of exactly
/// `fingerprint`, the one they were shown. The code is resolved again here, and a yes to code
/// that moved since is refused, so it is shown again.
#[tauri::command]
#[specta::specta]
pub async fn artifact_regenerate_approve(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
    fingerprint: String,
) -> Result<(), String> {
    let read = read(&planes, &plane, &workspace, &folder)?;
    let config = crate::extensions::config_root()
        .map_err(|_| "this machine has no config home, so purlis keeps no approvals".to_owned())?;
    blocking(move || {
        let resolved = regenerate::resolve(&read.root, &read.workspace, &read.pin)?;
        approve_shown(&config, &read, &resolved, &fingerprint)
    })
    .await
}

/// Records the approval of `resolved` only when it is the code that was shown.
fn approve_shown(
    config: &Path,
    read: &Read,
    resolved: &Resolved,
    shown: &str,
) -> Result<(), String> {
    if resolved.fingerprint != shown {
        return Err(
            "The code Regenerate runs changed since it was shown, so it was not approved. Read \
             it again and approve what it is now"
                .into(),
        );
    }
    regenerate::approve(
        config,
        &read.workspace,
        &read.pin.folder,
        &read.pin.id,
        resolved,
    )
}

/// **Runs artifact `folder`'s command in the session host, and puts its data in place.**
#[tauri::command]
#[specta::specta]
pub async fn artifact_regenerate(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<ArtifactRegenerated, String> {
    #[cfg(not(unix))]
    {
        let _ = (app, planes, plane, workspace, folder);
        Err(regenerate::NOT_HERE.to_owned())
    }
    #[cfg(unix)]
    {
        let read = read(&planes, &plane, &workspace, &folder)?;
        run_pressed(&app, read).await
    }
}

/// The run behind a press, once its pin is read.
#[cfg(unix)]
async fn run_pressed(app: &tauri::AppHandle, read: Read) -> Result<ArtifactRegenerated, String> {
    use tauri::Manager as _;
    let Some(host) = app.try_state::<crate::sessionhost::SessionHost>() else {
        return Err("No purlis session host is running here, so nothing was run".into());
    };
    let connection = host.connection();
    let config = purlis_core::machine::config_root();
    blocking(move || {
        let declared = declared(&read.folder)?;
        // Asked again right before it starts: only the approved fingerprint ever runs.
        let resolved = regenerate::resolve(&read.root, &read.workspace, &read.pin)?;
        let standing = regenerate::standing(
            config.as_deref(),
            &read.workspace,
            &read.pin.folder,
            &read.pin.id,
            &resolved,
        );
        if standing != Standing::Approved {
            return Err(not_approved(standing));
        }
        let held = tempfile::Builder::new()
            .prefix("purlis-regenerate-")
            .tempdir()
            .map_err(|e| format!("purlis could not make the run's folder ({e})"))?;
        let scratch = Scratch::made(held.path())?;
        let launch = regenerate::launch(&read.root, &resolved, &scratch)?;
        let link = connection.of_project(&read.root, LINK_WITHIN)?;
        let ran = run_in_the_host(&link, &read.root, launch);
        let done = finished(&read.folder, &declared, &scratch, ran);
        if done.failed.is_none() {
            // Kept for History (#5966), on this machine only.
            crate::artifacts::history::keep_after_regenerate(
                config.as_deref(),
                &read.workspace,
                &read.pin.folder,
                &read.folder,
                &declared,
            );
        }
        Ok(done)
    })
    .await
}

fn not_approved(standing: Standing) -> String {
    match standing {
        Standing::Changed => {
            "The code Regenerate runs changed since you approved it, so nothing was run. Approve \
             it again to run it"
        }
        Standing::PinChanged => {
            "This pin's command changed since you approved it, so nothing was run. Approve it \
             again to run it"
        }
        _ => "Regenerate runs only once you approve the code it runs, so nothing was run",
    }
    .to_owned()
}

/// How a run ended, as far as the window asks: its exit, or why there is none.
#[derive(Debug, PartialEq, Eq)]
enum Ran {
    Exited(Exit),
    TimedOut,
    NotRun(String),
}

/// Starts `launch` in the session host as a shell of the window's own and waits for its end.
#[cfg(unix)]
fn run_in_the_host(
    link: &std::sync::Arc<crate::hostedsessions::HostLink>,
    root: &Path,
    launch: regenerate::Launch,
) -> Ran {
    let project = match purlis_core::scaffold::planefile::id_to_hold_by(root) {
        Ok(project) => project,
        Err(why) => return Ran::NotRun(format!("this plane has no id the host knows: {why}")),
    };
    if let Err(why) = link.call(Command::HoldProject {
        project: project.clone(),
        project_path: root.display().to_string(),
    }) {
        return Ran::NotRun(why);
    }
    let start = Command::StartProgram(Box::new(StartProgram {
        launch: Some(Launch {
            program: launch.program,
            args: launch.args,
            cwd: Some(launch.cwd.display().to_string()),
            set: launch.set,
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: None,
            confinement: None,
            prompt_key: None,
        }),
        ends_with_its_window: true,
        ..StartProgram::of(project, ProgramKind::Shell)
    }));
    let program = match link.call(start) {
        Ok(Answer::ProgramStarted { program, .. }) => program,
        Ok(other) => {
            return Ran::NotRun(format!(
                "the session host answered the start with {other:?}"
            ));
        }
        Err(why) => return Ran::NotRun(why),
    };
    let (tell, told) = std::sync::mpsc::channel();
    link.when_it_ends(
        &program,
        Box::new(move |ending| {
            let _ = tell.send(ending);
        }),
    );
    match told.recv_timeout(Duration::from_secs(regenerate::MOST_SECONDS)) {
        Ok(Ending::Exited(exit)) => Ran::Exited(exit),
        Ok(Ending::WithTheHost) => Ran::NotRun("the session host ended during the run".into()),
        Err(_) => {
            let _ = link.call(Command::Stop { chat: program });
            Ran::TimedOut
        }
    }
}

/// What a run that ended `ran` comes to: its data placed, or the data as it was and why.
fn finished(
    folder: &Path,
    declared: &data::Declared,
    scratch: &Scratch,
    ran: Ran,
) -> ArtifactRegenerated {
    let why = match ran {
        Ran::Exited(Exit::Code(0)) => match regenerate::place(folder, declared, &scratch.out) {
            Ok(placed) => {
                return ArtifactRegenerated {
                    written: placed.written,
                    failed: None,
                    output: None,
                };
            }
            Err(why) => why,
        },
        Ran::Exited(Exit::Code(code)) => format!("its command exited with code {code}"),
        Ran::Exited(Exit::Signal(signal)) => format!("its command was ended by {signal}"),
        Ran::TimedOut => format!(
            "its command ran past {} seconds and was stopped",
            regenerate::MOST_SECONDS
        ),
        Ran::NotRun(why) => why,
    };
    let output = std::fs::read(&scratch.log)
        .ok()
        .map(|bytes| regenerate::shown_errors(&bytes))
        .filter(|text| !text.trim().is_empty());
    ArtifactRegenerated {
        written: Vec::new(),
        failed: Some(regenerate::failed_said(
            &why,
            regenerate::last_good(folder, declared),
        )),
        output,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn here() -> (tempfile::TempDir, PathBuf, Scratch, data::Declared) {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("board");
        std::fs::create_dir_all(&folder).unwrap();
        let scratch_at = dir.path().join("scratch");
        std::fs::create_dir_all(&scratch_at).unwrap();
        let scratch = Scratch::made(&scratch_at).unwrap();
        let declared = data::Declared {
            api: 1,
            data: vec!["data.json".into()],
            queries: Vec::new(),
        };
        (dir, folder, scratch, declared)
    }

    #[test]
    fn a_clean_exit_places_the_declared_data() {
        let (_dir, folder, scratch, declared) = here();
        std::fs::write(scratch.out.join("data.json"), "[1]").unwrap();
        let done = finished(&folder, &declared, &scratch, Ran::Exited(Exit::Code(0)));
        assert_eq!(done.written, ["data.json"]);
        assert!(done.failed.is_none());
        assert_eq!(
            std::fs::read_to_string(folder.join("data.json")).unwrap(),
            "[1]"
        );
    }

    #[test]
    fn a_failed_or_stopped_run_keeps_the_last_good_data_and_shows_its_output_masked() {
        let token = format!("ghp_{}", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8");
        for ran in [
            Ran::Exited(Exit::Code(2)),
            Ran::Exited(Exit::Signal("SIGKILL".into())),
            Ran::TimedOut,
        ] {
            let (_dir, folder, scratch, declared) = here();
            std::fs::write(folder.join("data.json"), "\"good\"").unwrap();
            std::fs::write(scratch.out.join("data.json"), "\"half\"").unwrap();
            std::fs::write(&scratch.log, format!("boom {token}\n")).unwrap();
            let done = finished(&folder, &declared, &scratch, ran);
            let failed = done.failed.unwrap();
            assert!(failed.contains("last good: "), "{failed}");
            let output = done.output.unwrap();
            assert!(output.contains("boom"), "{output}");
            assert!(!output.contains(&token), "{output}");
            assert_eq!(
                std::fs::read_to_string(folder.join("data.json")).unwrap(),
                "\"good\""
            );
        }
    }

    #[test]
    fn a_run_that_never_started_says_why() {
        let (_dir, folder, scratch, declared) = here();
        let done = finished(&folder, &declared, &scratch, Ran::NotRun("no host".into()));
        let failed = done.failed.unwrap();
        assert!(
            failed.starts_with("Regenerate failed: no host."),
            "{failed}"
        );
        assert!(done.output.is_none());
    }

    #[test]
    fn a_yes_to_code_that_moved_since_it_was_shown_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let workspace = base.join("plane/workspaces/alpha");
        std::fs::create_dir_all(workspace.join("artifacts/board")).unwrap();
        std::fs::create_dir_all(workspace.join("tools")).unwrap();
        std::fs::write(workspace.join("tools/make.sh"), "echo").unwrap();
        let pin = artifact::Pin::new(
            "board".into(),
            "Board".into(),
            "chart".into(),
            1,
            Some("/bin/sh tools/make.sh".into()),
            None,
        );
        let read = Read {
            root: base.join("plane"),
            workspace: workspace.clone(),
            folder: workspace.join("artifacts/board"),
            pin,
        };
        let shown = regenerate::resolve(&read.root, &read.workspace, &read.pin).unwrap();
        std::fs::write(workspace.join("tools/make.sh"), "echo; steal").unwrap();
        let now = regenerate::resolve(&read.root, &read.workspace, &read.pin).unwrap();
        let config = base.join("config");
        let why = approve_shown(&config, &read, &now, &shown.fingerprint).unwrap_err();
        assert!(why.contains("changed since it was shown"), "{why}");
        approve_shown(&config, &read, &now, &now.fingerprint).unwrap();
        assert_eq!(
            regenerate::standing(Some(&config), &workspace, "board", &read.pin.id, &now),
            Standing::Approved
        );
    }

    #[test]
    fn a_press_without_approval_runs_nothing_and_says_why() {
        assert!(not_approved(Standing::New).contains("nothing was run"));
        assert!(not_approved(Standing::Changed).contains("changed since you approved"));
        assert!(not_approved(Standing::PinChanged).contains("command changed"));
    }
}
