# Version 1 holds the three capabilities that are fully wired

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-02**, ticket #2786 (SK-23), by the operator's ruling of 2026-10-31 on that ticket: capabilities that aren't fully wired (#3089-#3092, M65.1) stay out of the v1 freeze. §7 said all six become version 1 when the flag flips. This narrows that, and moves the freeze ahead of the flip.

**`grader`, `studio-panel` and `skill-router` are version 1** of the public extension API, recorded in [`docs/extension-api.md`](../../extension-api.md) and held to [`tests/fixtures/extension-api/v1.json`](../../../tests/fixtures/extension-api/v1.json). The approval prompt and the Extensions list stop calling them experimental. From here on they change only as ADR 0053 says a capability changes: by a protocol version, with every earlier version still answered.

**`skill-source`, `skill-lint` and `skill-template` stay experimental**, and may change shape with no protocol promise, until their tickets close: #3089 and #3092, #3090, and #3091. The same word stays in the prompt. Each joins version 1 by an amendment of its own.

**The freeze does not turn anything on.** Where the `skill-studio` flag is off, all six words are still unknown and refuse a manifest, until #2786 removes the flag. When it goes, the three experimental words stay known and marked: the flag says who has the studio, not what is finished.

**A Grader's version 1 scores trigger tests alone.** Outcome evals are purlis's own runner's (ADR 0105, amended 2026-10-31), so a Grader's `outcomes` is empty, and giving one outcome evals is a new protocol.

**The wire keeps the word `charter`.** Every line of every protocol carries the protocol under that key, and version 1 freezes it as it is. Renaming it would be a new protocol for no gain to an extension.
