### Changed

- **The ask for a link whose words name somewhere else says which chat printed it**, under its
  title, as the ask for a file outside the workspace does: "From chat #12 · Fix the login form".
- **Links in a pane have a page of their own** (`docs/pane-links.md`): what links, what asks
  first and why, and what is refused.
- **A file link whose branch purlis could not read says so on its hover**: the editor checks the
  file as it opens, where before the link said nothing and the editor might then refuse it.
- **A pane's ⋯ menu draws ⇧⌘L beside _Links in this pane…_ only on the focused pane**, the one
  the key reaches. Another pane's row still opens its own list.

### Fixed

- **_Links in this pane…_ says so when it cannot open the list**, where it did nothing before:
  when no pane could list the chat's links yet, or the second of two panes showing one chat had
  gone.
- **A chat ended on purpose that its pane still shows has _Links in this pane…_ again.**
