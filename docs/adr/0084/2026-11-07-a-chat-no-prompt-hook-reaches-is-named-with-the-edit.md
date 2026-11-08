# Amended (2026-11-07): a chat no prompt hook reaches is named with the edit, and level 3 waits for its host (#5477)

§8 says where a **Human edit**'s line goes: the prompt hook at level 2, a `SessionStart` briefing
and purlis's MCP server on a harness with no prompt hook, and the structured context of the next
prompt at level 3. #5409 built the first. This records what holds for the other two, read off
what purlis runs on 2026-11-07.

## Level 2: every harness purlis ships is told

Claude Code, Codex and opencode each have a prompt hook purlis arms (`Harness::prompt_hook`):
Claude Code's and Codex's `UserPromptSubmit`, and opencode's `chat.message` in purlis's plugin.
Each carries the line as `additionalContext`. So no level-2 harness lacks a prompt hook, and §8's
`SessionStart` fallback has no harness to serve. It stays the rule for one that comes.

## Level 1: the chat is named with the edit

A harness a project or this machine declares runs at level 1, its terminal alone (ADR 0073,
V24c: level 2 is purlis's own code). It has no hook of any kind, so neither a prompt hook nor a
`SessionStart` briefing reaches it. purlis's tools answer only when the agent calls one, so they
can't announce anything either. And purlis never types a prompt for the operator (§8).

**So the window tells the operator instead.** A Discard or an Undo a turn names each chat working
in the folder whose harness has no prompt hook purlis arms: *{chat} is not told of this change at
its next turn, because purlis cannot add to its prompts. Tell it in its chat if it should know.*
The edit is still recorded, and a shell, which runs no agent, isn't named.

A built-in's chat whose hooks never armed (the operator chose *Continue without trusting* in
Codex, say) isn't told either, and isn't named: whether a hook will run is the harness's, and is
known only once it reports. The audit calls such a run **unarmed** (ADR 0075).

## Level 3: §8 stands, and waits for the host that delivers a prompt

No chat runs at level 3 yet. The core's ACP client (`purlis_core::acp`) is started only by its
own tests and the fake harness's. The app starts every chat in its terminal, and the 2026-11-02
amendment of ADR 0073 records dispatched tasks at level 3 ahead of the behaviour. With no
level-3 chat, there is nothing to tell and no chat to name.

When that host is built, it tells a level-3 chat the same way as any other. It adds the same
line (`humanedit::announcement` of `humanedit::since_last_turn`) to the next prompt it delivers,
as context beside the operator's prompt and never as one. Its own `prompted` move (ADR 0076: *the
host delivered a prompt*) is what closes those edits, so `since_last_turn` learns to stop at it as
it stops at `hook.userpromptsubmit` today. That is part of the host's definition of done, not a
change to make ahead of it.
