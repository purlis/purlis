# Amended 2026-10-14 (the operator's ruling on spec #2008, built in #2016): a browser login is performed in the chat's own browser

Under the ruling of 2026-10-14 (logins are the existing browser-login declarations, unchanged in
format, now performed by purlis as the browser's third driver; no cookie file reaches the chat;
the old command-line login and its session file are retired), the amendment of 2026-10-12 above
(D-PE-browser-1) is read as follows. Where it disagrees, this note holds.

1. **The declaration and its Allow are unchanged.** `browser = "<login page>"`, `key` and `user`
   declare it; it waits for the same Allow as the persona's hosts and tools, with the same digest
   line, and policy's `persona-tools` forbids it with the rest.
2. **The chat asks for it through its browser tools.** `browser_login` names the login, and that
   is all the chat chooses. The app reads the values from the vault for exactly the login the
   chat's start armed, held to what a declared tool's run is held to (D-PE-ruling-2), and hands
   them to the session host's browser, which performs it (ADR 0068, amended for #2016). A chat
   started without the sandbox has no login performed for it. *Decided in implementation,
   D-2016-6, to be confirmed.*
3. **It runs no program.** A login is armed without a program of its own, its shim runs nothing
   and says to use `browser_login`, and a brokered `secret exec` of it runs nothing either. No
   file of the session is written. *Decided in implementation, D-2016-7, to be confirmed.*
