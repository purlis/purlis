# ADR 0066, amended by #1435 (2026-10-07): a chat's persona never changes

**Ruled by the operator on 2026-10-07**, with the acceptance of [ADR 0090](../0090-agents-work-together-as-chats-that-are-listed-observable-and-stoppable-and-no-agents-word-is-consent.md)
as amended: a persona is a role a chat runs as for its whole life. That takes back the persona
half of V21's third call, *"A mid-chat persona adoption or model switch starts a new run, and
`purlis persona use` tells the host on the hook channel."*

- **No persona is adopted mid-chat.** `purlis persona use` inside a chat the app started is
  refused and writes nothing, so it has nothing to tell the host. Work for another persona goes
  to a chat of its own, by dispatch.
- **A chat's runs all have one persona.** `persona` stays a field of a run, because the run is
  who an action is attributed to, and its value is the chat's for every run of that chat.
- **The `switch` cause keeps its other rows:** another harness or profile, and a model a hook
  reports. *"a persona adopted mid-chat"* no longer happens. The cost this record names, *"Adopting
  a persona in the middle of a conversation starts a new run"*, goes with it.
- **A model switch still starts a new run**, as V21 ruled.

The text above is left as it was written, and this section is the amendment.
