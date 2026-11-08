# A skill source's files answer may hand its grade

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-08**, ticket #4346, by the operator's ruling of the same day. [ADR 0100's amendment of the same day](../0100/2026-11-08-a-source-carries-a-skills-grade-beside-its-folder.md) lets a git source carry a skill's grade beside its folder. An extension's source shows its skills through the same review, so it needs a way to hand one over.

- **A SkillSource's answer to a skill's files may hold `grade`**: the object a git source's `<folder>.grades.json` holds, read by the same rules. A malformed one is left out of the review with the reason, and the skill's files are still read. An answer without it reads as before, and its skill has no Grade row.
- **No new protocol.** `skill-source` is not in version 1 or 2 of the extension API. It is experimental, and `docs/extension-api.md` says an experimental word may change shape with no protocol promise. So `grade` is added to its files answer in every protocol that asks it, and it becomes part of the contract when `skill-source` joins a version.
- **The extension is not asked again at install.** The grade is part of what a review read (`Held`), beside the files, so an install shows what the review showed.
