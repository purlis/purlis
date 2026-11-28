# Amendment 2026-11-27: a chat put back is restarted onto a route of its own

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D3a ("What is not
built": a reattached chat has no route) and D3b (the sentinel), as D3a was amended for #7694, and
§3 of `2026-11-27-a-large-push-is-read-and-every-refused-push-says-why.md`. From #7962.

## What was wrong

After purlis restarted, a task chat the session host had kept running was put back on its
conversation as the run it was. The route its program had been handed went with the app that
started it. Since #7694 the new app makes a route for the run, but only the app's own git uses it:
the program still holds the old route's sentinel in its environment, and every request it sends
is refused `UnknownChat`, `WrongSentinel` or, after a few, `Locked`. Its own `git push` and `gh`
got HTTP 403 for as long as the program ran. Nothing restarted it, so the remedy, starting the chat
again, was left to the person, who was not told it was needed.

## Why the running program cannot be handed the new route

D3b holds the sentinel **in the app's memory and the chat's environment only**, **minted at every
chat start and at a resume and never reused**. So:

- **The old sentinel is not honoured again.** It was the old app's, in memory that is gone. Keeping
  it across a restart would mean writing it somewhere outside memory, which D3b rules out.
- **The new sentinel cannot reach the running program.** A process's environment is set when it
  starts. Writing the sentinel to a file the chat reads would be a second place it is held, which
  D3b rules out too. The sandbox profile the program runs under also names its own socket from
  its start (D3a, ADR 0067's amendment item 4), so a new socket could not be reached anyway.

So the restart is the remedy, as for a sandbox grant (#1342), and purlis now makes it.

## Decision

1. **A chat put back whose route this app made anew is owed a restart on its conversation.** Its
   start holds the route for the app's git as before (#7694), and the route's guard says that the
   program holds no sentinel of it. The chat is then owed a restart, with a line saying why.
   Nothing is owed where this app already held the run's route (its program's sentinel is that
   route's), where no route was made (nothing recorded, or a pinned forge), where the chat has no
   conversation to resume, and for a shell.
2. **The restart is the one restart** (`Chats::restart`). The window takes it at the chat's next
   pause between turns, never mid-turn. A harness that says nothing of its turns gets Restart now
   on its pane instead, as for any owed restart. The window reads the owed list again once the
   project's chats are back (`project-ready`), since they are put back after it first read it.
3. **The restart hands the new run its own route.** It is a start: a new run, a new sentinel from
   the operating system's random source, a new folder and socket, all in the program's
   environment. The old sentinel is refused on the new run. The put-back run's route stops when
   its session closes.
4. **Ownership records are unchanged.** A branch the chat made is recorded under the chat's stable
   id, not its run's (`createdbranches`), so the restarted chat may push the branches it made, from
   the tip it last pushed, with its own git and with `purlis worktree push`.

## Not changed

Nothing is handed to a running program. A chat on a pool's runner put back from the session host
is the runner's to route, as before. Until the restart, the refusal's words of §3 of the
2026-11-27 amendment still say to push with `purlis worktree push`.
