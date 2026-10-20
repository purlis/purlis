# Amended for #2753, a script's title is read and a runner's host logs its info, 2026-10-20

1. **A page's title is read from its document when the pages are listed.** The engine tells of
   a title by `Target.targetInfoChanged`, and Chrome does not always send it after a script sets
   `document.title`. Listing the pages reads each page's `document.title` in purlis's own world,
   one read per page within a second; a page whose title cannot be read keeps the title it was
   told of. A snapshot keeps the title its accessibility tree names, and the pages are listed
   under it from then on. An action's answer names its page by the title last read or told.
   *D-2753-1, decided in implementation, to be confirmed.*
2. **`purlis serve` keeps the core's `info` on its standard error.** A runner's host has no
   terminal and no log file of purlis's: its standard error is the journal of the service that
   runs it, or its container's log. It widens the CLI's warnings-only filter to what the app's
   log keeps (charter's crates at `info`, a library's warnings), through the same redaction. No
   store of purlis's is added, so the plane format does not change. The window's own session
   host keeps its warnings-only filter: the window reads its standard error into its own log.
   *D-2753-2, decided in implementation, to be confirmed.*
