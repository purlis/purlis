# Amendment, 2026-10-10: each side has an activity bar, and shows one view at a time

The operator's decision of 2026-10-10 (spec #1671, B-1, B-2, B-12 and B-13; built in #1673):
**each side gets an activity bar, a strip of icons at the window's edge that switches the side
between views, one view at a time.** It reverses, for this shape, the rejection above of _"a
single collapsible sidebar with panels, Zed-style"_: there is now agreement on what goes in each
side, which is what that rejection waited for.

- **The left side is navigation, as the reading above has it.** Its region is called
  _Navigation_ now, and holds its views: **Chats** (the chats and their tasks, as the
  2026-10-07 and first 2026-10-10 amendments describe them) and **Explorer** (the workspace, its
  repos and branches, and their files). Chats is open by default. **Search** (the files'
  content, ⌘⇧F) and **Changes** (⌃⇧G) joined it in #1676.
- **The bottom region is gone** (B-7, #1676). What it drew, each repo's branch, uncommitted
  files, branches and pipeline, is the Changes view, unchanged and still with nothing to press,
  so "the bottom is state" above now reads "the Changes view is state". The terminals have the
  window's whole height. Its icon counts the files git has uncommitted, in the clones and in
  each branch's own folder (#1718), in the plain count's colours since nothing is asking. A
  layout file that still places the `bottom` region is read past without a word. A bottom panel may come back one day for output only; it would be a new
  decision.
- **The right side stays the "for you" side**: Todos, Memory, Personas, Sessions, Vaults and the
  extensions' panels, each a view there (#1678). What it holds does not change, only that it
  shows one at a time. Memory is open by default. An extension's panel is a view of its own,
  after purlis's on the bar, so approving one never moves an icon the person already knows; the
  region still says nothing an extension declares decides where it goes (ADR 0043). Personas
  are the project's, so the Personas view lists them at the project root too (#1686).
- **Each thing lives in one view.** The explorer no longer draws a chat where it works: the
  Chats view is the one place a chat is listed. The explorer's own axis is still the place, and
  the paragraph above that says it draws each chat where it works is replaced by this one.
- **Pressing the open view's icon puts the side away**, and pressing any icon brings it back on
  that view. The bar stays at the edge while the side is away, with its badges: the Chats icon
  counts the chats that need the person, in the needs-you colours, and the Todos icon the
  focused workspace's open todos, in the plain count's. Putting a side away hides its
  views and never unmounts them, so a view keeps its folds, scroll and filter.
- **Keys and the palette.** ⌘B puts the left side away and brings it back and ⌥⌘B the right
  (each names its region, as VS Code's ⌘B follows its primary side bar), ⌘⇧E shows Explorer,
  ⌘⇧C shows Chats and ⌘⇧F shows Search (Ctrl, and Ctrl+Alt+B, on other platforms), and ⌃⇧G
  shows Changes on every platform; every view is a row in the palette (Search's is _Search in
  files_).
- **Remembered per project, on this machine**, in the layout file (version 2): which view each
  side shows, its width and whether it is away, and the Chats list's folds by each chat's id
  (#1687). The needs-you queue stays in the title bar, where nothing competes with it.

The status line's toggles stay as each region's way back. The bar is a tab list built on the
window's own roving focus, holding tabs and nothing else (#1204); `docs/ui-primitives.md` has the
detail.
