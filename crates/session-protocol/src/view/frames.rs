// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser view's bytes: whole frames, one after another (1.5, #2012).
//!
//! **On the wire,** each frame is its length (a big-endian `u32`, counting what follows it), its
//! format (one byte: [`JPEG`]) and the image. A browser view's snapshot is the browser's latest
//! frame so framed, or nothing before it has made one, and its live bytes are the frames after
//! it. So a view attached again after it fell behind starts on a whole frame, and a reader handed
//! the stream in pieces of any size ([`Frames`]) tells where each frame ends.
//!
//! A frame's image is opaque to the host and to this crate: the window decodes it on a canvas.
//! A reader reads a frame of a format it does not know whole, and passes it on for its caller
//! to skip, so a later minor may add a format.

use bytes::{Buf, Bytes, BytesMut};

/// A JPEG image, as the engine's screencast makes it.
pub const JPEG: u8 = 0x01;

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

/// One frame read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub format: u8,
    pub image: Bytes,
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
