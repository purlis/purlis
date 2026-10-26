// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser view's bytes are whole frames (1.6, #2012): each one says how long it is, so a
//! reader handed the stream in pieces of any size tells where one frame ends and the next
//! begins, and a frame larger than the bound is refused rather than held.

use purlis_session_protocol::browsing::{Aim, origin_of};
use purlis_session_protocol::view::frames::{
    self, AIMED_JPEG, Frame, Frames, JPEG, MOST_FRAME_BYTES, ORIGIN,
};

fn jpeg(byte: u8, len: usize) -> Vec<u8> {
    vec![byte; len]
}

#[test]
fn a_frame_is_its_length_its_format_and_its_image() {
    let framed = frames::frame(JPEG, &[0xff, 0xd8, 0xff]);
    assert_eq!(&framed[..], &[0, 0, 0, 4, 0x01, 0xff, 0xd8, 0xff]);
}

#[test]
fn frames_read_back_whole_whatever_pieces_the_stream_comes_in() {
    let first = jpeg(1, 5000);
    let second = jpeg(2, 3);
    let third = jpeg(3, 70_000);
    let mut stream = Vec::new();
    for image in [&first, &second, &third] {
        stream.extend_from_slice(&frames::frame(JPEG, image));
    }
    for piece in [1, 2, 7, 4096, 16 * 1024, stream.len()] {
        let mut reader = Frames::default();
        let mut read = Vec::new();
        for chunk in stream.chunks(piece) {
            read.extend(reader.push(chunk).unwrap());
        }
        let images: Vec<Vec<u8>> = read.iter().map(|f| f.image.to_vec()).collect();
        assert_eq!(
            images,
            [first.clone(), second.clone(), third.clone()],
            "{piece}"
        );
        assert!(read.iter().all(|f| f.format == JPEG));
        assert!(reader.is_between_frames(), "{piece}");
    }
}

#[test]
fn a_frame_of_a_format_the_reader_does_not_know_is_still_read_whole() {
    let mut reader = Frames::default();
    let mut stream = frames::frame(0x7f, b"later").to_vec();
    stream.extend_from_slice(&frames::frame(JPEG, b"now"));
    let read: Vec<Frame> = reader.push(&stream).unwrap();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].format, 0x7f);
    assert_eq!(&read[1].image[..], b"now");
}

#[test]
fn a_frame_longer_than_the_bound_is_refused_before_it_is_held() {
    let mut reader = Frames::default();
    let too_long = u32::try_from(MOST_FRAME_BYTES + 2).unwrap();
    let refused = reader.push(&too_long.to_be_bytes());
    assert!(refused.is_err());
}

#[test]
fn an_empty_snapshot_is_no_frame_yet() {
    let mut reader = Frames::default();
    assert!(reader.push(&[]).unwrap().is_empty());
    assert!(reader.is_between_frames());
}

#[test]
fn an_aimed_frame_names_the_page_and_document_it_shows_before_its_image() {
    let aim = Aim {
        page: "p2".into(),
        document: 7,
    };
    let framed = frames::aimed(&aim, &[0xff, 0xd8]);
    assert_eq!(
        &framed[..],
        &[
            0, 0, 0, 14,   // the length
            0x02, // the format: an aimed JPEG
            2, b'p', b'2', // the page, its length first
            0, 0, 0, 0, 0, 0, 0, 7, // the document
            0xff, 0xd8, // the image
        ]
    );
    let mut reader = Frames::default();
    let read = reader.push(&framed).unwrap();
    assert_eq!(read[0].format, AIMED_JPEG);
    let (said, image) = read[0].aimed().expect("an aimed frame reads back");
    assert_eq!(said, aim);
    assert_eq!(&image[..], &[0xff, 0xd8]);
}

#[test]
fn a_plain_or_cut_short_frame_names_no_aim() {
    let plain = &Frames::default().push(&frames::frame(JPEG, b"x")).unwrap()[0];
    assert_eq!(plain.aimed(), None);
    let short = &Frames::default()
        .push(&frames::frame(AIMED_JPEG, &[9, b'p']))
        .unwrap()[0];
    assert_eq!(short.aimed(), None);
}

#[test]
fn an_origin_frame_is_the_pages_origin_alone_and_an_empty_one_is_no_page() {
    // 1.35 (#2300): what the browser view's chip names, sent when it changes.
    let framed = frames::origin(Some("http://localhost:5173"));
    assert_eq!(framed[4], ORIGIN);
    let read = &Frames::default().push(&framed).unwrap()[0];
    assert_eq!(
        read.origin(),
        Some(Some("http://localhost:5173".to_owned()))
    );

    let none = &Frames::default().push(&frames::origin(None)).unwrap()[0];
    assert_eq!(none.origin(), Some(None), "no page, or one with no origin");

    let plain = &Frames::default().push(&frames::frame(JPEG, b"x")).unwrap()[0];
    assert_eq!(plain.origin(), None, "an image is no origin frame");
}

#[test]
fn an_origin_frame_whose_bytes_are_not_text_reads_as_no_origin_frame() {
    let bad = &Frames::default()
        .push(&frames::frame(ORIGIN, &[0xff, 0xfe]))
        .unwrap()[0];
    assert_eq!(bad.origin(), None);
}

#[test]
fn an_address_is_cut_to_its_origin_and_nothing_else() {
    for (address, origin) in [
        ("http://localhost:5173/login", Some("http://localhost:5173")),
        (
            "https://Shop.Example.test/basket?token=s3cret#pay",
            Some("https://shop.example.test"),
        ),
        (
            "https://ada:pw@shop.example.test/",
            Some("https://shop.example.test"),
        ),
        ("https://[::1]:8443/a", Some("https://[::1]:8443")),
        (
            "https://xn--bcher-kva.example/",
            Some("https://xn--bcher-kva.example"),
        ),
        ("about:blank", None),
        ("data:text/html,hunter2", None),
        ("file:///etc/passwd", None),
        ("chrome://settings", None),
        ("https:///nohost", None),
        ("https://b\u{fc}cher.example/", None),
        ("https://a%20b.example/", None),
        ("", None),
    ] {
        assert_eq!(origin_of(address).as_deref(), origin, "{address}");
    }
}

#[test]
fn an_origin_longer_than_a_whole_dns_name_is_none_and_an_origin_is_its_own_origin() {
    let long = format!("https://{}.example", "a".repeat(250));
    assert_eq!(origin_of(&long), None);
    for origin in [
        "http://localhost:5173",
        "https://docs.example.test",
        "https://[::1]:8443",
    ] {
        assert_eq!(origin_of(origin).as_deref(), Some(origin), "{origin}");
    }
}

#[test]
fn a_jpegs_size_is_read_from_its_header_and_anything_else_has_none() {
    use purlis_session_protocol::view::frames::jpeg_size;
    // SOI, an APP0 of 16 bytes, then a baseline SOF0: 8-bit, 800 high, 1280 wide.
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10];
    jpeg.extend_from_slice(&[0; 14]);
    jpeg.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08, 0x03, 0x20, 0x05, 0x00]);

    assert_eq!(jpeg_size(&jpeg), Some((1280, 800)));
    assert_eq!(jpeg_size(&jpeg[..jpeg.len() - 2]), None, "cut short");
    assert_eq!(jpeg_size(b"not a jpeg"), None);
    assert_eq!(
        jpeg_size(&[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x00]),
        None,
        "a bad length"
    );
}
