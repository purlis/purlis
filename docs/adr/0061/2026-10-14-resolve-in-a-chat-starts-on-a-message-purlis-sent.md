# Noted 2026-10-14 (qw382b): Resolve in a chat starts on a message purlis sent — ruled by the operator, 2026-10-15

Decided under delegation, and ruled by the operator, 2026-10-15 (#1961). The window's **Resolve in a
chat**, on a save's replay stopped on a conflict (ADR 0051, noted the same day), starts its chat
the way *Fix* does: through the same entry (`savefix::fix_with`), the same call that starts a
chat on a sent message, and the same reason (`SentFor::FixASave`, a step of a save that needs
mending). Its message is the core's (`purlis_core::savefix::conflict`), with the file names and
commit subjects quoted as data. The guard that pins that call now pins the two presses that
reach it.
