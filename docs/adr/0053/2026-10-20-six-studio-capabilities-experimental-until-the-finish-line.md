# Amended 2026-10-20: six studio capabilities, experimental until the finish line (ADR 0101)

[ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md) adds six words to the vocabulary, the studio capabilities: `skill-source`,
`grader`, `skill-lint`, `skill-template`, `studio-panel` and `skill-router`, each with a
built-in first user. None adds a permission. An extension that declares one is confined, so for
it a capability also limits what it can do. They are experimental while the `skill-studio` flag
is on, and become version 1 of the public extension API (PE-38) at the Skill studio's finish
line.

**Built by #2777, #2778 and #2779.**
