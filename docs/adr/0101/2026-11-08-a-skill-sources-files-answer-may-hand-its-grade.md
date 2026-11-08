# A skill source's files answer may hand its grade

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-08**, ticket #4346, by the operator's ruling of the same day. [ADR 0100's amendment of the same day](../0100/2026-11-08-a-source-carries-a-skills-grade-beside-its-folder.md) lets a git source carry a skill's grade beside its folder. An extension's source shows its skills through the same review, so it needs a way to hand one over.

- **A SkillSource's answer to a skill's files may hold `grade`**: the object a git source's `<folder>.grades.json` holds, read by the same rules. A malformed one is left out of the review with the reason, and the skill's files are still read.
- **It is a new protocol.** A field added to a frozen answer is a new protocol (the amendment of 2026-11-07). This one takes the next number after those already in `docs/extension-api.md`. An extension of an earlier protocol is asked and read byte for byte as before, and its skills have no Grade row.
- **The extension is not asked again at install.** The grade is part of what a review read (`Held`), beside the files, so the install shows what the review showed.
