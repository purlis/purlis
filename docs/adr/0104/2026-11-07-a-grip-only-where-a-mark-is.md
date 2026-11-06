# Amendment, 2026-11-07 (#5537, from #5494): a grip only where a mark is

The 2026-11-06 amendment ([the grip family](2026-11-06-the-grip-family.md)) stands. Two of its
cases are now decided, and the code's names follow its words.

**A view's blank head does not drag; only its grip does.** A view's head lies over the view's
own first row (#3147): Settings' levels, a file's toolbar, a view's own controls. It takes no
pointer there (`paneFrame.css`), and only its grip and its actions do. The blank part of the head
is the view's first row, not a strip of the head's own, so a drag from it would take presses
from what the view draws there. It would also need to know which of the view's pixels are blank,
and a view's content is its own. A chat's head has no row under it, so all of it drags. A view in
a split moves by its grip, by ⌘⇧M, and by its tab's Move….

**A chat with no row in the Chats list draws no grip.** A chat's mark is a fact of its row: its
persona, its program and its state. A chat ended in its pane (#5019), a task the plane lists no
more (#5106), and a chat in the moment before the list is read have no row. Each has an empty
head. A stand-in mark would make one up: a terminal's mark on a persona's chat says the wrong
thing. Its pane still moves: its blank head drags, ⌘⇧M picks it up when it has the keyboard, and
its tab's Move… does too. Once the list has it, its mark is its grip, as everywhere else.

**The code says Move.** What the menus' and palette's Move… row does is `verb: "move"`,
`doing.move`, `SAID.move`, `MovedFrom` and the shortcut `pane.move`. Its rows are
`chat.moveto:<session>` and `tab.moveto:<tab>`: `chat.move:` was already the move of a chat to
another pool, and "Move…" asks where to. **Placement mode** keeps its name: it is the spec's
word (#4764) for the keyboard's drag that Move… starts.
