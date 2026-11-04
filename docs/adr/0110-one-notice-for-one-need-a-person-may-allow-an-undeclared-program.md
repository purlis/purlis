# One notice for one need: a person may allow an undeclared program, and a need is remembered only when asked

**Accepted 2026-11-05**, from the operator's grill of #5227 (M61 · Sandbox experience). It amends
D-PE-ruling-2 (a sandboxed chat runs only its persona's declared tools with a vault, 2026-10-12)
and ADR 0109's offer of a Secret need after a first fill, and it retires #5159's dismissal store.

## Where purlis was

ADR 0109 let a chat ask for any secret, and D-PE-ruling-2 let a value reach only a declared tool.
Nobody put the two together. The person filled a key, the chat heard `filled … run your command
with purlis secret exec`, and the run was refused because `curl` was not a tool. A second Inbox item
then proposed a committed `secret-need.<key>` line, attributed to "chat 0 · Plane root", with the
chat's free-text purpose in it. One ask took two approvals and still got nothing done, and a persona
file collected a line for every one-off key.

## The decision

- **A person may allow an undeclared program, in the window: a Run grant.** One program, as the app
  resolves it where no chat writes, receives the keys its command names from one vault.
  **Allow once** binds the exact command, and stays good until used or the chat ends, because the
  run may have stopped waiting. **Allow for this chat** binds the program, keys and variables with
  any arguments, in memory only. **Always allow** writes the persona's committed tool line, shown
  first, derived from the command with no `args`. It also gives this chat a Run grant, because tools
  are armed at a chat's start. Where the persona was fully allowed on this machine, the allowed
  digest moves for that line alone; where anything else is pending, the usual Notice stays up.
- **What never changes.** A program that runs what it is told is still refused, policy still wins,
  and the run stays in the chat's own sandbox and hosts. A new policy lock turns Run grants off;
  where it or the tools lock applies, the notice offers no Allow and Fill still works. A runner chat
  gets no Run grant for now, since the window cannot show what limits that machine holds it to.
- **One notice for one run.** A run that misses a vault's access, keys or a program's allowance raises
  one notice with whichever of those it hit, one value field per missing key and one Allow. The value
  is stored whether or not the program is allowed; **Don't allow** tells the chat so, and that chat is
  not asked about that program and vault again. `secret_ask` takes an optional `program`, and chats
  are told to prefer simply running the command. An ask that names no program is Fill only, and a
  later run asks for the Allow: the use was not known yet.
- **What the person reads is told apart from what the chat wrote.** The program's path and the
  hosts come from the app. The command is the chat's, shown as plain monospace, wrapped, with
  control characters made visible.
- **A need is remembered only when asked.** The notice has an unticked *Remember this need for the
  persona*. Ticking it shows the exact line with its purpose editable, and Fill writes it, along with
  a `vault:` line where the persona names none. Unticked, nothing is written or offered later. The
  vault row's *Offer as a Secret need* stays as the way to do it afterwards, and #5159's dismissal
  store goes, with no offer left to dismiss.
- **Every grant is seen and audited.** Run grants appear in the Granted list with a revoke that
  applies from the next run, and write audit events naming the program, keys, vault, scope and chat,
  never a value. A proposal purlis itself holds says "purlis" and the chat and ask behind it.

## Why

- **Asking for a secret you cannot use is the dead end ADR 0109 meant to remove.** The route has to
  reach a run, or the fill only moves the stop.
- **A click, not a committed line, for one-off work.** A committed tool is the right home for what a
  persona always does, and the wrong one for a page fetched once.
- **The cost is accepted, and shown.** A chat steered by what it read can ask to hand a value to any
  host it can reach, and Allow for this chat takes any arguments. So the notice shows the full path,
  the command, the hosts and T-4's caution, the default is the narrowest scope, and policy can refuse
  all of it.
- **Purpose text can say what a secret is for.** It stays out of git unless the person, reading the
  line, says otherwise. This narrows ADR 0109's shared need without removing it.

## Considered and not taken

- **A second notice for the Allow, after Fill**: the two approvals #5227 reports.
- **Committed lines only**: keeps D-PE-ruling-2 whole, and leaves undeclared work a dead end.
- **Always allow on this machine only**: a second place tools live, which teammates never see.
- **Run grants on runners now**: a desktop click sending a value to a program under limits the notice
  cannot show.
- **Keeping the proposal and fixing its label**: still an approval nobody asked for, and still a
  committed line per one-off key.
