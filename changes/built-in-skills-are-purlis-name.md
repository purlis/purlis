### Changed

- **purlis's own skills are named `purlis-<name>` on every harness.** `handoff` is now `purlis-handoff`, and on Claude Code `/purlis:purlis-handoff`. Old names keep working where purlis reads them: `purlis:handoff` in a persona's `skills:` list, a skill's text or `skill_load` reads as the new name, so no project has to change. A bare `handoff` is now an ordinary skill name, so a project's skill named `handoff` loads. A bare `/smart-close` no longer starts purlis's smart close (#4524).
- **The `purlis-` prefix is reserved.** New, rename, install and share refuse it for any skill but a built-in one. A project's skill already named so stays on disk and is left out of chats, saying why, until it is renamed (#4524).
