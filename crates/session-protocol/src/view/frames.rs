// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser view's bytes: whole frames, one after another (1.6, #2012).
//!
//! **On the wire,** each frame is its length (a big-endian `u32`, counting what follows it), its
//! format (one byte: [`JPEG`]) and the image. A browser view's snapshot is the browser's latest
//! frame so framed, or nothing before it has made one, and its live bytes are the frames after
//! it. So a view attached again after it fell behind starts on a whole frame, and a reader handed
//! the stream in pieces of any size ([`Frames`]) tells where each frame ends.
//!
//! **An aimed frame** (1.14, [`AIMED_JPEG`]) says before its image what it shows: its page, as a
//! length byte and the page's id, and its documents, a big-endian `u64`. The person's input names
//! the frame it was aimed at by them ([`crate::browsing::Aim`]), so input meant for a page that
//! has since moved on is dropped rather than landing on another.
//!
//! **An origin frame** (1.35, #2300, [`ORIGIN`]) carries no image: the origin of the page the
//! browser shows (`https://host[:port]`, as UTF-8), or nothing where it shows none or a page
//! with no origin of its own. The host sends one where the origin changes and before the image
//! in a view's snapshot, cut from the engine's own record of the page
//! ([`crate::browsing::origin_of`]): never a path, a query, a fragment, a user or a title.
//!
//! **A closed frame** (1.44, #2028, [`CLOSED`]) is the last of a view of a browser's DevTools:
//! why they closed, as one of [`CLOSED_WHY`]'s words, which the window says in its own.
//!
//! A frame's image is opaque to the host and to this crate: the window decodes it on a canvas.
//! A reader reads a frame of a format it does not know whole, and passes it on for its caller
//! to skip, so a later minor may add a format.

use bytes::{Buf, Bytes, BytesMut};

use crate::browsing::Aim;

/// A JPEG image, as the engine's screencast makes it.
pub const JPEG: u8 = 0x01;

/// A JPEG image after what it shows (1.14): [`aimed`].
pub const AIMED_JPEG: u8 = 0x02;

/// The origin of the page a browser shows (1.35): [`origin`].
pub const ORIGIN: u8 = 0x03;

/// Why a browser's DevTools closed, the last frame of their view (1.44): [`closed`].
pub const CLOSED: u8 = 0x04;

/// The words a [`CLOSED`] frame says: the person handed the browser back, the page went,
/// DevTools fell behind, the page sent more than they show at once, they left their page, or
/// they ended.
pub const CLOSED_WHY: &[&str] = &[
    "handed_back",
    "page_gone",
    "fell_behind",
    "too_large",
    "moved",
    "ended",
];

/// The longest frame a reader takes, format byte included: far more than a screen's JPEG.
pub const MOST_FRAME_BYTES: usize = 16 << 20;

/// `image`, of `format`, as one frame on a browser view's stream.
pub fn frame(format: u8, image: &[u8]) -> Bytes {
    let length = u32::try_from(image.len() + 1).expect("a frame is under 4 GiB");
    let mut framed = BytesMut::with_capacity(5 + image.len());
    framed.extend_from_slice(&length.to_be_bytes());
    framed.extend_from_slice(&[format]);
    framed.extend_from_slice(image);
    framed.freeze()
}

/// `image`, a JPEG of what `aim` names, as one [`AIMED_JPEG`] frame. A page id longer than a
/// length byte holds is cut: ids are short (`p12`).
pub fn aimed(aim: &Aim, image: &[u8]) -> Bytes {
    let page = &aim.page.as_bytes()[..aim.page.len().min(usize::from(u8::MAX))];
    let mut body = Vec::with_capacity(1 + page.len() + 8 + image.len());
    body.push(u8::try_from(page.len()).expect("cut to a byte"));
    body.extend_from_slice(page);
    body.extend_from_slice(&aim.document.to_be_bytes());
    body.extend_from_slice(image);
    frame(AIMED_JPEG, &body)
}

/// `origin`, the origin of the page a browser shows, or none, as one [`ORIGIN`] frame.
pub fn origin(origin: Option<&str>) -> Bytes {
    frame(ORIGIN, origin.unwrap_or_default().as_bytes())
}

/// Why a browser's DevTools closed, one of [`CLOSED_WHY`], as one [`CLOSED`] frame.
pub fn closed(why: &str) -> Bytes {
    frame(CLOSED, why.as_bytes())
}

/// **A JPEG's size, width then height, from its first start-of-frame header**, read without
/// decoding it (#2050): what a window checks a frame from another host against before it draws
/// one. `None` for bytes that are not a JPEG, or whose header is cut short.
pub fn jpeg_size(jpeg: &[u8]) -> Option<(u16, u16)> {
    if jpeg.get(..2)? != [0xff, 0xd8] {
        return None;
    }
    let mut at = 2;
    loop {
        if *jpeg.get(at)? != 0xff {
            return None;
        }
        let marker = *jpeg.get(at + 1)?;
        let length = usize::from(u16::from_be_bytes([*jpeg.get(at + 2)?, *jpeg.get(at + 3)?]));
        // A start of frame of any coding: its height, then its width, after a precision byte.
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let height = u16::from_be_bytes([*jpeg.get(at + 5)?, *jpeg.get(at + 6)?]);
            let width = u16::from_be_bytes([*jpeg.get(at + 7)?, *jpeg.get(at + 8)?]);
            return Some((width, height));
        }
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
}

/// One frame read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub format: u8,
    pub image: Bytes,
}

impl Frame {
    /// What an [`AIMED_JPEG`] frame shows, and its JPEG; `None` for any other frame, or one cut
    /// short.
    pub fn aimed(&self) -> Option<(Aim, Bytes)> {
        if self.format != AIMED_JPEG {
            return None;
        }
        let body = &self.image;
        let length = usize::from(*body.first()?);
        let page = body.get(1..1 + length)?;
        let document = body.get(1 + length..1 + length + 8)?;
        let aim = Aim {
            page: String::from_utf8(page.to_vec()).ok()?,
            document: u64::from_be_bytes(document.try_into().ok()?),
        };
        Some((aim, body.slice(1 + length + 8..)))
    }

    /// What an [`ORIGIN`] frame says: the page's origin, or `Some(None)` for no page; `None` for
    /// any other frame, or one that is not text.
    pub fn origin(&self) -> Option<Option<String>> {
        if self.format != ORIGIN {
            return None;
        }
        let origin = std::str::from_utf8(&self.image).ok()?;
        Some((!origin.is_empty()).then(|| origin.to_owned()))
    }

    /// Why a [`CLOSED`] frame says DevTools closed: one of [`CLOSED_WHY`]; `None` for any other
    /// frame, or a word it does not know.
    pub fn closed(&self) -> Option<&'static str> {
        if self.format != CLOSED {
            return None;
        }
        let said = std::str::from_utf8(&self.image).ok()?;
        CLOSED_WHY.iter().copied().find(|why| *why == said)
    }
}

/// A frame said it was longer than [`MOST_FRAME_BYTES`]: the stream is not a browser view's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a frame of {0} bytes is longer than a browser view's frames are")]
pub struct TooLong(pub usize);

/// Reads whole frames out of a browser view's bytes, handed over in pieces of any size.
#[derive(Debug, Default)]
pub struct Frames {
    held: BytesMut,
}

impl Frames {
    /// Take `bytes`, and give every frame they complete, in order.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, TooLong> {
        self.held.extend_from_slice(bytes);
        let mut whole = Vec::new();
        while self.held.len() >= 4 {
            let length =
                u32::from_be_bytes([self.held[0], self.held[1], self.held[2], self.held[3]])
                    as usize;
            if length > MOST_FRAME_BYTES {
                self.held.clear();
                return Err(TooLong(length));
            }
            if self.held.len() < 4 + length {
                break;
            }
            self.held.advance(4);
            let mut body = self.held.split_to(length).freeze();
            if body.is_empty() {
                continue;
            }
            let format = body[0];
            body.advance(1);
            whole.push(Frame {
                format,
                image: body,
            });
        }
        Ok(whole)
    }

    /// Whether every byte taken so far belongs to a frame already given: the stream may end here.
    pub fn is_between_frames(&self) -> bool {
        self.held.is_empty()
    }
}
