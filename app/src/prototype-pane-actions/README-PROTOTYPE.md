# PROTOTYPE: a chat's actions in its pane (#4585)

Throwaway. Branch `prototype/pane-chat-actions`, never merged. Map #4575, ticket #4585, rule D9 of #4576.

## The question

Which of a chat's actions the pane head shows directly and which sit behind ⋯, in a wide and a narrow
pane, with the same list on the sidebar row's menu and the palette. What's left on a tab's
right-click. Which rows don't apply to a shell (#4588). Where Smart close (ADR 0064) lives now that it
no longer closes a tab.

## Open it

From any clone of purlis:

```bash
git fetch origin prototype/pane-chat-actions && git show FETCH_HEAD:app/src/prototype-pane-actions/PaneActions.prototype.html > "${TMPDIR:-/tmp}/pane-actions.html" && open "${TMPDIR:-/tmp}/pane-actions.html"
```

The dashed bar switches variant A / B / C, loads scenes 1–6, opens the palette and switches the theme.
URL params: `?variant=A&s=1&t=dark`. Keys: `[` `]` scene, `v` variant, `t` theme, ⌘K palette.
Drag the window's bottom-right corner to narrow it.

- **A · One state button + ⋯**: at most one chat action on the head, picked by state.
- **B · The name is the menu**: the icon and name open the list; no ⋯, no chat buttons.
- **C · A toolbar that folds**: six icons that fold into ⋯ from the right as the pane narrows.

Scenes: 1 a working chat and its asking task, 2 a shell beside a chat, 3 an ended chat, 4 Smart close
(with "record written" and "stopped without a record" events), 5 a split tab's right-click, 6 three
narrow panes.

`command-table.md` is the redrawn command table. Stand-in data in memory; nothing calls the core.

## Verdict (the person, 2026-11-02: accepted, all four as recommended)

1. **Variant A.** The pane head shows at most one chat action, chosen by the chat's state: Stop while it works, Answer while a task asks, Cancel smart close while it wraps up, Brief on a quiet task, and nothing on a quiet chat. Everything else is in ⋯. B and C were ruled out.
2. **A tab's right-click holds tab things only:** Pin tab, Tile all, Keep open, Close other tabs, Close tab. Closing never ends a chat.
3. **Smart close** stays the third answer in End…'s dialog. Its strip chip goes. Wrapping up is shown as a breathing mark plus Cancel smart close on the pane head and the sidebar row. Once the record is written, it ends like End: Resume, Clear pane, and a link to the record.
4. **A sidebar row shows ⋯ on hover and focus**, with the same list as the pane.

Screenshots weren't taken, because machine load was high. They can be added later. `command-table.md` is the final table.
