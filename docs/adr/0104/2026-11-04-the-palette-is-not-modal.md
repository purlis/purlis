# Amendment, 2026-11-04 (#4351): the palette is not modal

**The command palette (`DialogSurface`) is not modal**, as VS Code's is not. The operator's ruling
of 2026-11-04: it stops marking the window inert, keeps the keyboard in itself with its own
`FocusScope` (`contain`), and Esc and a press outside it close it. Only the palette changes:
`Dialog` and `AlertDialog` stay modal, on React Aria's `ModalOverlay`, with the window behind them
inert and a click outside answering nothing.

**Why.** A React Aria modal makes everything outside it inert (`useModalOverlay` asks for `inert`
always), and WebKit holds inertness as inherited style. Taking `inert` off the window's root as
the palette closed restyled the whole window, some 80 ms on the macOS runners, in the task before
whatever a palette row opened was drawn. Measured in #5001's A/B (a window whose root is never
made inert lost that pass and nothing else); it was one of the costs of the Skill studio's
opening bar (L10, 300 ms).

**What it costs.** The window behind the palette is no longer hidden from assistive technology
while the palette is up, and the browser no longer enforces that nothing behind it takes focus or
a press: the palette's focus scope holds the keyboard, and its scrim, which closes it, takes the
pointer. A palette is a command box over the window, not a question that must be answered, so
the window staying readable behind it is what VS Code's does too.
