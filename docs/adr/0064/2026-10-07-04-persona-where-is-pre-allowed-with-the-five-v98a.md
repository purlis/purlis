# Amended 2026-10-07: `persona_where` is pre-allowed with the five (V98a, #1450)

The operator's ruling V98a, first decided at the assembly of train 58 by its dispatcher as
D-T58-1 and confirmed by the operator: `mcp__purlis__persona_where` joins
`chattools::PRE_ALLOWED`, on the rules above (an `allow` only, by its full name, for Claude Code
only, and an operator's `ask` or `deny` still wins).

- **It only reads, and what it reads is the app's.** The tool takes no arguments and asks the
  app that started the chat over the chat's hook socket. The ask is bound to the sender, so a
  chat can ask about itself alone.
- **It answers nothing the chat is not told unasked.** Names, personas, workspaces, states and
  start times of the chat that asked for it, that chat's other tasks and the other chats running
  as its persona: what the chat's start briefing and its turn updates already say, and never
  another chat's brief or transcript.
- **The list is still written out.** A tool added later still asks until it is ruled in.
