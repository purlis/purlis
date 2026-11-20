// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Git's smart HTTP protocol: which of its three requests this is, and, for a push, which refs it
//! asks to move.

use super::request::Target;

/// The three requests of git over HTTP. The dumb protocol (`HEAD`, `objects/…`) and LFS are not
/// on the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitOp {
    /// `GET …/info/refs?service=git-upload-pack` or `git-receive-pack`: the ref advertisement.
    Advertise,
    /// `POST …/git-upload-pack`: a fetch or clone.
    UploadPack,
    /// `POST …/git-receive-pack`: a push.
    ReceivePack,
}

/// The repo's path segments and the operation, if `segments` is a smart-HTTP request. The last
/// repo segment may carry `.git`, which is left for the caller to strip.
pub(crate) fn classify(
    method: &str,
    segments: &[String],
    target: &Target,
) -> Option<(Vec<String>, GitOp)> {
    let n = segments.len();
    let (op, tail) = match segments {
        [.., a, b] if a == "info" && b == "refs" && method == "GET" => {
            match target.query_value("service") {
                Some("git-upload-pack" | "git-receive-pack") => (GitOp::Advertise, 2),
                _ => return None,
            }
        }
        [.., last] if last == "git-upload-pack" && method == "POST" => (GitOp::UploadPack, 1),
        [.., last] if last == "git-receive-pack" && method == "POST" => (GitOp::ReceivePack, 1),
        _ => return None,
    };
    let repo = &segments[..n - tail];
    if repo.is_empty() || repo.iter().any(|s| s.contains('/')) {
        return None;
    }
    Some((repo.to_vec(), op))
}

/// Why a push's commands could not be read. Both refuse the push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PushError {
    /// Not pkt-lines, not complete up to the flush, a malformed command, or no command at all.
    Unreadable,
    /// A signed push: its commands are inside the certificate, which is not read.
    PushCert,
}

/// The refs a `git-receive-pack` request asks to move, from the pkt-lines of its command list
/// (everything up to the first flush packet; the pack follows it and is not read).
pub(crate) fn pushed_refs(body: &[u8]) -> Result<Vec<String>, PushError> {
    let mut at = 0;
    let mut refs = Vec::new();
    loop {
        let len_hex = body.get(at..at + 4).ok_or(PushError::Unreadable)?;
        let len = std::str::from_utf8(len_hex)
            .ok()
            .filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|s| usize::from_str_radix(s, 16).ok())
            .ok_or(PushError::Unreadable)?;
        if len == 0 {
            break;
        }
        if len < 4 {
            return Err(PushError::Unreadable);
        }
        let payload = body.get(at + 4..at + len).ok_or(PushError::Unreadable)?;
        at += len;
        // Capabilities follow a NUL on the first command.
        let line = payload.split(|b| *b == 0).next().unwrap_or_default();
        let line = std::str::from_utf8(line).map_err(|_| PushError::Unreadable)?;
        let line = line.strip_suffix('\n').unwrap_or(line);
        if line.starts_with("push-cert") {
            return Err(PushError::PushCert);
        }
        if line.starts_with("shallow ") {
            continue;
        }
        let mut parts = line.split(' ');
        let (Some(old), Some(new), Some(name), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(PushError::Unreadable);
        };
        if !is_object_id(old) || !is_object_id(new) || name.is_empty() {
            return Err(PushError::Unreadable);
        }
        refs.push(name.to_string());
    }
    if refs.is_empty() {
        Err(PushError::Unreadable)
    } else {
        Ok(refs)
    }
}

fn is_object_id(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}
