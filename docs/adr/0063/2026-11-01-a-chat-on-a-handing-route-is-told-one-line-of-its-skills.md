# A chat on a route that hands skills is told one line of them by the server

**Amends [ADR 0063](../0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md), 2026-11-01**, ticket #4510. The 2026-10-20 record says Claude Code, opencode and Codex are not told the fallback route's line, because a chat is told about its skills once, and the 2026-10-21 record says the server's skill line is for a chat on `Mcp` alone.

Those three still do not get `POINTER`. The server's instructions now give them a different line, `NOT_KNOWN`, which says what to do when the harness does not know a skill. It does not tell the chat about its skills again. It covers a skill the route did not hand. Why and what was measured: [ADR 0102's record of the same day](../0102/2026-11-01-a-chat-is-told-to-load-a-skill-its-harness-does-not-know.md).
