// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Report drafts in the Inbox** (#5694, spec #5683, Q2/Q3): what a chat's `purlis report`
//! hands the app, held for the person, and the window's commands over it. Everything about a
//! report (its scrub, the consent it waits for, where it goes and the sent list) is the core's
//! Reports module's; this only carries a chat's line in and the person's presses out.
//!
//! **A chat only drafts.** It holds no sign-in (ADR 0070), so its line is held, never filed, and
//! held only with the person's yes to Agent drafts. **The window's commands are the window's
//! alone** (`WINDOW_ONLY`): File files as the person.

use purlis_core::forge::Caller;
use purlis_core::hookwire::{Answer, ReportDraftAsk};
use purlis_core::report::{
    AgentDraft, CameFrom, Evidence, Filing, Held as HeldDraft, HeldKind, How, Kind, Refusal,
    Reports, SentTo, Stream,
};

use crate::planes::{Held, PlaneId, Planes};

/// The event every window reads a project's drafts again on: one was held, filed, edited or
/// dismissed.
pub const REPORT_DRAFTS_CHANGED: &str = "report-drafts-changed";

/// The app, once its setup has run: what a held draft tells the windows through.
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// Has a held draft tell the windows of `app` from here on.
pub fn tells_through(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

fn changed() {
    if let Some(app) = APP.get() {
        let _ = tauri::Emitter::emit(app, REPORT_DRAFTS_CHANGED, ());
    }
}

/// What a chat is told when its draft was held.
pub const HELD_SAID: &str = "The draft is in the person's Inbox. They read it there, and file it \
                             or dismiss it. Nothing was sent.";

/// What a chat is told when the person has not allowed Agent drafts.
pub const NOT_ALLOWED: &str = "The person has not allowed Agent drafts (Settings › You › \
                               Reports), so the draft was not kept and nothing was sent. Tell \
                               them what you found instead.";

/// **Chat `ask.chat` drafted a report**: held for the person's Inbox, or why not. The chat the
/// line names is the one whose token it carries, which the listener checked; each chat is held
/// to the rate its brokered writes are, so a chat cannot fill the Inbox.
pub fn asked(held: &Held, ask: &ReportDraftAsk) -> Answer {
    let no = |why: String| Answer::No { why };
    let Some(name) = held.chats().shown_name(ask.chat) else {
        return no(format!("chat {} is not one this app has open", ask.chat));
    };
    if let Err(why) = held.brokered().allow(ask.chat, std::time::Instant::now()) {
        return no(why);
    }
    let evidence = Evidence::AgentDraft(AgentDraft {
        kind: if ask.feature {
            Kind::Feature
        } else {
            Kind::Bug
        },
        text: ask.text.clone(),
        title: ask.title.clone(),
        call: ask.call.clone(),
    });
    let from = CameFrom {
        session: Some(ask.chat),
        chat: name,
        project: held.root().display().to_string(),
    };
    match Reports::here(Some(held.root())).hold(
        evidence,
        &Caller::chat(&ask.chat.to_string()),
        from,
    ) {
        Ok(_) => {
            changed();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(HELD_SAID.into())],
                code: 0,
            }
        }
        Err(Refusal::NoConsent(Stream::AgentDrafts)) => no(NOT_ALLOWED.to_owned()),
        Err(refused) => no(said_of(&refused)),
    }
}

/// A draft held for the project's Inbox, as the window draws its card.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ReportDraft {
    pub id: String,
    /// When it was held, UTC, RFC 3339 to the second.
    pub at: String,
    pub feature: bool,
    pub title: String,
    pub body: String,
    /// The call that failed and its exit, where named, and the versions.
    pub line: String,
    /// What the scrub took out, by category.
    pub scrubbed: Vec<String>,
    /// The chat it came from, by its name, and its number while it means one.
    pub chat: String,
    pub session: Option<u32>,
    /// The person changed its words.
    pub edited: bool,
    /// What File must name for exactly these words to be filed.
    pub digest: String,
}

impl From<HeldDraft> for ReportDraft {
    fn from(one: HeldDraft) -> Self {
        Self {
            digest: one.digest(),
            id: one.id,
            at: one.at,
            feature: one.kind == HeldKind::Feature,
            title: one.title,
            body: one.body,
            line: one.line,
            scrubbed: one.scrubbed,
            chat: one.from.chat,
            session: one.from.session,
            edited: one.edited,
        }
    }
}

/// One issue a draft may duplicate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PossibleDuplicate {
    pub number: u32,
    pub title: String,
    /// `open` or `closed`.
    pub state: String,
    pub url: String,
}

/// How the person files a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case", tag = "how", content = "issue")]
pub enum FileHow {
    /// Search for duplicates first.
    Check,
    /// File it as a new issue.
    New,
    /// Add it to this issue instead.
    AddTo(u32),
}

/// What became of a draft the person filed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum DraftFiled {
    /// It left: `github` or `intake`, and where it was kept.
    Sent {
        to: String,
        address: String,
        title: String,
    },
    /// Issues that may already cover it: nothing was filed.
    Duplicates { issues: Vec<PossibleDuplicate> },
}

fn root_of(planes: &Planes, plane: &PlaneId) -> Result<String, String> {
    Ok(planes.held(plane)?.root().display().to_string())
}

async fn off_the_window<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("The report draft did not finish: {err}"))?
}

/// **The project's report drafts**, oldest first: what its chats drafted and the person has not
/// filed or dismissed. None on a machine with no config home.
#[tauri::command]
#[specta::specta]
pub async fn report_drafts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<ReportDraft>, String> {
    let root = root_of(&planes, &plane)?;
    off_the_window(move || {
        Ok(Reports::here(None)
            .held()
            .into_iter()
            .filter(|one| one.from.project == root)
            .map(ReportDraft::from)
            .collect())
    })
    .await
}

/// **The person's own words in place of a draft's**: kept, and answered with the draft as it
/// now reads, its new digest with it.
#[tauri::command]
#[specta::specta]
pub async fn edit_report_draft(
    id: String,
    title: String,
    body: String,
) -> Result<ReportDraft, String> {
    let edited = off_the_window(move || {
        Reports::here(None)
            .edit_held(&id, &title, &body)
            .map(ReportDraft::from)
            .map_err(|refused| said_of(&refused))
    })
    .await?;
    changed();
    Ok(edited)
}

/// **Files a draft, as the person**, only if `digest` is the one its card showed: on
/// purlis/purlis as them where they are signed in to GitHub, after the duplicate search unless
/// `how` says otherwise, and to purlis's report service where they are not. The sent list
/// records it, and the draft leaves the Inbox.
#[tauri::command]
#[specta::specta]
pub async fn file_report_draft(
    id: String,
    digest: String,
    how: FileHow,
) -> Result<DraftFiled, String> {
    let how = match how {
        FileHow::Check => How::Check,
        FileHow::New => How::New,
        FileHow::AddTo(number) => How::AddTo(u64::from(number)),
    };
    let title = id.clone();
    let filed = off_the_window(move || {
        let reports = Reports::here(None);
        let title = reports
            .held()
            .into_iter()
            .find(|one| one.id == title)
            .map(|one| one.title)
            .unwrap_or_default();
        Ok((
            reports.file_held(&id, &Caller::window(), &digest, how),
            title,
        ))
    })
    .await?;
    let (filing, title) = filed;
    match filing {
        Filing::Sent { to, address } => {
            changed();
            Ok(DraftFiled::Sent {
                to: match to {
                    SentTo::GitHub => "github",
                    SentTo::Intake => "intake",
                }
                .to_owned(),
                address,
                title,
            })
        }
        Filing::Duplicates(hits) => Ok(DraftFiled::Duplicates {
            issues: hits
                .into_iter()
                .map(|hit| PossibleDuplicate {
                    number: u32::try_from(hit.number).unwrap_or(u32::MAX),
                    title: hit.title,
                    state: hit.state.to_lowercase(),
                    url: hit.url,
                })
                .collect(),
        }),
        Filing::Refused(refused) => Err(said_of(&refused)),
    }
}

/// **Lets a draft go, unsent.**
#[tauri::command]
#[specta::specta]
pub async fn dismiss_report_draft(id: String) -> Result<(), String> {
    off_the_window(move || {
        Reports::here(None).dismiss_held(&id);
        Ok(())
    })
    .await?;
    changed();
    Ok(())
}

/// What the window says of a refusal: what happened and what to do.
fn said_of(refused: &Refusal) -> String {
    match refused {
        Refusal::NotTheShownDraft { .. } => {
            "The draft changed since it was shown, so nothing was sent. Read it again, then File."
                .to_owned()
        }
        Refusal::NotFiled { why, link, whole } => format!(
            "It was not filed: {why}. Open this link to file it in a browser{}: {link}",
            if *whole {
                ""
            } else {
                ", and paste the body from the draft"
            }
        ),
        Refusal::NotHeld => {
            "That draft is no longer in the Inbox: it was filed or dismissed already.".to_owned()
        }
        Refusal::NoIntake => "This build has no report service to send it to, and you are not \
                              signed in to GitHub. Sign in to GitHub in Settings to file it."
            .to_owned(),
        Refusal::NotSent(why) => format!("It was not sent: {why}. Try File again later."),
        Refusal::NotKept(why) | Refusal::Unusable(why) => why.clone(),
        Refusal::NoConsent(_) => NOT_ALLOWED.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window sends how it files in the shape the TypeScript bindings write.
    #[test]
    fn the_window_says_how_to_file_as_a_tagged_word() {
        let read = |text: &str| serde_json::from_str::<FileHow>(text).unwrap();
        assert_eq!(read(r#"{"how":"check"}"#), FileHow::Check);
        assert_eq!(read(r#"{"how":"new"}"#), FileHow::New);
        assert_eq!(read(r#"{"how":"add-to","issue":41}"#), FileHow::AddTo(41));
    }
}
