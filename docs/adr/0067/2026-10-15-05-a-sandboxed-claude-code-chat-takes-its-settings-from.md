# Amended (2026-10-15; corrects section 2's Claude Code row): a sandboxed Claude Code chat takes its settings from purlis and the person, not from files in the project

Decided by the dispatcher under delegation; the operator can veto.

Section 2's Claude Code row is made precise. The rule:

- **The person and purlis.** A sandboxed Claude Code chat, at the full sandbox or the floor,
  runs on what purlis hands it on `--settings`, the person's own settings and an
  administrator's settings. A chat with no sandbox loads what it always did.
- **The project's own files are loaded only while they hold what a chat keeps.** At each start
  purlis reads the project settings files Claude Code would load for the chat's folder: the
  folder's `.claude/settings.json` and `.claude/settings.local.json`, and the local file at the
  top of its repository and of the main checkout of a linked worktree. They are loaded as they
  are while every key in them is one of: `permissions.deny`, `permissions.ask`,
  `enabledPlugins`, `model`, `outputStyle`, `theme` and `$schema`. The list is small on purpose.
  A key Claude Code adds later is not loaded until it is ruled in.
- **Anything else fails closed.** Any other key, a kept key of the wrong shape, a file that is
  there and cannot be read as settings, or a chat whose folder is not known: the chat starts
  with `--setting-sources user`. purlis carries the project's deny and ask rules on its own
  `--settings`, after the rules it compiles. They can only make the chat refuse or ask more.
- **Said on the chat's tab.** The tab's Notice says the project's Claude Code settings are not
  applied in a sandboxed chat, names the keys that were not applied, and says what else does
  not load. Claude Code loads the project's `CLAUDE.md`, `.claude/rules`, skills, agents and
  commands with the project's settings, so a chat started this way does not have them either.
  The key names come from the project's files. They are shown in the window only, printable
  characters only, and a dozen at most.
- **The person's own settings are never written by the chat.** At both levels, the chat's
  commands and its Edit tool are denied the person's `~/.claude/settings.json` and
  `~/.claude/settings.local.json`.

## Decisions

- **D-PS-1: an allowlist, not `--setting-sources user` for every sandboxed chat.** That flag
  also turns off the project's `CLAUDE.md`, rules, skills, agents and commands. The floor is
  the default (2026-10-14-14), so every chat would lose them. So the project's sources stay on,
  and the flag is used only for a project whose files hold more than the allowlist.
- **D-PS-2: the files Claude Code reads, not the walk to the project's root.** The files read
  are the ones Claude Code itself loads for the chat's folder. A parent folder's shared
  settings file is not one, and reading it would take the project's instructions away from
  chats it never reaches.
- **D-PS-3: deny and ask rules are carried. Plugins and the cosmetic keys are not.** A
  project's `enabledPlugins` only chooses among plugins the person installed. purlis's own
  plugin choice and pins ride on `--settings` already (ADR 0050). A chat started on the
  person's settings goes without the project's choice, and its tab says the project's settings
  were not loaded.
- **D-PS-4: the note and the arguments are decided apart.** Both read the same files at the
  same start. The arguments decide what the chat runs on. The note only says so.
