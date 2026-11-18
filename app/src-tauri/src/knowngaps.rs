// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Known gaps, for the window** (M75 slice 37, #7046; spec §14.8): what `purlis_core::knowngaps`
//! declares for each harness on this system under the modes the open project runs at (Settings'
//! sandbox section), and the same entries by the ids a chat's start event recorded (its pane
//! head). The window only draws them: nothing here decides a start.

use purlis_core::harness::Harness;
use purlis_core::knowngaps::{Declared, Entry, GAPS};
use purlis_core::sandbox::Os;

use crate::planes::PlaneId;

/// One known gap, as the window says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct KnownGapShown {
    /// Its stable id.
    pub id: String,
    /// What it is, as a sentence.
    pub said: String,
    /// What purlis does there.
    pub instead: String,
    /// Where it comes from (`#6972 §4 gap 3`, `spec §11.5`).
    pub from: String,
    /// The families it affects whose mode is not `off`, by their `[policy]` keys.
    pub families: Vec<String>,
    /// Of those, the ones that are enforced: what `context.gaps` carries.
    pub enforced: Vec<String>,
    /// Whether it holds only for some starts of the harness (an older Claude Code, a lifted
    /// sandbox).
    pub conditional: bool,
}

/// The gaps of one harness.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HarnessGaps {
    /// The harness, by the word the plane calls it (`claude`, `codex`, `opencode`).
    pub harness: String,
    /// Its gaps, in the table's order.
    pub gaps: Vec<KnownGapShown>,
}

fn keys(families: &[purlis_core::enforcement::Family]) -> Vec<String> {
    families
        .iter()
        .map(|family| family.key().to_owned())
        .collect()
}

fn shown(entry: &Entry) -> KnownGapShown {
    KnownGapShown {
        id: entry.gap.id.to_owned(),
        said: entry.gap.said.to_owned(),
        instead: entry.gap.instead.to_owned(),
        from: entry.gap.from.to_owned(),
        families: keys(&entry.in_force),
        enforced: keys(&entry.enforced),
        conditional: entry.gap.conditional(),
    }
}

/// **The known gaps the open project declares**, per harness, on this system.
#[tauri::command]
#[specta::specta]
pub(crate) async fn known_gaps(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
) -> Result<Vec<HarnessGaps>, String> {
    planes.held(&plane)?;
    let root = plane.root().to_path_buf();
    crate::off_the_window("reading the known gaps", move || {
        let floor = purlis_core::enforcement::at_start(&root);
        Ok(Harness::ALL
            .into_iter()
            .map(|harness| HarnessGaps {
                harness: harness.name().to_owned(),
                gaps: Declared::on(harness, Os::this(), &floor)
                    .entries()
                    .iter()
                    .map(shown)
                    .collect(),
            })
            .collect())
    })
    .await
}

/// **The known gaps chat `session` started with**, as its start event recorded them
/// (`run.started`'s protection), each as the table says it now. An id this build does not know is
/// left out, never guessed. Not the Chat log's command: that one is read only when its tab opens
/// (R-225), and this is read for the pane's head.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_known_gaps(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Vec<KnownGapShown>, String> {
    let held = planes.held(&plane)?;
    let chat = held.chats().logged_id_of(session);
    let Some(Ok(dir)) = crate::EVENT_LOG
        .get()
        .map(|log| log.as_ref().map(Clone::clone))
    else {
        return Ok(Vec::new());
    };
    crate::off_the_window("reading a chat's known gaps", move || {
        let events = purlis_core::eventlog::read(&dir).map_err(|why| why.to_string())?;
        let log = crate::chatlog::chat_log_with(&events, chat.as_deref(), &|_| None);
        let ids: Vec<String> = log
            .rows
            .into_iter()
            .find_map(|row| match row {
                crate::chatlog::ChatLogRow::Started { protection, .. } => protection,
                _ => None,
            })
            .map(|protection| protection.known_gaps)
            .unwrap_or_default();
        Ok(GAPS
            .iter()
            .filter(|gap| ids.iter().any(|id| id == gap.id))
            .map(|gap| KnownGapShown {
                id: gap.id.to_owned(),
                said: gap.said.to_owned(),
                instead: gap.instead.to_owned(),
                from: gap.from.to_owned(),
                families: keys(gap.families),
                enforced: Vec::new(),
                conditional: gap.conditional(),
            })
            .collect())
    })
    .await
}
