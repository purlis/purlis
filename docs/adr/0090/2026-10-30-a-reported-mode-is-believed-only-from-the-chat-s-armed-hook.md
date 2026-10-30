# Amended (2026-10-30): a reported mode is believed only from the chat's armed hook

Follows [the 2026-10-29 amendment](2026-10-29-a-task-starts-in-its-asking-chat-s-mode-capped-at-auto.md),
as the operator ruled on 2026-10-29 (#3822: "#3787 ships first; this hardens it"). A hook
report was believed on the chat's token alone, and the token is in the chat's environment, so
it says only that a line came from inside the chat. Since #3787 a report's permission mode also
decides how the chat's tasks start, so that word now needs proof it came from the hook the app
armed.

- **An armed key per chat.** The app mints one (32 random bytes, in memory) as it issues the
  chat's token, and writes it for its owner alone to `armed/<chat>` in the hook spool beside the
  chat's socket (`hookwire::armed`). It is never in the chat's environment or on its command
  line. A hook reads it and sends it beside the token with each report, on the socket only: a
  spooled line never carries it. It is removed as the chat ends, and the key of every chat a
  project's open does not bring back is removed with that chat's spool keys. A listener's bind
  removes none: one listener reads a socket, and a key the app does not hold proves nothing.
- **A report without it reads as asking.** The listener compares the key in constant time; a
  report that carries none, or not its own chat's current one, has its mode read as asking
  before anything hears it. Such a line can lower the level a chat's next task starts at and
  can never raise it. A report that names no mode names none either way. Every other part of a
  report is heard as before. The unattended mark a report carries is still taken without the
  key: it can only restrict what a chat may do, so a report that cannot prove itself gains
  nothing by it.
- **What the hook can read that the chat's tools cannot (Claude Code).** The spool is in ADR
  0067 §5's integrity class: a sandboxed chat is denied reading and writing it, for its Bash
  tool by the sandbox's `denyRead` and for its Read and Edit tools by deny rules, which 2.1.285
  was measured to hold; a test holds the armed folder among them under every spelling of the
  state folder. Claude Code runs its hooks outside the sandbox it gives its tools (measured on
  2.1.285, recorded in `sandbox::claude`), so its hook reads the key. In a chat with no sandbox
  the tools have the person's own reach and read the key too, so there the key proves no more
  than the token did. Nor does it prove anything against a tool the chat drives that runs
  outside the sandbox with the person's own reach: such a tool reads what a hook reads. Not
  measured again live for this change: it rests on the measurements above and the sandbox
  tests that hold the spool's denial.
- **Codex and opencode prove nothing beyond the token.** Their hooks run inside purlis's wrap,
  which denies them the spool, so their reports carry no key. Neither harness's reports name a
  mode purlis reads (#3787), so nothing changes for their chats; each adapter says so.
- **Fail closed.** Where the spool is not one the denial covers (a socket outside a project's
  state folder), no key is written and every mode reads as asking. A key that cannot be minted
  or written is said in the log and none is held, with the same result.
