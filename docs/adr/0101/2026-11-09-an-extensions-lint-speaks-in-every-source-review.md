# An extension's lint speaks in every source review

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-09**, ticket #3090 (SK-14's follow-up from #3088). §1 says a SkillLint's findings are shown in the live lint and the install review. #2777 built the live lint alone. This says how the review shows them, and where.

- **Every review of a source's files asks the studio's lints**: an install's, an install of several together, and an update's, from a git source or an extension's, in the window and at `purlis skill install` and `update`. An update is the same review of the same files, so it is linted the same way.
- **Each lint is handed the skill's text files as the source holds them**, not as the review escapes them for the person, so a lint that looks for hidden characters sees them. The skill is handed as landing at the level it is reviewed for.
- **A finding is a lint line of the skill, after purlis's own, under the lint's name** (`<lint>: <finding>`), as the live lint says it. It warns and never stops the review, and the digest the install is handed back is not touched: a lint's answer is not part of what is accepted.
- **A lint that does not answer says so in a line** ("could not lint this skill: …"), as in the live lint, so a review never leaves one out without a word. It is asked as the live lint asks it, one question a skill, with no list's time: the person waits on the review anyway.
- **`skill-lint` stays experimental.** This closes #3090, which the amendment of 2026-11-02 named as what keeps it out of version 1, but joining version 1 is still an amendment of its own: the word's shape is unchanged here.
