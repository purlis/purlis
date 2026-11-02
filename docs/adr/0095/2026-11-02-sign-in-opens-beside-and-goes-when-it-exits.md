# Amended (2026-11-02): Sign in opens beside the focused pane, and goes when it exits (#4752)

Records [A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rules 1 and 8,
ruled by the person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and Sign
in's place as [Rewriting the records](https://github.com/purlis/purlis/issues/4704) decided it (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). Rule 8 names "a forge login typed into a
shell" among its one-job programs, and a harness's sign-in is one. This records the decision ahead
of the behaviour, which ships with that milestone's build.

- **§3: the Ask's action is a one-job program, not a shell tab.** Sign in opens a split beside the
  focused pane, in the same tab and with focus, and the core types the harness's own sign-in into
  it, as before. Pressed from Settings, the Inbox or the welcome, it opens beside the focused pane,
  which always exists. When its program exits, the split goes and focus returns. It is never
  recorded, and has no grip, no close and no swap. purlis still reads none of what it prints: the
  page or code the harness shows is seen in that split.
- **The 2026-10-27 amendment's runner sign-in** opens the same way: a one-job split on the
  desktop, beside the focused pane, that the person drives.
- **The 2026-10-31 amendment's end of the watch** is "the harness says the profile is signed in,
  or the program that sign-in was typed into has ended". Closing a pane is no longer a way to
  leave it, because a one-job split has no close.
