# Amended (2026-11-02): a shell follows a chat's rules, and is not a tab (#4752)

**This record's title says "a shell tab"; read "a shell".** A shell is no longer a tab of its
own. The decision this record makes, that the shims on a shell's `PATH` detect a harness started
by hand and the command that started it warns about it, is unchanged, so this is an amendment and
not a superseding record.

Records [A window with no tab per chat](https://github.com/purlis/purlis/issues/4588) rules 7 and
8 and [the pane's chat actions](https://github.com/purlis/purlis/issues/4585) rule 3, ruled by the
person on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575) (spec
[#4750](https://github.com/purlis/purlis/issues/4750)). This records the decision ahead of the
behaviour, which ships with that milestone's build.

**A shell follows the same rules as a chat.**

- **New shell takes the focused pane**, as New chat does, and the shell has a sidebar row filed
  where it started. It still wears a terminal's mark.
- **Replacing it, or closing its pane, leaves it running** in the sidebar, so a dev server
  survives a closed pane. Ending it is **End…**, which on a shell asks only Cancel or End.
- **When its program exits**, its row goes and its pane clears. The accepted cost: a forgotten
  shell stays listed until it is ended.
- **A pin works as a chat's does**: it is the tab's, not the shell's.
- **Its actions** are a chat's list without the harness rows: Rename, Link, Open in new tab,
  Activity, Network, Stop, the branch rows and End… stay; Use skill, Ask, Skills, Harness,
  Restart, Start fresh, Move to, Stop all tasks, Smart close and the task rows are left out.

**A terminal editor or a forge login is not a shell in this sense.** A one-job program opens as a
split beside the focused pane, in the same tab and with focus, and the split goes when its
program exits.

**Where the warning is drawn.** The `StartedByHand` banner, with **Open as chat** and **Dismiss**,
is drawn on the shell's pane band when the shell is on screen. When it is in no pane, the note goes
to the Notices centre and marks the shell's row. The shims go on a shell's `PATH` and nothing
else's, as before. The Saving tab's "Open terminal here" opens a shell like any other.

The CONTEXT.md term becomes **Shell**, and the shim's line that a chat tab is where a harness
should run says "a chat". Both change with the build that ships this behaviour.
