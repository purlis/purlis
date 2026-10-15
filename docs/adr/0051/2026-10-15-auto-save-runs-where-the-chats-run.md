# Noted 2026-10-15 (#2002, spec #1974): auto-save runs where the chats run

Decided in implementation, to be confirmed.

Where a project's chats run in the session host, behind the start-time switch, the host runs
the project's auto-save and the plane's watcher, by the same code, and the window starts none:
exactly one saves a project. What the record lists under "Auto-save lives in the app" holds as
written; ADR 0068's amendment for #2002 says how.
