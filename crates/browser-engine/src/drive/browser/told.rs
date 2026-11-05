// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one way out of a browser to a chat** (#5310): every answer and refusal the agent's calls
//! get passes [`Browser::kept_out`], which takes each text kept out of every string in it, however
//! that string came to be there. Each field is named below with no catch-all, so a field or a
//! refusal added later does not build until this says what it does with it.
//!
//! A text kept out of a page (what the person typed into it) is taken out of what is said about
//! that page, and, since a string about no page may hold it too, of what a call says beside the
//! page. What a page's own text and lines say is scrubbed by that page's alone: what the person
//! typed on one page takes no ordinary words out of another.
//!
//! **What the chat wrote itself is said back as it wrote it** (a reference, a page's name, a key,
//! an option). It holds nothing the chat did not have, and an echo with a guess blanked would tell
//! the chat its guess was right, in one call and with no page, so a refusal's echoes are left
//! alone (D-5115-13).

use super::super::kept::Kept;
use super::super::scrub::Scrub;
use super::super::{
    Answer, Content, DownloadInfo, FieldInfo, MOST_FILE_NAME_CHARS, PageId, PageInfo, Refusal,
    Settled, lock, plain_file_name,
};
use super::Browser;

/// What a call says of a site it held back whose address holds a text kept out: that site is not
/// said, and so nobody is asked about it.
const A_SITE_NOT_SAID: &str = "a request went nowhere because its address holds a value purlis \
                               keeps out of what you read, so nobody is asked about it";

/// What is kept out, as a call is answered: of the whole browser, and of each page.
struct Keeping {
    every: Scrub,
    /// What purlis filled and what the browser keeps of every page, apart from what the person
    /// typed on one: what a site's name is held to, since what the person typed is often part of
    /// a site's name and says nothing of it.
    filled: Scrub,
    browser: Kept,
    pages: Vec<(PageId, Kept)>,
}

impl Keeping {
    /// What is said of `page`: its own texts, or where it has gone, the browser's.
    fn of_page(&self, page: &PageId) -> Scrub {
        self.pages
            .iter()
            .find(|(id, _)| id == page)
            .map_or_else(|| Scrub::of(&self.browser), |(_, kept)| Scrub::of(kept))
    }

    fn page(&self, page: PageInfo) -> PageInfo {
        let scrub = self.of_page(&page.id);
        let PageInfo {
            id,
            address,
            title,
            current,
        } = page;
        PageInfo {
            id,
            address: scrub.text(&address),
            title: scrub.text(&title),
            current,
        }
    }

    fn answer(&self, answer: Answer) -> Answer {
        let Answer {
            page,
            settled,
            opened,
            content,
            mut note,
            repeated,
            asked,
            downloads,
        } = answer;
        let every = &self.every;
        let own = page
            .as_ref()
            .map_or_else(|| Scrub::of(&self.browser), |page| self.of_page(&page.id));
        let content = match content {
            Content::Text(text) => Content::Text(own.text(&text)),
            Content::Pages(pages) => {
                Content::Pages(pages.into_iter().map(|page| self.page(page)).collect())
            }
            Content::Field(FieldInfo {
                origin,
                label,
                kind,
            }) => Content::Field(FieldInfo {
                // An origin is compared and approved exactly, so one that holds a text kept out
                // is one the engine can't name, and nothing is filled on it.
                origin: origin.filter(|origin| !self.filled.holds(origin)),
                label: every.text(&label),
                kind,
            }),
            kept @ (Content::Nothing
            | Content::Image(_)
            | Content::Seen(_)
            | Content::AsksToSignIn(_)) => kept,
        };
        // A site asked about is read as a site, so one that would be changed is not said at all.
        let told = asked.len();
        let asked: Vec<String> = asked
            .into_iter()
            .filter(|site| !self.filled.holds(site))
            .collect();
        note = note.map(|note| every.text(&note));
        if asked.len() < told {
            note = Some(match note.take() {
                Some(note) => format!("{note}; {A_SITE_NOT_SAID}"),
                None => A_SITE_NOT_SAID.to_owned(),
            });
        }
        Answer {
            page: page.map(|page| self.page(page)),
            settled: match settled {
                Settled::Yes => Settled::Yes,
                Settled::No(waiting) => {
                    Settled::No(waiting.iter().map(|one| every.text(one)).collect())
                }
            },
            opened: opened.into_iter().map(|page| self.page(page)).collect(),
            content,
            note,
            repeated,
            asked,
            downloads: downloads
                .into_iter()
                .map(|download| {
                    let DownloadInfo {
                        id,
                        name,
                        origin,
                        received,
                        total,
                        state,
                        file,
                    } = download;
                    DownloadInfo {
                        id,
                        name: name_shown(&name, every),
                        origin: origin.map(|origin| every.text(&origin)),
                        received,
                        total,
                        state,
                        file,
                    }
                })
                .collect(),
        }
    }
}

/// A download's name as said once anything is kept out. A name as long as a name is cut to says
/// `download`: what it cut may have left part of a text, which no scrub knows.
fn name_shown(name: &str, scrub: &Scrub) -> String {
    if name.chars().count() >= MOST_FILE_NAME_CHARS {
        return plain_file_name("");
    }
    plain_file_name(&scrub.text(name))
}

/// What a refusal says, each string a page or the engine gave it with what is kept out taken out.
/// Its fixed words are purlis's, and what the chat wrote is said back as it wrote it (module docs).
fn refusal_kept_out(refusal: Refusal, scrub: &Scrub) -> Refusal {
    let text = |text: String| scrub.text(&text);
    match refusal {
        Refusal::KeptOff {
            site,
            asks_to_sign_in,
        } => Refusal::KeptOff {
            site: text(site),
            asks_to_sign_in,
        },
        Refusal::Partly { done, why } => Refusal::Partly {
            done: text(done),
            why: text(why),
        },
        Refusal::NoSuchOption {
            target,
            missing,
            options,
        } => Refusal::NoSuchOption {
            target,
            missing,
            options: text(options),
        },
        Refusal::Engine(why) => Refusal::Engine(text(why)),
        Refusal::Withheld(why) => Refusal::Withheld(text(why)),
        Refusal::NotLoggedIn(why) => Refusal::NotLoggedIn(text(why)),
        Refusal::NoFormOnCleanLoad(why) => Refusal::NoFormOnCleanLoad(text(why)),
        Refusal::NotFilled(why) => Refusal::NotFilled(text(why)),
        said_back @ (Refusal::NoSuchPage(_)
        | Refusal::NoSuchRef(_)
        | Refusal::StaleRef(_)
        | Refusal::NotShown(_)
        | Refusal::NotAFileInput(_)
        | Refusal::NotADropDown(_)
        | Refusal::OneOption(_)
        | Refusal::NotAKey(_)
        | Refusal::IdReused(_)
        | Refusal::HandedBack(_)) => said_back,
        fixed @ (Refusal::NoPage
        | Refusal::PageUntold
        | Refusal::FrameUnread
        | Refusal::FrameOpaque
        | Refusal::BlankTab
        | Refusal::NothingFocused
        | Refusal::FocusUnclear
        | Refusal::PageUnreadInTime
        | Refusal::PageReadTooLong
        | Refusal::PageReadPastItsLimit
        | Refusal::FocusUnseen
        | Refusal::NoEarlierPage
        | Refusal::EarlierNotWeb
        | Refusal::EarlierHoldsKept
        | Refusal::TooLarge
        | Refusal::Ended
        | Refusal::Unknown
        | Refusal::PersonDriving
        | Refusal::Interrupted
        | Refusal::NotDriving
        | Refusal::NotKeptOutYet
        | Refusal::PasteWithheld
        | Refusal::CopyWithheld
        | Refusal::KeysWithheld
        | Refusal::PurlisDriving
        | Refusal::Missed
        | Refusal::FieldUnread
        | Refusal::FieldUnreadAfterKey
        | Refusal::DragKeptOut) => fixed,
    }
}

impl Browser {
    /// What is kept out now, or `None` where nothing is.
    fn keeping(&self) -> Option<Keeping> {
        let (browser, pages): (Kept, Vec<(PageId, Kept)>) = {
            let state = lock(&self.inner.state);
            let pages = state
                .pages
                .iter()
                .map(|page| (page.id.clone(), page.scrubbed.clone()))
                .collect();
            (state.kept.clone(), pages)
        };
        // Worked out with no lock held: the engine's events wait on that one.
        let mut every = browser.clone();
        for (_, kept) in &pages {
            every.take_in(kept);
        }
        if every.is_empty() {
            return None;
        }
        Some(Keeping {
            every: Scrub::of(&every),
            filled: Scrub::of(&browser),
            browser,
            pages,
        })
    }

    /// **`outcome`, as the chat may read it** (module docs). Nothing the agent's calls answer
    /// leaves the browser but through this.
    pub(super) fn kept_out(&self, outcome: Result<Answer, Refusal>) -> Result<Answer, Refusal> {
        let Some(keeping) = self.keeping() else {
            return outcome;
        };
        match outcome {
            Ok(answer) => Ok(keeping.answer(answer)),
            Err(refusal) => Err(refusal_kept_out(refusal, &keeping.every)),
        }
    }

    /// `downloads`, each named as an answer would name it.
    pub(super) fn names_kept_out(&self, downloads: Vec<DownloadInfo>) -> Vec<DownloadInfo> {
        let Some(keeping) = self.keeping() else {
            return downloads;
        };
        downloads
            .into_iter()
            .map(|mut download| {
                download.name = name_shown(&download.name, &keeping.every);
                download
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
