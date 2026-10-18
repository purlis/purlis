// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_connection_from_inside_a_chat_is_refused_before_the_handshake() {
    let refused = refused_inside(4242, Ok(Some(4000))).expect("refused");
    assert!(
        matches!(refused, LinkRefused::InsideAChat(_)),
        "{refused:?}"
    );
    assert!(
        refused.to_string().contains("inside chat program 4000"),
        "{refused}"
    );
}

#[test]
fn a_connection_whose_place_cannot_be_read_is_refused_as_if_it_were_inside_a_chat() {
    let unread = std::io::Error::new(std::io::ErrorKind::NotFound, "no such process");
    let refused = refused_inside(4242, Err(unread)).expect("refused");
    assert!(
        matches!(refused, LinkRefused::InsideAChat(_)),
        "{refused:?}"
    );
    assert!(
        refused.to_string().contains("could not be read"),
        "{refused}"
    );
}

#[test]
fn a_connection_from_outside_every_chat_goes_on_to_the_handshake() {
    assert!(refused_inside(4242, Ok(None)).is_none());
}
