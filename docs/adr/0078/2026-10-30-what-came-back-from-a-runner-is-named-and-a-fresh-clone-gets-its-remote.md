# Amended 2026-10-30: what came back from a runner is named, and a fresh clone gets its remote

**Built by #3990 and #3985**, under spec #2041, after the amendment of the same day (a clone the
runner chat made comes back with Move here). The guards are the person's ruling of 2026-10-27
(item 8 of that day's amendment), unchanged; the rest was **decided in implementation, to be
confirmed** (D-3990-1 to D-3990-2 and D-3985-1 to D-3985-5 in the pull request).

1. **The harnesses' instruction files are what a tool here reads** (#3990). A harness here reads
   `CLAUDE.md`, `CLAUDE.local.md`, `AGENTS.md`, `AGENTS.override.md`, `GEMINI.md`,
   `.cursorrules`, `.windsurfrules` and `.clinerules` in any folder of a clone, and Copilot its
   `.github/copilot-instructions.md` and `.github/instructions/`, as instructions, so the chat's
   tab names each that came back, with the other files of the ruling's third guard. Names are
   compared without case, as a disk here may not tell them apart.
2. **A clone made fresh is said to be whole from the runner** (#3990): the chat's tab says it was
   cloned there and all of it was made there, before the files it names.
3. **A clone made fresh gets the runner's `origin`** (#3985; this amends item 2 of the
   amendment of 2026-10-30, *"No remote … of the runner's comes with it"*, for `origin` alone).
   Session protocol 1.54: the `clones` answer may carry `origins`, the `origin` URL of each
   clone's own configuration on the runner, read without includes and sent only where it is a
   forge's HTTPS or SSH address with no sign-in in it, and only to a link of 1.54 or later.
   **A runner-supplied URL is untrusted data**, so the desktop holds it again to the rules a URL
   pasted here is held to (`getproject::ProjectUrl::parse`: no other transport, no local path,
   no token or user name over HTTPS, nothing git or ssh reads as an option, no whitespace or
   control character) and to 2,048 bytes, and writes nothing else of the runner's: only
   `remote.origin.url` and git's default fetch refspec. It is never recorded as a remote purlis's
   saves push to with the person's sign-in: the person pushes it themselves. Where none comes,
   one is refused, the pool keeps its branches under `refs/remotes/origin/`, or the clone has an
   `origin`, the clone stays here with no remote, and the chat's tab says why and how to give it
   one. The chat's tab names the remote it was given, so the person sees where a push goes.
