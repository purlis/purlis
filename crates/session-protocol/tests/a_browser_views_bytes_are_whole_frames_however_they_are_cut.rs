// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser view's bytes are whole frames (1.6, #2012): each one says how long it is, so a
//! reader handed the stream in pieces of any size tells where one frame ends and the next
//! begins, and a frame larger than the bound is refused rather than held.

use purlis_session_protocol::view::frames::{self, Frame, Frames, JPEG, MOST_FRAME_BYTES};

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
