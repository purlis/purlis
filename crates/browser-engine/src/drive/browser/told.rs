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
//! typed on one page takes no ordinary words out of another. A window takes on what the page that
//! opened it keeps, and what a page kept outlives it, in what is said beside the pages.
//!
//! **A site is said only where its name holds nothing purlis filled**, or only what went to that
//! very site, or a site under it: such a name says nothing of the text that the site's own name
//! did not. What the person typed is often part of a site's name, and doesn't hide it.
//!
//! **What the chat wrote itself is said back as it wrote it** (a reference, a page's name, a key,
//! an option). It holds nothing the chat did not have, and an echo with a guess blanked would tell
//! the chat its guess was right, in one call and with no page, so a refusal's echoes are left
//! alone (D-5115-13).

use super::super::kept::Kept;
use super::super::scrub::Scrub;
use super::super::{
    Answer, Content, DownloadInfo, FieldInfo, KeptOff, MOST_FILE_NAME_CHARS, PageId, PageInfo,
    Refusal, Settled, lock, plain_file_name,
};
use super::{Browser, State, login};

/// What a call says of a site it held back whose address holds a text kept out: that site is not
/// said, and so nobody is asked about it.
const A_SITE_NOT_SAID: &str = "a request went nowhere because its address holds a value purlis \
                               keeps out of what you read, so nobody is asked about it";

/// What a chat is told where the page an act answered with is on `site`, the site its browser's
/// login signs in on (#5152).
fn on_kept_off_said(site: &str) -> String {
    format!(
        "this page is on {site}, the site this browser signs in on: you may not read or act on it \
         outside a sign-in. Go back, or navigate to the login's own site; if it asks to sign in \
         again, call browser_login"
    )
}

/// **`answer`, with each page on the site the agent is kept off said by its origin alone, and no
/// title** (#5152), and the chat told where the page it acted on is one. Judged by each address as
/// it is: taken out of first, an address may no longer be on the site it is on.
fn kept_off_told(mut answer: Answer, kept_off: &KeptOff) -> Answer {
    let on = answer
        .page
        .as_ref()
        .and_then(|page| kept_off(&page.address));
    let listed = match &mut answer.content {
        Content::Pages(pages) => pages.as_mut_slice(),
        _ => &mut [],
    };
    for page in answer
        .page
        .iter_mut()
        .chain(answer.opened.iter_mut())
        .chain(listed)
    {
        if kept_off(&page.address).is_some() {
            page.address = login::origin_of(&page.address).unwrap_or_default();
            page.title.clear();
        }
    }
    if let Some(site) = on {
        let said = on_kept_off_said(&site);
        answer.note = Some(match answer.note.take() {
            Some(note) => format!("{note}; {said}"),
            None => said,
        });
    }
    answer
}

/// What is kept out, as a call is answered: of the whole browser, and of each page.
struct Keeping {
    every: Scrub,
    /// What purlis filled: what a site's name is held to (module docs).
    filled: Kept,
    /// Each text of `filled` that the name of the site it went to holds, with that site's host.
    named_by_their_sites: Vec<(zeroize::Zeroizing<String>, String)>,
    browser: Kept,
    pages: Vec<(PageId, Kept)>,
}

/// The host of `site`, a site asked about (`host:port`) or an origin (`https://host[:port]`), in
/// lower case.
fn host_of(site: &str) -> String {
    let rest = site.split_once("://").map_or(site, |(_, rest)| rest);
    rest.rsplit_once(':')
        .map_or(rest, |(host, _)| host)
        .to_ascii_lowercase()
}

impl State {
    /// **What purlis fills on `origin` is kept out from now** ([`State::keep_out_everywhere`]), and
    /// is what a site's name is held to (module docs).
    pub(super) fn keep_filled_out(&mut self, held: &Kept, origin: &str) {
        self.keep_out_everywhere(held);
        self.filled.take_in(held);
        let host = host_of(origin);
        for text in held.texts() {
            let text = zeroize::Zeroizing::new(text.to_lowercase());
            if host.contains(text.as_str())
                && !self
                    .named_by_their_sites
                    .iter()
                    .any(|(one, on)| *one == text && *on == host)
            {
                self.named_by_their_sites.push((text, host.clone()));
            }
        }
    }
}

impl Keeping {
    /// Whether `site`'s name holds something purlis filled that its own name doesn't account for
    /// (module docs).
    fn holds_back(&self, site: &str) -> bool {
        let host = host_of(site);
        let own: Vec<&str> = self
            .named_by_their_sites
            .iter()
            .filter(|(_, on)| false && (host == *on || host.ends_with(&format!(".{on}"))))
            .map(|(text, _)| text.as_str())
            .collect();
        Scrub::of(&self.filled.without(&own)).holds(site)
    }

    /// What is said of `page`: its own texts, or where it has gone, everything kept.
    fn of_page(&self, page: &PageId) -> Scrub {
        self.pages
            .iter()
            .find(|(id, _)| id == page)
            .map_or_else(|| self.every.clone(), |(_, kept)| Scrub::of(kept))
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
        let field_origin = match &content {
            Content::Field(field) => field.origin.clone().filter(|one| !self.holds_back(one)),
            _ => None,
        };
        // Read by the window alone, which compares the page's origin with the field's.
        let on_field_origin = page.as_ref().is_some_and(|page| {
            field_origin.is_some() && login::origin_of(&page.address) == field_origin
        });
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
                // An origin is compared and approved exactly, so one that holds what purlis
                // filled, past what its own name accounts for, is one the engine can't name, and
                // nothing is filled on it.
                origin: origin.filter(|_| field_origin.is_some()),
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
            .filter(|site| !self.holds_back(site))
            .collect();
        note = note.map(|note| every.text(&note));
        if asked.len() < told {
            note = Some(match note.take() {
                Some(note) => format!("{note}; {A_SITE_NOT_SAID}"),
                None => A_SITE_NOT_SAID.to_owned(),
            });
        }
        Answer {
            page: page.map(|page| {
                let mut said = self.page(page);
                if on_field_origin && let Some(origin) = &field_origin {
                    said.address.clone_from(origin);
                }
                said
            }),
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
    /// [`kept_off_told`], where this browser keeps the agent off a site.
    pub(super) fn with_kept_off_told(&self, answer: Answer) -> Answer {
        match lock(&self.inner.kept_off).clone() {
            Some(kept_off) => kept_off_told(answer, &kept_off),
            None => answer,
        }
    }

    /// What is kept out now, or `None` where nothing is.
    fn keeping(&self) -> Option<Keeping> {
        let (browser, pages, filled, named_by_their_sites, closed) = {
            let state = lock(&self.inner.state);
            let pages: Vec<(PageId, Kept)> = state
                .pages
                .iter()
                .map(|page| (page.id.clone(), page.scrubbed.clone()))
                .collect();
            (
                state.kept.clone(),
                pages,
                state.filled.clone(),
                state.named_by_their_sites.clone(),
                state.closed_kept.clone(),
            )
        };
        // Worked out with no lock held: the engine's events wait on that one.
        let mut every = browser.clone();
        every.take_in(&closed);
        for (_, kept) in &pages {
            every.take_in(kept);
        }
        if every.is_empty() {
            return None;
        }
        Some(Keeping {
            every: Scrub::of(&every),
            filled,
            named_by_their_sites,
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
