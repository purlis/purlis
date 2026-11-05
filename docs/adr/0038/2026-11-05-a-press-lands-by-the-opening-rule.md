# Amended (2026-11-05): a press lands by the opening rule (#5289)

Records the person's acceptance of [the moving panes spec](https://github.com/purlis/purlis/issues/5288);
[ADR 0113](../0113-panes-make-room-while-a-drag-holds-a-preview-not-a-change.md) says what it amends.
It replaces the amendment of 2026-11-02 on where a press lands.

**Pressing a chat's row lands by one rule** (`opensAt` in the tabs model), rather than always in the
focused pane. A chat on screen is jumped to, and never shows twice. A chat off screen fills an empty
pane of the front tab, shows in a one-pane tab with the old chat running on in the list, and gets a
new tab if the tab is split. A press still never replaces a view or a one-job program. "No row press
makes a tab" no longer holds: a press in a split makes one, so the layout survives.
