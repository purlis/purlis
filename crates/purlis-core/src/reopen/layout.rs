// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every tab as a tree of panes** (#4751, ruled on #4587): what the window had on its strips,
//! kept so a split, its sizes, its views and the tab in front of each strip come back after a
//! restart.
//!
//! A tab is a binary tree. A **node** is a pane or a split; a **split** holds a direction, the
//! first child's share in tenths of a percent and exactly two children; a **pane** holds a chat
//! by its number, a view, or nothing. A tab records the strip it is on (its workspace, or none
//! for the strip of chats outside every workspace), its pin, its window, whether it is its
//! strip's front tab, and its focused pane as a path from the root. Pane numbers are the
//! window's and launch-local, so a pane is named here by its path.
//!
//! **What is on screen is derived from this and nothing else** ([`Layout::on_screen`]): every
//! chat in a pane of the front tab of the drawn strip in the project window, and of the front
//! tab of each other window. Empty panes and views count as nothing.
//!
//! **Held to a shape on the way in**, for the reason every field of the record is: this file is
//! writable by whoever can write the plane's state directory. A node that is not one reads as an
//! empty pane, a share that is not one reads as even, a chat named twice keeps its first pane
//! and empties the rest, and a tab past the bounds is dropped. Nothing here starts anything: a
//! chat in a pane is only drawn if the record also brought that chat back.

use super::{View, ViewOnDisk};

#[path = "layout_windows.rs"]
mod windows;
pub use windows::{Away, MOST_TAB_WINDOWS, Refused};

/// The most tabs one record puts back: far more than a strip draws, so it only ever bounds a
/// hand-written record.
pub const MOST_TABS: usize = 256;

/// The most panes one tab holds, empty ones counted: a tab is split by a person, by hand.
pub const MOST_PANES: usize = 64;

/// The deepest a tab's tree goes. A node below it reads as an empty pane.
pub const DEEPEST: usize = 16;

/// The whole of a split, which a share is a part of, in tenths of a percent.
pub const WHOLE: u16 = 1000;

/// An even split: what a new split starts at and what a share that is not one reads as.
pub const EVEN: u16 = 500;

/// Every tab the window had, and which strip its project window drew.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    /// The strip the project window drew: a workspace, or `None` for the strip of chats outside
    /// every workspace.
    pub strip: Option<String>,
    /// Every tab, in each strip's order from the left.
    pub tabs: Vec<Tab>,
}

/// One tab: a layout of panes the person made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    /// The strip it is on: a workspace, or `None` for the strip outside every workspace. A
    /// fact the person made, which nothing else knows: a chat is still filed by where it works,
    /// never by the tab it sits in (ADR 0036 as amended by #4587).
    pub workspace: Option<String>,
    /// Whether the tab is pinned.
    pub pinned: bool,
    /// The window it is in: `0` for the project window, else one of its tab windows.
    pub window: u32,
    /// Whether it is the front tab of its strip in its window.
    pub front: bool,
    /// The focused pane, as a path from the root. Always ends at a pane once read.
    pub focused: Vec<Side>,
    /// What it holds.
    pub tree: Node,
}

/// Which child of a split a path goes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The first child: left of a row, top of a column.
    A,
    /// The second child.
    B,
}

impl Side {
    /// A path as the record and the window spell it, `a` and `b` from the root, read as far as
    /// it is one.
    pub fn path_of(word: &str) -> Vec<Self> {
        word.chars()
            .map_while(|side| match side {
                'a' => Some(Self::A),
                'b' => Some(Self::B),
                _ => None,
            })
            .collect()
    }

    /// `path` as the record and the window spell it.
    pub fn word_of(path: &[Self]) -> String {
        path.iter()
            .map(|side| match side {
                Self::A => 'a',
                Self::B => 'b',
            })
            .collect()
    }
}

/// A pane or a split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Pane(Pane),
    Split(Box<Split>),
}

/// Two nodes side by side or one above the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub direction: Direction,
    /// The first child's share, in tenths of a percent: `0..=`[`WHOLE`].
    pub share: u16,
    pub a: Node,
    pub b: Node,
}

/// Which way a split divides: `Row` side by side, `Column` one above the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Row,
    Column,
}

/// What a pane holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pane {
    /// A chat, by its number in this plane.
    Chat(u32),
    /// A view: a [`View`]'s own fields, without the ones a tab holds.
    View(PaneView),
    /// Nothing.
    Empty,
}

/// A view in a pane: who draws it, which of theirs, what it is about, what it is called, and
/// where its own divider was. The rules are [`View`]'s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneView {
    pub from: Option<String>,
    pub view: String,
    pub key: String,
    pub title: String,
    pub split: Option<u8>,
}

impl PaneView {
    /// This view as a tab of its own on `workspace`'s strip: the shape the rules for a view
    /// (and a workspace rename) are written against.
    pub fn as_tab(&self, workspace: Option<String>) -> View {
        View {
            from: self.from.clone(),
            view: self.view.clone(),
            key: self.key.clone(),
            title: self.title.clone(),
            workspace,
            at: 0,
            active: false,
            pinned: false,
            split: self.split,
        }
    }

    /// The pane's part of `view`.
    pub fn of(view: View) -> Self {
        Self {
            from: view.from,
            view: view.view,
            key: view.key,
            title: view.title,
            split: view.split,
        }
    }
}

impl Node {
    /// A pane showing chat `number`.
    pub fn chat(number: u32) -> Self {
        Self::Pane(Pane::Chat(number))
    }

    /// `a` and `b` split `direction`, `a` holding `share` of it.
    pub fn split(direction: Direction, share: u16, a: Node, b: Node) -> Self {
        Self::Split(Box::new(Split {
            direction,
            share,
            a,
            b,
        }))
    }

    /// Every chat in a pane of this tree, in reading order (each split's first child first).
    pub fn chats(&self) -> Vec<u32> {
        let mut found = Vec::new();
        self.each_pane(&mut |pane| {
            if let Pane::Chat(number) = pane {
                found.push(*number);
            }
        });
        found
    }

    /// Whether any pane of this tree holds a view.
    pub fn holds_a_view(&self) -> bool {
        let mut any = false;
        self.each_pane(&mut |pane| any |= matches!(pane, Pane::View(_)));
        any
    }

    fn each_pane(&self, see: &mut dyn FnMut(&Pane)) {
        match self {
            Self::Pane(pane) => see(pane),
            Self::Split(split) => {
                split.a.each_pane(see);
                split.b.each_pane(see);
            }
        }
    }

    fn each_pane_mut(&mut self, change: &mut dyn FnMut(&mut Pane)) {
        match self {
            Self::Pane(pane) => change(pane),
            Self::Split(split) => {
                split.a.each_pane_mut(change);
                split.b.each_pane_mut(change);
            }
        }
    }

    /// This tree with every pane of a chat `gone` names taken out, and where `focused` (a path
    /// into it, or `None` where the focus is not in it) is now: a split that loses one child
    /// is its other child, and a focus that was in the child taken out goes to the one kept.
    /// `None` where nothing is left.
    fn without(
        self,
        gone: &dyn Fn(u32) -> bool,
        focused: Option<&[Side]>,
    ) -> Option<(Self, Option<Vec<Side>>)> {
        match self {
            Self::Pane(Pane::Chat(number)) if gone(number) => None,
            Self::Pane(pane) => Some((Self::Pane(pane), focused.map(|_| Vec::new()))),
            Self::Split(split) => {
                let Split {
                    direction,
                    share,
                    a,
                    b,
                } = *split;
                // A path that ends at a split goes on into its first child (`to_a_pane`).
                let (in_a, in_b) = match focused {
                    Some([Side::B, rest @ ..]) => (None, Some(rest)),
                    Some([Side::A, rest @ ..]) => (Some(rest), None),
                    Some([]) => (Some(&[][..]), None),
                    None => (None, None),
                };
                let was_focused = focused.is_some();
                match (a.without(gone, in_a), b.without(gone, in_b)) {
                    (Some((a, at_a)), Some((b, at_b))) => {
                        let focused = at_a
                            .map(|path| [&[Side::A][..], &path].concat())
                            .or_else(|| at_b.map(|path| [&[Side::B][..], &path].concat()));
                        Some((Self::split(direction, share, a, b), focused))
                    }
                    (Some((kept, at)), None) | (None, Some((kept, at))) => {
                        Some((kept, at.or_else(|| was_focused.then(Vec::new))))
                    }
                    (None, None) => None,
                }
            }
        }
    }

    /// `path` as far as it goes in this tree, then on into each split's first child until it
    /// ends at a pane.
    pub fn to_a_pane(&self, path: &[Side]) -> Vec<Side> {
        let mut held = Vec::new();
        let mut at = self;
        let mut rest = path.iter();
        while let Self::Split(split) = at {
            let side = rest.next().copied().unwrap_or(Side::A);
            held.push(side);
            at = match side {
                Side::A => &split.a,
                Side::B => &split.b,
            };
        }
        held
    }
}

impl Tab {
    /// A tab of one pane showing chat `number`, on `workspace`'s strip in the project window.
    pub fn of_chat(workspace: Option<&str>, number: u32) -> Self {
        Self {
            workspace: workspace.map(str::to_owned),
            pinned: false,
            window: 0,
            front: false,
            focused: Vec::new(),
            tree: Node::chat(number),
        }
    }
}

impl Layout {
    /// **The chats on screen**: every chat in a pane of the front tab of the drawn strip in the
    /// project window, and of the front tab of each tab window, with no window away
    /// ([`Self::on_screen_but`]). In the order of their numbers.
    pub fn on_screen(&self) -> Vec<u32> {
        self.on_screen_but(&Away::default())
    }

    /// **The chat in front**: the first chat in reading order of the project window's front
    /// tab on its drawn strip, or none where that tab holds no chat.
    pub fn lead(&self) -> Option<u32> {
        self.tabs
            .iter()
            .find(|tab| tab.window == 0 && tab.front && tab.workspace == self.strip)
            .and_then(|tab| tab.tree.chats().first().copied())
    }

    /// This layout with each tab's focused path ending at a pane, as a record reads it back, so
    /// the focused pane the core holds is the one the next launch reads.
    #[must_use]
    pub fn held(mut self) -> Self {
        for tab in &mut self.tabs {
            tab.focused = tab.tree.to_a_pane(&tab.focused);
        }
        self
    }

    /// **This layout with every pane of a chat `gone` names taken out** (#4761): what a
    /// one-job program's split leaves, since one is never recorded (spec #4750). The pane it
    /// was beside takes its split's place, as the window draws it once the program exits, and
    /// takes the focus where the one-job pane had it. A tab with nothing else in it keeps an
    /// empty pane, so no tab goes because of one.
    #[must_use]
    pub fn without(mut self, gone: impl Fn(u32) -> bool) -> Self {
        for tab in &mut self.tabs {
            let tree = std::mem::replace(&mut tab.tree, Node::Pane(Pane::Empty));
            match tree.without(&gone, Some(&tab.focused)) {
                Some((tree, focused)) => {
                    tab.focused = tree.to_a_pane(&focused.unwrap_or_default());
                    tab.tree = tree;
                }
                None => tab.focused = Vec::new(),
            }
        }
        self
    }

    /// Every chat in a pane of any tab, in strip order.
    pub fn chats(&self) -> Vec<u32> {
        self.tabs.iter().flat_map(|tab| tab.tree.chats()).collect()
    }

    /// The first chat this layout names in two panes, across every tab and window.
    pub fn named_twice(&self) -> Option<u32> {
        let mut seen = std::collections::HashSet::new();
        self.chats()
            .into_iter()
            .find(|number| !seen.insert(*number))
    }

    /// Whether any pane holds a view.
    pub fn holds_a_view(&self) -> bool {
        self.tabs.iter().any(|tab| tab.tree.holds_a_view())
    }

    /// Chat `from` is chat `to` now (started again under a new number): every pane that showed
    /// it shows it still. `true` when a pane changed.
    pub fn renumber(&mut self, from: u32, to: u32) -> bool {
        let mut changed = false;
        for tab in &mut self.tabs {
            tab.tree.each_pane_mut(&mut |pane| {
                if *pane == Pane::Chat(from) {
                    *pane = Pane::Chat(to);
                    changed = true;
                }
            });
        }
        changed
    }

    /// Each view pane, with the strip of the tab it is in, for a change that follows a rename.
    pub fn each_view_mut(&mut self, change: &mut dyn FnMut(&mut PaneView, &Option<String>)) {
        for tab in &mut self.tabs {
            let workspace = tab.workspace.clone();
            tab.tree.each_pane_mut(&mut |pane| {
                if let Pane::View(view) = pane {
                    change(view, &workspace);
                }
            });
        }
    }
}

/// One tab as JSON. Every field is the plainest type JSON has, so reading cannot fail on one,
/// and [`TabOnDisk::held`] is where each is held to what it has to be.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct TabOnDisk {
    /// The workspace's name, or empty for the strip outside every workspace.
    #[serde(default)]
    workspace: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pinned: bool,
    /// `0`, the project window, is written as nothing.
    #[serde(default, skip_serializing_if = "is_zero")]
    window: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    front: bool,
    /// The focused pane's path from the root: `a` for a split's first child and `b` for its
    /// second, so `""` is a tab of one pane and `"ba"` the first child of the second.
    #[serde(default)]
    focused: String,
    /// The tree, read by hand so a node that is not one is an empty pane rather than a record
    /// that does not read.
    #[serde(default)]
    tree: serde_json::Value,
}

fn is_zero(number: &u32) -> bool {
    *number == 0
}

fn workspace_of(word: String) -> Option<String> {
    Some(word).filter(|name| crate::contain::workspace_name_ok(name))
}

impl TabOnDisk {
    /// The tab this line names, or nothing where it is past [`MOST_PANES`]. `seen` is every
    /// chat an earlier pane named, so a chat named twice keeps its first pane.
    fn held(self, seen: &mut std::collections::HashSet<u32>) -> Option<Tab> {
        // Read against its own set first: a tab dropped whole takes nothing from the tabs after
        // it, and a chat it names may be put back in one of them.
        let mut panes = 0;
        let mut named = seen.clone();
        let tree = node_of(&self.tree, 0, &mut panes, &mut named);
        if panes > MOST_PANES {
            return None;
        }
        *seen = named;
        let path: Vec<Side> = Side::path_of(&self.focused)
            .into_iter()
            .take(DEEPEST)
            .collect();
        Some(Tab {
            workspace: workspace_of(self.workspace),
            pinned: self.pinned,
            window: self.window,
            front: self.front,
            focused: tree.to_a_pane(&path),
            tree,
        })
    }

    fn of(tab: &Tab) -> Self {
        Self {
            workspace: tab.workspace.clone().unwrap_or_default(),
            pinned: tab.pinned,
            window: tab.window,
            front: tab.front,
            focused: Side::word_of(&tab.focused),
            tree: value_of(&tab.tree),
        }
    }
}

/// The node `value` names, or an empty pane where it names none. `panes` counts the panes read.
fn node_of(
    value: &serde_json::Value,
    depth: usize,
    panes: &mut usize,
    seen: &mut std::collections::HashSet<u32>,
) -> Node {
    let Some(object) = value.as_object() else {
        *panes += 1;
        return Node::Pane(Pane::Empty);
    };
    if let (Some(a), Some(b)) = (object.get("a"), object.get("b"))
        && depth < DEEPEST
    {
        let direction = match object.get("direction").and_then(serde_json::Value::as_str) {
            Some("column") => Direction::Column,
            _ => Direction::Row,
        };
        let share = object
            .get("share")
            .and_then(serde_json::Value::as_u64)
            .and_then(|share| u16::try_from(share).ok())
            .filter(|share| *share <= WHOLE)
            .unwrap_or(EVEN);
        let a = node_of(a, depth + 1, panes, seen);
        let b = node_of(b, depth + 1, panes, seen);
        return Node::split(direction, share, a, b);
    }
    *panes += 1;
    if let Some(number) = object.get("chat").and_then(serde_json::Value::as_u64) {
        return match u32::try_from(number) {
            Ok(number) if number > 0 && seen.insert(number) => Node::chat(number),
            _ => Node::Pane(Pane::Empty),
        };
    }
    if let Some(view) = object.get("view") {
        let held = serde_json::from_value::<ViewOnDisk>(view.clone())
            .ok()
            .and_then(ViewOnDisk::held);
        return Node::Pane(held.map_or(Pane::Empty, |view| Pane::View(PaneView::of(view))));
    }
    Node::Pane(Pane::Empty)
}

fn value_of(node: &Node) -> serde_json::Value {
    match node {
        Node::Pane(Pane::Chat(number)) => serde_json::json!({ "chat": number }),
        Node::Pane(Pane::Empty) => serde_json::json!({ "empty": true }),
        Node::Pane(Pane::View(view)) => {
            let mut written = serde_json::to_value(ViewOnDisk::from(&view.as_tab(None)))
                .expect("a view is plain data serde can always write");
            // A view in a pane has none of a tab's own fields: the tab holds those.
            if let Some(object) = written.as_object_mut() {
                for tab_level in ["workspace", "at", "active", "pinned"] {
                    object.remove(tab_level);
                }
            }
            serde_json::json!({ "view": written })
        }
        Node::Split(split) => serde_json::json!({
            "direction": match split.direction {
                Direction::Row => "row",
                Direction::Column => "column",
            },
            "share": split.share,
            "a": value_of(&split.a),
            "b": value_of(&split.b),
        }),
    }
}

/// The layout a record's `tabs` and `strip` name. `tabs` is `None` for a record written before
/// tabs were recorded, which is read the old way.
pub(super) fn held(tabs: Option<Vec<serde_json::Value>>, strip: String) -> Option<Layout> {
    let tabs = tabs?;
    let mut seen = std::collections::HashSet::new();
    Some(
        Layout {
            strip: workspace_of(strip),
            tabs: tabs
                .into_iter()
                .take(MOST_TABS)
                .filter_map(|tab| serde_json::from_value::<TabOnDisk>(tab).ok())
                .filter_map(|tab| tab.held(&mut seen))
                .collect(),
        }
        .held_to_most_windows(),
    )
}

/// `layout`'s `tabs` and `strip`, as a record writes them.
pub(super) fn on_disk(layout: &Layout) -> (Vec<serde_json::Value>, String) {
    (
        layout
            .tabs
            .iter()
            .map(|tab| {
                serde_json::to_value(TabOnDisk::of(tab))
                    .expect("a tab is plain data serde can always write")
            })
            .collect(),
        layout.strip.clone().unwrap_or_default(),
    )
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
