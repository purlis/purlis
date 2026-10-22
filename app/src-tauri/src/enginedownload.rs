// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The browser engine's first download, shown as it goes** (#2207, session protocol 1.37).
//!
//! The first browser on a machine downloads the pinned engine (about 150 MB) before it starts.
//! The session host pushes how far the download has come, a few times a second at most
//! (`engine_download`), and this keeps the latest word of each download until its start answers,
//! and tells the window ([`ENGINE_DOWNLOAD`]), which shows each in a Notice with Cancel
//! ([`cancel_engine_download`]). A download the person cancels keeps nothing, and its start is
//! refused in words that say so; one that fails is refused in words that say what to do. Both
//! are the core's (`purlis_core::browsers::download_failed_said`).
//!
//! The Notice stands wherever the person is, not on a browser's tab: a chat's browser has no tab
//! of its own while it starts, and the person's gets one only once it has started.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock, PoisonError};

/// The event that says the downloads changed: the window reads [`engine_downloads`] again.
pub const ENGINE_DOWNLOAD: &str = "engine-download";

/// One download, as its Notice shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct EngineDownloadShown {
    /// The browser whose start it is for: what Cancel names.
    pub program: String,
    /// The sentence.
    pub says: String,
}

/// What a chat is told where the person cancelled the engine's download its browser waited on.
pub const CANCELLED_FOR_THE_CHAT: &str = "The person cancelled the browser engine's download, so \
     no browser started. Don't retry; go on with what needs no browser, and tell the person if the \
     task needs one.";

/// What the person is told where they cancelled the engine's download a browser waited on.
pub const CANCELLED_FOR_THE_PERSON: &str =
    "You cancelled the browser engine's download. Open the browser again to download it.";

/// What a download that has come to `done` bytes, of `total` where known, says: in megabytes of
/// a million bytes, rounded down.
pub fn says(done: u64, total: Option<u64>) -> String {
    let mb = |bytes: u64| bytes / 1_000_000;
    match total {
        Some(total) => format!(
            "Downloading the browser engine… {} of {} MB.",
            mb(done),
            mb(total)
        ),
        None => format!("Downloading the browser engine… {} MB so far.", mb(done)),
    }
}

/// The last word of a download: how far it has come, and of how much where that is known.
type Word = (u64, Option<u64>);

/// The downloads under way, by program, and what tells the window of each change. Only a start
/// this window made and has not had answered yet is shown: a word for any other program (another
/// window's start, or one heard after its start answered) is not.
#[derive(Default)]
pub struct Downloads {
    /// Each start this window waits on, with the last word of its download once one came.
    under_way: Mutex<BTreeMap<String, Option<Word>>>,
    tell: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Downloads {
    /// Downloads that tell `tell` of each change.
    pub fn telling(tell: Box<dyn Fn() + Send + Sync>) -> Self {
        Downloads {
            under_way: Mutex::default(),
            tell: Some(tell),
        }
    }

    fn told(&self) {
        if let Some(tell) = &self.tell {
            tell();
        }
    }

    /// This window is starting `program`: a download the host tells of for it is shown.
    pub fn started(&self, program: &str) {
        self.under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(program.to_owned(), None);
    }

    /// The host said the download for `program`'s start has come to `done`, of `total`: shown
    /// only where this window is waiting on that start.
    pub fn heard(&self, program: &str, done: u64, total: Option<u64>) {
        {
            let mut under_way = self
                .under_way
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let Some(word) = under_way.get_mut(program) else {
                return;
            };
            *word = Some((done, total));
        }
        self.told();
    }

    /// `program`'s start answered: its download, if it had one, is over.
    pub fn finished(&self, program: &str) {
        let was = self
            .under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(program);
        if matches!(was, Some(Some(_))) {
            self.told();
        }
    }

    /// Every download under way, by program.
    pub fn shown(&self) -> Vec<EngineDownloadShown> {
        self.under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(|(program, word)| {
                word.map(|(done, total)| EngineDownloadShown {
                    program: program.clone(),
                    says: says(done, total),
                })
            })
            .collect()
    }
}

static HERE: OnceLock<Downloads> = OnceLock::new();

/// This window's downloads, telling it with `tell` once [`telling_the_window`] gave it.
pub fn here() -> &'static Downloads {
    HERE.get_or_init(Downloads::default)
}

/// Tells the window of each change with `tell`, set once as the app starts.
pub fn telling_the_window(tell: Box<dyn Fn() + Send + Sync>) {
    let _ = HERE.set(Downloads::telling(tell));
}

/// **The browser engine's downloads under way.**
#[tauri::command]
#[specta::specta]
pub async fn engine_downloads() -> Result<Vec<EngineDownloadShown>, String> {
    crate::off_the_window("reading the engine's downloads", || Ok(here().shown())).await
}

/// **The person cancels the engine's download for browser `program`'s start**: nothing of it
/// is kept, and its start is refused in words that say so.
#[tauri::command]
#[specta::specta]
pub async fn cancel_engine_download(app: tauri::AppHandle, program: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        let browsers = crate::browsers::browsers(&app)?;
        crate::off_the_window("cancelling the engine's download", move || {
            browsers.cancel_download(&program)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, program);
        Err("browsers run on macOS and Linux only".to_owned())
    }
}

#[cfg(test)]
mod tests;
