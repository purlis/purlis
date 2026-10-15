# Amended 2026-10-07: a dispatch runs without the harness asking (V98b, #1436, #1437)

The operator's ruling V98b, on train 59 (D-T59-prealllow): a Claude Code chat the app starts
carries an `allow` for the dispatch command by each spelling it has, `Bash(purlis dispatch
--name *)`, `Bash(purlis dispatch --to *)`, `Bash(purlis dispatch --profile *)` and
`Bash(purlis dispatch report *)`, and for `mcp__purlis__dispatch` and
`mcp__purlis__dispatch_report`, on the rules above (an `allow` only, each by its full name, for
Claude Code only, and an operator's `ask` or `deny` still wins).

- **No bare trailing wildcard.** `Bash(purlis dispatch *)` would also allow every subcommand
  `dispatch` grows later. The four rules are the task, started by any of its three flags, and
  its report; a flag-first line cannot reach a subcommand. A later subcommand is added by name
  when it lands.
- **These are not reads, and the reason is not that they are harmless.** A dispatch starts a
  chat. What consents to it is purlis's own **dispatch grant** (spec #1434, decision 4, ADR
  0090 as amended): the app decides every dispatch from its own record of the asking chat, asks
  the person once for a pair of personas on the asking chat's tab with the brief in front of
  them, and starts nothing until they allow it. That prompt is the same on every harness. The
  harness's prompt beside it asked twice for one thing, and asked for a chat's own persona,
  where no grant is needed.
- **What it does not allow.** The handoff command keeps its harness prompt until it is
  converted (#1444). A compound command that holds `purlis dispatch` beside another command
  is expected to be asked about still, as one holding `purlis session record` was measured to
  be above; that was measured for the record command and is carried over here, not measured
  again. Nothing is added for Codex or opencode by this ruling.
- **Where the grant is not the control.** An unattended chat is never asked for a grant and
  dispatches only under one that stands (#1446). A helper sub-agent's dispatch is refused by
  purlis's hook where the hook can tell one made the call, and that refusal is advice: a grant
  covers what a chat's sub-agents ask in its name, and the grant Notice says so (V98d).
- **The two tools are named apart** (`chattools::DISPATCH_TOOLS`), never added to
  `PRE_ALLOWED`, which stays the tools that only read. A tool added later still asks.
