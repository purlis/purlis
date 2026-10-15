# Amended 2026-10-06: the record is a brokered write, and a person's act is the pass (#1332)

The operator's ruling of 2026-10-06 on spec #1330 ("smart close must work without approval;
starting it is the approval"), with ADR 0067 §2's brokered writes:

- **The record command's twin is pre-allowed beside it.** purlis's MCP server offers
  `session_record`, the same operation as `purlis session record` through the other entrance
  (ADR 0067 §2). The same session `--settings` allow it by its full name,
  `mcp__purlis__session_record`, on the rules above: an `allow` only, and an operator's `ask` or
  `deny` still wins. It was the one write tool allowed when this was written; the two
  dispatch tools joined it by ruling V98b (above), and the others still ask.
- **The app writes the record.** In a chat the app started, the command and the tool hand the
  title and body to the app over the chat's hook socket, and the app writes it from its own
  record of the chat. Where no app takes it, the command writes it itself, as above.
- **The smart-close pass.** The tab closes on a record only while a person's smart close is under
  way: the tab's **Smart close**, or the person typing `/smart-close` into a waiting chat. A
  `UserPromptSubmit` that says `/smart-close` is believed only beside the person's own Enter in
  that chat's pane moments before. Under the pass, a record the app wrote closes the tab when
  that turn ends, so the chat's own call has its answer first.
