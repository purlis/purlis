# Amended for #2243, a chat's browser is tested end to end on a real engine, 2026-10-20

The spec's second seam for browsers (#2008, Testing Decisions) is one test that drives every hop
at once: the chat's browser tools through the tool server on the hook socket, the window's Ask,
the session host, and a real engine on the fixture site. This note says how that test is built.
Nothing a person or a chat sees changes.

1. **Every hop is the one the person runs.** The chat is the fake harness, run in a real
   `purlis serve` as the window runs its chats there by default. The window is the test's own
   process, with its planes, asks and browsers wired as the app wires them, and no Tauri window.
   The person's presses are the functions the window's commands call. The engine is the pinned
   Chrome for Testing, downloaded into the host's data home on first use and checked as on any
   machine. The site is the engine's fixture site, served on a loopback port that the person
   allowed as a browser site. *D-2243-1, decided in implementation, to be confirmed.*
2. **The fake harness starts the tool server as a harness does.** `fake-harness --mcp-relay`
   starts `purlis mcp` as its own child, on stdio, and hands that stdio to the test over a
   socket. The test then speaks to the tool server as the model does. The server's lines to the
   hook socket come from inside the chat, and the chat's hook channel admits only those, so a
   tool server the test started itself would be refused. *D-2243-2, decided in implementation,
   to be confirmed.*
3. **It runs on Linux CI, in the `rust` job.** The behaviour suite's host process runs there,
   because each test can give its host a runtime directory, a config home and a data home of its
   own (on macOS the host's socket is in this user's own temporary folder). The job downloads
   the engine for each test that starts one, so the tests are few and each covers a whole story.
   Off CI they run when `PURLIS_TEST_PINNED_ENGINE` is set. *D-2243-3, decided in
   implementation, to be confirmed.*
