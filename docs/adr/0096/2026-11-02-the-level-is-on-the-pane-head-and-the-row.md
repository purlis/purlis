# Amended (2026-11-02): the level is shown on the pane head and the row (#4752)

Records the [charting decisions](https://github.com/purlis/purlis/issues/4576) D3 and
[one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4582) rule 1, ruled by
the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), with the
badge's place as spec [#4750](https://github.com/purlis/purlis/issues/4750) records it. This records
the decision ahead of the behaviour, which ships with that milestone's build.

"The title bar shows the level of the chat in front, as its tab does" reads: the title bar shows
the level of **the focused pane's chat**, as that chat's **pane head and sidebar row** do. A chat has
no tab of its own, and a tab shows only its focused pane's icon. This is the same move as
[ADR 0067's amendment of this date](../0067/2026-11-02-the-sandbox-badge-is-on-the-pane-head-and-the-row.md).
