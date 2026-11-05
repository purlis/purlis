// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project says when it has finished opening** (#5322, the spec #5319).
//!
//! A project's chats come back behind the window (`crate::restoring`), and the window used to
//! have no word for when a whole project was back: only a log line, "plane X, N of N chats
//! back". The core now tells it, once per project put back, on [`EVENT`]: ready, with how many
//! of the chats its record held came back, or failed, with why nothing was put back. A chat
//! that did not start leaves its project ready, short of one (the ruling of 2026-11-05: "N of
//! M chats back", not failed).

use crate::planes::PlaneId;

/// The event the window is sent as each project it opened has finished opening.
pub const EVENT: &str = "project-ready";

/// What `project-ready` carries: a project, and how its opening came out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectReady {
    pub plane: PlaneId,
    pub outcome: ProjectOutcome,
}

/// How a project's opening came out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProjectOutcome {
    /// Every chat its record held has been tried, and `back` of the `wanted` came back. The
    /// ones that did not are listed as chats that would not start, as they always were.
    Ready { back: u32, wanted: u32 },
    /// Nothing was put back, and this is why: the record could not be read, or the launch's
    /// gate refused (#2004).
    Failed { error: String },
}

/// Told each project put back, as it finishes opening.
pub type Teller = std::sync::Arc<dyn Fn(ProjectReady) + Send + Sync + 'static>;
