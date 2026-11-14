// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Report drafts a chat wrote, held for the person's Inbox** (#5694, spec #5683, Q2).
//!
//! A chat holds no sign-in (ADR 0070), so a report a chat drafts waits here until the person
//! reads it, edits it, files it or dismisses it from the Inbox. Each names the chat it came from
//! and the project it was written in, so the project's window lists it and Go to chat finds it.
//!
//! **Tier: Machine, device-bound** (ADR 0069): `<config>/report-drafts.json`, beside
//! `reports.json` and `reports-sent.json`, through the same contained read and write
//! ([`crate::machine::read_beside`], [`crate::machine::write_beside`]), `0600`. Kept
//! [`DAYS_KEPT`] days and at most [`AT_MOST_KEPT`]: an older or further draft is dropped the next
//! time one is written. Nothing sends it anywhere: only the person's File does, or Always send
//! after its hold (#5695).
//!
//! **Filing a held draft** is here too ([`Reports::file_held`]): the duplicate search on the
//! person's yes, the filing as them on GitHub, or the Report intake where they are not signed in.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    BODY_MAX, Consent, Draft, Evidence, Hit, Kind, Known, PurlisFailure, RanAs, RanAsSaid, Refusal,
    Reports, Sent, SentDraft, SentList, SentTo, Stream, UPSTREAM, as_the_reporter, crash, file,
    intake, line_of, members_may_always_send, merge, not_filed, now, search_duplicates, sent,
};
use crate::forge::backend::Caller;

/// How long a draft Always send sends is held first, with Undo (#5695, Q2b): "Sent a report"
/// says it, and it leaves only once this has passed.
pub const HOLD_SECS: u32 = 10;

/// Taken by every change of whether a held draft is sent (#5695): the send after the hold and
/// Undo, so an Undo either comes before the send and nothing leaves, or after it and is refused.
static HOLDING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn holding() -> std::sync::MutexGuard<'static, ()> {
    HOLDING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What a chat wrote about purlis, and the call that failed if it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDraft {
    pub kind: Kind,
    pub text: String,
    /// Overrides the text's first line.
    pub title: Option<String>,
    /// The purlis command or tool that failed, and its exit code.
    pub call: Option<(String, i32)>,
    /// What the chat ran under (Q5), carried in the draft's footer and in what is sent.
    pub ran_as: RanAs,
}

/// What became of words a chat added to a draft purlis made in it (#5697).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    /// Added to the draft, which still waits in the Inbox, or in its hold.
    ToTheDraft(Box<Held>),
    /// The draft had already gone to the Report intake unread (#5695): the words followed it
    /// there, linked to it, and the sent list records them. `None` while they wait on this
    /// machine for the intake to answer.
    SentAfter(Option<String>),
}

/// How the person files a held draft from the Inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Search for possible duplicates first, and file only when there are none.
    Check,
    /// File it as a new issue, whatever the search found.
    New,
    /// Add it to issue `n` as a comment instead of filing it.
    AddTo(u64),
}

/// What became of a held draft the person filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filing {
    /// It left, to `to`, kept at `address`, and the sent list records it.
    Sent { to: SentTo, address: String },
    /// Not sent yet: the Report intake gave no answer, and it waits on this machine to go when
    /// it answers again (#5693, Q24), listed as waiting to send. The draft leaves the Inbox.
    Queued,
    /// Issues that may already cover it: nothing was filed, and the person chooses.
    Duplicates(Vec<Hit>),
    /// Nothing was sent, and why.
    Refused(Refusal),
}

/// The file, inside [`crate::machine::DIR`].
pub const FILE: &str = "report-drafts.json";

/// The format's version. Another one reads as no drafts.
pub const VERSION: u32 = 1;

/// How long a draft waits for the person.
pub const DAYS_KEPT: i64 = 14;

/// The most drafts kept; the oldest go first.
pub const AT_MOST_KEPT: usize = 50;

/// The most the file may hold. A larger one is not read.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// The chat a draft came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameFrom {
    /// The chat's number in the app while it means one, for Go to chat.
    pub session: Option<u32>,
    /// The chat's name as the window shows it (`steward 4`): data, drawn as text.
    pub chat: String,
    /// The project it was written in, as its root's path: whose Inbox lists it.
    pub project: String,
}

/// A draft held for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    /// Its own name, for the window to act on it by.
    pub id: String,
    /// When it was held, UTC, RFC 3339 to the second.
    pub at: String,
    pub kind: HeldKind,
    pub title: String,
    pub body: String,
    /// The call that failed, its exit and the versions, in one line: the card's sentence.
    pub line: String,
    /// The categories the scrub removed.
    #[serde(default)]
    pub scrubbed: Vec<String>,
    pub from: CameFrom,
    /// The person changed its words in the Inbox: no longer only the agent's.
    #[serde(default)]
    pub edited: bool,
    /// When its hold ends, UTC, RFC 3339 to the second: a draft Always send sends (#5695), shown
    /// as "Sent a report" with Undo until then, and sent to the Report intake after. `None` is a
    /// draft that waits in the Inbox for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sends_at: Option<String>,
    /// What the chat ran under, in a report's closed words (Q5): sent with the draft. None for a
    /// draft held before it was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ran_as: Option<RanAsSaid>,
    /// What makes a failure of purlis's own the same as another (#5697, Q15): the call and its
    /// error's class, in this purlis version. The same failure again, from any chat, is counted
    /// on this draft, or on the sent list's entry once it left, and never drafted again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// How many times the same failure was met again while this draft waited.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub repeats: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// [`Kind`], as the file keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeldKind {
    Bug,
    Feature,
}

impl From<Kind> for HeldKind {
    fn from(kind: Kind) -> HeldKind {
        match kind {
            Kind::Bug => HeldKind::Bug,
            Kind::Feature => HeldKind::Feature,
        }
    }
}

impl Held {
    /// Twelve hex characters over the repository, the title and the body, as
    /// [`super::Draft::digest`] has them: what the window's File must name for exactly these
    /// words to be filed.
    pub fn digest(&self) -> String {
        super::Draft {
            kind: self.report_kind(),
            title: self.title.clone(),
            body: self.body.clone(),
            scrubbed: Vec::new(),
        }
        .digest()
    }

    /// What the sent list keeps of it: its id and the chat it came from.
    fn sent_draft(&self) -> SentDraft {
        SentDraft {
            id: self.id.clone(),
            session: self.from.session,
        }
    }

    pub(super) fn report_kind(&self) -> Kind {
        match self.kind {
            HeldKind::Bug => Kind::Bug,
            HeldKind::Feature => Kind::Feature,
        }
    }
}

/// **A filed case-6 draft's miss** (#5739, spec #5714): one `report` miss in the skill log of
/// the project `one` was written in, of the skill and harness its words name
/// ([`crate::skilluse::Reported`]), filed under this machine's log name (`config` is the config
/// root its id is read from). Nothing for a draft of another case. Never the words.
fn missed_as_reported(one: &Held, config: Option<&Path>) {
    let Some(reported) = crate::skilluse::Reported::of(&one.body) else {
        return;
    };
    crate::skilluse::missed(
        Path::new(&one.from.project),
        &crate::skilluse::Missing {
            skill: &reported.skill,
            harness: reported.harness.as_deref(),
            harness_version: reported.harness_version.as_deref(),
            persona: None,
            signal: crate::skilluse::Signal::Report,
        },
        chrono::Utc::now(),
        &crate::machine::log_name(config, &crate::dispatch::host()),
    );
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    held: Vec<Held>,
}

/// The drafts held under a config root.
pub struct HeldDrafts;

impl HeldDrafts {
    /// Every draft held under `config_root`, oldest first; none when there is no file or one
    /// purlis will not read.
    pub fn read(config_root: &Path) -> Vec<Held> {
        crate::machine::read_beside(
            config_root,
            FILE,
            MAX_BYTES,
            "the report drafts waiting for you",
        )
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<OnDisk>(&text).ok())
        .filter(|on_disk| on_disk.version == VERSION)
        .map(|on_disk| on_disk.held)
        .unwrap_or_default()
    }

    /// Keeps `held` in place of what was held, dropping what is past [`DAYS_KEPT`] or
    /// [`AT_MOST_KEPT`].
    pub fn write(config_root: &Path, held: Vec<Held>) -> io::Result<()> {
        crate::machine::supported()?;
        let oldest = chrono::Utc::now() - chrono::Duration::days(DAYS_KEPT);
        let mut kept: Vec<Held> = held
            .into_iter()
            .filter(|one| {
                chrono::DateTime::parse_from_rfc3339(&one.at).is_ok_and(|at| at >= oldest)
            })
            .collect();
        let over = kept.len().saturating_sub(AT_MOST_KEPT);
        kept.drain(..over);
        let dir = crate::machine::private_dir(config_root)?;
        let text = serde_json::to_string_pretty(&OnDisk {
            version: VERSION,
            held: kept,
        })
        .map_err(io::Error::other)?;
        crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
    }
}

/// A new draft's id: random, so two drafts of the same words are still two.
pub(super) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
}

impl Reports {
    /// Holds a chat's `evidence` for the person's Inbox, naming the chat it came `from`: only
    /// with the yes to its stream, and nothing is sent (#5694). With the yes to Always send too,
    /// and room in today's cap, it is held to send instead (#5695): its `sends_at` says when the
    /// hold ends, and [`Reports::send_held_due`] sends it to the Report intake after.
    pub fn hold(
        &self,
        evidence: Evidence,
        source: &Caller,
        from: CameFrom,
    ) -> Result<Held, Refusal> {
        self.hold_as(evidence, source, from, None)
    }

    /// [`Self::hold`], the draft named by `fingerprint` where it has one.
    fn hold_as(
        &self,
        evidence: Evidence,
        source: &Caller,
        from: CameFrom,
        fingerprint: Option<String>,
    ) -> Result<Held, Refusal> {
        self.consented(&evidence, source)?;
        let ran_as = ran_as_of(&evidence);
        let root = self.kept_under()?;
        let line = line_of(&evidence, &self.known);
        let draft = self.draft(evidence)?;
        let _turn = holding();
        let mut all = HeldDrafts::read(&root);
        // With Always send it is held to send, not to read, while today has room (#5695).
        let sends_at = self
            .may_send_unread(held_to_send(&all, None), &from.project)
            .is_ok()
            .then(hold_ends);
        let one = Held {
            id: new_id(),
            at: now(),
            kind: draft.kind.into(),
            title: draft.title,
            body: draft.body,
            line,
            scrubbed: draft.scrubbed,
            from,
            edited: false,
            sends_at,
            ran_as,
            fingerprint,
            repeats: 0,
        };
        all.push(one.clone());
        HeldDrafts::write(&root, all).map_err(|e| Refusal::NotKept(e.to_string()))?;
        Ok(one)
    }

    /// [`Self::hold`], once for each failure of purlis's own (#5697, Q15): the same failure (the
    /// same call and error class, in this purlis version: [`fingerprint_of`]) met again, from
    /// this chat or another, is counted on the draft that waits in the Inbox, or on the sent
    /// list's entry once that draft left, and `None` is answered: nothing new is held, and no
    /// chat is told again. Else the new draft.
    pub fn hold_once(
        &self,
        evidence: Evidence,
        source: &Caller,
        from: CameFrom,
    ) -> Result<Option<Held>, Refusal> {
        self.consented(&evidence, source)?;
        let Evidence::PurlisFailure(failure) = &evidence else {
            return self.hold(evidence, source, from).map(Some);
        };
        let fingerprint = fingerprint_of(failure, &self.known);
        let root = self.kept_under()?;
        {
            let _turn = holding();
            let mut all = HeldDrafts::read(&root);
            if let Some(one) = all
                .iter_mut()
                .find(|one| one.fingerprint.as_deref() == Some(fingerprint.as_str()))
            {
                one.repeats += 1;
                HeldDrafts::write(&root, all).map_err(|e| Refusal::NotKept(e.to_string()))?;
                return Ok(None);
            }
        }
        if SentList::count_repeat(&root, &fingerprint).is_some() {
            return Ok(None);
        }
        self.hold_as(evidence, source, from, Some(fingerprint))
            .map(Some)
    }

    /// **Chat `session`'s words, added to the draft `id` purlis made in that chat** (#5697):
    /// what the agent was doing when purlis failed, scrubbed, after the draft's own words.
    /// Where Always send already sent that draft to the Report intake unread (#5695), the words
    /// follow it there, linked to it by its event's id, and are listed in the sent list: they
    /// are not lost for arriving after the hold. Refused as no longer held for a draft of
    /// another chat's, one filed or dismissed, or one sent while Always send is off now.
    pub fn add_to_held(&self, id: &str, session: u32, text: &str) -> Result<Added, Refusal> {
        let text = text.trim();
        if text.is_empty() {
            return Err(Refusal::Unusable(
                "there is nothing to add: say what you were doing".to_string(),
            ));
        }
        if !self
            .held()
            .iter()
            .any(|one| one.id == id && one.from.session == Some(session))
        {
            return self.follow_up(id, session, text);
        }
        let (words, scrubbed) = self.known.scrub(text);
        let added = format!("\n\n**What the agent was doing:**\n\n{words}");
        if self
            .held()
            .iter()
            .find(|one| one.id == id)
            .is_some_and(|one| one.body.chars().count() + added.chars().count() > BODY_MAX)
        {
            return Err(Refusal::Unusable(format!(
                "the report would be over {BODY_MAX} characters with these words. Shorten them."
            )));
        }
        self.change_held(id, |one| {
            one.body.push_str(&added);
            one.scrubbed = merge(std::mem::take(&mut one.scrubbed), scrubbed);
        })
        .map(|held| Added::ToTheDraft(Box::new(held)))
    }

    /// Chat `session`'s words `text` about the draft `id` that Always send already sent, sent
    /// after it to the Report intake as a follow-up of its event, with the same yes it left
    /// under. Refused as [`Refusal::NotHeld`] for a draft that was not sent so, or not by that
    /// chat.
    fn follow_up(&self, id: &str, session: u32, text: &str) -> Result<Added, Refusal> {
        let Some((intake, root)) = &self.intake else {
            return Err(Refusal::NotHeld);
        };
        let first = SentList::read(root).into_iter().find(|one| {
            one.unread_draft
                && one.to == SentTo::Intake
                && one
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == id && draft.session == Some(session))
        });
        let Some(first) = first else {
            return Err(Refusal::NotHeld);
        };
        if !self.consent.unwrap_or_else(Consent::here).always_sends() {
            return Err(Refusal::NoConsent(Stream::AgentDrafts));
        }
        let (words, _) = self.known.scrub(text);
        if words.chars().count() > BODY_MAX {
            return Err(Refusal::Unusable(format!(
                "these words are over {BODY_MAX} characters. Shorten them."
            )));
        }
        let report = crash::CrashReport::follow_up(
            &crash::event_id_of(&first.bytes),
            &first.title,
            &words,
            intake.dsn(),
        );
        self.send_waiting();
        let listed = |address: String, bytes: String| Sent {
            at: now(),
            title: report.title.clone(),
            to: SentTo::Intake,
            address,
            bytes,
            fingerprint: None,
            repeats: 0,
            repeats_sent: 0,
            // Words about a draft already counted: not one more against the day's cap.
            unread_draft: false,
            waiting: false,
            draft: first.draft.clone(),
        };
        match intake.send(report.bytes.as_bytes(), &report.event_id) {
            Ok(address) => {
                // It has left: a list that cannot be written does not unsend it.
                let _ = SentList::add(root, listed(address.clone(), report.bytes.clone()));
                Ok(Added::SentAfter(Some(address)))
            }
            Err(intake::Failed::NoAnswer(why)) => {
                match SentList::wait(root, listed(String::new(), report.bytes.clone())) {
                    Ok(true) => Ok(Added::SentAfter(None)),
                    Ok(false) => Err(Refusal::NotSent(format!(
                        "{why}, and {} reports already wait to send",
                        sent::WAITING_AT_MOST
                    ))),
                    Err(err) => Err(Refusal::NotSent(format!(
                        "{why}, and it could not be kept to send later: {err}"
                    ))),
                }
            }
            Err(intake::Failed::Refused(why)) => Err(Refusal::NotSent(why)),
        }
    }

    /// Every draft held for the Inbox, oldest first.
    pub fn held(&self) -> Vec<Held> {
        self.root
            .as_deref()
            .map(HeldDrafts::read)
            .unwrap_or_default()
    }

    /// The held draft `id` with the person's own `title` and `body` in place of its words.
    pub fn edit_held(&self, id: &str, title: &str, body: &str) -> Result<Held, Refusal> {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() || body.trim().is_empty() {
            return Err(Refusal::Unusable(
                "a report needs a title and a body".to_string(),
            ));
        }
        if body.chars().count() > BODY_MAX {
            return Err(Refusal::Unusable(format!(
                "the report is over {BODY_MAX} characters; GitHub would refuse it. Shorten it."
            )));
        }
        self.change_held(id, |one| {
            one.title = title.clone();
            one.body = body.to_string();
            one.edited = true;
        })
    }

    /// Lets the held draft `id` go, unsent.
    pub fn dismiss_held(&self, id: &str) {
        if let Some(root) = self.root.as_deref() {
            let all = HeldDrafts::read(root);
            if all.iter().any(|one| one.id == id) {
                let _ =
                    HeldDrafts::write(root, all.into_iter().filter(|one| one.id != id).collect());
            }
        }
    }

    /// Files the held draft `id`, only if `shown` is its digest, as `source` (the person in the
    /// window): on [`UPSTREAM`] as them where they are signed in to GitHub, after the duplicate
    /// search unless `how` says otherwise; to the Report intake where they are not. What left is
    /// listed in the sent list, and the draft leaves the Inbox.
    ///
    /// A filed case-6 draft, one saying a purlis skill should have started (#5739), logs a
    /// `report` miss of the skill and harness its filed words name in its project's skill log.
    pub fn file_held(&self, id: &str, source: &Caller, shown: &str, how: How) -> Filing {
        // Read before filing, as it leaves the Inbox once filed; the digest holds it to the words
        // that were filed.
        let one = self.held().into_iter().find(|one| one.id == id);
        let filing = self.file_held_once(id, source, shown, how);
        if let (Filing::Sent { .. } | Filing::Queued, Some(one)) = (&filing, one) {
            missed_as_reported(&one, self.root.as_deref());
        }
        filing
    }

    fn file_held_once(&self, id: &str, source: &Caller, shown: &str, how: How) -> Filing {
        let Some(one) = self.held().into_iter().find(|one| one.id == id) else {
            return Filing::Refused(Refusal::NotHeld);
        };
        let digest = one.digest();
        if digest != shown {
            return Filing::Refused(Refusal::NotTheShownDraft { digest });
        }
        let draft = Draft {
            kind: one.report_kind(),
            title: one.title.clone(),
            body: one.body.clone(),
            scrubbed: Vec::new(),
        };
        let sent = match as_the_reporter(source) {
            Ok(tracker) => {
                if how == How::Check {
                    match search_duplicates(tracker.as_ref(), source, &draft) {
                        Ok(hits) if !hits.is_empty() => return Filing::Duplicates(hits),
                        Ok(_) => {}
                        Err(why) => return Filing::Refused(not_filed(&draft, why)),
                    }
                }
                let filed = match how {
                    How::AddTo(number) => tracker.add_comment(
                        source,
                        UPSTREAM,
                        number,
                        &format!("**{}**\n\n{}", draft.title, draft.body),
                    ),
                    How::Check | How::New => file(tracker.as_ref(), source, &draft),
                };
                match filed {
                    Ok(address) => Sent {
                        at: now(),
                        title: draft.title.clone(),
                        to: SentTo::GitHub,
                        address,
                        bytes: format!("{}\n\n{}", draft.title, draft.body),
                        fingerprint: one.fingerprint.clone(),
                        repeats: one.repeats,
                        repeats_sent: 0,
                        unread_draft: false,
                        waiting: false,
                        draft: Some(one.sent_draft()),
                    },
                    Err(why) => return Filing::Refused(not_filed(&draft, why)),
                }
            }
            Err(_) => return self.draft_to_the_intake(&one, true),
        };
        let to = sent.to;
        let address = sent.address.clone();
        if let Some(root) = self.root.as_deref() {
            // The report has left: a list that cannot be written does not unsend it.
            let _ = SentList::add(root, sent);
        }
        self.dismiss_held(id);
        Filing::Sent { to, address }
    }

    /// Files the held draft `one` to the Report intake, the way #5693 sends any report there:
    /// the reports that wait go first, it carries the counts of repeats not yet told (its own
    /// among them, for a failure of purlis's own met again while it waited), and it waits on
    /// this machine when the intake gives no answer. Read and filed by the person when
    /// `reviewed`; sent unread by Always send, and counted against the daily cap, when not
    /// (#5695).
    fn draft_to_the_intake(&self, one: &Held, reviewed: bool) -> Filing {
        let Some((intake, root)) = &self.intake else {
            return Filing::Refused(Refusal::NoIntake);
        };
        let draft = Draft {
            kind: one.report_kind(),
            title: one.title.clone(),
            body: one.body.clone(),
            scrubbed: Vec::new(),
        };
        let report = crash::CrashReport::of_draft(
            &draft,
            one.edited,
            reviewed,
            one.ran_as.as_ref(),
            one.fingerprint.as_deref(),
            intake.dsn(),
        );
        self.send_waiting();
        let mut told = self.counts_to_tell(root);
        let own = one
            .fingerprint
            .clone()
            .filter(|_| one.repeats > 0)
            .map(|fingerprint| (fingerprint, one.repeats));
        told.extend(own.clone());
        let bytes = crash::with_repeats(&report.bytes, &told);
        let listed = |address: String, bytes: String, repeats_sent: u32| Sent {
            at: now(),
            title: report.title.clone(),
            to: SentTo::Intake,
            address,
            bytes,
            fingerprint: one.fingerprint.clone(),
            repeats: one.repeats,
            repeats_sent,
            unread_draft: !reviewed,
            waiting: false,
            draft: Some(one.sent_draft()),
        };
        match intake.send(bytes.as_bytes(), &report.event_id) {
            Ok(address) => {
                // The report has left: a list that cannot be written does not unsend it.
                let _ =
                    SentList::add_telling(root, listed(address.clone(), bytes, one.repeats), &told);
                self.dismiss_held(&one.id);
                Filing::Sent {
                    to: SentTo::Intake,
                    address,
                }
            }
            Err(intake::Failed::NoAnswer(why)) => {
                match SentList::wait(root, listed(String::new(), report.bytes.clone(), 0)) {
                    Ok(true) => {
                        self.dismiss_held(&one.id);
                        Filing::Queued
                    }
                    Ok(false) => Filing::Refused(Refusal::NotSent(format!(
                        "{why}, and {} reports already wait to send",
                        sent::WAITING_AT_MOST
                    ))),
                    Err(err) => Filing::Refused(Refusal::NotSent(format!(
                        "{why}, and it could not be kept to send later: {err}"
                    ))),
                }
            }
            Err(intake::Failed::Refused(why)) => Filing::Refused(Refusal::NotSent(why)),
        }
    }

    /// Refused unless a draft written in `project` may go to the Report intake unread now, with
    /// `holding` more held to send (#5695): the yes to Always send, a project whose rule lets its
    /// members (#5696), an intake to send to, and room in today's cap.
    fn may_send_unread(&self, holding: usize, project: &str) -> Result<(), Refusal> {
        if !self.consent.unwrap_or_else(Consent::here).always_sends() {
            return Err(Refusal::NoConsent(Stream::AgentDrafts));
        }
        if !members_may_always_send(Path::new(project)) {
            return Err(Refusal::ProjectForbids);
        }
        let Some((_, root)) = &self.intake else {
            return Err(Refusal::NoIntake);
        };
        SentList::within_the_daily_cap(root, chrono::Utc::now(), holding)
    }

    /// **Always send, pressed on a draft in the Inbox** (#5695): the held draft `id` is held to
    /// send, as a chat's draft is when Always send is on. Refused, and left in the Inbox, without
    /// the standing yes (the window asks for it first), without an intake, or past today's cap.
    pub fn always_send_held(&self, id: &str) -> Result<Held, Refusal> {
        let root = self.kept_under()?;
        let _turn = holding();
        let all = HeldDrafts::read(&root);
        let Some(one) = all.iter().find(|one| one.id == id) else {
            return Err(Refusal::NotHeld);
        };
        self.may_send_unread(held_to_send(&all, Some(id)), &one.from.project)?;
        self.change_held(id, |one| one.sends_at = Some(hold_ends()))
    }

    /// **Undo, within the hold** (#5695): the held draft `id` is not sent, and waits in the Inbox
    /// for the person to read. Refused as [`Refusal::NotHeld`] once it has left.
    pub fn undo_held(&self, id: &str) -> Result<Held, Refusal> {
        let _turn = holding();
        self.change_held(id, |one| one.sends_at = None)
    }

    /// Sends every held draft whose hold ended by `now` to the Report intake, unread, and never
    /// anywhere else (ADR 0118), and answers how many left the Inbox: sent, or waiting on this
    /// machine for the intake to answer. One that may no longer go unread (Always send was
    /// turned off, today's cap was reached, the intake did not take it) falls back to asking: it
    /// waits in the Inbox.
    pub fn send_held_due(&self, now: chrono::DateTime<chrono::Utc>) -> usize {
        let Some(root) = self.root.as_deref() else {
            return 0;
        };
        let _turn = holding();
        let due = HeldDrafts::read(root).into_iter().filter(|one| {
            one.sends_at.as_deref().is_some_and(|at| {
                chrono::DateTime::parse_from_rfc3339(at).map_or(true, |at| at <= now)
            })
        });
        let mut left = 0;
        for one in due {
            let filed = match self.may_send_unread(0, &one.from.project) {
                Ok(()) => self.draft_to_the_intake(&one, false),
                Err(refused) => Filing::Refused(refused),
            };
            match filed {
                Filing::Sent { .. } | Filing::Queued => left += 1,
                _ => {
                    let _ = self.change_held(&one.id, |one| one.sends_at = None);
                }
            }
        }
        left
    }

    fn change_held(&self, id: &str, change: impl FnOnce(&mut Held)) -> Result<Held, Refusal> {
        let root = self.kept_under()?;
        let mut all = HeldDrafts::read(&root);
        let Some(one) = all.iter_mut().find(|one| one.id == id) else {
            return Err(Refusal::NotHeld);
        };
        change(one);
        let changed = one.clone();
        HeldDrafts::write(&root, all).map_err(|e| Refusal::NotKept(e.to_string()))?;
        Ok(changed)
    }

    fn kept_under(&self) -> Result<PathBuf, Refusal> {
        self.root.clone().ok_or_else(|| {
            Refusal::NotKept("this machine has no config home to keep a draft in".to_string())
        })
    }
}

/// How many of `all` are held to send, leaving out `but`.
fn held_to_send(all: &[Held], but: Option<&str>) -> usize {
    all.iter()
        .filter(|one| one.sends_at.is_some() && Some(one.id.as_str()) != but)
        .count()
}

/// When a hold that starts now ends.
fn hold_ends() -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(i64::from(HOLD_SECS)))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

/// What a report of `evidence` says its chat ran under, where it is a chat's.
fn ran_as_of(evidence: &Evidence) -> Option<RanAsSaid> {
    match evidence {
        Evidence::AgentDraft(AgentDraft { ran_as, .. })
        | Evidence::PurlisFailure(
            PurlisFailure::Internal { ran_as, .. } | PurlisFailure::SandboxBlock { ran_as, .. },
        ) => Some(ran_as.said()),
        _ => None,
    }
}

/// **What makes a failure of purlis's own the same as another** (#5697, Q15): the call, its
/// error's class and this purlis version. The class is the error's first line as a Report would
/// carry it (scrubbed, every path taken out) with each run of digits as one `#`, so the same failure
/// on another file, port or count is still the same; a sandbox block's is the kind of path or
/// host it was. Only its digest leaves the machine.
pub(super) fn fingerprint_of(failure: &PurlisFailure, known: &Known) -> String {
    let class = match failure {
        PurlisFailure::Internal { caught, .. } => {
            let first = caught
                .error
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default();
            let (line, _) = known.scrub(&crash::without_paths(first));
            let mut class = String::new();
            for c in line.chars().take(160) {
                // A run of digits, whatever its length, is one `#`.
                if !c.is_ascii_digit() {
                    class.push(c);
                } else if !class.ends_with('#') {
                    class.push('#');
                }
            }
            class
        }
        PurlisFailure::SandboxBlock { block, .. } => block.kind.word().to_owned(),
    };
    crash::fingerprint(&[
        "purlis-failure",
        crate::adopt::release_version(),
        &failure.call(),
        &class,
    ])
}
