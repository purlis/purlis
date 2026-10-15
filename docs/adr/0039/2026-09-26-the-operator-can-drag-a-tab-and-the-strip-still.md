# Amendment, 2026-09-26: the operator can drag a tab, and the strip still never moves itself

This record's first rule is **fixed tab order**, and CONTEXT.md put it in one line: *a strip's
order never changes on its own.* The operator asked for drag-to-reorder on all three strips
(SI-6). That is not a reversal, and the reason is the rule's own: a tab that moves **under the
cursor** breaks aiming, and the refusal was of the strip moving something the operator was
reaching for. A tab the operator carries is the operator's hand doing the arranging. Nothing
reorders a strip by itself — not activity, not a chat that needs you, not a pin arriving from
somewhere else — and that part is unchanged.

**What a drop does**, the same on the project, workspace and chat strips (`app/src/reorder.ts`):

- **Inside one group it moves the tab to where it was put down.** Every strip draws its pinned
  tabs first, so a strip is two groups side by side.
- **Across the boundary it pins or unpins the tab**, where it was put down. It is the pin the
  tab's menu sets, not a second kind of pin; a drag is a second way to set it.
- **A fixed tab cannot be picked up, and nothing is put down in its place.** "Outside every
  workspace" is fixed now, because it is not a directory and so cannot be pinned (the store
  refuses a name with a `/` in it). It is also the tab a later change draws first on the
  workspace strip, which needs no second drag rule.
- **Only what the strip draws moves.** A tab behind show-more keeps its place among the others,
  and the palette is still the way to find one.
- **A tab stays on its own strip.** Nothing is dragged between strips, or out of the window into
  a split window of its own. A split window's strips are its own, and are arranged there.

**Where each order is kept is where that strip's arrangement was already kept**, so there is no
new store, and ADR 0040's ruling that an arrangement is machine state holds for all three:

- **Projects:** the window's own list of projects, which is already the machine store's record
  of the window (`window_holds_planes`, ADR 0033). The strip draws them pinned first and
  otherwise in that list's order, so a dragged strip comes back at the next cold launch by the
  record that already brought it back.
- **Workspaces:** the pinned workspaces' list in the machine store (ADR 0040, amended
  2026-09-26), which is now in the order the operator pinned **or dragged** them into
  (`Store::arrange_workspaces`). The strip draws only the pins and the workspace you are in, so
  the pins' order is the strip's. Arranging never pins: a drop across the boundary pins first.
- **Chats and view tabs:** the plane's own `.charter/app/reopen.json`, whose `chats[]` are now
  written in the strip's order and put back in it — the file a chat's pin is already in, for
  ADR 0040's reason that a chat number means nothing outside its plane. It is out of git, so the
  order is this machine's as a pin is. View tabs already recorded where they were (`at`); with
  the chats in strip order too, a relaunch draws the whole strip as it was left. **Not the
  machine store**, which ADR 0034 forbids holding a chat, and **not the window's browser
  storage**, which a reloaded window would have and the core — which answers a reloaded window
  what is open — would not.

**Every standard way in.** `@dnd-kit/core` and `@dnd-kit/sortable`: a pointer has to travel five
pixels before a press is a drag, so a click still selects, a double-click still renames and the
`×` still closes; after a drag the library swallows the click the release would send. The title
bar's window drag is untouched, because Tauri's drag region stops at a `<button>` and every tab
is one. The keyboard picks a focused tab up with **Shift+Space** — bare Space and Enter select a
focused tab, which is the WAI-ARIA Tabs pattern `Window.keyboard.test.tsx` holds the window to —
and from there it is the library's own keyboard sensor: the arrows carry the tab, Space or Enter
puts it down, Escape puts it back. A screen reader is told how, and hears where the tab is.

**What it costs:** each strip's drag context keeps a hidden live region of role `status` for
what it announces. So there is no longer one status on the page, and a test about what purlis
says asks for the status lines that are saying something (`app/src/test-strips.ts`).
