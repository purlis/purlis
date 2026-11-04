// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's window set** (#4768, ruled on #4701): its project window, the one window that
//! holds it (ADR 0033 as amended), and its **tab windows**, bare windows of the same project
//! holding a strip and panes but no sidebar. A tab window belongs to its project, not to the
//! window holding it: when the project moves windows its tab windows stay its own, and when
//! the project closes they close.
//!
//! Each tab window has a label of its own, `window-<n>` like a split window, so the
//! capabilities need nothing new; this is where a label is told apart as a tab window, and
//! where the project's own number for it (the one its tabs name in the record) is kept.

use super::{PlaneId, Showing};

/// One tab window: the project it is of, and its number in that project's record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TabWindow {
    pub plane: PlaneId,
    /// Never `0`, which is the project window.
    pub number: u32,
}

impl Showing {
    pub(super) fn tab_windows(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, TabWindow>> {
        self.tab_windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Window `label` is tab window `number` of `plane` from here on. Made before the window
    /// exists, so its first ask ([`Self::tab_window`]) finds it.
    pub fn tab_window_made(&self, label: &str, plane: &PlaneId, number: u32) {
        self.tab_windows().insert(
            label.to_owned(),
            TabWindow {
                plane: plane.clone(),
                number,
            },
        );
    }

    /// What window `label` is a tab window of, or none for a window that is not one.
    pub fn tab_window(&self, label: &str) -> Option<TabWindow> {
        self.tab_windows().get(label).cloned()
    }

    /// Window `label` is no tab window any more: it closed, or its project did. Answers what
    /// it was, once.
    pub fn tab_window_gone(&self, label: &str) -> Option<TabWindow> {
        self.tab_windows().remove(label)
    }

    /// The label of tab window `number` of `plane`, where it is open.
    pub fn tab_window_label(&self, plane: &PlaneId, number: u32) -> Option<String> {
        self.tab_windows()
            .iter()
            .find(|(_, tab)| tab.plane == *plane && tab.number == number)
            .map(|(label, _)| label.clone())
    }

    /// Every tab window of `plane` that is open, by label, in number order.
    pub fn tab_windows_of(&self, plane: &PlaneId) -> Vec<(String, u32)> {
        let mut found: Vec<(String, u32)> = self
            .tab_windows()
            .iter()
            .filter(|(_, tab)| tab.plane == *plane)
            .map(|(label, tab)| (label.clone(), tab.number))
            .collect();
        found.sort_by_key(|(_, number)| *number);
        found
    }

    /// **The window set** of `plane`: the window holding it, where one does, then each of its
    /// tab windows. A project's events go to every one of them.
    pub fn window_set(&self, plane: &PlaneId) -> Vec<String> {
        self.holder(plane)
            .into_iter()
            .chain(
                self.tab_windows_of(plane)
                    .into_iter()
                    .map(|(label, _)| label),
            )
            .collect()
    }

    /// Which of `plane`'s windows `label` is: `0` for the window holding it, a tab window's
    /// number, or none for a window that is neither. What a window's report of its tabs is
    /// filed under, and why a window cannot report another's.
    pub fn window_of(&self, label: &str, plane: &PlaneId) -> Option<u32> {
        if let Some(tab) = self.tab_window(label) {
            return (tab.plane == *plane).then_some(tab.number);
        }
        let holding = self.holding(label)?;
        holding.planes.contains(plane).then_some(0)
    }

    /// **The window a notification about a chat asks** whether the person is looking: the one
    /// whose front tab shows it, `showing` (`Chats::window_showing`), where that is an open tab
    /// window; and otherwise the window holding the project. A chat in a tab window is asked of
    /// that window, so the project window having the keyboard never holds back a notification
    /// about a chat the person cannot see there.
    pub fn asked_of(&self, plane: &PlaneId, showing: Option<u32>) -> Option<String> {
        showing
            .filter(|number| *number != 0)
            .and_then(|number| self.tab_window_label(plane, number))
            .or_else(|| self.holder(plane))
    }
}
