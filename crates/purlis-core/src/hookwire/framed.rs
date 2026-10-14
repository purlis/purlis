// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The hook channel's framed stream (#1996): length-prefixed frames on the same socket as the
//! line protocol, so an answer may be larger than a line and may carry bytes.
//!
//! **Negotiated, so a line client never sees a frame.** A client that wants frames opens its
//! connection with one line, [`Hello`], carrying the chat's number and the chat's own token,
//! checked as every line is (the same token, the same check of the process that sent it). An
//! app that speaks frames answers one line, [`Accepted`], and from then on that connection
//! carries frames both ways. An app from before frames answers the hello with
//! [`super::UNKNOWN_LINE`], and the client asks again in lines ([`accepted`] says which).
//! Nothing changes for a connection that never says hello.
//!
//! **A frame** is a four-byte big-endian length, one byte saying its kind, and that many bytes:
//!
//! | kind | byte | what it holds |
//! |---|---|---|
//! | JSON | `1` | one JSON value: an [`super::Ask`] going to the app, a [`Reply`]'s head coming back |
//! | bytes | `2` | one attachment of a reply, as it is |
//!
//! A request is one JSON frame holding an [`super::Ask`] for the hello's own chat. Its reply is
//! one JSON frame, the [`Reply`]'s head (the [`super::Answer`] and what is attached), then one
//! bytes frame for each attachment, in order.
//!
//! **Every bound fails closed.** A frame longer than [`A_FRAME_IS_AT_MOST`] is refused from its
//! length, before any of it is read; an unknown kind, a frame cut short, a JSON frame that does
//! not read, a bytes frame where JSON belongs or the other way round, more attachments than
//! [`A_REPLY_ATTACHES_AT_MOST`], and an ask for another chat each end the connection. Nothing
//! read past a bad frame is acted on.
//!
//! Everything here reads and writes a byte stream, so it is tested in memory; the socket's half
//! (the hello on a real connection) is in [`super::serve`] and [`Framed`].

use std::io::{self, Read, Write};

use super::{Answer, Ask};

/// The framed stream's version, said in the hello and in its acceptance.
pub const VERSION: u32 = 1;

/// The most one frame may hold: a screenshot of a large display, with room.
pub const A_FRAME_IS_AT_MOST: u32 = 16 * 1024 * 1024;

/// The most attachments one reply may carry.
pub const A_REPLY_ATTACHES_AT_MOST: usize = 16;

/// The longest media type an attachment may name.
const A_MEDIA_TYPE_IS_AT_MOST: usize = 127;

/// The byte a JSON frame's kind is.
const JSON: u8 = 1;

/// The byte a bytes frame's kind is.
const BYTES: u8 = 2;

/// The field the hello and its acceptance carry the version in.
pub const FRAMES: &str = "frames";

/// One frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// One JSON value, as its bytes.
    Json(Vec<u8>),
    /// Bytes, as they are.
    Bytes(Vec<u8>),
}

/// Writes `frame` to `writer`. A frame longer than [`A_FRAME_IS_AT_MOST`] is refused unwritten.
pub fn write(writer: &mut impl Write, frame: &Frame) -> io::Result<()> {
    let (kind, body) = match frame {
        Frame::Json(body) => (JSON, body),
        Frame::Bytes(body) => (BYTES, body),
    };
    let length = u32::try_from(body.len())
        .ok()
        .filter(|length| *length <= A_FRAME_IS_AT_MOST)
        .ok_or_else(|| too_long(body.len()))?;
    let mut head = [0u8; 5];
    head[..4].copy_from_slice(&length.to_be_bytes());
    head[4] = kind;
    writer.write_all(&head)?;
    writer.write_all(body)?;
    writer.flush()
}

/// The next frame on `reader`, or `None` where the stream ended cleanly between frames.
///
/// A length past [`A_FRAME_IS_AT_MOST`] is refused before any of the frame is read, and an
/// unknown kind as soon as it is; a stream that ends inside a frame is an error, never a frame.
pub fn read(reader: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut head = [0u8; 5];
    let mut got = 0;
    while got < head.len() {
        match reader.read(&mut head[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(cut_short()),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let length = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
    if length > A_FRAME_IS_AT_MOST {
        return Err(too_long(length as usize));
    }
    let kind = head[4];
    if kind != JSON && kind != BYTES {
        return Err(bad(format!(
            "a frame of a kind this purlis does not know ({kind})"
        )));
    }
    // Read through `take`, never into a buffer of the length said: a length is only a claim
    // until its bytes have come.
    let mut body = Vec::new();
    reader.take(u64::from(length)).read_to_end(&mut body)?;
    if body.len() != length as usize {
        return Err(cut_short());
    }
    Ok(Some(if kind == JSON {
        Frame::Json(body)
    } else {
        Frame::Bytes(body)
    }))
}

/// What a client says first to ask for frames: the version it speaks and the chat it is in.
/// The chat's token goes beside them, as on every line ([`super::line_with`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hello {
    pub frames: u32,
    pub chat: u32,
}

impl Hello {
    /// A hello for `chat`, at this purlis's [`VERSION`].
    pub fn of(chat: u32) -> Self {
        Self {
            frames: VERSION,
            chat,
        }
    }
}

/// What an app that speaks frames answers a hello with, on one line: the version it will speak
/// and the most a frame may hold. Frames follow on the connection from the next byte.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accepted {
    pub frames: u32,
    pub most: u32,
}

impl Accepted {
    /// The acceptance this purlis says.
    pub fn now() -> Self {
        Self {
            frames: VERSION,
            most: A_FRAME_IS_AT_MOST,
        }
    }
}

/// What the app's one line after a hello says: frames from here (`Some`), or an app from
/// before frames (`None`), which the client asks again in lines. Any other refusal is an error
/// with the app's sentence, and so is a version this purlis does not speak.
pub fn accepted(line: &str) -> io::Result<Option<Accepted>> {
    if let Ok(accepted) = serde_json::from_str::<Accepted>(line) {
        if accepted.frames != VERSION {
            return Err(bad(format!(
                "the app offered frames version {}, and this purlis speaks {VERSION}",
                accepted.frames
            )));
        }
        return Ok(Some(accepted));
    }
    match serde_json::from_str::<Answer>(line) {
        Ok(Answer::No { why }) if why == super::UNKNOWN_LINE => Ok(None),
        Ok(Answer::No { why }) => Err(io::Error::new(io::ErrorKind::PermissionDenied, why)),
        _ => Err(bad(
            "the app answered the hello in a way this purlis cannot read".to_owned(),
        )),
    }
}

/// One attachment of a reply: its media type, and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// What the app answers a framed ask with: the [`Answer`] a line would have carried, and what
/// is attached to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub answer: Answer,
    pub attached: Vec<Attached>,
}

impl Attached {
    /// Its bytes in standard base64, as JSON-RPC carries bytes (MCP's image and blob content).
    pub fn base64(&self) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
}

impl From<Answer> for Reply {
    fn from(answer: Answer) -> Self {
        Self {
            answer,
            attached: Vec::new(),
        }
    }
}

/// A reply's head, its JSON frame: the answer, and each attachment's media type in order.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    answer: Answer,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attached: Vec<AttachedHead>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachedHead {
    media_type: String,
}

/// Whether `said` is a media type an attachment may name: `type/subtype`, short, in the
/// characters RFC 6838 allows, and nothing else (no parameters, no white space).
fn a_media_type(said: &str) -> bool {
    let allowed = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&byte))
    };
    said.len() <= A_MEDIA_TYPE_IS_AT_MOST
        && said
            .split_once('/')
            .is_some_and(|(kind, sub)| allowed(kind) && allowed(sub))
}

/// Writes `reply`: its head, then each attachment. One the reader would refuse is refused here
/// unwritten.
pub fn write_reply(writer: &mut impl Write, reply: &Reply) -> io::Result<()> {
    if reply.attached.len() > A_REPLY_ATTACHES_AT_MOST {
        return Err(bad(format!(
            "a reply attaches at most {A_REPLY_ATTACHES_AT_MOST} things"
        )));
    }
    if let Some(odd) = reply.attached.iter().find(|a| !a_media_type(&a.media_type)) {
        return Err(bad(format!(
            "{:?} is not a media type an attachment may name",
            odd.media_type
        )));
    }
    let head = Head {
        answer: reply.answer.clone(),
        attached: reply
            .attached
            .iter()
            .map(|a| AttachedHead {
                media_type: a.media_type.clone(),
            })
            .collect(),
    };
    let head = serde_json::to_vec(&head).map_err(io::Error::other)?;
    if head.len() > A_FRAME_IS_AT_MOST as usize {
        return Err(too_long(head.len()));
    }
    write(writer, &Frame::Json(head))?;
    for attached in &reply.attached {
        write(writer, &Frame::Bytes(attached.bytes.clone()))?;
    }
    Ok(())
}

/// Reads one reply: its head and exactly the attachments it names.
pub fn read_reply(reader: &mut impl Read) -> io::Result<Reply> {
    let head = match read(reader)? {
        Some(Frame::Json(head)) => head,
        Some(Frame::Bytes(_)) => return Err(bad("a reply began with bytes".to_owned())),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the app closed the connection without answering",
            ));
        }
    };
    let head: Head = serde_json::from_slice(&head)
        .map_err(|_| bad("a reply's head that does not read".to_owned()))?;
    if head.attached.len() > A_REPLY_ATTACHES_AT_MOST {
        return Err(bad(format!(
            "a reply that attaches more than {A_REPLY_ATTACHES_AT_MOST} things"
        )));
    }
    let mut attached = Vec::with_capacity(head.attached.len());
    for one in head.attached {
        if !a_media_type(&one.media_type) {
            return Err(bad("an attachment that names no media type".to_owned()));
        }
        match read(reader)? {
            Some(Frame::Bytes(bytes)) => attached.push(Attached {
                media_type: one.media_type,
                bytes,
            }),
            Some(Frame::Json(_)) => {
                return Err(bad("JSON where a reply's attachment belongs".to_owned()));
            }
            None => return Err(cut_short()),
        }
    }
    Ok(Reply {
        answer: head.answer,
        attached,
    })
}

/// Writes `ask` as a request frame.
pub fn write_ask(writer: &mut impl Write, ask: &Ask) -> io::Result<()> {
    let body = serde_json::to_vec(ask).map_err(io::Error::other)?;
    write(writer, &Frame::Json(body))
}

/// Asks `ask` on a framed stream and reads its reply.
pub fn ask(reader: &mut impl Read, writer: &mut impl Write, ask: &Ask) -> io::Result<Reply> {
    write_ask(writer, ask)?;
    read_reply(reader)
}

/// Serves a framed stream for `chat` until it ends, at most `most` asks: each ask read from
/// `reader` is handed to `answer`, and its reply written to `writer`. `arm` is called as each
/// ask begins to be read, so a socket can give the frame a deadline of its own.
///
/// **It fails closed.** A frame that is not an ask, or an ask for another chat, is answered no
/// once, where it could be read at all, and ends the stream unacted on; so does a frame past a
/// bound, unanswered. A clean end between frames is the stream's end.
pub fn serve(
    reader: &mut impl Read,
    writer: &mut impl Write,
    chat: u32,
    most: usize,
    arm: &mut dyn FnMut(),
    answer: &mut dyn FnMut(Ask) -> Reply,
) -> io::Result<()> {
    for _ in 0..most {
        arm();
        let body = match read(reader)? {
            None => return Ok(()),
            Some(Frame::Json(body)) => body,
            Some(Frame::Bytes(_)) => {
                no(writer, "an ask is JSON, and this was bytes".to_owned())?;
                return Err(bad("bytes where an ask belongs".to_owned()));
            }
        };
        let Ok(asked) = serde_json::from_slice::<Ask>(&body) else {
            no(writer, super::UNKNOWN_LINE.to_owned())?;
            return Err(bad("an ask that does not read".to_owned()));
        };
        let names = super::Line::Ask(asked).chat();
        if names != chat {
            no(
                writer,
                format!("this stream is chat {chat}'s, and the ask names chat {names}"),
            )?;
            return Err(bad("an ask for another chat".to_owned()));
        }
        // Read again: `Line::chat` took the ask, and an ask is not `Clone`-cheap (it may carry
        // a whole brief). The bytes are what was checked.
        let asked: Ask = serde_json::from_slice(&body).map_err(io::Error::other)?;
        let reply = answer(asked);
        if let Err(why) = write_reply(writer, &reply) {
            // A reply this side could not write is said as a refusal, so the asker is never
            // left waiting on a silence.
            if why.kind() == io::ErrorKind::InvalidData {
                no(writer, format!("the app could not send its answer: {why}"))?;
            }
            return Err(why);
        }
    }
    Ok(())
}

/// A framed conversation with the app that started this chat: the client's half of the hello,
/// then asks answered on the same connection.
#[cfg(unix)]
pub struct Framed {
    reader: io::BufReader<std::os::unix::net::UnixStream>,
    writer: std::os::unix::net::UnixStream,
}

/// The most the app's answer to a hello may be: an acceptance, or a sentence why not.
#[cfg(unix)]
const A_HELLO_IS_ANSWERED_IN_AT_MOST: u64 = 4096;

#[cfg(unix)]
impl Framed {
    /// Says hello to the app at `path` for `chat`, carrying the chat's own `token`, and waits
    /// at most `within` for each half. `None` where the app is from before frames: ask it again
    /// in lines ([`super::Asking`]).
    pub fn on(
        path: &std::path::Path,
        token: Option<&super::ChatToken>,
        chat: u32,
        within: std::time::Duration,
    ) -> io::Result<Option<Self>> {
        use std::io::BufRead;

        let writer = std::os::unix::net::UnixStream::connect(path)?;
        writer.set_write_timeout(Some(within))?;
        writer.set_read_timeout(Some(within))?;
        let mut reader = io::BufReader::new(writer.try_clone()?);
        (&writer).write_all(&super::line_with(token, &Hello::of(chat))?)?;
        let mut said = String::new();
        (&mut reader)
            .take(A_HELLO_IS_ANSWERED_IN_AT_MOST)
            .read_line(&mut said)?;
        if said.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the app closed the connection without answering",
            ));
        }
        Ok(accepted(&said)?.map(|_| Self { reader, writer }))
    }

    /// Writes `asked` and reads the app's reply, waiting at most `within` for each read and
    /// write.
    pub fn ask(&mut self, asked: &Ask, within: std::time::Duration) -> io::Result<Reply> {
        self.writer.set_write_timeout(Some(within))?;
        self.writer.set_read_timeout(Some(within))?;
        ask(&mut self.reader, &mut self.writer, asked)
    }
}

/// Answers no, with `why`.
fn no(writer: &mut impl Write, why: String) -> io::Result<()> {
    write_reply(writer, &Reply::from(Answer::No { why }))
}

fn too_long(length: usize) -> io::Error {
    bad(format!(
        "a frame of {length} bytes, past the {A_FRAME_IS_AT_MOST} a frame may hold"
    ))
}

fn cut_short() -> io::Error {
    io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "the stream ended inside a frame",
    )
}

fn bad(why: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why)
}

#[cfg(test)]
mod tests;
