// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser's downloads** (#2020), as the engine tells of them: each begins with the page's
//! suggested name and its address, and moves on until it is done or cancelled. Nothing here
//! touches a file: the engine writes each into the folder the browser was given, under a name
//! of its own (`allowAndName`), and purlis keeps account and says which is to be discarded for
//! passing the browser's limits.
//!
//! **Where a download comes from** is the origin of the address the engine names it by, which
//! is the last of its redirects (Chrome's `DownloadItem::GetURL`), never the first: a site's yes
//! is never stretched by its open redirect to a file of another. Only a download at no web
//! address (`blob:`, `data:`) is from the page that began it.

use crate::drive::{DownloadInfo, DownloadState, Limits, plain_file_name};

use super::login::origin_of;

/// A download to discard (#2020): its engine name, and whether it still downloads (it is then
/// cancelled first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Discard {
    pub(super) guid: String,
    pub(super) going: bool,
}

/// Every download a browser began, in order, and which moved on since they were last told.
#[derive(Default)]
pub(super) struct Downloads {
    all: Vec<DownloadInfo>,
    /// The ids of those that began or moved on since the last answer, in order.
    untold: Vec<String>,
    /// How many began past the most a browser makes, each cancelled and listed nowhere.
    refused: usize,
    /// The engine's names of those refused that it has not yet said ended: one it finishes
    /// before the cancel reaches it has its file removed then.
    refused_going: std::collections::HashSet<String>,
    /// How many of those were told.
    refused_told: usize,
}

impl Downloads {
    /// Download `guid` began at `address`, its page suggesting `suggested`, on a page whose
    /// document is at `page_address`. One past `limits`' most downloads is listed nowhere,
    /// only counted, and answered to be discarded. One the browser has already is not begun
    /// again.
    pub(super) fn began(
        &mut self,
        guid: &str,
        address: &str,
        suggested: &str,
        page_address: Option<&str>,
        limits: &Limits,
    ) -> Option<Discard> {
        if self.all.iter().any(|one| one.file == guid) {
            return None;
        }
        if self.all.len() >= limits.most_downloads {
            self.refused = self.refused.saturating_add(1);
            self.refused_going.insert(guid.to_owned());
            return Some(Discard {
                guid: guid.to_owned(),
                going: true,
            });
        }
        let own = address.strip_prefix("blob:").unwrap_or(address);
        let origin = origin_of(own).or_else(|| page_address.and_then(origin_of));
        let id = format!("d{}", self.all.len() + 1);
        self.all.push(DownloadInfo {
            id: id.clone(),
            name: plain_file_name(suggested),
            origin,
            received: 0,
            total: None,
            state: DownloadState::Going,
            file: guid.to_owned(),
        });
        self.untell(&id);
        None
    }

    /// Download `guid` came to `received` bytes, of `total` where that is not 0, and is now in
    /// the engine's `state`. Answers it to be discarded as it first passes `limits`' most bytes,
    /// for itself or with the browser's others. What purlis discarded stays said so, and is
    /// answered to be discarded again should the engine still finish it, as is one refused as it
    /// began.
    pub(super) fn moved(
        &mut self,
        guid: &str,
        received: u64,
        total: u64,
        state: &str,
        limits: &Limits,
    ) -> Option<Discard> {
        if self.refused_going.contains(guid) {
            if state == "inProgress" {
                return None;
            }
            self.refused_going.remove(guid);
            return (state == "completed").then(|| Discard {
                guid: guid.to_owned(),
                going: false,
            });
        }
        let others: u64 = self
            .all
            .iter()
            .filter(|one| one.file != guid)
            .map(|one| one.received)
            .fold(0, u64::saturating_add);
        let one = self.all.iter_mut().find(|one| one.file == guid)?;
        one.received = received;
        let held = others.saturating_add(received);
        one.total = (total > 0).then_some(total);
        let discard = match one.state {
            // Cut already; but the engine may finish it before the cancel reaches it, and the
            // file it then writes goes too.
            DownloadState::TooLarge => (state == "completed").then(|| Discard {
                guid: guid.to_owned(),
                going: false,
            }),
            _ if received > limits.most_download_bytes
                || held > limits.most_held_bytes
                || one
                    .total
                    .is_some_and(|total| total > limits.most_download_bytes) =>
            {
                one.state = DownloadState::TooLarge;
                Some(Discard {
                    guid: guid.to_owned(),
                    going: state == "inProgress",
                })
            }
            _ => {
                one.state = match state {
                    "completed" => DownloadState::Done,
                    "canceled" => DownloadState::Cancelled,
                    _ => DownloadState::Going,
                };
                None
            }
        };
        let id = one.id.clone();
        self.untell(&id);
        discard
    }

    /// Those that began or moved on since this was last asked, each once.
    pub(super) fn untold(&mut self) -> Vec<DownloadInfo> {
        let ids = std::mem::take(&mut self.untold);
        ids.iter()
            .filter_map(|id| self.all.iter().find(|one| &one.id == id).cloned())
            .collect()
    }

    /// How many began past the most since this was last asked, where any did.
    pub(super) fn refused_since(&mut self) -> Option<usize> {
        let since = self.refused - self.refused_told;
        self.refused_told = self.refused;
        (since > 0).then_some(since)
    }

    /// Every one.
    pub(super) fn all(&self) -> Vec<DownloadInfo> {
        self.all.clone()
    }

    fn untell(&mut self, id: &str) {
        if !self.untold.iter().any(|one| one == id) {
            self.untold.push(id.to_owned());
        }
    }
}
