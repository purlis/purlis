# Amended (2026-11-02): the sandbox badge is on the pane head and the row (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D3 and D4,
[A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rule 5 and
[one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4582) rule 1, ruled by
the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and
the badge's place as spec [#4750](https://github.com/purlis/purlis/issues/4750) records it. This
records the decision ahead of the behaviour, which ships with that milestone's build.

A chat no longer has a tab of its own: it is listed in the sidebar, and shown in a pane when the
person picks it, so a chat may run in no pane at all. A tab shows only its focused pane's icon. So
"that chat's tab shows it for as long as the chat lives" moves to where an open chat always is.

- **SD-3: the badge is on every pane head that shows a chat, and on the chat's sidebar row in
  every scope that lists it.** An unsandboxed chat's badge cannot be hidden or dismissed on
  either, and that is what "cannot be hidden" now means. The badge still names the backend.
- **"Until the person allows its own on its tab"** (a handoff's grants, D-1362-5) and **"the tab
  offers Restart now"** are offered on the chat's pane band when it is on screen, and on its row
  and in the Inbox when it is not.
- **The 2026-10-09 (#1538) amendment's chip** with one hand for each chat waiting off screen goes
  with the tab's count. A blocking ask from a chat in no pane shows on its row, the rows above
  it, the title bar's ✋ and the workspace chip's count, and is answered in the Inbox; it never
  takes a pane or focus. The question for several tasks is a Notice on the asking chat's pane
  band when that chat is on screen, and in the Inbox when it is not.

Where this record's other amendments say a Notice, a block or an offer is shown "on the chat's
tab", read the chat's pane band when it is on screen, and its row, the ✋ and the Inbox when it is
not. The wording sweep at the milestone's end rewrites them
([#4770](https://github.com/purlis/purlis/issues/4770)).
