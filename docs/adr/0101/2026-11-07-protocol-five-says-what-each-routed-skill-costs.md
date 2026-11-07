# Protocol 5 says what each routed skill costs

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-11-07**, ticket #4680, by the operator's choice on #4654 to keep protocol 4 as frozen and do this as a new protocol. The amendment of 2026-11-02 froze version 1 at protocol 4. This adds version 2, protocol 5, beside it.

**A route question in protocol 5 gives each skill `chars`**: what it costs of the budget's `chars` on the chat's harness, counted as `besides` is. On Claude Code a skill costs its whole line (its name, description, `when_to_use` and the line around them, #4610), so a router that adds description lengths under-counts there, and the chat falls back to the built-in router.

**It also gives `index`, what the index skill costs**, one skill and its line, counted the same way. purlis lists the index whenever a router hides a skill, and its line, like any other, depends on the harness, so a router could not work it out. With `besides`, each skill's `chars` and `index`, a router adds what purlis adds. The issue asked for `chars` alone. `index` is in the same protocol because without it a router that follows `chars` exactly can still go over by the index's line and fall back, and adding it later would be protocol 6.

**A field added to a frozen question is a new protocol.** The freeze and `docs/extension-api.md` say a change to what a request or an answer holds is a new protocol. That rules out adding `chars` to protocol 4, even as a key a router may ignore, because the recorded protocol 4 question would no longer be the question purlis asks.

**purlis asks each router in the protocol its manifest names.** A router of protocol 3 or 4 is asked the route question as protocol 4 froze it, with no `chars` and no `index`, byte for byte. Every other message of protocol 5 is protocol 4's under `"charter": 5`.

**Version 2 is recorded in [`tests/fixtures/extension-api/v2.json`](../../../tests/fixtures/extension-api/v2.json)**, beside `v1.json`, and purlis-core's tests hold each build to both. Neither file changes once recorded. A later change is a new protocol and a new file.
