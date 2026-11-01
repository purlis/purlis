// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Code over the link** (1.34, #2046, ADR 0078 §7): a repository's history moves between the
//! desktop and a runner as a git bundle, git's own file of a pack and the refs it carries, on a
//! stream of the link of its own.
//!
//! **The host opens the stream, as it opens a view.** A client asks with `take_code` (the client
//! writes a bundle into the runner's repository) or `give_code` (the host writes a bundle of its
//! branches back), naming a transfer number of its own; the host opens a stream that starts with
//! [`Header`], [`CODE`] and that number, so the client knows which of its transfers it is. Then
//! the bundle's bytes follow, in the direction the command says, and the stream is shut for
//! writing once they are all there. A relaying host joins the stream to one on its client's link
//! under the same header, byte for byte both ways, as it joins a view.
//!
//! **A bundle is held to [`MOST_BUNDLE_BYTES`]** on either end: one larger is not read past that,
//! and the transfer is refused in a sentence.
//!
//! **What a bundle may carry is the host's to check, never the stream's.** This module says how
//! the bytes move; which refs a host takes from a bundle, and where, is the binary's (`purlis
//! serve`'s, from `purlis_core::codelink`), and so is every git call.

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::link::LinkError;

/// What opens a code stream (1.34). A view's opens with [`crate::view::VIEW`].
pub const CODE: u8 = 0x02;

/// The most bytes one bundle may be (1.34): a gibibyte, far past a repository a chat works in,
/// and small enough that a transfer cut off at it says so rather than filling a disk.
pub const MOST_BUNDLE_BYTES: u64 = 1024 * 1024 * 1024;

/// **The most bytes a client may hold a bundle it sends to** (`CodeHeld`, 1.65, spec #4275): a
/// host takes a higher ask at this. A bundle goes to a file on the host's disk, so this is what
/// one send may ask of that disk.
pub const HIGHEST_BUNDLE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// **The longest a client may give a transfer it sends** (`CodeHeld`, 1.65, spec #4275): half a
/// day, so a client that asks more has a host give up all the same.
pub const LONGEST_TRANSFER_SECS: u64 = 12 * 60 * 60;

/// What a code stream opens with: [`CODE`], then the client's number for the transfer, a
/// big-endian `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// The number the client named the transfer by in its `take_code` or `give_code`.
    pub transfer: u64,
}

impl Header {
    /// How long a header is on the wire.
    pub const LEN: usize = 9;

    /// The header as it is written.
    pub fn to_bytes(&self) -> [u8; Header::LEN] {
        let mut bytes = [0u8; Header::LEN];
        bytes[0] = CODE;
        bytes[1..].copy_from_slice(&self.transfer.to_be_bytes());
        bytes
    }

    /// The rest of a code stream's header, once its first byte, [`CODE`], has been read.
    pub async fn read_after_kind<R: AsyncRead + Unpin>(input: &mut R) -> Result<Header, LinkError> {
        Ok(Header {
            transfer: input.read_u64().await?,
        })
    }
}

/// Why a bundle could not be read whole.
#[derive(Debug, thiserror::Error)]
pub enum Unread {
    /// It is larger than [`MOST_BUNDLE_BYTES`], or than the bound the reader was given.
    #[error(
        "the bundle is larger than {most} bytes, the most one transfer carries, so it was not \
         taken"
    )]
    TooLarge { most: u64 },
    #[error("the bundle's stream broke: {0}")]
    Io(#[from] std::io::Error),
}

/// Copy a bundle from `input` into `output` until `input` ends, refusing it once it passes
/// `most` bytes. Answers how many bytes it copied.
pub async fn copy_held_to<R, W>(input: &mut R, output: &mut W, most: u64) -> Result<u64, Unread>
where
    R: AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // One byte past the bound is read, so a bundle exactly at it is taken and one past it is not.
    let mut held = input.take(most.saturating_add(1));
    let copied = tokio::io::copy(&mut held, output).await?;
    if copied > most {
        return Err(Unread::TooLarge { most });
    }
    Ok(copied)
}

#[cfg(test)]
mod tests;
