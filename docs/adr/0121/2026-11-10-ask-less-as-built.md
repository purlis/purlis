# Amendment, 2026-11-10: ask less, as built

Built for #6010 from [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
decision 7, which takes over #5285 (a cool-off). What the decision leaves open, as built:

- **The same answer to the same request.** A request is what identical requests share
  (decision 6): a sandbox's host, folder or service by its target, a permission by its line, a
  run of a program by its program, vault, persona, folder and command (ADR 0112 binds a grant to
  that). The same Allow, given twice within a week with no other answer between, offers a rule
  on the request's next ask. A rule made for the request starts the count again.
- **The scopes are the ask's own grants.** The row offers the ask's grants wider than the Allow
  given, each in its own words ("Allow for me on this machine", "Allow for everyone in this
  project"), so a rule is written where its source always wrote that grant, through the path
  the ask names, checked and audited as any answer is, and held by the row's settle guard
  (ADR 0097). Each grant is classed this chat, this project or this machine; "Allow for me on
  this machine" is this project's on this machine, and a host is never every project's (a host
  stays a project's). The suggested one is drawn first: the narrowest that would have prevented
  every one of the asks (one chat's asks: this chat; several chats of one project: this project;
  several projects: this machine), else the widest the ask offers.
- **A rule is offered only where Rules I made can undo it.** That is a sandbox's host, folder or
  service, and a reviewed program's Allow for this chat (a Run grant), both on Settings' Granted
  list, whose Revoke is Undo. Not yet: a reviewed program's Always allow, which declares a tool
  in the persona's committed definition, and a permission rule a harness saves in its own
  settings, which purlis does not take out again. Each needs its own undo before it is offered.
  Never an arbitrary command, never a high-risk ask (decision 8; the seam is #6011's).
- **Rules I made** is linked at the head of Asks once a rule is made. Each rule says what it
  allows, for whom, in which project, and how many asks it has saved; Undo takes its grant away
  as Revoke does and forgets the rule. A grant already gone is forgotten without an error. The
  count of asks saved is kept for #6021 to read; nothing feeds it yet.
- **The cool-off.** Don't allow, twice in a row within an hour, from one chat to one request,
  holds that chat's same ask quietly for an hour after the last: listed apart, as a snoozed ask
  is, so neither the ✋ nor a notification counts it. Another chat's same ask is counted. An
  Allow between the two resets it. The head of Asks says whose asks are held, and until when,
  with End cool-off. The held ask's own timeout is unchanged: a permission's falls back to its
  harness's own prompt, as an unanswered one always has.
- **Where it is kept.** `<data>/inbox/rules.json`, this machine's alone (docs/plane-format.md).
  It holds the record of a grant, never the grant.
