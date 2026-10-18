# Amended 2026-10-18: the window takes back its own runner chats' sandboxes

**Built by #2511** on item 5 of the amendment of 2026-10-17 on the desktop's relay, as **ruled by
the person on 2026-10-17**: *after a re-link, the window may take back the sandboxes of the chats
it started on a runner, its own chats only, never the runner's own window's.* Their plans cross
the encrypted, key-pinned link. Item 1 is that ruling; items 2 to 4 say how it was built
(D-2511-1 to D-2511-4 in the pull request). ADR 0068, amended for #2511, records the host's side.

1. **After a re-link the window takes back its own runner chats' sandboxes, and only its own.**
   Over each new link to the runner, a renewal included, the window answers its runner chats'
   asks, changes their reach and forgets the Allows the person gave, as it did before the link
   dropped. The runner names that link the confinements of the chats that window started there
   and no other: never one the runner's own window made, and never one made or composed for
   another window's link. *Ruled 2026-10-17.*
2. **The desktop's relay names the window on each link it makes for it.** The relay gives each
   window link it relays a name of its own, and says it on every link to the runner it makes
   for that window, as it carries over the projects the window holds (`take_back`, session
   protocol 1.19). The runner answers with the confinements it names to that link. It is the
   relay's own word: a client's `take_back` is refused. *D-2511-1, D-2511-2.*
3. **A chat is its window's on each of that window's links.** While a renewal overlaps two
   links of one window, a chat the window starts on either is named to both. *D-2511-3.*
4. **So a window links to a runner through a kept relay** (`Relay::kept`): item 5's "until
   then" no longer holds. A port withdrawn when the last link a chat's confinement was named to
   ended stays withdrawn after the take-back, as the plan the runner keeps records it.
   *D-2511-4.*
