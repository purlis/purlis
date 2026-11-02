# Amended (2026-11-02): the third pin is a tab's, kept in the plane's reopen record (#4752)

Records [Pinning a tab now that a chat isn't a tab](https://github.com/purlis/purlis/issues/4700)
and [Storing a tab's layout](https://github.com/purlis/purlis/issues/4587), ruled by the person on
2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

**A project, a workspace and a tab can each be pinned.** The chat pin retires. The tab pin is a
flag on the tab's own entry, `tabs[].pinned`, in the plane's `.charter/app/reopen.json`. It stays
per plane, in the app's own record and out of git, so this record's ruling holds: a pin is an
arrangement, never a fact of the plane.

**The reason changes, and the conclusion does not.** This record put the chat pin in the reopen
record because a pin on a chat had to disappear when the chat did. A tab's pin does the opposite:
it keeps the tab across relaunch, upgrade and cleared panes, and outlives what the tab shows. It
goes when its tab is closed, or when the tab's workspace is removed, which closes the tab. It
belongs in the reopen record now because a tab exists only there: a tab's number means nothing
outside its plane's record.

- **Absence still reads as unpinned**, with no version bump, now on the `tabs` list. A record
  with no `tabs` list is read the old way exactly once, and a pinned chat or view in it becomes a
  pinned tab.
- **The 2026-09-26 amendment's "chat pins are unchanged"** reads "tab pins": each is a flag on
  its own entry.
- **The open questions** this record left to ADR 0039, whether a pinned tab is drawn first and
  whether it is exempt from the overflow, are answered by
  [ADR 0039's amendment of this date](../0039/2026-11-02-the-third-pin-is-a-tab-s-and-it-is-exempt.md):
  it is drawn first, and it is exempt.
