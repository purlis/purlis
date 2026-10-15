# Amendment, 2026-09-26: the ask rule is written by default

The operator ruled on #363 (D11): keep `purlis report --yes <digest>`, and have purlis write
an **ask** permission rule for `purlis report *--yes*` by default, so the harness always asks
the operator before a report is filed. The residual named above, an agent previewing and then
filing in one turn, is now met by a prompt every plane has, not one a plane has to add.

- **`purlis init` writes it** beside the handoff rule, in each harness's own syntax:
  `Bash(purlis report *--yes*)` in `.claude/settings.json`'s `permissions.ask`, and
  `"purlis report *--yes*": "ask"` in `opencode.json`'s `permission.bash`. As with the
  handoff rule, it goes into every harness or none: a file purlis cannot read stops both
  writes.
- **`purlis reinit` adds it** to a plane made before this. It appends to the existing lists
  and touches no other rule, and it mentions the rule only when it added it.
- **Workspace layers carry it at once.** When `init` or `reinit` writes the rule, every
  workspace layer purlis generates is rewritten through the same writer `purlis guard ask`
  uses (#449), so a chat started in a workspace is asked too.
- **`purlis doctor`'s `ask rules` row warns when it is missing** from the settings a chat
  started in that directory reads, or from `opencode.json`, and names the harnesses that lack
  it. `purlis guard report` puts it back, as `purlis guard handoff` does for the handoff
  rule, and `purlis doctor --fix` adds it through the same writer. Removing it stays the
  operator's choice: the row warns and never fails, and only `guard report`, `reinit` and
  `--fix` put it back, all of which the operator runs.
- **Codex has no equivalent.** Codex's `.rules` files (`prefix_rule`) match a command's
  arguments as a prefix, in order, so they cannot say "`--yes` anywhere after
  `purlis report`". A `prefix_rule(["purlis", "report"], decision = "prompt")` would ask
  before every preview as well, and a prefix ending in `--yes` misses a `--yes` that follows
  another flag. So purlis writes nothing for Codex. There, the digest and Codex's own
  approval policy are what stand in front of a filing.

The rule matches `--yes` anywhere after `purlis report`, so both `--yes=<digest>` and a
`--yes <digest>` that follows `--title` get the prompt. Answering `y` at the terminal prompt
is unaffected, because the person typing it is already the operator.
