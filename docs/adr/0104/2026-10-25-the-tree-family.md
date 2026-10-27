# Amendment, 2026-10-25 (#2911): the tree family is React Aria's tree, walked as the WAI-ARIA tree

A tree of the window is the tree family (`app/src/ui/tree/Tree.tsx`), on React Aria's `Tree`.

- **It is a `treegrid` of rows, not a `tree` of `treeitem`s.** React Aria builds one tree, a grid
  whose rows may hold their own buttons (a chat's, a file's), and draws its rows one after
  another, each with its `aria-level`, `aria-posinset`, `aria-setsize` and `aria-expanded`. A
  reader announces a row, its level and whether it is open as it does a tree's item. A row is
  named by its `textValue`, what typing finds it by, so a call site writes there the whole of
  what the row says, its kind first.
- **← and → are the WAI-ARIA tree's**, where React Aria's grid would walk into a row's controls:
  → on an open row moves to its first child, ← on a top-level row that is closed does nothing.
  Heard before React Aria's row hears them, by an element around the tree that draws nothing.
  A row's own buttons are reached by → on a row with no rows under it, as a grid's are, and by
  its menu.
- **The twist is React Aria's toggle** (its `chevron` slot): a button the pointer and a reader's
  own cursor press, named "Expand"/"Collapse" and the row, never a Tab stop. It leads (in the step
  before the row's words, R129) or trails (a toggle of its own at the row's end, #2799 rule 1, for
  a row that is itself a page). A tree whose rows never fold (`fixed`, the Changes view) draws
  none and has nothing to press.
- **No virtualizer yet.** A folder's entries are capped by the core (FM-1's D-8) and drawn as
  they are; the virtualizer comes when a tree is measured to need it.
