// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's tabs, as the core holds them** (#4751): every tab as a tree of panes, told to
//! the core in one command whenever it changes, and asked for once at a launch.
//!
//! The shape is `purlis_core::reopen::layout`'s, spelt for the window: a pane is a chat, a view
//! or nothing; a split is a direction, the first child's share in tenths of a percent, and two
//! children; a tab is its strip, its pin, its window, whether it is its strip's front tab, its
//! focused pane's path (`a` and `b` from the root) and its tree.

use purlis_core::reopen::layout::{self as core, Direction, Side};

use crate::views::ViewTab;

/// Every tab the window has, and the strip its project window draws.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub(crate) struct LaidOut {
    /// The strip drawn: a workspace, or `null` for the strip outside every workspace.
    pub strip: Option<String>,
    /// Every tab, in each strip's order from the left.
    pub tabs: Vec<LaidTab>,
}

/// One tab.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub(crate) struct LaidTab {
    /// Its strip: a workspace, or `null` for the strip outside every workspace.
    pub workspace: Option<String>,
    pub pinned: bool,
    /// `0` for the project window.
    pub window: u32,
    /// Whether it is its strip's front tab.
    pub front: bool,
    /// The focused pane's path from the root: `a` for a split's first child, `b` for its second.
    pub focused: String,
    pub tree: LaidNode,
}

/// A pane (a chat by its number, a view as a view tab spells it, or nothing) or a split: two
/// nodes side by side (`row`) or one above the other (`column`), the first holding `share` in
/// tenths of a percent (0 to 1000). A view's `workspace`, `at`, `active` and `pinned` are its
/// tab's and are not read here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum LaidNode {
    Chat {
        chat: u32,
    },
    View {
        view: ViewTab,
    },
    Empty,
    Split {
        direction: LaidDirection,
        share: u16,
        a: Box<LaidNode>,
        b: Box<LaidNode>,
    },
}

/// Which way a split divides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LaidDirection {
    Row,
    Column,
}

impl LaidNode {
    fn held(self) -> core::Node {
        match self {
            Self::Chat { chat } => core::Node::chat(chat),
            Self::View { view } => {
                core::Node::Pane(core::Pane::View(core::PaneView::of(view.into())))
            }
            Self::Empty => core::Node::Pane(core::Pane::Empty),
            Self::Split {
                direction,
                share,
                a,
                b,
            } => core::Node::split(
                match direction {
                    LaidDirection::Row => Direction::Row,
                    LaidDirection::Column => Direction::Column,
                },
                share.min(core::WHOLE),
                a.held(),
                b.held(),
            ),
        }
    }

    fn of(node: core::Node, workspace: &Option<String>) -> Self {
        match node {
            core::Node::Pane(core::Pane::Chat(chat)) => Self::Chat { chat },
            core::Node::Pane(core::Pane::View(view)) => Self::View {
                view: view.as_tab(workspace.clone()).into(),
            },
            core::Node::Pane(core::Pane::Empty) => Self::Empty,
            core::Node::Split(split) => {
                let split = *split;
                Self::Split {
                    direction: match split.direction {
                        Direction::Row => LaidDirection::Row,
                        Direction::Column => LaidDirection::Column,
                    },
                    share: split.share,
                    a: Box::new(Self::of(split.a, workspace)),
                    b: Box::new(Self::of(split.b, workspace)),
                }
            }
        }
    }
}

impl From<LaidOut> for core::Layout {
    fn from(laid: LaidOut) -> Self {
        Self {
            strip: laid.strip,
            tabs: laid
                .tabs
                .into_iter()
                .map(|tab| core::Tab {
                    workspace: tab.workspace,
                    pinned: tab.pinned,
                    window: tab.window,
                    front: tab.front,
                    focused: Side::path_of(&tab.focused),
                    tree: tab.tree.held(),
                })
                .collect(),
        }
    }
}

impl From<core::Layout> for LaidOut {
    fn from(layout: core::Layout) -> Self {
        Self {
            strip: layout.strip,
            tabs: layout
                .tabs
                .into_iter()
                .map(|tab| LaidTab {
                    focused: Side::word_of(&tab.focused),
                    tree: LaidNode::of(tab.tree, &tab.workspace),
                    workspace: tab.workspace,
                    pinned: tab.pinned,
                    window: tab.window,
                    front: tab.front,
                })
                .collect(),
        }
    }
}

/// **Every tab the window has, told whenever it changes** (#4751): the record keeps it, so a
/// relaunch puts each split, its sizes, its views and each strip's front tab back, and what is
/// on screen is derived from it (`Chats::looked_at`).
///
/// **Each window says its own tabs** (#4768): the project window's, or a tab window's, filed
/// under that window whatever window the tabs name, and every other window's stand. A window
/// that is not one of the project's is refused: it cannot report another's tabs.
///
/// **A press sends its new layout here, in the same call** that shows the chat, so a task the
/// person presses onto is held from that moment and is not ended under them. A layout that
/// names a chat twice across the project's windows, or a chat that is not open, is refused
/// with why, and nothing changes. A window command only: no chat can call it.
#[tauri::command]
#[specta::specta]
pub(crate) fn window_layout(
    app: tauri::AppHandle,
    window: tauri::Window,
    showing: tauri::State<'_, crate::planes::Showing>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    layout: LaidOut,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    // A window that has not said it holds the project yet (it is drawing it for the first
    // time) reports as its project window, as every window did before tab windows.
    let number = match showing.tab_window(window.label()) {
        Some(tab) if tab.plane == plane => tab.number,
        Some(_) => return Err("This window does not show that project.".to_owned()),
        None => 0,
    };
    held.chats().lay_out_window(number, layout.into())?;
    // A tab window whose last tab the person closed goes with it.
    if number != 0 {
        crate::tabwindows::close_unnamed(&app, &plane, &held);
    }
    // A reported task that was held because the person was reading it is looked at again, as
    // it is whenever what is on screen moves (#1485).
    crate::dispatched::front_moved(&held);
    Ok(())
}

/// The tabs the record put back, or `null` for a record written before tabs were recorded,
/// which the window puts back the old way: each chat a tab of its own, a task beside the chat
/// its record names. Asked once, when the window first draws a project.
#[tauri::command]
#[specta::specta]
pub(crate) fn reopened_layout(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Option<LaidOut>, String> {
    Ok(planes.held(&plane)?.chats().layout().map(LaidOut::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_s_spelling_of_a_tab_reads_as_the_core_s_and_back() {
        let laid = LaidOut {
            strip: Some("ops".into()),
            tabs: vec![LaidTab {
                workspace: Some("ops".into()),
                pinned: true,
                window: 0,
                front: true,
                focused: "b".into(),
                tree: LaidNode::Split {
                    direction: LaidDirection::Column,
                    share: 333,
                    a: Box::new(LaidNode::Chat { chat: 3 }),
                    b: Box::new(LaidNode::Empty),
                },
            }],
        };

        let layout = core::Layout::from(laid.clone());

        assert_eq!(layout.on_screen(), [3]);
        assert_eq!(layout.tabs[0].focused, [Side::B]);
        assert_eq!(LaidOut::from(layout), laid);
    }

    #[test]
    fn the_window_s_layout_is_the_json_the_bindings_name() {
        let json = serde_json::json!({
            "strip": null,
            "tabs": [{
                "workspace": null, "pinned": false, "window": 0, "front": true, "focused": "",
                "tree": {"kind": "split", "direction": "row", "share": 500,
                    "a": {"kind": "chat", "chat": 4}, "b": {"kind": "empty"}}
            }]
        });

        let laid: LaidOut = serde_json::from_value(json).expect("the window's shape");

        assert_eq!(core::Layout::from(laid).on_screen(), [4]);
    }
}
