# Amended for #2745, a browser's end is said and its engine's log kept, 2026-10-20

A browser's engine speaks DevTools over a pipe, and its end reaches the host as a program's end.
This note says what the connection reads, what a browser's end leaves in the logs, and where its
engine's log is kept. The session protocol does not change.

1. **What JSON allows is read.** The engine writes each character of a JavaScript string past
   ASCII as a `\uXXXX` escape, so a string holding half of a surrogate pair (an emoji a page's
   script cut in two) arrives as a lone `\ud83d`, which JSON allows. Such a half, and any byte
   that is not UTF-8, is read as U+FFFD; a message that still does not read as JSON ends the
   connection. *D-2745-1, decided in implementation, to be confirmed.*
2. **A network event past its bound is dropped.** The driver reads a request's events only to
   tell when its page has settled, and the reach proxy judges each request itself, so a network
   event past the 64 MiB bound is dropped as a console event is. An event about a page's frames
   or targets past it still ends the connection. *D-2745-2, decided in implementation, to be
   confirmed.*
3. **A connection's end ends its engine.** When the DevTools connection ends, the driver ends
   the engine's process group before anyone is told: an engine whose pipe closed has two seconds
   to end by itself, and one whose connection closed on a message is ended at once. The host
   removes the browser's folder only after. *D-2745-3, decided in implementation, to be
   confirmed.*
4. **Every end is said.** Whoever waits on a browser's end is told who ended it (purlis stopping
   it, the engine by itself, or purlis because its connection closed), its engine's exit code or
   signal, and why its connection closed, naming a message by its DevTools method alone. The
   session host writes one line for each end, with the host's own act that ended it where one
   did, into the window's log; the window writes one line for each browser it ends, with why, and
   one for each end it is told of, with its exit. The program's end carries the engine's exit.
   The session host's warnings from the core reach the window's log as its other lines do.
   *D-2745-4, decided in implementation, to be confirmed.*
5. **An engine's log outlives its browser.** The engine writes its standard output and error to
   `<data>/browsers/<program>.engine.log`, beside the browser's folder, and the newest 20 are kept,
   swept as a browser starts; a running browser's log is never removed. It stays inside
   `<data>/browsers/`, which a chat's sandbox denies. *D-2745-5, decided in implementation, to be
   confirmed.*
