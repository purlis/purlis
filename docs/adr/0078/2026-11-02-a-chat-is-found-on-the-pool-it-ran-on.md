# Amended 2026-11-02: a chat is found on the pool it ran on

**Built by #4590**, on the amendment of 2026-10-20 on a workspace running its chats on a pool's
runner. Its item 1 keeps its rule that after a relaunch a chat left running is asked of this
machine's host and of **one pool, and no other**. It changes which pool that is: the one the chat
runs on, where item 1 said the one its workspace runs its chats on, so a pool the setting has left
is asked for the chats that run there. Item 5 is tightened, and item 8 gains the one exception this
new record needs, in point 8.

**Ruled by the person, 2026-11-02:** *the one pool is the chat's own recorded pool, and it is
never sent code when it is asked.* Points 1 and 3 are that ruling. The rest is *decided in
implementation, to be confirmed* (D-4590-2, D-4590-4 to D-4590-8).

1. **The one pool is the one the chat runs on.** The window keeps where each runner chat runs in
   this machine's store (`chatsOnPools` in `machine.json`), before its runner starts it, so its end
   never comes first, and with the workspace it runs in. A relaunch asks that pool, and only when
   the workspace it is asking about is the one kept. A chat with no such record is asked of the
   pool its workspace runs its chats on, as before. Where a record names a pool, the pool the
   setting names isn't asked as well. A start that names its own pool is kept the same way.
   *(D-4590-1, ruled 2026-11-02)*
2. **A record is what runs there now.** It comes out as the chat ends, as it closes, as a start or
   a reattach lands on this machine under its number, as another pool's start replaces it, and
   after a put-back where the pool says the chat isn't there. It stays only where the chat's link
   went or the pool's answer isn't known, and the next put-back settles it. A runner before
   session protocol 1.67 is answered for by the relay with no chats, so its records go at the first
   put-back, and such a runner takes back no chat in any case. A keep that meets the
   pool's removal keeps nothing. Renaming a workspace carries it. *(D-4590-2)*
3. **Asking a pool sends it no code.** This tightens item 5: the project's own repository goes to
   a runner as a chat starts or moves to it, and never because a project was held there, or as a
   chat moves off it. A hold asks what the runner has, and that is all it does.
   *(D-4590-3, ruled 2026-11-02)*
4. **A pool nothing routes to keeps what isn't this window's.** After a put-back, the pools the
   settings run a workspace on have every chat nothing took back closed, as before. On any other
   pool only a chat with a record is closed. *(D-4590-4)*
5. **What a chat did comes back from the pool it did it on.** A chat that leaves its record leaves
   its workspace owed from that pool (`branchesToFetch`), and each fetch, as a chat ends or as the
   window comes into focus, reads that as well as the settings. A chat's end fetches from its own
   pool alone. What is owed goes only once a fetch of it has no failure, and it is never asked for
   a chat. A fetch from a pool the workspace has left tells it only what came back from it, nothing
   of what happened here since. *(D-4590-5)*
6. **A closed chat's branches come with the next fetch**, as they did. A chat that moves is closed
   before its work comes back into the same clones, so a close starts no fetch. *(D-4590-6)*
7. **Changes reads what came back, not the setting.** Changes and the runners' summary show
   `refs/remotes/<pool>/*` for every pool this machine keeps, skipping a clone's own remote of that
   name. *(D-4590-7)*
8. **What a runner can say of the record.** Item 8 holds for the setting and for the router's own
   record. For this one the window writes it, and no line from a runner adds one or points one
   elsewhere. The exception: a runner's word that a chat ended, or that it doesn't hold one, takes
   that chat's record on that pool out, which is only ever that runner's own and asks nothing
   further of any other host. *(D-4590-8)*
