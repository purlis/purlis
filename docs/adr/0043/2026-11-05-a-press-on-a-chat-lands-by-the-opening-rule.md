# Amended (2026-11-05): a press on a chat lands by the opening rule (#5289)

Records the person's acceptance of [the moving panes spec](https://github.com/purlis/purlis/issues/5288);
[ADR 0113](../0113-panes-make-room-while-a-drag-holds-a-preview-not-a-change.md) says what it amends.

In the amendment of 2026-11-02, "a press on a chat never replaces a view: the chat goes to the most
recently used pane that shows a chat or nothing" holds only with a view alone in the front tab, and
only for an empty pane or the chat of a one-pane tab. A view in a split gets a new tab, like any
split. A view still never replaces a pane's content, and a press still never replaces a view.
