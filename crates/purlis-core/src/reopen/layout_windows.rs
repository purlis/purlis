// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's windows** (#4768, ruled on #4701): the project window and its tab windows,
//! as the tabs name them.
//!
//! A **tab window** is a bare window of the same project, holding a strip and panes but no
//! sidebar. Each tab records its window: `0` is the project window, any other number one of its
//! tab windows. A tab window has one strip whatever workspaces its tabs are on, so it has one
//! front tab. A tab window is there while a tab names it, and goes with its last tab.
//!
//! Everything that moves a tab or a pane between windows is here, on the whole layout at once,
//! so a move is one step and never shows a chat twice: the core holds it, and each window
//! draws its own share of what it answers. **What is on screen** spans every window, less the
//! ones that are away ([`Away`]): a minimised or hidden window counts as nothing.

use std::collections::BTreeSet;

use super::{Layout, Node, Pane, Side, Tab};

/// The most tab windows one record puts back: a launch makes a window for each, so a
/// hand-written record cannot ask for hundreds. A tab past them goes back to the project window.
pub const MOST_TAB_WINDOWS: usize = 16;

/// The windows of a project that are not on screen: minimised or hidden. What the window says,
/// as it happens; never recorded, since a launch shows every window it makes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Away(BTreeSet<u32>);

impl Away {
    /// `windows` away.
    pub fn of(windows: impl IntoIterator<Item = u32>) -> Self {
        Self(windows.into_iter().collect())
    }

    /// Window `window` is away, or back. Answers whether that changed anything.
    pub fn set(&mut self, window: u32, away: bool) -> bool {
        if away {
            self.0.insert(window)
        } else {
            self.0.remove(&window)
        }
    }

    /// Whether window `window` is away.
    pub fn holds(&self, window: u32) -> bool {
        self.0.contains(&window)
    }
}

/// Why a move between windows was refused. A refusal changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// No tab is at that place.
    NoSuchTab,
    /// The path does not end at a pane of that tab.
    NoSuchPane,
    /// An empty pane is never picked up.
    EmptyPane,
    /// Nor is a tab of only empty panes.
    EmptyTab,
    /// It is in that window already.
    AlreadyThere,
    /// The project has no window of that number.
    NoSuchWindow(u32),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchTab => write!(f, "that tab is not open in this project"),
            Self::NoSuchPane => write!(f, "that pane is not in the tab"),
            Self::EmptyPane => write!(f, "an empty pane has nothing to move"),
            Self::EmptyTab => write!(f, "an empty tab has nothing to move"),
            Self::AlreadyThere => write!(f, "it is in that window already"),
            Self::NoSuchWindow(n) => write!(f, "this project has no tab window {n}"),
        }
    }
}

impl Layout {
    /// **The chats on screen** with `away` windows counting as nothing: every chat in a pane of
    /// the front tab of the drawn strip in the project window, and of the front tab of each tab
    /// window, where that window is not away. In the order of their numbers.
    pub fn on_screen_but(&self, away: &Away) -> Vec<u32> {
        let mut seen: Vec<u32> = self
            .front_tabs()
            .into_iter()
            .filter(|tab| !away.holds(tab.window))
            .flat_map(|tab| tab.tree.chats())
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen
    }

    /// The front tab of each window: the drawn strip's in the project window, and the first
    /// front tab of each tab window.
    pub(super) fn front_tabs(&self) -> Vec<&Tab> {
        let mut windows = BTreeSet::new();
        self.tabs
            .iter()
            .filter(|tab| tab.front && (tab.window != 0 || tab.workspace == self.strip))
            .filter(|tab| windows.insert(tab.window))
            .collect()
    }

    /// The window whose front tab shows chat `number`, away or not, or none where no front tab
    /// does.
    pub fn window_showing(&self, number: u32) -> Option<u32> {
        self.front_tabs()
            .into_iter()
            .find(|tab| tab.tree.chats().contains(&number))
            .map(|tab| tab.window)
    }

    /// **The window a press on chat `number` goes to** (#4768): the one with a tab that has it
    /// in a pane, in front or not, or none where no tab does. A chat is in one pane at most, so
    /// there is one.
    pub fn window_naming(&self, number: u32) -> Option<u32> {
        self.tabs
            .iter()
            .find(|tab| tab.tree.chats().contains(&number))
            .map(|tab| tab.window)
    }

    /// Where the `nth` tab of window `window` is among every tab: what a window, which knows
    /// only its own tabs, names a tab by.
    pub fn place_of(&self, window: u32, nth: usize) -> Option<usize> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.window == window)
            .nth(nth)
            .map(|(at, _)| at)
    }

    /// Every tab window a tab names, in number order.
    pub fn tab_windows(&self) -> Vec<u32> {
        let windows: BTreeSet<u32> = self
            .tabs
            .iter()
            .map(|tab| tab.window)
            .filter(|window| *window != 0)
            .collect();
        windows.into_iter().collect()
    }

    /// **Window `window`'s own report**: its tabs, in its order, in place of the ones held for
    /// it, and every other window's kept as they were. The project window also says its drawn
    /// strip; a tab window's report keeps the one held. A tab a window reports is its own,
    /// whatever window it names.
    #[must_use]
    pub fn with_window(&self, window: u32, report: Layout) -> Layout {
        let at = self
            .tabs
            .iter()
            .position(|tab| tab.window == window)
            .unwrap_or(self.tabs.len());
        let before = self.tabs[..at].iter().filter(|tab| tab.window != window);
        let after = self.tabs[at..].iter().filter(|tab| tab.window != window);
        let reported = report.tabs.into_iter().map(|mut tab| {
            tab.window = window;
            tab
        });
        Layout {
            strip: if window == 0 {
                report.strip
            } else {
                self.strip.clone()
            },
            tabs: before
                .cloned()
                .chain(reported)
                .chain(after.cloned())
                .collect(),
        }
    }

    /// **Tab window `window` closed**: each of its tabs goes back to the project window, behind
    /// the front tab of its strip there, in its order, and nothing it held comes in front. A
    /// tab whose strip has no tab in the project window is that strip's front. Answers whether
    /// any tab moved.
    pub fn send_back(&mut self, window: u32) -> bool {
        if window == 0 {
            return false;
        }
        let (going, staying): (Vec<Tab>, Vec<Tab>) = std::mem::take(&mut self.tabs)
            .into_iter()
            .partition(|tab| tab.window == window);
        self.tabs = staying;
        let moved = !going.is_empty();
        // Where the next tab of each strip goes: just behind the tabs sent back before it.
        let mut next: Vec<(Option<String>, usize)> = Vec::new();
        for mut tab in going {
            tab.window = 0;
            tab.front = false;
            let strip = tab.workspace.clone();
            let place = match next.iter().find(|(one, _)| *one == strip) {
                Some((_, at)) => *at,
                None => {
                    let home = |one: &Tab| one.window == 0 && one.workspace == strip;
                    let front = self.tabs.iter().position(|one| home(one) && one.front);
                    let last_home = self.tabs.iter().rposition(home);
                    match front.or(last_home) {
                        Some(found) => found + 1,
                        None => {
                            tab.front = true;
                            self.tabs
                                .iter()
                                .rposition(|one| one.window == 0)
                                .map_or(0, |last| last + 1)
                        }
                    }
                }
            };
            self.tabs.insert(place, tab);
            for (_, at) in &mut next {
                if *at >= place {
                    *at += 1;
                }
            }
            match next.iter_mut().find(|(one, _)| *one == strip) {
                Some((_, at)) => *at = place + 1,
                None => next.push((strip, place + 1)),
            }
        }
        moved
    }

    /// **Move to new window** (`to` none) **or Move to window ▸** (`to` a window of this
    /// project): tab `at` leaves its window and is the front tab of `to`, at the end of its
    /// strip. Answers the window it is in now.
    ///
    /// The tab beside it comes forward where it was the front. The drawn strip's last tab in
    /// the project window leaves an empty pane behind, so the strip is never bare; a tab
    /// window's last tab leaving takes the window with it. A tab moved into the project window
    /// brings its strip forward there. A new window is the lowest number no tab names. A tab of
    /// only empty panes is never picked up: it has nothing to move.
    pub fn move_tab(&mut self, at: usize, to: Option<u32>) -> Result<u32, Refused> {
        let tab = self.tabs.get(at).ok_or(Refused::NoSuchTab)?;
        if tab.tree.chats().is_empty() && !tab.tree.holds_a_view() {
            return Err(Refused::EmptyTab);
        }
        let to = self.window_for(tab.window, to)?;
        let mut tab = self.tabs.remove(at);
        self.left(&tab, at);
        self.land(&mut tab, to);
        self.tabs.push(tab);
        Ok(to)
    }

    /// **A pane's Move to new window or Move to window ▸**: the pane at `path` in tab `at`
    /// leaves its split to its sibling and lands as a tab of its own, the front tab of `to`, on
    /// its tab's strip. A tab's only pane moves as the tab. An empty pane is never picked up.
    pub fn move_pane(&mut self, at: usize, path: &[Side], to: Option<u32>) -> Result<u32, Refused> {
        let tab = self.tabs.get(at).ok_or(Refused::NoSuchTab)?;
        match tab.tree.at(path) {
            Some(Node::Pane(Pane::Empty)) => return Err(Refused::EmptyPane),
            Some(Node::Pane(_)) => {}
            _ => return Err(Refused::NoSuchPane),
        }
        if path.is_empty() {
            return self.move_tab(at, to);
        }
        let to = self.window_for(tab.window, to)?;
        let source = &mut self.tabs[at];
        let pane = source.tree.take_out(path);
        source.focused = after_taking_out(&source.focused, path);
        source.focused = source.tree.to_a_pane(&source.focused);
        let mut moved = Tab {
            workspace: source.workspace.clone(),
            pinned: false,
            window: to,
            front: true,
            focused: Vec::new(),
            tree: pane,
        };
        self.land(&mut moved, to);
        self.tabs.push(moved);
        Ok(to)
    }

    /// The window a move from `from` goes to: `to`, where it is one of the project's windows and
    /// not `from`, or the lowest number no tab names.
    fn window_for(&self, from: u32, to: Option<u32>) -> Result<u32, Refused> {
        match to {
            Some(to) if to == from => Err(Refused::AlreadyThere),
            Some(0) => Ok(0),
            Some(to) if self.tab_windows().contains(&to) => Ok(to),
            Some(to) => Err(Refused::NoSuchWindow(to)),
            None => {
                let taken = self.tab_windows();
                Ok((1..).find(|n| !taken.contains(n)).unwrap_or(u32::MAX))
            }
        }
    }

    /// Tab `gone` left place `at`: where it was its strip's front tab in its window, the tab
    /// beside it comes forward, or an empty pane takes its place on the project window's strip.
    fn left(&mut self, gone: &Tab, at: usize) {
        if !gone.front {
            return;
        }
        let same = |tab: &Tab| {
            tab.window == gone.window && (gone.window != 0 || tab.workspace == gone.workspace)
        };
        let beside = self.tabs[at..]
            .iter()
            .position(same)
            .map(|found| at + found)
            .or_else(|| self.tabs[..at].iter().rposition(same));
        match beside {
            Some(beside) => self.tabs[beside].front = true,
            None if gone.window == 0 => {
                let mut empty = Tab::of_chat(gone.workspace.as_deref(), 0);
                empty.tree = Node::Pane(Pane::Empty);
                empty.front = true;
                self.tabs.insert(at, empty);
            }
            None => {}
        }
    }

    /// `tab` lands in window `to` as its front tab: every other tab there (on its strip, in the
    /// project window) goes behind it, and the project window draws its strip.
    fn land(&mut self, tab: &mut Tab, to: u32) {
        tab.window = to;
        tab.front = true;
        for other in &mut self.tabs {
            if other.window == to && (to != 0 || other.workspace == tab.workspace) {
                other.front = false;
            }
        }
        if to == 0 {
            self.strip = tab.workspace.clone();
        }
    }

    /// Holds the record's tab windows to [`MOST_TAB_WINDOWS`]: a tab of a window past them goes
    /// back to the project window, behind.
    pub(super) fn held_to_most_windows(mut self) -> Self {
        let mut kept = Vec::new();
        for tab in &mut self.tabs {
            if tab.window == 0 || kept.contains(&tab.window) {
                continue;
            }
            if kept.len() < MOST_TAB_WINDOWS {
                kept.push(tab.window);
            } else {
                tab.window = 0;
                tab.front = false;
            }
        }
        self
    }
}

impl Node {
    /// The node at `path`, or none where the path goes past a pane.
    pub fn at(&self, path: &[Side]) -> Option<&Node> {
        let Some((side, rest)) = path.split_first() else {
            return Some(self);
        };
        match self {
            Self::Pane(_) => None,
            Self::Split(split) => match side {
                Side::A => split.a.at(rest),
                Side::B => split.b.at(rest),
            },
        }
    }

    /// Takes the node at `path` (not empty, and ending at a node) out of this tree: its sibling
    /// takes its parent's place. Answers what was taken.
    fn take_out(&mut self, path: &[Side]) -> Node {
        let (last, parent) = path.split_last().expect("a path into a split");
        let mut at = self;
        for side in parent {
            let Self::Split(split) = at else {
                unreachable!("the path was read as far as a node")
            };
            at = match side {
                Side::A => &mut split.a,
                Side::B => &mut split.b,
            };
        }
        let empty = Node::Pane(Pane::Empty);
        let Self::Split(split) = std::mem::replace(at, empty) else {
            unreachable!("the path's parent is a split")
        };
        let split = *split;
        let (taken, sibling) = match last {
            Side::A => (split.a, split.b),
            Side::B => (split.b, split.a),
        };
        *at = sibling;
        taken
    }
}

/// The focused path `focused` once the node at `taken` is out of its tree: on the sibling, where
/// it was on the taken node, and one step shorter, where it went through the sibling.
fn after_taking_out(focused: &[Side], taken: &[Side]) -> Vec<Side> {
    let (last, parent) = taken.split_last().expect("a path into a split");
    if !focused.starts_with(parent) || focused.len() == parent.len() {
        return focused.to_vec();
    }
    if focused[parent.len()] == *last {
        return parent.to_vec();
    }
    let mut kept = parent.to_vec();
    kept.extend_from_slice(&focused[parent.len() + 1..]);
    kept
}

#[cfg(test)]
#[path = "layout_windows_tests.rs"]
mod tests;
