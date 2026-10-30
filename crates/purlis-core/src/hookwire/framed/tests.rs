// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::Cursor;

use super::*;
use crate::hookwire::{Answer, Ask, UNKNOWN_LINE};

/// What a line may be at most on the hook channel, which every test below goes past.
const A_LINE: usize = 64 * 1024;

fn framed(frames: &[Frame]) -> Vec<u8> {
    let mut wire = Vec::new();
    for frame in frames {
        write(&mut wire, frame).expect("written");
    }
    wire
}

fn header(length: u32, kind: u8) -> Vec<u8> {
    let mut wire = length.to_be_bytes().to_vec();
    wire.push(kind);
    wire
}

fn ticket(chat: u32) -> Ask {
    Ask::Ticket { chat }
}

/// Serves `wire` for chat 7 with `answer`, and what it wrote back.
/// The bound an ask's frame is held to below: a line's, as the socket holds it.
const AN_ASK: u32 = 6 * crate::handoff::FIRST_MESSAGE_MAX_BYTES as u32 + 4096;

fn served(wire: Vec<u8>, answer: &mut dyn FnMut(Ask) -> Reply) -> (io::Result<()>, Vec<u8>) {
    served_admitting(wire, &mut || Ok(()), answer)
}

/// Serves `wire` for chat 7, each ask admitted or not by `admit`.
fn served_admitting(
    wire: Vec<u8>,
    admit: &mut dyn FnMut() -> Result<(), String>,
    answer: &mut dyn FnMut(Ask) -> Reply,
) -> (io::Result<()>, Vec<u8>) {
    let mut back = Vec::new();
    let served = serve(
        &mut Cursor::new(wire),
        &mut back,
        Serving {
            chat: 7,
            asks: 4,
            an_ask_is_at_most: AN_ASK,
            arm: &mut || {},
            admit,
        },
        answer,
    );
    (served, back)
}

#[test]
fn a_frame_larger_than_a_line_comes_back_whole() {
    let big: Vec<u8> = (0..3 * A_LINE).map(|i| (i % 251) as u8).collect();
    let wire = framed(&[Frame::Bytes(big.clone())]);
    let mut reader = Cursor::new(wire);
    assert_eq!(read(&mut reader).expect("read"), Some(Frame::Bytes(big)));
    assert_eq!(read(&mut reader).expect("the end"), None);
}

#[test]
fn a_frame_whose_length_is_past_the_bound_is_refused_before_any_of_it_is_read() {
    // The length alone, and nothing after it: a reader that waited for the bytes, or made
    // room for them, would block or allocate instead of refusing.
    let mut reader = Cursor::new(header(A_FRAME_IS_AT_MOST + 1, 2));
    let why = read(&mut reader).expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::InvalidData, "{why}");
}

#[test]
fn a_frame_cut_short_is_an_error_and_never_a_frame() {
    let mut wire = framed(&[Frame::Bytes(b"0123456789".to_vec())]);
    wire.truncate(wire.len() - 3);
    let why = read(&mut Cursor::new(wire)).expect_err("cut short");
    assert_eq!(why.kind(), io::ErrorKind::UnexpectedEof, "{why}");

    // And inside the header too.
    let why = read(&mut Cursor::new(vec![0, 0])).expect_err("cut short");
    assert_eq!(why.kind(), io::ErrorKind::UnexpectedEof, "{why}");
}

#[test]
fn a_frame_of_an_unknown_kind_is_refused() {
    let mut wire = header(1, 9);
    wire.push(b'x');
    let why = read(&mut Cursor::new(wire)).expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::InvalidData, "{why}");
}

#[test]
fn a_frame_past_the_bound_is_never_written() {
    let mut wire = Vec::new();
    let why = write(
        &mut wire,
        &Frame::Bytes(vec![0; A_FRAME_IS_AT_MOST as usize + 1]),
    )
    .expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::InvalidData);
    assert!(wire.is_empty(), "nothing is written of it");
}

#[test]
fn a_reply_carries_its_answer_and_binary_attachments_in_order() {
    let picture: Vec<u8> = (0..=255u8).cycle().take(2 * A_LINE).collect();
    let reply = Reply {
        answer: Answer::Ticket {
            ticket: "t".repeat(3 * A_LINE),
        },
        attached: vec![
            Attached {
                media_type: "image/png".to_owned(),
                bytes: picture.clone(),
            },
            Attached {
                media_type: "application/octet-stream".to_owned(),
                bytes: vec![0, 10, 13, 255],
            },
        ],
    };
    let mut wire = Vec::new();
    write_reply(&mut wire, &reply).expect("written");
    assert_eq!(read_reply(&mut Cursor::new(wire)).expect("read"), reply);
}

#[test]
fn a_reply_missing_an_attachment_it_names_is_refused() {
    let mut wire = Vec::new();
    write(
        &mut wire,
        &Frame::Json(
            br#"{"answer":{"no":{"why":"x"}},"attached":[{"media_type":"image/png"}]}"#.to_vec(),
        ),
    )
    .expect("written");
    let why = read_reply(&mut Cursor::new(wire)).expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::UnexpectedEof, "{why}");
}

#[test]
fn a_reply_with_json_where_an_attachment_belongs_is_refused() {
    let wire = framed(&[
        Frame::Json(
            br#"{"answer":{"no":{"why":"x"}},"attached":[{"media_type":"image/png"}]}"#.to_vec(),
        ),
        Frame::Json(b"{}".to_vec()),
    ]);
    let why = read_reply(&mut Cursor::new(wire)).expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::InvalidData, "{why}");
}

#[test]
fn an_attachment_naming_no_media_type_is_neither_written_nor_read() {
    for odd in ["", "png", "image/png; charset=x", "image/ png", "a/b\n"] {
        let reply = Reply {
            answer: Answer::No { why: "x".into() },
            attached: vec![Attached {
                media_type: odd.to_owned(),
                bytes: vec![1],
            }],
        };
        assert!(write_reply(&mut Vec::new(), &reply).is_err(), "{odd:?}");
        let head = serde_json::json!({
            "answer": {"no": {"why": "x"}},
            "attached": [{"media_type": odd}],
        });
        let wire = framed(&[
            Frame::Json(serde_json::to_vec(&head).unwrap()),
            Frame::Bytes(vec![1]),
        ]);
        assert!(read_reply(&mut Cursor::new(wire)).is_err(), "{odd:?}");
    }
}

#[test]
fn a_reply_attaching_more_than_the_bound_is_refused() {
    let attached: Vec<serde_json::Value> = (0..=A_REPLY_ATTACHES_AT_MOST)
        .map(|_| serde_json::json!({"media_type": "image/png"}))
        .collect();
    let head = serde_json::json!({"answer": {"no": {"why": "x"}}, "attached": attached});
    let wire = framed(&[Frame::Json(serde_json::to_vec(&head).unwrap())]);
    let why = read_reply(&mut Cursor::new(wire)).expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::InvalidData, "{why}");
}

#[test]
fn a_served_stream_answers_each_ask_of_its_chat_in_order() {
    let mut wire = Vec::new();
    write_ask(&mut wire, &ticket(7)).unwrap();
    write_ask(&mut wire, &ticket(7)).unwrap();
    let mut asked = 0;
    let (served, back) = served(wire, &mut |ask| {
        assert_eq!(ask, ticket(7));
        asked += 1;
        Reply {
            answer: Answer::Ticket {
                ticket: format!("n{asked}"),
            },
            attached: vec![Attached {
                media_type: "image/png".into(),
                bytes: vec![asked],
            }],
        }
    });
    served.expect("served to its end");
    let mut back = Cursor::new(back);
    for n in 1..=2u8 {
        let reply = read_reply(&mut back).expect("a reply");
        assert_eq!(
            reply.answer,
            Answer::Ticket {
                ticket: format!("n{n}")
            }
        );
        assert_eq!(reply.attached[0].bytes, vec![n]);
    }
    assert_eq!(read(&mut back).unwrap(), None);
}

#[test]
fn an_ask_for_another_chat_is_answered_no_and_never_acted_on() {
    let mut wire = Vec::new();
    write_ask(&mut wire, &ticket(8)).unwrap();
    write_ask(&mut wire, &ticket(7)).unwrap();
    let (served, back) = served(wire, &mut |_| panic!("acted on"));
    assert!(served.is_err());
    let reply = read_reply(&mut Cursor::new(back)).expect("a refusal");
    assert!(
        matches!(&reply.answer, Answer::No { why } if why.contains("chat 8")),
        "{reply:?}"
    );
}

#[test]
fn bytes_where_an_ask_belongs_end_the_stream_unacted_on() {
    let mut wire = framed(&[Frame::Bytes(b"{}".to_vec())]);
    write_ask(&mut wire, &ticket(7)).unwrap();
    let (served, back) = served(wire, &mut |_| panic!("acted on"));
    assert!(served.is_err());
    let reply = read_reply(&mut Cursor::new(back)).expect("a refusal");
    assert!(matches!(reply.answer, Answer::No { .. }), "{reply:?}");
}

#[test]
fn an_ask_from_a_newer_purlis_is_answered_with_the_unknown_line_sentence() {
    let wire = framed(&[Frame::Json(br#"{"from_the_future":{"chat":7}}"#.to_vec())]);
    let (served, back) = served(wire, &mut |_| panic!("acted on"));
    assert!(served.is_err());
    let reply = read_reply(&mut Cursor::new(back)).expect("a refusal");
    assert_eq!(
        reply.answer,
        Answer::No {
            why: UNKNOWN_LINE.to_owned()
        }
    );
}

#[test]
fn a_stream_past_its_bound_frame_is_ended_unanswered() {
    let mut wire = header(A_FRAME_IS_AT_MOST + 1, 1);
    wire.extend_from_slice(b"anything");
    let (served, back) = served(wire, &mut |_| panic!("acted on"));
    assert!(served.is_err());
    assert!(
        back.is_empty(),
        "nothing is said to a stream past its bound"
    );
}

#[test]
fn a_stream_serves_at_most_its_bound_of_asks() {
    let mut wire = Vec::new();
    for _ in 0..6 {
        write_ask(&mut wire, &ticket(7)).unwrap();
    }
    let mut asked = 0;
    let (served, _) = served(wire, &mut |_| {
        asked += 1;
        Reply::from(Answer::Ticket { ticket: "t".into() })
    });
    served.expect("ended at its bound");
    assert_eq!(asked, 4);
}

#[test]
fn an_app_that_speaks_frames_accepts_the_hello() {
    let line = serde_json::to_string(&Accepted::now()).unwrap();
    assert_eq!(
        line,
        format!(r#"{{"frames":{VERSION},"most":{A_FRAME_IS_AT_MOST}}}"#)
    );
    assert_eq!(
        answered(&line).expect("read"),
        Answered::Accepted(Accepted::now())
    );
}

#[test]
fn an_app_from_before_frames_is_asked_again_in_lines() {
    // What an older app answers a line of a kind it does not know (#1333).
    let line = serde_json::to_string(&Answer::No {
        why: UNKNOWN_LINE.to_owned(),
    })
    .unwrap();
    assert_eq!(answered(&line).expect("read"), Answered::Older);
}

#[test]
fn an_app_that_ends_the_connection_at_the_hello_is_asked_again_in_lines() {
    // An app from before #1333 closes a connection on a line it cannot read, saying nothing.
    assert_eq!(answered("").expect("read"), Answered::Older);
}

#[test]
fn an_app_that_refuses_the_hello_is_an_error_with_its_sentence() {
    let line = serde_json::to_string(&Answer::No {
        why: "this line does not carry chat 7's token, so nothing was done".to_owned(),
    })
    .unwrap();
    assert!(
        matches!(answered(&line).expect("read"), Answered::Refused(why) if why.contains("token"))
    );
}

#[test]
fn a_hello_at_another_version_is_not_spoken() {
    let line = format!(r#"{{"frames":{},"most":1}}"#, VERSION + 1);
    assert!(answered(&line).is_err());
}

#[test]
fn the_hello_is_a_line_of_its_own_kind_carrying_its_chat_and_token() {
    let token = crate::hookwire::ChatToken::read(&|name| {
        (name == crate::hookwire::TOKEN_ENV).then(|| "abc".to_owned())
    });
    let line = crate::hookwire::line_with(token.as_ref(), &Hello::of(7)).unwrap();
    let said: serde_json::Value = serde_json::from_slice(&line).unwrap();
    assert_eq!(
        said,
        serde_json::json!({"frames": VERSION, "chat": 7, "token": "abc"})
    );
    let crate::hookwire::ReadLine {
        line: read, token, ..
    } = crate::hookwire::read_delivered(std::str::from_utf8(&line).unwrap())
        .expect("a line the app reads");
    assert!(
        matches!(&read, crate::hookwire::Line::Frames(hello) if *hello == Hello::of(7)),
        "{read:?}"
    );
    assert_eq!(read.chat(), 7);
    assert_eq!(token.as_deref(), Some("abc"));
}

#[test]
fn an_ask_frame_at_the_line_bound_is_read_and_one_past_it_is_refused_from_its_length() {
    // An ask is what a line carries, so its frame is held to a line's bound, not a reply's.
    let at = AN_ASK as usize;
    let mut wire = framed(&[Frame::Json(vec![b' '; at])]);
    let read = read_at_most(&mut Cursor::new(wire.clone()), AN_ASK).expect("at the bound");
    assert!(matches!(read, Some(Frame::Json(body)) if body.len() == at));

    wire = header(AN_ASK + 1, 1);
    let (served, back) = served(wire, &mut |_| panic!("acted on"));
    let why = served.expect_err("refused");
    assert_eq!(why.kind(), io::ErrorKind::InvalidData, "{why}");
    assert!(back.is_empty(), "nothing is said to a frame past its bound");
}

#[test]
fn every_ask_is_admitted_again_and_one_not_admitted_is_answered_no_and_never_acted_on() {
    let mut wire = Vec::new();
    write_ask(&mut wire, &ticket(7)).unwrap();
    write_ask(&mut wire, &ticket(7)).unwrap();
    let mut admitted = 0;
    let mut answered = 0;
    let (served, back) = served_admitting(
        wire,
        &mut || {
            admitted += 1;
            if admitted == 1 {
                Ok(())
            } else {
                Err("from a process outside the chat".to_owned())
            }
        },
        &mut |_| {
            answered += 1;
            Reply::from(Answer::Ticket { ticket: "t".into() })
        },
    );
    assert_eq!(
        served.expect_err("refused").kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!((admitted, answered), (2, 1));
    let mut back = Cursor::new(back);
    assert!(matches!(
        read_reply(&mut back).unwrap().answer,
        Answer::Ticket { .. }
    ));
    assert_eq!(
        read_reply(&mut back).unwrap().answer,
        Answer::No {
            why: "from a process outside the chat".to_owned()
        }
    );
}

#[cfg(unix)]
#[test]
fn the_bound_held_here_is_the_line_bound_the_socket_holds_an_ask_to() {
    assert_eq!(u64::from(AN_ASK), crate::hookwire::A_LINE_IS_AT_MOST);
    const { assert!(AN_ASK < A_FRAME_IS_AT_MOST) };
}

#[test]
fn a_browsers_screenshot_goes_back_as_an_image_after_its_answer_and_never_inside_it() {
    use purlis_session_protocol::browsing::Browsed;
    let png = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
    let answer = Answer::Browsed {
        browser: "b1".into(),
        browsed: Box::new(Browsed {
            settled: true,
            png: Some(png.clone()),
            ..Browsed::default()
        }),
    };

    let reply = Reply::from(answer);

    assert_eq!(
        reply.attached,
        [Attached {
            media_type: "image/png".into(),
            bytes: png,
        }]
    );
    match reply.answer {
        Answer::Browsed { browsed, .. } => assert_eq!(browsed.png, None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn on_a_line_a_browsers_screenshot_is_left_out_and_the_answer_says_so() {
    use purlis_session_protocol::browsing::Browsed;
    let answer = Answer::Browsed {
        browser: "b1".into(),
        browsed: Box::new(Browsed {
            settled: true,
            png: Some(vec![0x89, b'P', b'N', b'G']),
            ..Browsed::default()
        }),
    };

    let line = super::on_a_line(answer);

    match line {
        Answer::Browsed { browsed, .. } => {
            assert_eq!(browsed.png, None);
            assert!(
                browsed
                    .note
                    .as_deref()
                    .is_some_and(|note| note.contains("purlis mcp")),
                "{browsed:?}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_theme_specimen_s_picture_goes_back_as_an_image_in_the_order_drawn() {
    use crate::theme::Appearance;
    use crate::themepreview::Painted;
    let painted = |appearance, png: &[u8]| Painted {
        theme: "purlis-classic".into(),
        appearance,
        high_contrast: false,
        preview: false,
        width: 2,
        height: 1,
        millis: 9,
        png: png.to_vec(),
    };
    let answer = Answer::Previewed {
        painted: vec![
            painted(Appearance::Dark, b"dark"),
            painted(Appearance::Light, b"light"),
        ],
    };

    let reply = Reply::from(answer);

    let image = |bytes: &[u8]| Attached {
        media_type: "image/png".into(),
        bytes: bytes.to_vec(),
    };
    assert_eq!(reply.attached, [image(b"dark"), image(b"light")]);
    match &reply.answer {
        Answer::Previewed { painted } => {
            assert!(painted.iter().all(|one| one.png.is_empty()), "{painted:?}");
        }
        other => panic!("{other:?}"),
    }
    // And it is read back whole: the answer, then each picture after it.
    let mut wire = Vec::new();
    write_reply(&mut wire, &reply).expect("written");
    let read = read_reply(&mut Cursor::new(wire)).expect("read");
    assert_eq!(read, reply);
}
