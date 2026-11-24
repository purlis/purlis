### Added

- **`purlis policy why <action> <resource>` says why an action is allowed, asked or refused, and
  runs nothing.** It names the rules that decided, with their level and the project file and line
  they are on, the mode when it changed the answer, any lock and who holds it, a grant that
  decided, the permits a forbid overrode, and the nearest level that could loosen it. `--json`
  answers for a script; `--workspace`, `--persona`, `--unattended`, `--sandbox` and `--depth` ask
  for another kind of chat; and `--policy <file or branch>` answers as that policy would and shows
  what differs from the one in force, the same rows the loosening press shows. A chat may ask only
  about itself, so inside a chat none of those flags are taken.
