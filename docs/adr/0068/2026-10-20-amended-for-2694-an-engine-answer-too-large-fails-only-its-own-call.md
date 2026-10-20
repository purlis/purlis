# Amended for #2694, an engine answer too large fails only its own call, 2026-10-20

The browser engine speaks DevTools over a pipe whose messages are held to a bound (64 MiB). This
note says what a message past its bound does; nothing else changes, and the session protocol does
not.

1. **Past its bound, a message is read past, never kept.** JSON never carries a raw NUL, so the
   next NUL ends the message: purlis keeps only its first bytes and reads the rest away. An
   answer, told by the `{"id":N,` its first bytes begin with, fails its own call in words (the
   agent is told the page's answer was too large to read, and that the browser still answers),
   and an answer to a call no longer waiting is dropped. An event past its bound is dropped only
   where it is of a kind nobody reads (what a page writes to its console, its uncaught errors);
   any other event past its bound ends the connection, as before, since what purlis knows of a
   page's frames and targets is kept from events and is never left stale. A message past its
   bound that does not begin as a DevTools message ends the connection too, as any message that
   is not JSON does. *D-2694-1, decided in implementation, to be confirmed.*
2. **A call may ask for a smaller bound.** Each call is held to the connection's bound, or to a
   smaller one it names. The read of a page's layout that marks what a box or a frame does not
   show (#2564) is held to 16 MiB: a node count does not bound the inline script and attribute
   text it carries, so past that the read fails alone and nothing is marked cut off.
   *D-2694-2, decided in implementation, to be confirmed.*
