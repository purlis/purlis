# Amended (2026-11-06): a tab's name has room: a 160px floor, an × that takes no room, and its tasks on its pane head (#5341)

Records [Room for a tab's name](https://github.com/purlis/purlis/issues/5341) of the spec
[What a tab shows](https://github.com/purlis/purlis/issues/5339), accepted by the person on
2026-11-05 on its prototype (`prototype/tab-faces`). The spec's rule: **a tab says which chat
this is and whether it needs you**; a chat's other facts are on the list, the head or the foot.

## The floor

**A chat tab's floor is 160px at the default window text** (14px), up from the 2026-11-02
amendment's 120px, scaling with the window's text as before (`fits.leastAt`): room for its mark
and about fifteen letters of its name. Fewer tabs fit, and the rest go behind "N more ▾" as
before; pinned tabs stay exempt, and the front tab is always drawn. A value measured on real
names replaces 160 without a new record.

**A narrow window keeps the same floor.** The 2026-11-02 amendment's narrow strip, where a tab
under 140px showed its × only in front or under the pointer (#4702), is retired: the × now
behaves so at every width, so there is no second rule to keep.

## The ×

**The × takes no room.** It stands over the end of the name, out of the tab's flow, on a fade of
the tab's own ground, so the name never moves when it comes and has that room at rest. It is
seen on the front tab, under the pointer and with the keyboard in the tab. **A tab with room to spare keeps the end of its name clear of the ×**: the room after the
name gives way before the name does, so where a theme sizes a tab to its words a short name is
never cut by its ×, and at the floor that room is the name's. **The front tab's × is
the one Tab stop after the strip**, so a keyboard reaches it without fifty stops (charter-app#189
still holds for every other tab's); Delete on a tab, its menu and the palette still close it.

## The tasks count

**A tab wears no count of its tasks.** The menu its chip opened moves as it is to the state word
on its chat's pane head ("waiting on 3 tasks"), the same component ⌘⇧J opens, so the two cannot
drift: each task with its mark, End a task, Activity, and a summary of the counts at its foot. A
chat with no tasks keeps its state word as it was. A pick shows the task in that pane, as ⌘⇧J's
did (#5058); the menu lists the chat first and its tasks under who asked for them, as the chip's
did, and a keyboard opens it on the first task. A tab's hover says how many of its tasks run: its open tasks, whose programs run whether they
work or wait.

**What this changes of ⌘⇧J's own menu (#5058).** It was a menu of its own, floating under the
focused head, listing the chat's own tasks and those that had finished, a finished one passed by.
Now it is the head's menu: the chat first, every task under who asked for it (a task's tasks
too), the finished ones in the fold, and Enter on a finished one opens its report. It goes with
its pane, so a pick never lands in a pane that has closed. Its foot says the limits of the pane's
own chat, not of the tab's first.
**The hand stays on the tab** where a chat of it waits for the person off screen (V100-37): that
is "whether it needs you". The theme part `tabs.count` keeps its name, since the manifest never
takes a part away, and draws the summary's counts. ADR 0036's amendment of 2026-10-12, where
a tab gives up its task chip before its title (#1867), no longer has a chip to give up: a tab at
its floor keeps its title, and the rest go behind "N more ▾".
