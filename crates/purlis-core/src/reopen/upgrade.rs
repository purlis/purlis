// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The switch: a chat isn't a tab** (#4758, ruled on #4587). A record written before tabs
//! were recorded holds no `tabs` list: every chat that had a tab was one, a chat's `beside`
//! put it in another chat's tab, its `shows` switched its pane to a task, and a pin was the
//! chat's. This reads such a record into a [`Layout`] **once**: the layout is written into the
//! record with the next write, and a record that holds one is never read this way again.
//!
//! What it keeps, as the person ruled:
//! - **the front tab at quit stays as it was**, with whatever it showed;
//! - **a tab with a split, a view or a pin stays**: chats sharing a `beside` become one tab, in a
//!   flat row with even shares; a pinned chat or view becomes a pinned tab;
//! - **a plain one-chat tab closes**, and its chat runs on in the sidebar;
//! - **a strip left with no tab** gets one showing its first chat in the old strip order (pinned
//!   chats were first on it), or an empty pane where it had none. The plane root's strip always
//!   gets one;
//! - a view that was beside a chat was never recorded, so it is lost.
//!
//! **A chat that never had a tab gets none**: a task the person never opened, and a grade's
//! chat. **Nothing here keeps the old model**: there is no setting for it.

use std::collections::HashSet;

use super::layout::{Direction, Layout, Node, Pane, PaneView, Tab, WHOLE};
use super::{Chat, Record};

/// The most asks a task's chain is walked up to find whether it is below a chat: as deep as a
/// dispatch goes, and a bound for a record written by hand.
const DEEPEST_CHAIN: usize = 32;

/// One tab of the old strip, before it is decided whether it stays.
struct OldTab {
    workspace: Option<String>,
    pinned: bool,
    /// Whether it says it was the tab in front at quit.
    front: bool,
    panes: Vec<Pane>,
    /// Whether it held a view, which keeps it.
    view: bool,
    /// The tab's own chat, whatever its pane was switched to; none for a view's tab.
    own: Option<u32>,
}

impl OldTab {
    /// A tab with a split, a view or a pin stays, as the front tab at quit does.
    fn kept(&self) -> bool {
        self.pinned || self.view || self.panes.len() > 1
    }
}

/// **The layout a record written before tabs were recorded stands for**, as ruled for the
/// switch. `workspace_of` is where a chat is filed, which is the strip its tab was on: the
/// plane's answer for its directory, `None` outside every workspace.
pub fn upgraded(record: &Record, workspace_of: &dyn Fn(&Chat) -> Option<String>) -> Layout {
    let chats: Vec<(u32, &Chat)> = {
        let mut seen = HashSet::new();
        record
            .chats
            .iter()
            .filter(|chat| !chat.graded)
            .filter_map(|chat| Some((chat.number?, chat)))
            .filter(|(number, _)| seen.insert(*number))
            .collect()
    };
    let find = |number: u32| chats.iter().find(|(n, _)| *n == number).map(|(_, c)| *c);
    // A tab's own chat: a chat that had a tab of its own, not in another chat's.
    let owns_a_tab = |chat: &Chat| chat.has_tab() && chat.beside.is_none_or(|b| find(b).is_none());
    let mut placed: HashSet<u32> = HashSet::new();

    // The old strips' tabs that held a chat, in the order the strip drew them.
    let mut old: Vec<OldTab> = Vec::new();
    for &(number, chat) in &chats {
        if !owns_a_tab(chat) || placed.contains(&number) {
            continue;
        }
        let mut panes = Vec::new();
        // The tab's own pane, showing the task it was switched to where that is one below it.
        let shown = chat
            .shows
            .filter(|task| *task != number && !placed.contains(task))
            .filter(|task| {
                find(*task).is_some_and(|t| !owns_a_tab(t) && below(&find, *task, number))
            });
        let own = shown.unwrap_or(number);
        placed.insert(own);
        panes.push(Pane::Chat(own));
        for &(beside, other) in &chats {
            if other.beside == Some(number) && beside != number && placed.insert(beside) {
                panes.push(Pane::Chat(beside));
            }
        }
        old.push(OldTab {
            workspace: workspace_of(chat),
            pinned: chat.pinned,
            front: chat.active,
            panes,
            view: false,
            own: Some(number),
        });
    }
    // The view tabs, each at its place among its strip's tabs.
    let mut views: Vec<&super::View> = record.views.iter().collect();
    views.sort_by_key(|view| view.at);
    for view in views {
        let at = old
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.workspace == view.workspace)
            .map(|(index, _)| index)
            .nth(view.at as usize)
            .unwrap_or(old.len());
        old.insert(
            at,
            OldTab {
                workspace: view.workspace.clone(),
                pinned: view.pinned,
                front: view.active,
                panes: vec![Pane::View(PaneView::of(view.clone()))],
                view: true,
                own: None,
            },
        );
    }

    // The front tab at quit: the first that says it was.
    let front_at_quit = old.iter().position(|tab| tab.front);
    let strip = front_at_quit.and_then(|index| old[index].workspace.clone());

    // Every strip the old record drew, the plane root's first.
    let mut strips: Vec<Option<String>> = vec![None];
    for tab in &old {
        if !strips.contains(&tab.workspace) {
            strips.push(tab.workspace.clone());
        }
    }

    let mut tabs = Vec::new();
    for on in strips {
        let mut kept: Vec<(bool, Tab)> = old
            .iter()
            .enumerate()
            .filter(|(index, tab)| {
                tab.workspace == on && (Some(*index) == front_at_quit || tab.kept())
            })
            .map(|(index, tab)| (Some(index) == front_at_quit, tab_of(tab)))
            .collect();
        if kept.is_empty() {
            // A strip left with no tab shows its first chat itself, never the task its tab was
            // switched to, or nothing.
            let first = old
                .iter()
                .filter(|tab| tab.workspace == on)
                .filter_map(|tab| tab.own.map(|own| (tab.pinned, own)))
                .min_by_key(|(pinned, _)| !pinned)
                .map_or(Pane::Empty, |(_, own)| Pane::Chat(own));
            kept.push((
                true,
                Tab {
                    tree: Node::Pane(first),
                    ..tab_of(&OldTab {
                        workspace: on.clone(),
                        pinned: false,
                        front: false,
                        panes: Vec::new(),
                        view: false,
                        own: None,
                    })
                },
            ));
        }
        // Pinned tabs first on their strip, as they always were.
        kept.sort_by_key(|(_, tab)| !tab.pinned);
        let front = kept.iter().position(|(front, _)| *front).unwrap_or(0);
        for (index, (_, mut tab)) in kept.into_iter().enumerate() {
            tab.front = index == front;
            tabs.push(tab);
        }
    }
    Layout { strip, tabs }.held()
}

/// The tab `old` becomes: its panes in a flat row with even shares.
fn tab_of(old: &OldTab) -> Tab {
    Tab {
        workspace: old.workspace.clone(),
        pinned: old.pinned,
        window: 0,
        front: false,
        focused: Vec::new(),
        tree: row_of(&old.panes),
    }
}

/// `panes` side by side, each as wide as the others: the first child of each split holds one
/// pane's share of what the split holds.
fn row_of(panes: &[Pane]) -> Node {
    match panes {
        [] => Node::Pane(Pane::Empty),
        [one] => Node::Pane(one.clone()),
        [first, rest @ ..] => {
            let count = u16::try_from(panes.len()).unwrap_or(u16::MAX);
            Node::split(
                Direction::Row,
                WHOLE / count,
                Node::Pane(first.clone()),
                row_of(rest),
            )
        }
    }
}

/// Whether chat `task` was asked for, at any depth, by chat `asker`.
fn below<'a>(find: &dyn Fn(u32) -> Option<&'a Chat>, task: u32, asker: u32) -> bool {
    let mut at = task;
    for _ in 0..DEEPEST_CHAIN {
        let Some(above) = find(at)
            .and_then(|chat| chat.from.as_ref())
            .map(|from| from.chat)
        else {
            return false;
        };
        if above == asker {
            return true;
        }
        at = above;
    }
    false
}

#[cfg(test)]
#[path = "upgrade_tests.rs"]
mod tests;
