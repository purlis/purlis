// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the system's notifications are told for each act of the notifier (#6059): a chat's
//! update shown under the id it stands under, an answered chat's taken away by it, and an update
//! telling of nothing new kept from a system that cannot replace one.

use std::cell::RefCell;
use std::path::Path;

use super::*;
use crate::asknotify::{Act, Notice};
use crate::planes::PlaneId;

#[derive(Debug, PartialEq, Eq)]
enum Told {
    Shown { id: i32, body: String },
    Removed(i32),
}

/// A system's notifications that keep what they were told.
struct Kept {
    replaces: bool,
    told: RefCell<Vec<Told>>,
}

impl Kept {
    fn replacing(replaces: bool) -> Self {
        Self {
            replaces,
            told: RefCell::new(Vec::new()),
        }
    }
}

impl Center for Kept {
    fn replaces(&self) -> bool {
        self.replaces
    }
    fn show(&self, notice: &Notice) {
        self.told.borrow_mut().push(Told::Shown {
            id: notice.id,
            body: notice.body.clone(),
        });
    }
    fn remove(&self, id: i32) {
        self.told.borrow_mut().push(Told::Removed(id));
    }
}

fn notice(id: i32, replaces: bool, news: bool, body: &str) -> Notice {
    Notice {
        id,
        replaces,
        news,
        chats: vec![(PlaneId::for_tests(Path::new("/a")), 3)],
        title: "steward 12 › chat 3".to_owned(),
        body: body.to_owned(),
        update: None,
        sound: false,
    }
}

#[test]
fn a_chat_s_next_ask_is_shown_under_the_id_its_notification_stands_under() {
    let system = Kept::replacing(true);
    let first = notice(7, false, true, "a: Asks your permission");
    let update = notice(7, true, false, "a: Asks your permission, and 1 more");
    assert_eq!(act(&system, Act::Show(first.clone())), Some(first));
    assert_eq!(act(&system, Act::Show(update.clone())), Some(update));
    assert_eq!(
        system.told.into_inner(),
        [
            Told::Shown {
                id: 7,
                body: "a: Asks your permission".to_owned()
            },
            Told::Shown {
                id: 7,
                body: "a: Asks your permission, and 1 more".to_owned()
            },
        ]
    );
}

#[test]
fn answering_every_ask_of_a_chat_takes_its_notification_away_by_its_id() {
    let system = Kept::replacing(true);
    assert_eq!(act(&system, Act::Remove(7)), None);
    assert_eq!(system.told.into_inner(), [Told::Removed(7)]);
}

#[test]
fn a_system_that_cannot_replace_is_not_shown_an_update_telling_of_no_new_chat() {
    let system = Kept::replacing(false);
    let update = notice(7, true, false, "a: Asks your permission, and 1 more");
    assert_eq!(act(&system, Act::Show(update)), None);
    assert!(system.told.into_inner().is_empty());
}
