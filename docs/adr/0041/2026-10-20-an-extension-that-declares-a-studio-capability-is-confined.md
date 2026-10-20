# Amended 2026-10-20: an extension that declares a studio capability is confined (ADR 0101)

The operator ruled on 2026-10-20 (#2777), **"confine first"**. [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md)
amends decision 1, and the amendment of 2026-09-22 that shipped the runtime with no OS sandbox,
for one kind of extension:

- **An extension that declares SkillSource, Grader, SkillLint, SkillTemplate, StudioPanel or
  SkillRouter runs confined**, in the sandbox profile a chat gets (ADR 0067): network only to the
  hosts its manifest declares, writes only to its own state folder and the paths a request hands
  it, and the denial classes held. Its whole program is confined, for every request.
- **Decision 2's "declared, not granted" network row becomes enforced for it**: a declared host
  is what it can reach.
- **Its approval prompt says it runs confined**, from the core and pinned by a test, in place of
  the sentence the 2026-09-22 amendment requires, which would be false for it.
- **A program purlis cannot confine does not run** (ADR 0031), and third parties can register
  the six capabilities only once the confinement is in. Built-in extensions are trusted through
  the app, as before.

Every other extension stays watched, not confined, as the 2026-09-22 amendment rules, until
M47's E2.

**Built by #2777.**
