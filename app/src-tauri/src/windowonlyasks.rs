// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A refusal a chat's own `purlis` handed the app to raise as an ask** (A3, R-328): the app's
//! answer to [`Ask::WindowOnly`], beside [`crate::removalasks`]'s.
//!
//! The chat's `forge_milestones` tool runs in the `purlis mcp` process, which holds no window,
//! so it cannot raise the ask itself ([`purlis_core::windowonly::raise`] is in-memory, by
//! project). It hands the refusal here. The app raises it, by its own record of the chat's
//! workspace and the clone's own origin, and answers with the sentence the chat is told: the
//! person was asked, or nobody was, in which case the block is written to the chat's Chat log
//! by the writer this process registered ([`purlis_core::windowonly::blocking`]). **The app
//! records nothing for it:** only the person's press of Record this repo does.

use purlis_core::hookwire::{Answer, WindowOnlyAsk, WindowOnlyRefusal};

use crate::planes::Held;

/// What the app answers chat `ask.chat`'s refusal: [`Answer::No`] with the sentence the chat is
/// told. Held to the rate a chat's brokered writes are, so it cannot fill the person's Inbox.
pub fn asked(held: &Held, ask: &WindowOnlyAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    // The workspace is the app's record of the chat, never a word of the line's.
    let Some(workspace) = held.chats().workspace_of(ask.chat) else {
        return no(format!("chat {} is not one this app has open", ask.chat));
    };
    let Some(workspace) = workspace else {
        return no("A chat at the project's root has no workspace of repos to record.".to_owned());
    };
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let WindowOnlyRefusal::RecordRepo { clone } = &ask.refused;
    match purlis_core::windowonly::ask_to_record(held.root(), ask.chat, &workspace, clone) {
        Ok(said) | Err(said) => no(said),
    }
}

/// **A removal's row, as the window sent it back** (T1.1-67): what the row showed, so that the
/// press removes exactly that and nothing else. `what` is `persona` or `branch-folder`.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct RemovalRow {
    pub workspace: String,
    pub what: String,
    pub name: String,
    pub piece: String,
    pub force: bool,
    pub ignored: bool,
    pub listed: Vec<String>,
    /// Whether `listed` was everything that goes. A row from a window that does not say reads as
    /// not exact, so it offers Keep only.
    #[serde(default)]
    pub exact: bool,
    /// The digest of everything that goes, whole, as the row was raised with it.
    #[serde(default)]
    pub digest: String,
}

impl RemovalRow {
    /// The control the row was raised as, with the exactness and the digest it was raised with:
    /// a row is pressed only where its listing was exact, and Keep clears it by the same control.
    fn control(self) -> Result<purlis_core::windowonly::Control, String> {
        use purlis_core::hookwire::Removing;
        let removing = match self.what.as_str() {
            "persona" => Removing::Persona { name: self.name },
            "branch-folder" => Removing::BranchFolder {
                repo: self.name,
                piece: self.piece,
                force: self.force,
                ignored: self.ignored,
            },
            other => return Err(format!("'{other}' is not something a chat asks to remove")),
        };
        Ok(purlis_core::windowonly::Control::Remove {
            removing,
            workspace: self.workspace,
            listed: self.listed,
            exact: self.exact,
            digest: self.digest,
        })
    }
}

/// **The person's press on a removal ask** (T1.1-67, Q304 Q2): removes the persona or branch
/// folder the row listed, as their own `purlis persona remove` or `purlis worktree remove`
/// would, and only while it still holds exactly what the row listed. Anything that differs (a new
/// change, a new commit, another ignored file) refreshes the row and removes nothing. Answers what
/// the removal said, or why nothing was removed.
///
/// Window only (`WINDOW_ONLY`): a removal is for good.
#[tauri::command]
#[specta::specta]
pub async fn remove_listed(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    row: RemovalRow,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    // The window's own claim on a folder is the person's, named by this machine's device.
    let who = purlis_core::pieces::Who::here(held.config(), None, None);
    tauri::async_runtime::spawn_blocking(move || {
        purlis_core::forge::route::HostScope::claim()
            .ok_or_else(|| "A removal is pressed in the window alone.".to_owned())?;
        purlis_core::windowonly::remove_listed(&root, &row.control()?, &who)
    })
    .await
    .map_err(|err| format!("the removal did not finish: {err}"))?
}

/// **The person kept it** (Keep on a removal ask): every chat's ask for it goes, and nothing is
/// removed.
///
/// Window only (`WINDOW_ONLY`), as the press it answers.
#[tauri::command]
#[specta::specta]
pub async fn keep_removal(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    row: RemovalRow,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        purlis_core::forge::route::HostScope::claim()
            .ok_or_else(|| "Keeping a removal is answered in the window alone.".to_owned())?;
        purlis_core::windowonly::keep_listed(&root, &row.control()?);
        Ok(())
    })
    .await
    .map_err(|err| format!("keeping it did not finish: {err}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::hookwire::Removing;
    use purlis_core::windowonly::{self, Asking, Control};

    fn git(dir: &std::path::Path, args: &[&str]) {
        let mut command = std::process::Command::new("git");
        command
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args);
        let ran = purlis_core::forklock::output(&mut command).expect("git runs");
        assert!(ran.status.success(), "git {args:?}");
    }

    /// The row the window sends back for what the app showed: read from the `Shown` the app
    /// draws for the refusal, as the window does.
    fn row_of(shown: &crate::asking::Shown) -> RemovalRow {
        let crate::asking::AnswerPath::Removal {
            workspace,
            what,
            name,
            piece,
            force,
            ignored,
            listed,
            exact,
            digest,
        } = &shown.answer
        else {
            panic!("not a removal: {:?}", shown.answer);
        };
        RemovalRow {
            workspace: workspace.clone(),
            what: what.clone(),
            name: name.clone(),
            piece: piece.clone(),
            force: *force,
            ignored: *ignored,
            listed: listed.clone(),
            exact: *exact,
            digest: digest.clone(),
        }
    }

    /// **Keep clears a Keep-only row** (T1.1-67 round 2): a listing the code could not give whole
    /// (an uncommitted file git quotes with an escape) is raised for real, drawn as the app's row,
    /// sent back as the window sends it, and Keep clears it. A control rebuilt `exact: true` never
    /// matched the raised one, so Keep did nothing.
    #[test]
    fn keep_clears_a_keep_only_row_the_code_listed() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n[policy]\ntemplate = \"strict\"\n",
        )
        .unwrap();
        let clone = root.join("workspaces/alpha/thing");
        std::fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q", "-b", "main", "."]);
        std::fs::write(clone.join("README.md"), "one\n").unwrap();
        git(&clone, &["add", "README.md"]);
        git(&clone, &["commit", "-q", "-m", "one"]);
        let piece = purlis_core::worktree::add(&root, "alpha", "thing", "quoted", None)
            .expect("a piece is cut")
            .path;
        std::fs::write(piece.join("caf\u{e9}.txt"), "x\n").unwrap();

        let loss = purlis_core::removalask::folder_loss(&root, "alpha", "thing", "quoted").unwrap();
        assert!(!loss.listing.exact, "{:?}", loss.listing);
        let who = purlis_core::pieces::Who::here(None, Some("3".to_owned()), None);
        let asking = Asking {
            session: 3,
            who: &who,
            live: Some(&[]),
            claimed: false,
        };
        let removing = Removing::BranchFolder {
            repo: "thing".to_owned(),
            piece: "quoted".to_owned(),
            force: false,
            ignored: false,
        };
        windowonly::telling(&root, std::sync::Arc::new(|| {}));
        windowonly::ask_to_remove(&root, &asking, Some("alpha"), &removing);
        let [raised] = &windowonly::every(&root)[..] else {
            panic!("one ask: {:?}", windowonly::every(&root));
        };
        let shown = crate::asking::window_only(raised);
        // Keep only, in the app's row.
        let ids: Vec<_> = shown.options.iter().map(|one| one.id.as_str()).collect();
        assert_eq!(ids, ["keep"]);

        let control = row_of(&shown).control().unwrap();

        assert_eq!(control, raised.control, "the row names the raised control");
        let Control::Remove { exact, .. } = &control else {
            panic!("not a removal");
        };
        assert!(!*exact);
        windowonly::keep_listed(&root, &control);
        assert_eq!(windowonly::every(&root), []);
        assert!(piece.is_dir(), "Keep removes nothing");
        windowonly::project_ended(&root);
    }

    #[test]
    fn a_row_from_a_window_that_does_not_say_is_keep_only() {
        // A window older than `exact` and `digest` sends neither: it reads as not exact, so a
        // press removes nothing.
        let row: RemovalRow = serde_json::from_str(
            r#"{"workspace":"","what":"persona","name":"steward","piece":"",
                "force":false,"ignored":false,"listed":["personas/steward/persona.md"]}"#,
        )
        .unwrap();
        let control = row.control().unwrap();
        assert!(!control.pressable(), "{control:?}");
        let dir = tempfile::tempdir().unwrap();
        let why = windowonly::remove_listed(
            dir.path(),
            &control,
            &purlis_core::pieces::Who::here(None, None, None),
        )
        .unwrap_err();
        assert!(why.contains("could not list everything"), "{why}");
    }
}
