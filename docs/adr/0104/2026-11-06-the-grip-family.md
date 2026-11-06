# Amendment, 2026-11-06 (#5290, spec #5288): the grip family

**What a pane or a chat is picked up by is one family, `ui/grip`**, drawn in a chat's row in the list, in a chat's pane head, and at the head of a view in a split. It stands in the chat mark's `who` part, so the state's badge stays beside it. Its parts are `root`, `mark`, `dots` and `dot`; only `dot` is themable, in `hover` and `focused`.

**Why.** The head's grip was an icon button, always drawn, so it read as an action. On a view alone in its tab it stood over the view's own first row. A chat's row had a separate Place… button beside its other marks. The person's ask, on the moving-panes prototype (`prototype/moving-panes`), was one quiet grip: the persona's mark turns into dots on hover, with no button shape and an open hand for the cursor.

**It has no button ground, and a theme cannot give it one.** The root has no themable property, and the controls guard fails any stylesheet in the window that gives it a fill, an edge or a shadow.

**Where none is drawn.** A one-job program stays beside the pane it opened from, and an empty pane has nothing to move. A view alone in its tab moves by its tab, so nothing stands on its content. The keyboard's routes stay: the grip of a pane is a Tab stop, and the menus' and palette's row, renamed from Place… to **Move…** to match the grip's name, and ⌘⇧M pick up what has the keyboard.
