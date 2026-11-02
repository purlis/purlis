# Amended (2026-11-02): a tab window's place is kept here, and a tab's pin is not (#4752)

Records [Several windows](https://github.com/purlis/purlis/issues/4701) (rule 4) and
[Pinning a tab now that a chat isn't a tab](https://github.com/purlis/purlis/issues/4700), ruled
by the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

- **"How they were arranged in windows at the last quit"** now includes each tab window's place
  on screen ([ADR 0033's amendment of this date](../0033/2026-11-02-a-project-may-have-tab-windows.md)),
  keyed by the project and the tab window's number. Which tab is in which window is not here: it
  is in the plane's `reopen.json`, beside the tab.
- **The note on pins** names the wrong pin now. The chat pin retires, and the third pin is a
  **tab's**. It is still not here: it is `tabs[].pinned` in the plane's `reopen.json`, because a
  tab exists only in that record ([ADR 0040's amendment of this date](../0040/2026-11-02-the-third-pin-is-a-tab-s.md)).
