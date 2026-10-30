# Amended 2026-10-30: a Claude Code chat's server dials in

Follows [why a Claude Code chat's server has no socket of its own yet](2026-10-29-why-a-claude-code-chat-s-server-has-no-socket-of-its-own.md),
and replaces the route it left to try. Ruled by the person on 2026-10-30 (#3316, rulings R1 to R3;
spec #2008).

A Claude Code chat can make no socket of its own, and its sandbox binds every port or none. So its
server makes nothing and binds nothing: it **dials in**.

1. **The server dials the hook socket.** `purlis listen N -- <command>` in a Claude Code chat hands
   the command `PURLIS_SERVE_DIAL` (the chat's hook socket, which its sandbox already lets it
   connect to) in place of `PURLIS_SERVE_SOCKET`. The Node shim (the 2026-10-26 ruling's one
   preload shim, which gains this mode, R2) keeps four connections dialled in. On each it writes
   one `serve` line, `{"chat": <n>, "serve": N}` with the chat's token.
2. **Each line is admitted as every hook line is**: the chat's token, and a sender running inside
   the chat. Then the window checks that the chat's door on N is open, and parks the connection
   as an idle slot of that door, answering `{"taken": true}`. Otherwise it answers
   `{"taken": false, "refused": "<why>"}` and closes the connection. Slots are capped at 16 a
   door and 32 a chat.
3. **The door carries over a slot.** It runs every check it runs for a Codex or opencode chat's
   server (item 3 of the 2026-10-26 amendment). Then it takes one slot and writes the `go` line,
   `{"go": true}`. The shim hands the connection to the server and writes `go` back, and only
   then does the door send the request, so nothing of it is read before the server reads it. The
   shim dials another. With no slot within 3 seconds the door answers `502`, and it never carries
   to any socket.
4. **Where the session host owns the hook socket**, it admits the line and hands the connection
   on to the window, which holds the doors. It sends the descriptor beside the ask over a socket
   of the window's own, `app/slots.sock` beside the hook socket. Only this user's processes are
   read there, and no chat's sandbox reaches it.
5. **Refusals fall back.** A server whose first slot is refused (no grant, no door, a wrong
   token) listens as it would without purlis, and the sandbox refuses that bind and raises the
   Notice as before. A listen on any other port is left as it was.
6. **Nothing of Claude Code's own sandbox changes**: its compiled settings are the same byte for
   byte, and `allowLocalBinding` stays off. The chat makes no socket, binds nothing, accepts
   nothing and is handed no descriptor. It only reads and writes on connections it opened itself
   (R1).
7. **Taking the grant back closes the door at once**, for every chat serving there. A dialled
   door's parked slots close with it, and they close when the chat ends too. Every slot refused,
   and the first each door is given, are audited as `trust.sandbox.serve` with `what = "listen"`.
   Every request spends a slot, so a park after the first says nothing new.
8. **The Notice** for a refused listen in a Claude Code chat offers **Allow only for this chat**
   and **Allow for me on this machine**, as in any chat, with the ruled line (R3): *In a Claude
   Code chat, start the server with `purlis listen 5173 -- npm run dev`. This works for Node
   servers such as Vite, Next.js and Express. Another kind of server cannot be given this one port
   here: start it without the sandbox, or in a Codex or opencode chat.* A server that binds a port
   the chat serves on itself is told the same line and offered Start without the sandbox.

**Limits.** Node servers alone. macOS alone: on Linux a Claude Code chat reaches no unix socket,
so its Notice is as before. A port the system picks stays as #3396 ruled. Handing a direct-exec
server that is not Node a listening descriptor, Claude Code chats on Linux, and moving Claude
Code onto purlis's own wrap (which would retire this) are their own tickets.
