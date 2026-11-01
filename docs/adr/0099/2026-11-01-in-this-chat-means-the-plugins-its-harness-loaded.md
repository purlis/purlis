# In this chat means the plugins its harness loaded

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-01**, ticket #4477 (M65). Layout and wording ruled by the operator on the prototype (`prototype/skills-plugin-off`).

A harness's record of what is installed says what is on the machine. A plugin's skills were listed as a chat's from that record alone, so the Skills view named skills the chat did not have, and Use on any plugin's skill typed nothing.

## What a chat has

Claude Code decides at a chat's start, as measured on 2.1.316 from the `init` event in a throwaway `CLAUDE_CONFIG_DIR`:

- a plugin loads only where `enabledPlugins[id]` is literally `true`; absent is off, and so is a list of versions;
- later wins: the user's `settings.json`, the chat folder's `.claude/settings.json`, its `.claude/settings.local.json`, the repository top's `.claude/settings.local.json`, then `--settings`; a parent folder's `.claude/settings.json` is not read;
- `--setting-sources user` drops the project files, both ways;
- an enabled plugin needs a known marketplace, in `plugins/known_marketplaces.json` or an `extraKnownMarketplaces`;
- a running chat keeps what it started with; a resumed one reads these again.

So purlis reads these files at the start, parses no harness output (`harness_plugin::on_here`), and holds the answer for the chat (`hoststart::ChatSkills::plugins`), in memory. A plugin installed since the start is one the chat lacks. A restart resumes the chat, which is why the window may say "restart this chat". An administrator's managed settings are not read. The answer is not in the chat's record, so a chat reattached after purlis restarts is read again, and a change since its start shows early there.

## What the person sees

In this chat holds a plugin's skills only while the chat has it. The others fold to one line per plugin under Left out, with no Use, and the run closes on why and, where a command puts it right, Copy command. A marketplace that is gone says why and offers no command. A filter on a name still shows them. Use on a plugin the chat has types `/plugin:skill`; the picker's own list stays the chat's set. The header's count of the harness's own skills counts what the chat has.

With no chat in front, the rows say what a new chat in the workspace's folder would have, reading that folder's settings. A runner's chat reads nothing here, so its rows are as the record says. The rows themselves come from the app's own environment: a chat whose profile points at another config folder gets its answer from that folder, and the rows list what the app's folder holds. Codex and opencode say nothing off yet: only Claude Code reads this.
