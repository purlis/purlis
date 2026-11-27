# Amendment, 2026-11-26: a start held by a prerequisite exits 75

Amends [ADR 0120](../0120-a-purlis-command-exits-70-for-an-internal-failure-and-only-then.md), by
the person's decisions recorded in
[ADR 0130](../0130-prerequisites-what-a-project-needs-from-the-person-before-its-chats-can-work.md).

**The documented codes are now:**

- **70**, `EX_SOFTWARE`: a failure inside purlis, as decided.
- **75**, `EX_TEMPFAIL`: a chat start from the CLI that an open **prerequisite** held and that ended
  unanswered, because there was no terminal to wait at or because its wait of up to 90 seconds ran
  out. It says what is open and that it is answered in purlis's window on this machine. It is a
  failure to try again once the person answers, as `secrets::brokered::WAITING_ON_THE_PERSON` (a
  command that stopped waiting on a Secret ask) already exits.
- **1**: a refusal, a policy's block, a missing sign-in or a usage error, as decided.

A Ctrl-C during the wait ends the command as any interrupted command ends.
