// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn restored(chat: u32, standing: Standing) -> Restored {
    Restored { chat, standing }
}

#[test]
fn no_news_leaves_the_line_as_it_was() {
    assert_eq!(with(None, &News::default()), None);
    assert_eq!(
        with(Some("told".to_owned()), &News::default()),
        Some("told".to_owned())
    );
}

#[test]
fn a_chat_told_nothing_else_is_given_a_line_for_its_tasks_alone() {
    let news = News {
        restored: vec![
            restored(7, Standing::CarriesOn),
            restored(9, Standing::KeptRunning),
            restored(8, Standing::Ended),
        ],
        reported: Vec::new(),
    };

    let said = with(None, &news).expect("a line");

    assert!(said.starts_with(RESTARTED), "{said}");
    assert!(
        said.contains(
            "chat 7: brought back on its conversation, carrying on; chat 9: still running, \
             as the restart did not stop it; chat 8: ended without a report, as purlis could \
             not bring it back."
        ),
        "{said}"
    );
    assert!(said.contains("dispatch none of them again"), "{said}");
}

#[test]
fn reports_that_waited_are_named_by_chat_and_how_each_ended() {
    let news = News {
        restored: Vec::new(),
        reported: vec![
            Reported {
                chat: 7,
                ended: "done",
            },
            Reported {
                chat: 8,
                ended: "blocked",
            },
        ],
    };

    let said = with(None, &news).expect("a line");

    assert!(
        said.contains("chat 7 reported (done); chat 8 reported (blocked)"),
        "{said}"
    );
    assert!(said.contains("handed to this turn as context"), "{said}");
}

#[test]
fn the_news_follows_a_line_the_chat_is_given_already() {
    let news = News {
        restored: vec![restored(7, Standing::CarriesOn)],
        reported: Vec::new(),
    };

    let said = with(Some("carry on.".to_owned()), &news).expect("a line");

    assert!(said.starts_with("carry on. The tasks this chat"), "{said}");
}

#[test]
fn a_long_list_names_the_first_ones_and_counts_the_rest() {
    let news = News {
        restored: (1..=25).map(|n| restored(n, Standing::CarriesOn)).collect(),
        reported: Vec::new(),
    };

    let said = with(None, &news).expect("a line");

    assert!(said.contains("chat 20: "), "{said}");
    assert!(!said.contains("chat 21: "), "{said}");
    assert!(said.contains("; and 5 more."), "{said}");
}
