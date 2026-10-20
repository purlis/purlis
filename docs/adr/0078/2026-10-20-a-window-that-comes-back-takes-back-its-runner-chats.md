# Amended 2026-10-20: a window that comes back takes back its runner chats

**Built by #2623** on the amendment of 2026-10-18 for #2511, under which the desktop's relay
named each window link with a name of that link's own: a window that restarted, or linked again
to its own desktop host, was a new window to the runner, and its runner chats stayed out of its
reach until they ended. What follows was decided in #2623. Items 1 and 3 are **as the steward
directed** on 2026-10-20; the rest are *decided in implementation, to be confirmed*.

1. **The window keeps a name for each project root's link, on this machine.** A random 128-bit
   name, minted the first time the root's link opens, kept in
   `<config>/purlisd/window-names.json`: `0600`, in the session host's `0700` folder, which
   every chat is denied; never committed, never sent anywhere but to the window's own host,
   never logged (plane-format.md). Not derived from anything another process can read, such as
   the device id or the root: a client that could compute it could claim it first.
   *As the steward directed* (D-2623-1).
2. **The window names itself on the link, first** (`name_window`, session protocol 1.31). It is
   the link's first command, sent only to a host that knows the word. A refusal leaves the link
   as it was, one that takes nothing back after a restart. Only `local-ui` is granted it.
   *Decided in implementation, to be confirmed* (D-2623-2).
3. **The relay names the window to the runner by a digest of the name, never the name.** On each
   link to the runner it makes for a named client link, `take_back` says a SHA-256 of the name
   under a fixed prefix, the same after the window restarts, so the runner names the window that
   comes back its chats again. `name_window` is never sent on. A client link that names itself
   otherwise, or not at all, gets a name of its own and none of those chats. *As the steward
   directed* (D-2623-3).
4. **One link holds a name at a time, and names itself before anything else.** While a client
   link holds a name, another that names itself so is refused `not_allowed`; a name sent after
   any other command on the link is refused `malformed`. A host that relays nothing answers
   `done`: its window takes its chats back with `programs_of`. *Decided in implementation, to
   be confirmed* (D-2623-4).
