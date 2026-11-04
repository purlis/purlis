# The studio words are known on every machine

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-04**, ticket #2786 (SK-23). The `skill-studio` flag is removed ([ADR 0099 as amended 2026-11-04](../0099/2026-11-04-the-skill-studio-flag-is-gone.md)). This changes no ruling. §7 said the six words are unknown on a machine with the flag off. All six are now known on every machine (`Capability::known`).

[The 2026-11-02 amendment](2026-11-02-version-one-holds-the-three-capabilities-fully-wired.md) holds as written: when the flag went, `skill-lint`, `skill-template` and `studio-panel` stayed version 1, and `skill-source`, `grader` and `skill-router` stayed known and marked experimental. An extension that declares any of the six still runs confined.
