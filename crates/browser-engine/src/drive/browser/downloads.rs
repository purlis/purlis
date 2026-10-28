// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser's downloads** (#2020), as the engine tells of them: each begins with the page's
//! suggested name and its address, and moves on until it is done or cancelled. Nothing here
//! touches a file: the engine writes each into the folder the browser was given, under a name
//! of its own (`allowAndName`), and purlis only keeps account and says when one is to be
//! cancelled for passing the browser's limits.

use crate::drive::{DownloadInfo, DownloadState, Limits, plain_file_name};

use super::login::origin_of;

/// Every download a browser began, in order, and which moved on since they were last told.
#[derive(Default)]
pub(super) struct Downloads {
    all: Vec<DownloadInfo>,
    /// The ids of those that began or moved on since the last answer, in order.
    untold: Vec<String>,
}

impl Downloads {
    /// Download `guid` began at `address`, its page suggesting `suggested`, on a page whose
    /// document is at `page_address`. Answers the guid to cancel where it is one past
    /// `limits`' most downloads. One the browser has already is not begun again.
    pub(super) fn began(
        &mut self,
        guid: &str,
        address: &str,
        suggested: &str,
        page_address: Option<&str>,
        limits: &Limits,
    ) -> Option<String> {
        if self.all.iter().any(|one| one.file == guid) {
            return None;
        }
        let own = address.strip_prefix("blob:").unwrap_or(address);
        let origin = origin_of(own).or_else(|| page_address.and_then(origin_of));
        let past = self.all.len() >= limits.most_downloads;
        let id = format!("d{}", self.all.len() + 1);
        self.all.push(DownloadInfo {
            id: id.clone(),
            name: plain_file_name(suggested),
            origin,
            received: 0,
            total: None,
            state: if past {
                DownloadState::TooMany
            } else {
                DownloadState::Going
            },
            file: guid.to_owned(),
        });
        self.untell(&id);
        past.then(|| guid.to_owned())
    }

    /// Download `guid` came to `received` bytes, of `total` where that is not 0, and is now in
    /// the engine's `state`. Answers the guid to cancel where it passed `limits`' most bytes.
    /// What purlis cancelled stays said as purlis cancelled it.
    pub(super) fn moved(
        &mut self,
        guid: &str,
        received: u64,
        total: u64,
        state: &str,
        limits: &Limits,
    ) -> Option<String> {
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
        let cancel = match one.state {
            DownloadState::TooLarge | DownloadState::TooMany => false,
            _ if received > limits.most_download_bytes
                || held > limits.most_held_bytes
                || one
                    .total
                    .is_some_and(|total| total > limits.most_download_bytes) =>
            {
                one.state = DownloadState::TooLarge;
                state == "inProgress"
            }
            _ => {
                one.state = match state {
                    "completed" => DownloadState::Done,
                    "canceled" => DownloadState::Cancelled,
                    _ => DownloadState::Going,
                };
                false
            }
        };
        let id = one.id.clone();
        self.untell(&id);
        cancel.then(|| guid.to_owned())
    }

    /// Those that began or moved on since this was last asked, each once.
    pub(super) fn untold(&mut self) -> Vec<DownloadInfo> {
        let ids = std::mem::take(&mut self.untold);
        ids.iter()
            .filter_map(|id| self.all.iter().find(|one| &one.id == id).cloned())
            .collect()
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
