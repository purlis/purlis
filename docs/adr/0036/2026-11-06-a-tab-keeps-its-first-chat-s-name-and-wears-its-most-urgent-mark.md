# Amended (2026-11-06): a tab keeps its first chat's name and wears its most urgent pane's mark (#5342)

Records the decision the person accepted on 2026-11-05 on the prototype of
[what a tab shows](https://github.com/purlis/purlis/issues/5339) (`prototype/tab-faces`, its
scenarios "A split tab", "A pane needs you, but another is focused" and "Splitting doesn't rename
the tab"), built by [#5342](https://github.com/purlis/purlis/issues/5342). It amends the
2026-11-02 amendment's rule, from
[one tab, the strip and its icons](https://github.com/purlis/purlis/issues/4762), that "a tab
shows its focused pane's combined persona-and-state icon and its chat's name, and a split adds
`⊟+N`".

**The rule: a tab says which chat this is and whether it needs you.** Following the focused pane
did neither well. The front tab's name changed on every click, so a tab left alone showed
whichever pane was last pressed. And a pane that needed the person stayed hidden behind the
focused one.

## A tab's name is its first pane's

- **A tab is called by its own name if the person gave it one, otherwise by its first pane's**:
  the layout's first leaf, at top left. Which pane has the keyboard plays no part, so splitting
  and moving between panes never rename a tab. The person asked for this directly, choosing
  between a tab renamed on a split and one that keeps its first chat's name.
- **The name follows the first pane, not a chat**. When the first chat closes or moves out, the
  tab takes the next chat's name, so it never names a chat that isn't there (#5308). A chat put
  left of the first pane becomes first, and the tab takes its name. Undo puts the name back with
  the layout.
- **A name the person gave the tab itself stays through all of this.** It is window state, as a
  tab's name always was. The record keeps none, and Rename (#5343) is what sets and clears it.
  Renaming a chat still goes through the core, and the tab follows where that chat is first.
- One pure function answers it (`tabs.tabNameOf`). The window keeps the first pane's name
  current after every change in one place (`tabs.namedForFirstPanes`), so the rule can't vary
  from one path to another.

## A tab's mark is its most urgent pane's

- **A split tab wears the mark of the pane whose state comes first** in the one order (#5340):
  needs you, a failed task, working, waiting on tasks, idle, ended. So a pane that needs the
  person can't hide behind the focused one. Of two panes in the same state, the first in reading
  order wins, so the mark changes only for a state that outranks it.
- **Every chat on screen counts**: each pane's own chat, and a task it shows in that chat's place
  (#1486). A view and an empty pane have no state and are passed over. A tab with no state to show
  wears its first pane's mark, as a view's tab or an empty pane's does.
- The mark reads each chat's state through the one function every surface reads it through, so
  the tab and the row can't disagree.

## A split tab shows its shape

`⊟+N` gives way to a small picture of the layout's shape, drawn from the tab's layout tree as an
icon in the family: past four panes it's a grid. Its text alternative says how many panes there
are, and a hover says how many and lists them.
