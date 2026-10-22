# Amendment, 2026-10-21: Codex turns a plugin off per chat, and every harness a skill

Ticket #2769 (SK-6, spec #2762, decisions 5 and 25), which finishes #274's Codex and opencode
adapters where the harnesses allow it now.

## Codex: a plugin off, for one chat

Measured again on **codex-cli 0.163.0**, in a throwaway `CODEX_HOME` with one plugin from a local
marketplace (`codex plugin marketplace add <dir>`, then `codex plugin add zp@zmkt`), reading
which of its skills `codex debug prompt-input` showed the model:

| Run | `config.toml` | Session flag | The plugin's skill reached the model |
|---|---|---|---|
| 1 | `enabled = true` | none | yes |
| 2 | `enabled = true` | `-c 'plugins."zp@zmkt".enabled=false'` | **no** |
| 3 | `enabled = true` | `-c 'plugins={"zp@zmkt"={enabled=false}}'` | **no** |
| 4 | `enabled = false` | none | no |
| 5 | `enabled = false` | `-c 'plugins."zp@zmkt".enabled=true'` | no |

0.147.0 ignored every one of these flags (the record above). 0.163.0 takes a plugin turned
**off** for one session, and still takes one turned **on** as nothing. So Codex's adapter is a
new kind, `Support::OffOnly`: a chat is handed `-c 'plugins."<id>".enabled=false'` for each
installed plugin the project turned off, and nothing for one it turned on, which is the
harness's own `config.toml` to decide. The settings tab and the Skill studio say so in one line
("Codex is handed a plugin only to turn it off — …"), and no choice for Codex is said ignored
any more. purlis still never writes Codex's config.

## opencode: still not supported yet, for plugins

Measured on **opencode 1.18.33**. A base config (`OPENCODE_CONFIG`) names two file plugins, `a.js` and `b.js`, and each row adds a session layer in `OPENCODE_CONFIG_CONTENT`, the layer purlis already writes. The columns are what `opencode debug config` resolves:

| Session layer | `plugin` | `plugin_origins` (what `plugin/index.ts` loads) |
|---|---|---|
| none | a, b | a, b |
| `{"plugin":["file://…/b.js"]}` | a, b | a (base), b (session): the lists are concatenated |
| `{"plugin":[]}` | `[]` | a, b: still both |

An empty session list clears the shown key, but the loader reads `plugin_origins`, which keeps every config's entries. The only switches are `--pure` (no plugin at all, purlis's shim included) and `OPENCODE_DISABLE_DEFAULT_PLUGINS` (the built-in ones). So a project's choice for an opencode plugin is still listed, said *not supported yet*, and handed to nothing. **This amends spec #2762's decision 25** ("plugin on/off on all three harnesses") for opencode. #3009 keeps it open, re-measured with each opencode release. opencode's plugins bring no skills, so the Skill studio loses no skill switch to it.

## A harness's own skill, off for one chat, on all three

A project can turn off one of a harness's own skills (one in the person's folders for it, a
plugin's, or a clone's committed one) in `[harness_skills.<harness>]`, on the same three layers
as `[harness_plugins]` and through the same precedence. Each adapter hands a `false` to the chat
alone, with nothing written:

| Harness | How | Measured |
|---|---|---|
| Claude Code 2.1.304 | `--settings` `{"skillOverrides": {"<name>": "off"}}` | `langfuse` left the init event's skills |
| Codex 0.163.0 | `-c 'skills.config=[{name="<name>",enabled=false},…]'` | the skill left the catalog; a plugin's skill (`zp:zp-skill`) too; an entry in `config.toml` kept beside it |
| opencode 1.18.33 | `permission.skill` `{"<name>": "deny"}` in `OPENCODE_CONFIG_CONTENT` | `opencode debug agent build` showed the rule `skill <name> deny` after the operator's own |

**Where a harness cannot, purlis says so.** Claude Code's `skillOverrides` does not reach a
plugin's skill (its settings reference), so a `[harness_skills.claude]."<plugin>:<skill>"` is
refused by the save, and the Skill studio draws no switch for one: it says the plugin is what
turns it off. A built-in skill's name is refused under every harness, and never handed as turned off. Only while the Skill studio's flag is on is the table read at a chat's start. Codex's off-only plugin rule is not behind the flag: it honours the `[harness_plugins]` keys #274 already reads, and the Skill studio's way back for a Codex plugin is "Leave to Codex", since Codex's own config decides whether one is on.
