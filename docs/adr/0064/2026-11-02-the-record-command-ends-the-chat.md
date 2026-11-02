# Amended (2026-11-02): the record command ends the chat, and closes no tab (#4752)

**This record's title says the record command "closes the tab"; read "ends the chat".** The
decision stands: the person's click is consent, so the prompt is sent, and the command that writes
the record is the signal that the chat is done. It is amended, not superseded, because the
amendments that pre-allow tools under this record hang on it.

Records [the pane's chat actions](https://github.com/purlis/purlis/issues/4585) rule 5, the
[charting decisions](https://github.com/purlis/purlis/issues/4576) D2 and
[Storing a tab's layout](https://github.com/purlis/purlis/issues/4587) rule 3, ruled by the person
on 2026-11-02 on the map
[Chats, tabs and panes: a chat isn't a tab](https://github.com/purlis/purlis/issues/4575), and
spec [#4750](https://github.com/purlis/purlis/issues/4750), which settles which answer has the
keyboard. This records the decision ahead of the behaviour, which ships with that milestone's
build.

- **Asking.** Closing a pane or a tab asks nothing and ends nothing. **Smart close is the third
  answer of End…'s dialog**, reached wherever End… is: the pane head's ⋯, the sidebar row's ⋯ and
  right-click, and the palette. **End keeps the keyboard**, so "Smart close (primary)" no longer
  holds. On a shell, End… has no Smart close.
- **While it runs.** The 2026-09-28 chip goes, and so does `tabs.frontWithout`'s part in it: a chat
  has no tab to shrink, and nothing moves on the strip. A chat wrapping up shows a breathing mark
  on its pane head and its sidebar row. **Cancel smart close** is the pane head's state button
  and the first row of the chat's list, and the person typing into the chat still cancels it.
- **How it ends.** When the record is written, the chat **ends like End**. If it is on screen,
  its pane keeps the last screen, read-only, under an "ended" band with **Resume** and **Clear
  pane**, and links the record. Its row stays in the sidebar.
- **When it does not end.** A stop without a record (five minutes with none, the chat ended
  first, or a start refused) goes to the pane's band, the bell and the needs-you row, as before,
  with Dismiss on the list. The chat is left running, as before.
- **The 2026-10-06 amendment's pass.** "The tab closes on a record only while a person's smart
  close is under way" reads "the chat ends on a record only while…", and "the tab's **Smart
  close**" reads the chat's.

Where this record and its amendments say the chat's tab shows a mark or a notice, that is its pane
band when the chat is on screen, and its row, the ✋ and the Inbox when it is not.
