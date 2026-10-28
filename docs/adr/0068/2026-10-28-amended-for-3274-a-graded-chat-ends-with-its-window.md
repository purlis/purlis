# Amended 2026-10-28 for #3274: a graded chat ends with its window

§2 keeps every program for the grace period after a window goes without a quit, so a window that
comes back takes it back. A grade's chat (SK-16, ADR 0105) is never taken back: the window that
started it is the only one that watches it, and the record a relaunch reads leaves it out. Keeping
it for the grace period would only leave it running with nothing watching it.

**A start may say that its program ends with its window** (session protocol 1.42,
`StartProgram::ends_with_its_window`). The host notes the link the start came on. Once that link
ends without a `quit`, the host ends the program at once, as a quit would end it: forgotten, its
confinements with it, and its end kept as `quit`. A quit ends it as it ends every other program.
Every other program waits out the grace period as before.

The window sets it for a grade's chat alone (`reopen::Chat::graded`). A host before 1.42 ignores
the word and keeps the program as any other. The window that comes back then ends it, as nothing
takes it back (#2006's end of what was not reattached), and its grade is recorded as interrupted
(#3353). A host in the window's own process ends with the window anyway.
