// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A refused git request, answered in git's own words** (ADR 0128, amended 2026-11-23).
//!
//! A `403` from the route reads to git as `RPC failed; HTTP 403 … the remote end hung up
//! unexpectedly`, which looks like a fault at the forge. Git has two standard ways for a server
//! to say *no, and why*, and the listener uses both, so the person reads purlis's reason in git's
//! own output:
//!
//! - **A refused push** (`POST …/git-receive-pack`) is answered `200` with a well-formed
//!   *report-status*: `unpack ok` and one `ng <ref> <reason>` for each ref the push named, in the
//!   side-band the client asked for. Git prints `! [remote rejected] <branch> (<reason>)`.
//! - **A refused advertisement** (`GET …/info/refs?service=…`) is answered `200` with the
//!   service banner and one `ERR <reason>` pkt-line, which git prints as `fatal: remote error:
//!   <reason>`.
//!
//! **Nothing here lets a request through.** It runs only after the core refused, writes only what
//! it builds, and never reads the forge. A reason is purlis's own sentence for the refusal, a
//! fixed one per kind, or the sentence a held act or a pin already tells the chat (which may
//! name the branch, tag or repo it is about, and a forge's own error): **never a token, the
//! sentinel or a path**. It is one short line (control
//! characters made spaces, cut at [`MOST_REASON`]). A push whose commands cannot be read, or a
//! request that is not one of these two, is answered as it was, `403`.

use std::io::{self, Write};

use super::super::Refusal;
use super::http::write_head;
use crate::forge::proxy::git;

/// The most bytes of a reason that is sent: git prints it on one line.
const MOST_REASON: usize = 200;

/// The data bytes of one side-band packet: `side-band` takes 999 with its channel byte and length,
/// `side-band-64k` 65519. The smaller is used for both; a report is a few lines.
const BAND_CHUNK: usize = 900;

/// What git asked for, as far as an answer in its words needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Call<'a> {
    /// `GET …/info/refs?service=<service>`.
    Advertise(&'a str),
    /// `POST …/git-receive-pack`.
    ReceivePack,
}

/// Which of the two requests `method` and `target` are, from the request line alone.
pub(crate) fn call_of<'a>(method: &str, target: &'a str) -> Option<Call<'a>> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if method.eq_ignore_ascii_case("GET") && path.ends_with("/info/refs") {
        return match query {
            "service=git-receive-pack" => Some(Call::Advertise("git-receive-pack")),
            "service=git-upload-pack" => Some(Call::Advertise("git-upload-pack")),
            _ => None,
        };
    }
    (method.eq_ignore_ascii_case("POST") && path.ends_with("/git-receive-pack"))
        .then_some(Call::ReceivePack)
}

/// purlis's sentence for a refusal of a git request: short, plain, and what to do. Never any text
/// of the request.
pub(crate) fn reason_for(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::ProtectedRef => {
            "purlis: a chat pushes only a branch it made, from the tip it last pushed; ask the person to push this one"
        }
        Refusal::NotYourBranch => {
            "purlis: this branch is not one this chat made, and no ask was raised to let it push (it is protected or unlisted, the chat is closed, or the policy does not ask); ask the person to push it"
        }
        Refusal::ReleaseOrTag => "purlis: a chat does not push tags or releases; ask the person",
        Refusal::PushOption => "purlis: push options are not carried; push without them",
        Refusal::NoAdvertisement => {
            "purlis: the forge's branches were not read yet, so a new branch is not made; fetch, then push"
        }
        Refusal::OtherRepo | Refusal::HostNotAllowed | Refusal::AmbiguousHost => {
            "purlis: this repo is not one of this chat's"
        }
        Refusal::NoCredential => {
            "purlis: no sign-in for this repo; sign in under Settings › You › Accounts"
        }
        Refusal::Pinned => "purlis: an administrator turned this forge off for chats",
        _ => "purlis: a chat may not make this request",
    }
}

/// A sentence purlis already told the chat (a pin's, a held act's), as one short line.
pub(crate) fn one_line(said: &str) -> String {
    let flat: String = said
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut end = flat.len().min(MOST_REASON);
    while !flat.is_char_boundary(end) {
        end -= 1;
    }
    flat[..end].to_owned()
}

/// The longest ref name repeated in a report: git itself refuses far shorter, and a pkt-line's
/// length is four hex digits.
const MOST_REF: usize = 4096;

/// One pkt-line holding `data`.
fn pkt(out: &mut Vec<u8>, data: &[u8]) {
    out.extend(format!("{:04x}", data.len() + 4).bytes());
    out.extend(data);
}

/// Answers `call` with `reason` in git's words, and says whether it did. `false` leaves the
/// answer to the caller (a push whose commands do not read is not answered here).
pub(crate) fn answer<W: Write>(
    out: &mut W,
    call: &Call<'_>,
    body: &[u8],
    reason: &str,
) -> io::Result<bool> {
    let reason = one_line(reason);
    let (kind, bytes) = match call {
        Call::Advertise(service) => {
            let mut bytes = Vec::new();
            pkt(&mut bytes, format!("# service={service}\n").as_bytes());
            bytes.extend(b"0000");
            pkt(&mut bytes, format!("ERR {reason}\n").as_bytes());
            (format!("application/x-{service}-advertisement"), bytes)
        }
        Call::ReceivePack => {
            let Some(bytes) = report_status(body, &reason) else {
                return Ok(false);
            };
            ("application/x-git-receive-pack-result".to_owned(), bytes)
        }
    };
    let headers = vec![
        ("Content-Type".to_owned(), kind),
        ("Content-Length".to_owned(), bytes.len().to_string()),
        ("Cache-Control".to_owned(), "no-cache".to_owned()),
    ];
    write_head(out, 200, &headers)?;
    out.write_all(&bytes)?;
    out.flush()?;
    Ok(true)
}

/// The report-status refusing every ref of the push `body`, in the side-band its client asked
/// for; `None` where the commands do not read.
fn report_status(body: &[u8], reason: &str) -> Option<Vec<u8>> {
    let pushed = git::pushed_commands(body).ok()?;
    let mut report = Vec::new();
    pkt(&mut report, b"unpack ok\n");
    for command in &pushed.commands {
        // A ref name is the client's, and may hold anything: only a well-formed one is repeated.
        let name = &command.name;
        // Printable ASCII only, and short enough that the line's four-digit length holds it.
        if name.len() > MOST_REF || !name.bytes().all(|b| b.is_ascii_graphic()) {
            return None;
        }
        pkt(&mut report, format!("ng {name} {reason}\n").as_bytes());
    }
    report.extend(b"0000");
    if !asks_for_side_band(body) {
        return Some(report);
    }
    let mut wrapped = Vec::new();
    for chunk in report.chunks(BAND_CHUNK) {
        let mut data = vec![1u8];
        data.extend(chunk);
        pkt(&mut wrapped, &data);
    }
    wrapped.extend(b"0000");
    Some(wrapped)
}

/// Whether the first command of the push asks for either side-band capability.
fn asks_for_side_band(body: &[u8]) -> bool {
    let Some(len) = body
        .get(..4)
        .and_then(|hex| std::str::from_utf8(hex).ok())
        .and_then(|hex| usize::from_str_radix(hex, 16).ok())
        .filter(|len| *len >= 4)
    else {
        return false;
    };
    let Some(first) = body.get(4..len) else {
        return false;
    };
    let mut parts = first.splitn(2, |b| *b == 0);
    let _command = parts.next();
    parts
        .next()
        .and_then(|caps| std::str::from_utf8(caps).ok())
        .is_some_and(|caps| {
            caps.split_ascii_whitespace()
                .any(|c| c == "side-band" || c == "side-band-64k")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "0000000000000000000000000000000000000000";
    const NEW: &str = "1111111111111111111111111111111111111111";

    fn push(caps: &str, refs: &[&str]) -> Vec<u8> {
        let mut body = Vec::new();
        for (i, name) in refs.iter().enumerate() {
            let mut line = format!("{OLD} {NEW} {name}").into_bytes();
            if i == 0 {
                line.push(0);
                line.extend(caps.bytes());
            }
            pkt(&mut body, &line);
        }
        body.extend(b"0000PACK");
        body
    }

    fn body_of_answer(call: &Call<'_>, body: &[u8], reason: &str) -> (String, Vec<u8>) {
        let mut out = Vec::new();
        assert!(answer(&mut out, call, body, reason).unwrap());
        let at = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        (
            String::from_utf8_lossy(&out[..at]).into_owned(),
            out[at..].to_vec(),
        )
    }

    /// The pkt-lines of a bare (no side-band) report, up to its flush.
    fn lines_of(report: &[u8]) -> Vec<String> {
        let (mut at, mut lines) = (0, Vec::new());
        loop {
            let len = usize::from_str_radix(std::str::from_utf8(&report[at..at + 4]).unwrap(), 16)
                .unwrap();
            if len == 0 {
                assert_eq!(at + 4, report.len(), "nothing after the flush");
                return lines;
            }
            lines.push(String::from_utf8(report[at + 4..at + len].to_vec()).unwrap());
            at += len;
        }
    }

    #[test]
    fn a_hostile_reason_stays_inside_its_one_ng_line_per_ref() {
        let body = push("report-status", &["refs/heads/a", "refs/heads/b"]);
        for reason in [
            "x\n0000\n",
            "x\n000eunpack ok\n",
            "x\nok refs/heads/a\n0000",
            "x\r\n0000ok refs/heads/b",
            "\u{0}0000",
        ] {
            let (_, report) = body_of_answer(&Call::ReceivePack, &body, reason);
            let lines = lines_of(&report);
            assert_eq!(lines.len(), 3, "{reason:?}: {lines:?}");
            assert_eq!(lines[0], "unpack ok\n");
            for (line, name) in lines[1..].iter().zip(["refs/heads/a", "refs/heads/b"]) {
                assert!(
                    line.starts_with(&format!("ng {name} ")),
                    "{reason:?}: {line:?}"
                );
                assert_eq!(line.matches('\n').count(), 1, "{reason:?}: {line:?}");
                assert!(line.ends_with('\n'));
            }
        }
    }

    #[test]
    fn a_very_long_hostile_reason_is_cut_and_the_report_still_reads() {
        let body = push("report-status", &["refs/heads/a"]);
        let reason = format!("0000\n{}", "é".repeat(100_000));
        let (_, report) = body_of_answer(&Call::ReceivePack, &body, &reason);
        assert_eq!(lines_of(&report).len(), 2);
    }

    #[test]
    fn a_refused_push_is_a_report_status_in_the_side_band_the_client_asked_for() {
        let body = push(
            "report-status side-band-64k agent=git/2",
            &["refs/heads/work"],
        );
        let (head, bytes) = body_of_answer(&Call::ReceivePack, &body, "purlis: no");
        assert!(head.starts_with("HTTP/1.1 200"));
        assert!(head.contains("application/x-git-receive-pack-result"));
        // Channel 1 carries the report: `unpack ok`, one `ng`, and the report's own flush.
        assert_eq!(bytes[4], 1, "channel 1");
        assert!(bytes.ends_with(b"0000"));
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(text.contains("unpack ok\n"));
        assert!(text.contains("ng refs/heads/work purlis: no\n"));
    }

    #[test]
    fn without_a_side_band_the_report_is_bare_pkt_lines() {
        let body = push("report-status", &["refs/heads/a", "refs/heads/b"]);
        let (_, bytes) = body_of_answer(&Call::ReceivePack, &body, "purlis: no");
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "000eunpack ok\n001fng refs/heads/a purlis: no\n001fng refs/heads/b purlis: no\n0000"
        );
    }

    #[test]
    fn a_ref_name_with_a_space_a_control_or_too_many_bytes_is_left_to_the_caller() {
        let long = format!("refs/heads/{}", "a".repeat(5000));
        for name in ["refs/heads/a\u{7}b", "refs/heads/é", long.as_str()] {
            assert!(
                report_status(&push("report-status", &[name]), "purlis: no").is_none(),
                "{name:.40}"
            );
        }
        assert!(report_status(&push("report-status", &["refs/heads/ok"]), "purlis: no").is_some());
    }

    #[test]
    fn a_push_whose_commands_do_not_read_is_left_to_the_caller() {
        let mut out = Vec::new();
        assert!(!answer(&mut out, &Call::ReceivePack, b"0000", "purlis: no").unwrap());
        assert!(out.is_empty());
    }

    #[test]
    fn a_refused_advertisement_is_the_banner_and_one_err_line() {
        let (head, bytes) = body_of_answer(&Call::Advertise("git-receive-pack"), b"", "purlis: no");
        assert!(head.contains("application/x-git-receive-pack-advertisement"));
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "001f# service=git-receive-pack\n00000013ERR purlis: no\n"
        );
    }

    #[test]
    fn a_reason_is_one_short_line() {
        assert_eq!(one_line("a\r\nb\u{0}  c"), "a b c");
        assert!(one_line(&"é".repeat(500)).len() <= MOST_REASON);
    }

    #[test]
    fn only_the_two_git_requests_are_answered_this_way() {
        assert_eq!(
            call_of("GET", "/o/r.git/info/refs?service=git-receive-pack"),
            Some(Call::Advertise("git-receive-pack"))
        );
        assert_eq!(
            call_of("POST", "/o/r.git/git-receive-pack"),
            Some(Call::ReceivePack)
        );
        assert_eq!(call_of("POST", "/o/r.git/git-upload-pack"), None);
        assert_eq!(call_of("GET", "/o/r/info/refs"), None);
        assert_eq!(call_of("GET", "/o/r/info/refs?service=x"), None);
    }
}
