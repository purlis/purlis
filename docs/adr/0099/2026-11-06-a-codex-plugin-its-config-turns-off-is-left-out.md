# A Codex plugin its config turns off is left out

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-06**, ticket #4616 (M65.1), which carries the amendment of 2026-11-01 (#4477) to Codex.

That amendment said Codex says nothing off yet, so every plugin Codex's `config.toml` names was listed as the chat's, an `enabled = false` one too.

## What a Codex chat has

Measured on **codex-cli 0.163.0** (ADR 0050 as amended 2026-10-21, runs 4 and 5): a plugin whose `[plugins."<id>"]` table says `enabled = false` reaches no model, and a session's `-c 'plugins."<id>".enabled=true'` does not bring it back. A table that does not say `enabled` is on, as `Codex::installed` already read it.

So `harness_plugin::on_here::codex` reads `$CODEX_HOME/config.toml` (else `~/.codex/config.toml`) at the chat's start, parses no harness output, and the chat holds the answer as a Claude Code chat does. An off plugin folds to one line under Left out, "Off in Codex", and its run closes on the file and the key to change, then "restart this chat" (or "start a new chat" with no chat in front). No command is offered: no Codex command that turns one plugin back on was measured.

## Known limits

- A project's own `.codex/config.toml` is not read for plugins: what it does to one was never measured.
- opencode still says nothing off.
