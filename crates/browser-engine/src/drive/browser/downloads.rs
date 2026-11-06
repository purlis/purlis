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
//! address (`blob:`, `data:`) is from the document that began it: its frame's own web address,
//! or, for a blank or srcdoc frame, the nearest document around it that has an origin. One begun
//! in a sandboxed frame, or one whose document is not known yet, is from nowhere known.

use std::io::Read;

use zeroize::Zeroizing;

use crate::drive::kept::KEPT_WHOLE_FROM;
use crate::drive::scrub::every_form;
use crate::drive::{DownloadHolds, DownloadInfo, DownloadState, Limits, lock, plain_file_name};

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

/// How much of a download is read at a time, looking for a filled value in it.
const READ_AT_A_TIME: usize = 1 << 20;

impl super::Browser {
    /// **What the file at `file` holds of what this browser keeps out of a chat's reads**: a value
    /// purlis filled into it (#5115), or a text of [`KEPT_WHOLE_FROM`] characters or more the
    /// person typed in it (the person's ruling of 2026-11-06), in any of the forms a string a
    /// chat reads has it taken out in ([`every_form`]), whatever its case: a download is brought
    /// into a chat's folder only where it holds neither. Read once, in pieces, so a large
    /// download is never held whole.
    ///
    /// # Errors
    ///
    /// Where the file can't be read.
    pub fn holds_kept_out(&self, file: &std::path::Path) -> std::io::Result<DownloadHolds> {
        let (filled, every) = {
            let state = lock(&self.inner.state);
            (state.filled.clone(), state.every_kept())
        };
        let typed = every
            .at_least(KEPT_WHOLE_FROM)
            .without(&filled.texts());
        let forms = [every_form(&filled), every_form(&typed)];
        if forms.iter().all(Vec::is_empty) {
            return Ok(DownloadHolds::Nothing);
        }
        Ok(
            match holds_a_form(std::fs::File::open(file)?, &forms)? {
                Some(0) => DownloadHolds::Filled,
                Some(_) => DownloadHolds::Typed,
                None => DownloadHolds::Nothing,
            },
        )
    }
}

/// Which of `sets` of forms what `from` reads holds one of first, whatever its case: read
/// [`READ_AT_A_TIME`] bytes at a time, each piece with the end of the one before, so a form
/// across two pieces is found. An earlier set is looked for first in each piece.
fn holds_a_form(
    mut from: impl Read,
    sets: &[Vec<Zeroizing<String>>],
) -> std::io::Result<Option<usize>> {
    let needles: Vec<Vec<Zeroizing<String>>> = sets
        .iter()
        .map(|forms| {
            forms
                .iter()
                .map(|form| Zeroizing::new(form.to_lowercase()))
                .collect()
        })
        .collect();
    // A form's bytes, and a character cut at the piece's start.
    let carried = needles
        .iter()
        .flatten()
        .map(|one| one.len())
        .max()
        .unwrap_or(0)
        + 4;
    let mut piece: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(READ_AT_A_TIME));
    let mut read = Zeroizing::new(vec![0u8; READ_AT_A_TIME]);
    loop {
        let got = match from.read(&mut read) {
            Ok(got) => got,
            Err(why) if why.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(why) => return Err(why),
        };
        if got == 0 {
            return Ok(None);
        }
        piece.extend_from_slice(&read[..got]);
        let text = Zeroizing::new(String::from_utf8_lossy(&piece).to_lowercase());
        if let Some(set) = needles
            .iter()
            .position(|set| set.iter().any(|needle| text.contains(needle.as_str())))
        {
            return Ok(Some(set));
        }
        let keep = piece.len().min(carried);
        let from_here = piece.len() - keep;
        piece.drain(..from_here);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forms(of: &str) -> Vec<Zeroizing<String>> {
        let mut kept = crate::drive::kept::Kept::default();
        kept.run(&[of]);
        every_form(&kept)
    }

    fn holds(bytes: &[u8], of: &str) -> bool {
        holds_a_form(bytes, &[forms(of)]).unwrap().is_some()
    }

    #[test]
    fn a_file_holding_a_filled_value_in_any_case_or_in_base64_or_hex_is_found() {
        assert!(holds(b"API_KEY=Open-Sesame-42\n", "open-sesame-42"));
        assert!(holds(b"token: b3Blbi1zZXNhbWUtNDI=", "open-sesame-42"));
        assert!(holds(b"6f70656e2d736573616d652d3432", "open-sesame-42"));
        assert!(holds(b"\x00\xff\xfeopen-sesame-42\x00", "open-sesame-42"));
    }

    #[test]
    fn a_file_holding_only_a_part_of_a_filled_value_is_not() {
        assert!(!holds(b"open-sesame-4 and sesame-42", "open-sesame-42"));
        assert!(!holds(b"", "open-sesame-42"));
    }

    #[test]
    fn a_filled_value_is_said_before_a_typed_text_in_the_same_file() {
        let both = [forms("open-sesame-42"), forms("plumtreewalker")];
        let found = |bytes: &[u8]| holds_a_form(bytes, &both).unwrap();
        assert_eq!(found(b"plumtreewalker open-sesame-42"), Some(0));
        assert_eq!(found(b"plumtreewalker"), Some(1));
        assert_eq!(found(b"neither"), None);
    }

    #[test]
    fn a_value_across_two_pieces_of_the_file_is_found() {
        let mut bytes = vec![b'x'; READ_AT_A_TIME - 5];
        bytes.extend_from_slice(b"open-sesame-42");
        bytes.extend(vec![b'y'; 100]);
        assert!(holds(&bytes, "open-sesame-42"));
        // And a marked letter cut at the edge of a piece.
        let mut marked = vec![b'x'; READ_AT_A_TIME - 1];
        marked.extend_from_slice("ÿes-sésame-42".as_bytes());
        assert!(holds(&marked, "ÿes-sésame-42"));
    }
}
