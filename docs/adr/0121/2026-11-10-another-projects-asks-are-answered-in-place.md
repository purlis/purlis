# Amended 2026-11-10: another project's asks are answered in place

Decision 4 (Asks spans every open project) and decision 6 (answer once for all, in any project)
are built for asks from other projects (#1921). Until now, another project's asks were listed
after this project's as text, with Go to project, as [the amendment on order](2026-11-10-asks-are-ordered-by-facts-as-built.md)
left them.

- **Each window tells the others its projects' asks, ask by ask** (`windows.ts`, `WindowSaid.asks`),
  as it already told them its chats and its fleet. A project of this window is read from its own
  registry. A window that tells none (an older build) still has its chats listed a chat at a time,
  with Go to chat, and the ✋ counts them a chat at a time; once told, it counts asks.
- **Another project's ask is a row of Asks like this project's own**: its options on its row
  (the Q55 amendment), a visible Answer where it needs typing, the settle guard (ADR 0097), and
  New by its own project's seen record. Its row names its project and, where known, its
  workspace and persona. Ignore, Snooze and Stop stay with its own project's Asks.
- **Every project's asks stand in one order by facts** (`asksOrder.ts`): a chat is its project
  and its session, so two projects' chats with the same number are two groups. Of two groups of
  the same age, this project's comes first. This replaces "listed after this project's, not
  interleaved".
- **Identical requests group across projects** (`asksSame.ts`), and Allow for all answers each
  ask in its own project. A high-risk ask is never grouped, whichever project it is in (decision
  8's seam, `highRisk`, is unchanged).
- **An answer is sent to the core for the ask's own project.** The core holds every project of
  every window in one process (`Planes`), so no window relays it: the answer is the same command
  that project's own Asks sends, named with that project. Before sending, the window reads that
  project's asks again: an ask answered meanwhile, in its own window or anywhere, sends nothing,
  and its row says "This ask was already answered, in its own project or another window, so
  nothing was sent." Its own project's window hears the change from the core as before and tells
  the others.
- **An answer that opens something in its project's window is left to that project's Asks**
  (`opensItsWindow`): Open on a proposed change or a runner's write or dispatch, a sign-in,
  setting up signing, removing old harness plugins and making a sandbox-only profile. Go to chat
  or Go to project leads there. A dispatch grant, which its own project's Notice draws, is
  answered in place only for a project of this window; from another window its row has Go to chat.
- **The Inbox's scope switch reads narrowest first** (This workspace, This project, All
  projects), as the Chats switch does (#1921). The default is still All projects.
- Nothing new is stored, so no tier is named (ADR 0069). The seen records written are each
  project's own (`inbox_seen`), as before.
