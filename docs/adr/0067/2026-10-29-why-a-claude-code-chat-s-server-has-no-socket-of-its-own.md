# Amended 2026-10-29: why a Claude Code chat's server has no socket of its own yet

Follows [a chat's server behind purlis's front door](2026-10-26-a-chat-may-listen-on-a-port.md),
item 6. Decided in implementation (#3316, spec #2008).

A Codex or opencode chat serves on a socket of its own behind purlis's door. The same was tried for
a Claude Code chat, through its sandbox's own settings, and measured under Seatbelt on the macOS
confinement job (`sandbox::localport::tests::under_seatbelt`):

1. **Its socket rules would do.** Claude Code 2.1.312 writes each `network.allowUnixSockets` path
   as `network-bind` and `network-outbound` on that path, with no `network-inbound`. Under those
   rules, with the socket's file allowed to be made, a server binds, listens and is reached
   through the door.
2. **Its file rules do not.** Binding a socket makes its file, which is a write. Claude Code
   writes every `denyWrite` path after every `allowWrite`, so a denial always outranks an allow
   inside it: a server cannot bind a socket in a denied folder, whatever the socket rules allow.
3. **Only a denied folder keeps another chat out.** At the floor, the default level, a Claude Code
   chat may write every folder no denial holds. A socket path the chat may make is then one any
   other chat may make too, and a chat that could take another's socket would be handed that
   chat's browser traffic by its door. purlis keeps every chat's sockets in one folder denied to
   every chat's writes (item 4), and a denial cannot leave out the one chat's own socket.

So no setting of Claude Code's own gives its chat one socket that only it can make. A Claude Code
chat whose server is refused its port is still offered Start without the sandbox (item 6).

**The route left to try** is one in which the chat makes nothing: purlis binds and listens on the
socket itself, outside the sandbox, and hands the listening socket to `purlis listen` over the
chat's hook socket, which hands it to the server. The chat then needs no write and no socket rule.
It is a new way into a sandboxed chat, so it is left to its own ticket and design.

**Replaced 2026-10-30** by [a Claude Code chat's server dials in](2026-10-30-a-claude-code-chat-s-server-dials-in.md):
the app hands the chat connections, never a listening socket.
