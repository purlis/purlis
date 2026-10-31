# Amended 2026-10-30: a runner chat's open or dispatch waits for the person's Approve

**Ruled by the person on 2026-10-30, built by #4147.** It follows the 2026-10-20 note on a
runner chat's change to the project, which holds its writes for the person. This note holds its
opens and dispatches the same way.

1. **Held, whatever the persona and whatever the pool.** A handoff (`Ask::Open`) or a task
   (`Ask::Dispatch`) that a chat on a pool's runner asks this machine for starts nothing when it
   arrives. That covers a dispatch to the chat's own persona, which needs no dispatch grant from
   a chat here, and every kind of pool, Docker and cluster alike. It is decided the way a runner
   write is: by the link the line came over (`hooks::heard_from_a_pool`), and for a line that
   came over this machine's own link, by the router's record of the chat on a pool. It is held
   right after its ticket is spent and before anything else decides it (`runnerdispatches`).
   The chat is told it is held for the person.
2. **Approve in its own tab, Refuse anywhere, in the window only.** The held ask shows in the
   chat's tab and the Inbox: the pool, the persona, the task's name or the workspace a handoff
   moves to, and the brief on one line, drawn inertly. A row cuts the brief, so it offers
   **Open** and **Refuse**, never Approve. Open shows the ask whole in its own tab: the asking
   chat by name, the persona, the task or the workspace it moves to, where it works, the
   profile it would start on, whether it creates a workspace or runs long, the whole brief, and
   that the runner chat can keep instructing a task and read what it says. **Approve** is
   pressed there and decides it the way a dispatch of a chat here is decided, against the
   limits, grants and chats as they stand, so a dispatch to another persona still needs its
   grant. **Refuse** starts nothing. The chat is told in words on its next turn, and the ask is
   listed on the Dispatches tab with the ones kept blocked. None of these commands is served over
   a link. There is no "always approve", and a chat has at most 8 held.
3. **In memory only**, as a runner write is. A relaunch starts with none, and a chat's held
   asks end with it.
4. **The person's own dispatches are unchanged**, including "Ask a persona" from a runner
   chat's tab.
