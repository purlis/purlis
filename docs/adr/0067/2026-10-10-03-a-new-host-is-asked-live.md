# Amended (2026-10-10, #1666): a new host is asked live

Section 3's "a host no preset lists is refused and shown to the operator" now reads: **it is
held and asked about.** A chat's connection to a host nothing lists, through purlis's proxy, waits
while a Notice asks the person, and an Allow lets the same connection carry on. Nothing fails and
nothing restarts.

1. **Held about a minute.** If nobody answers in that time, the connection is refused as before,
   the refusal is in the network record as a `timeout`, and the Notice stays. A later Allow
   applies at once, and the chat is told, in purlis's fixed words, to run the command again.
2. **One Notice for a burst.** The new hosts one chat reaches within about two seconds are one
   ask. Each is listed whole, as host and port, and never as a wildcard.
3. **Scopes.** The main button allows the host for this project on this machine; a menu offers
   only this chat, or everyone in the project. Each is kept where it was before (the app's
   memory, your hosts, the committed hosts), each is in Settings' Granted list with Remove, and
   each reaches every running chat on the proxy at once. A chat on no proxy (an older Claude
   Code) takes it by restarting, as before. A host allowed already is never offered again: its
   Notice says so, and offers to restart a chat that started before it.
4. **Keep blocked** refuses what is held, and every later connection to that host from the chat,
   until an Allow; the chat is told, in fixed words, not to try it again unless asked.
5. **Only the window answers**, in every permission mode: the board a connection waits on is
   answered by the window's commands alone (`allow_sandbox_block`, `keep_sandbox_block` and the
   tasks' pair, all window-only), and nothing a connection or a chat sends is read as an answer.
6. **Policy only takes away.** `live-asks: false` turns the hold off (a host is refused at
   once, as before); `allow-scopes` removes scopes from every Notice; `never-hosts` pins hosts no
   level may add and no Allow may keep, which are refused and never asked about. The Notice says
   when policy ruled a choice out, and who set it.
7. **Bounded.** At most a few connections are held per chat at once; one more is refused at
   once, as before, and raises its own Block.
