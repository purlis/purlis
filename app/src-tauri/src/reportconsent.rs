// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person's answers about reports (#5688, spec #5683): the window's two commands over
//! `purlis_core::report::Consent`, which holds the file, its tier and the reasons; the reports
//! this machine sent (#5689); and the window's own errors handed in as Crash reports (#5690).
//!
//! **The window's alone.** Each is on `WINDOW_ONLY`: whether this machine sends reports is the
//! person's to say, never a chat's or a link's.

use purlis_core::report::{Consent, CrashNotice};

/// The person's answers: `None` is unanswered, which purlis reads as no. `asked` is whether the
/// first-run question was answered, by Save or by Not now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ReportConsent {
    pub crash_reports: Option<bool>,
    pub agent_drafts: Option<bool>,
    /// Always send (#5695): drafts go to purlis's report service unread, held first with Undo.
    /// Meant only while Agent drafts is allowed.
    pub always_send: Option<bool>,
    pub asked: bool,
    /// Whether purlis asks again after a crash (#5700): `due` once a crash was caught after Not
    /// now, `settled` once its Notice was dismissed or answered, `None` while neither.
    pub crash_notice: Option<CrashNoticeSaid>,
}

/// The crash Notice's state (#5700), as the window names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CrashNoticeSaid {
    /// A crash was caught after Not now: the Notice is raised.
    Due,
    /// The Notice was dismissed or answered: never raised again.
    Settled,
}

impl From<CrashNotice> for CrashNoticeSaid {
    fn from(kept: CrashNotice) -> Self {
        match kept {
            CrashNotice::Due => Self::Due,
            CrashNotice::Settled => Self::Settled,
        }
    }
}

impl From<CrashNoticeSaid> for CrashNotice {
    fn from(said: CrashNoticeSaid) -> Self {
        match said {
            CrashNoticeSaid::Due => Self::Due,
            CrashNoticeSaid::Settled => Self::Settled,
        }
    }
}

impl From<Consent> for ReportConsent {
    fn from(kept: Consent) -> Self {
        Self {
            crash_reports: kept.crash_reports,
            agent_drafts: kept.agent_drafts,
            always_send: kept.always_send,
            asked: kept.asked,
            crash_notice: kept.crash_notice.map(Into::into),
        }
    }
}

impl From<ReportConsent> for Consent {
    fn from(sent: ReportConsent) -> Self {
        Self {
            crash_reports: sent.crash_reports,
            agent_drafts: sent.agent_drafts,
            always_send: sent.always_send,
            asked: sent.asked,
            crash_notice: sent.crash_notice.map(Into::into),
        }
    }
}

/// What this machine keeps of the person's answers about reports: nothing answered and not yet
/// asked on a machine with no config home, or a file purlis cannot read. On a blocking thread.
#[tauri::command]
#[specta::specta]
pub async fn report_consent() -> Result<ReportConsent, String> {
    tauri::async_runtime::spawn_blocking(|| Consent::here().into())
        .await
        .map_err(|err| format!("reading your answers about reports did not finish: {err}"))
}

/// Keeps the person's answers about reports, replacing what was kept. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn keep_report_consent(consent: ReportConsent) -> Result<(), String> {
    let root = purlis_core::machine::config_root().ok_or_else(|| {
        "This machine has no config home, so purlis cannot keep your answers about reports."
            .to_owned()
    })?;
    tauri::async_runtime::spawn_blocking(move || Consent::from(consent).write(&root))
        .await
        .map_err(|err| format!("Keeping your answers about reports did not finish: {err}"))?
        .map_err(|err| format!("purlis could not keep your answers about reports: {err}"))
}

/// One report this machine sent (#5689): its title, where it went, when, and exactly the bytes
/// that left.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SentReport {
    /// When it left, UTC, RFC 3339 to the second.
    pub at: String,
    pub title: String,
    /// Where it went, in the window's words: "purlis's report service".
    pub to: String,
    /// Where the destination kept it.
    pub address: String,
    /// Exactly what was sent.
    pub bytes: String,
    /// Not sent yet (#5693): the report service could not be reached, and it waits on this
    /// machine; `at` is when it was made, `address` is empty, and `bytes` is what will go.
    pub waiting: bool,
}

impl From<purlis_core::report::Sent> for SentReport {
    fn from(sent: purlis_core::report::Sent) -> Self {
        Self {
            at: sent.at,
            title: sent.title,
            to: match sent.to {
                purlis_core::report::SentTo::Intake => "purlis's report service".to_owned(),
                purlis_core::report::SentTo::GitHub => "purlis's issues on GitHub".to_owned(),
            },
            address: sent.address,
            bytes: sent.bytes,
            waiting: sent.waiting,
        }
    }
}

/// Every report this machine sent in the last 90 days, newest first, each with its exact bytes:
/// the sent list Settings › You › Reports shows. Empty on a machine with no config home.
#[tauri::command]
#[specta::specta]
pub async fn reports_sent() -> Result<Vec<SentReport>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(root) = purlis_core::machine::config_root() else {
            return Vec::new();
        };
        let mut sent: Vec<SentReport> = purlis_core::report::SentList::read(&root)
            .into_iter()
            .map(SentReport::from)
            .collect();
        sent.reverse();
        sent
    })
    .await
    .map_err(|err| format!("Reading the reports this machine sent did not finish: {err}"))
}

/// An error the window did not catch (#5690), as its JavaScript saw it: thrown and not caught, or
/// a promise rejected with nobody to handle it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct WindowErrorSeen {
    /// The error's name (`TypeError`), or the window's word for a reason that is no error.
    pub name: String,
    pub message: String,
    /// The address of the script it was thrown in, when the window knows it.
    pub source: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// A promise rejected with nobody to handle it, rather than an error thrown.
    pub rejection: bool,
}

/// Hands an error the window did not catch to the core's Reports, as a panic is handed: whether
/// it may go, what of it goes and where are the core's to say, so without a yes to Crash reports
/// nothing leaves. Answered at once; the send is on a thread of its own, so the window never
/// waits on the network. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn report_window_error(error: WindowErrorSeen) -> Result<(), String> {
    let evidence = purlis_core::report::Evidence::WindowError(purlis_core::report::WindowError {
        name: error.name,
        message: error.message,
        source: error.source,
        line: error.line,
        column: error.column,
        rejection: error.rejection,
    });
    std::thread::Builder::new()
        .name("reports-window-error".into())
        .spawn(move || {
            purlis_core::report::Reports::for_a_crash()
                .take(evidence, &purlis_core::forge::Caller::window())
        })
        .map(drop)
        .map_err(|err| format!("purlis could not start sending the window's error: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window's words for the answers are the core's, both ways.
    #[test]
    fn what_the_window_sends_is_what_the_core_keeps_and_reads_back() {
        for crash_reports in [None, Some(true), Some(false)] {
            for (agent_drafts, always_send) in [None, Some(true), Some(false)]
                .into_iter()
                .flat_map(|one| [(one, None), (one, Some(true)), (one, Some(false))])
            {
                for asked in [false, true] {
                    for crash_notice in [
                        None,
                        Some(CrashNoticeSaid::Due),
                        Some(CrashNoticeSaid::Settled),
                    ] {
                        let sent = ReportConsent {
                            crash_reports,
                            agent_drafts,
                            always_send,
                            asked,
                            crash_notice,
                        };
                        assert_eq!(ReportConsent::from(Consent::from(sent)), sent);
                    }
                }
            }
        }
    }
}
