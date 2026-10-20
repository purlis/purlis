# Amended 2026-10-20: an extension that declares a studio capability is confined (ADR 0101)

[ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md) amends decision 1, and the amendment of 2026-09-22, under the operator's
ruling of 2026-10-20 (#2777), "confine first". An extension that declares a studio capability
runs confined in a chat's sandbox profile: it reads and writes only its own folder and what
purlis hands it, and reaches only the hosts it declares, never a model provider's. A program
purlis cannot confine does not run. Every other extension stays watched, not confined.

**Built by #2777.**
