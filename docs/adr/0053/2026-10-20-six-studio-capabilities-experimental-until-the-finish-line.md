# Amended 2026-10-20: six studio capabilities, experimental until the finish line (ADR 0101)

[ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md) adds six words to the vocabulary, each with a built-in first user:

| Word | What purlis does | First user |
|---|---|---|
| `skill-source` | asks for a source's skills, and one skill's folder at an immutable revision | the git source |
| `grader` | asks for a grade, and runs the chats the Grader asks for | the built-in runner |
| `skill-lint` | asks for findings on a skill's files | the built-in lints |
| `skill-template` | offers starting points for a new skill, as a draft skill | the blank and starter templates |
| `studio-panel` | asks for a panel on a skill's tab, drawn with panel blocks | the Runs and Grades panels |
| `skill-router` | asks which skills a chat sees, within a deadline (ADR 0102) | the built-in router |

None adds a permission, and "what no capability grants" holds. Two things differ from every word
before them:

- **An extension that declares one is confined** (ADR 0101 §2), so for it a capability is no
  longer only a description of purlis's conduct: `RUNS_AS_YOU` is false for it.
- **They are experimental while the `skill-studio` flag is on**, unknown where it is off, and
  become version 1 of the public extension API (PE-38) when it flips at the Skill studio's finish
  line.

A grade is one question with requests inside it: the Grader asks purlis to run chats, and its
question has no fixed deadline while they run. The protocol version that carries this is #2777's
and #2779's to set, under this record's rule.

**Built by #2777, #2778 and #2779.**
