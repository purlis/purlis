# Amended (2026-11-07): a split tab's picture is its panes' tiles, and a view keeps its own title (#5577)

Follows [the 2026-11-06 amendment](2026-11-06-a-tab-keeps-its-first-chat-s-name-and-wears-its-most-urgent-mark.md).

- **The picture draws a tile for each pane**, parted by a gap, where it drew the lines between
  them. At a tab's size three panes drawn as lines read as the quad, a half line short; three
  tiles never do. **Past four panes it is a 3 × 3 grid**, which no layout of four or fewer
  draws, so a tab of five is not read as a quad either. The theme part `tabs.picture` keeps its
  one colour.
- **A view keeps the title it was opened with** (a file's name, a settings page's) on its pane.
  A split tab's hover names it by that title, never by its view's word, and a tab whose first
  pane it becomes takes it as its name. The stored layout already had a title for each view; a
  view beside another now writes its own there. A view opened with none is called what its tab
  is, as before.
- **A tab's mark reads only its own chats' tasks**, so a task of one chat redraws the marks of
  the tabs that hold it and no other.
