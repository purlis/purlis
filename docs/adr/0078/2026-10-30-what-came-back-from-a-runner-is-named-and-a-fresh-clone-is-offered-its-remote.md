# Amended 2026-10-30: what came back from a runner is named, and a fresh clone is offered its remote

**Built by #3990 and #3985**, under spec #2041, after the amendment of the same day (a clone the
runner chat made comes back with Move here). The guards are the person's ruling of 2026-10-27
(item 8 of that day's amendment), unchanged; the rest was **decided in implementation, to be
confirmed** (D-3990-1 to D-3990-2 and D-3985-1 to D-3985-6 in the pull request).

1. **The harnesses' instruction files are what a tool here reads** (#3990). A harness here reads
   `CLAUDE.md`, `CLAUDE.local.md`, `AGENTS.md`, `AGENTS.override.md`, `GEMINI.md`, `.cursorrules`,
   `.windsurfrules` and `.clinerules` in any folder of a clone, Copilot its
   `.github/copilot-instructions.md`, `.github/instructions/`, prompts and agents, opencode its
   `.opencode/` (whose plugins it runs) and `opencode.jsonc`, and the other agents their own folders
   (`.agents/`, `.windsurf/`, `.roo/`, `.kiro/`, `.continue/`, `.amazonq/`, `.junie/`, `.qwen/`) and
   files (`QWEN.md`, `CRUSH.md`, Zed's `.rules`, `.goosehints`, `.aider.conf.yml`), as instructions,
   so the chat's tab names each that came back, with the other files of the ruling's third guard.
   Names are compared without case, as a disk here may not tell them apart.
2. **A clone made fresh is said to be whole from the runner** (#3990): the chat's tab says it was
   cloned there and all of it was made there, before the files it names.
3. **A clone made fresh is offered the runner's `origin`** (#3985). Session protocol 1.54: the
   `clones` answer may carry `origins`, the `origin` URL in each clone's own config file on the
   runner (its own `.git/config` alone, a regular file: no include, no `commondir`, no `.git` file
   or link to another repository's), sent only where it is a forge's HTTPS or SSH address with no
   sign-in in it, and only to a link of 1.54 or later. **A runner-supplied URL is untrusted data**,
   so the desktop holds it again to the rules a URL pasted here is held to
   (`getproject::ProjectUrl::parse`: no other transport, no local path, no token or user name over
   HTTPS, nothing git or ssh reads as an option), to a lower-case `https://` or `ssh://`, to ASCII
   letters, digits and `-._~:/@+` alone with no leading `~`, and to 2,048 bytes. **It is never
   written into the clone.** The chat's tab offers it with the one command that adds it, which a
   shell takes as it is (the URL single-quoted), so a runner's remote becomes the clone's only by
   the person's own act. A remote written for them would be one purlis's saves push to on their own,
   with the forge CLI's sign-in, as for any clone whose remote purlis has not recorded, and no mark
   of a clone's own config, which a chat in it can rewrite, could hold them back. Where none is
   offered, because the runner named none, it was refused, or the pool keeps its branches under
   `refs/remotes/origin/`, the tab says why and how to give the clone a remote. This amends nothing
   of the amendment of 2026-10-30: no remote of the runner's comes with the clone.
