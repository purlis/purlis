# Amended 2026-11-12: a persona's Allow is a Blocking ask

The person reported, from the live app (0.5.2-dev.2190), that steward's tools had changed since
their Allow on this machine (`changed: true` in the machine's grants record). steward's chats
were refused their tool, and only the chat's terminal said so. The one thing that asked was
`PersonaHostsNotice`, a Notice among the project's others, and they found it only in Settings ›
Sandbox. The core, the window and Settings each drew the state correctly. The Allow was not
missing. It was out of sight, at the level of a Notice, while it stopped chats.

Decision 2 names "a persona grant" among the decisions that stop no chat. A persona's hosts and
tools waiting for an Allow on this machine do stop chats: each chat as that persona reaches none
of its hosts and runs none of its tools until the person allows them. **So it is a Blocking ask**,
built as #6000 built the project's "expects" consent (amended 2026-11-10):

- **Which persona asks.** The same rule as its Notice (`waitingPersonas`): not allowed as its list
  stands, with hosts a chat would reach (none where policy forbids persona hosts) or tools. A list
  allowed once and changed since asks again. A persona grant that stops no chat (a dispatch grant
  a teammate committed, say) stays a Notice or an Update.
- **One ask per persona**, of no chat's (`persona-allow:<persona>`), however many of its chats are
  refused. It is counted on the ✋, listed in Asks under its project, and notified, like every
  Blocking ask.
- **Its answers are its Notice's.** Allow (`allow_persona_hosts`) is sent the digest of the list
  the row showed, and a list changed since is refused and asks anew. Its label says who it is for,
  as the Notice's does ("Allow for steward chats", and for the default persona "and chats that
  name no persona"). Not now (`persona_allow_not_now`) puts it away while the project is open and
  the list stands. Nothing is kept, as the Notice's Not now kept nothing; a changed list, or the
  project opened again, asks again.
- **Risk and settle apply.** A host of its list that looks like production is its risk
  (decision 8), so its Allow asks once more and keyboard triage passes it by. The settle guard
  holds its Allow as it holds every grant (ADR 0097).
- **Its Notice stays, drawn from the ask.** In the window the Notice is shown only for a persona
  whose ask the project's asks list, so one answer in either place settles both. A link reads no
  asks and has no Allow, so it draws the Notice as before, saying where to allow it.
- **A refusal names the row.** A chat refused a persona's tool or browser login for the want of
  this Allow is told where it waits: "the persona's row in Asks, under the ✋ in the purlis window,
  or Settings › Sandbox" (`sandbox::tools::ALLOW_IS_ASKED`), and no longer "the project's Notice".
- **The asks follow the project's settings.** A project's list of asks is read again when its
  settings change on disk (a pull, a branch switched, a hand's edit), as its Notice already was,
  since a teammate's change to a persona's list asks again.

Not now is held in memory with the open project (`Held::persona_later`), so it is no store and
names no tier (ADR 0069).
