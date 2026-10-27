# Amended (2026-10-16, #1040; the operator's rulings of 2026-10-14 and 2026-10-16): purlis's own wrap on Linux

Ruled by the operator in spec #2041 (2026-10-14) and on #1040 (2026-10-16). This replaces the
bubblewrap invocation that section 2's table and the 2026-10-03 amendment named for opencode and
Codex on Linux.

**A Codex or opencode chat on Linux runs inside purlis's own wrap: Landlock, a seccomp filter
and a supervisor, built from the same compiled profile as on macOS.** It needs no namespace and
no privilege, so the same wrap works on a Linux desktop, in a Docker container and in a
Kubernetes pod under their default seccomp profiles, and a chat on a runner is sandboxed exactly
as a local one (spec #2041). *(A chat's server listening there needs one call more, which a
Docker pool's runner is given and a Kubernetes runner is not yet: amended 2026-10-26, a Linux
chat listens only where it was bound.)*

- **One profile, two mechanisms.** purlis compiles one profile for a wrapped chat. macOS applies
  it with Seatbelt. On Linux the supervisor reads the same profile and judges every connect and
  every file operation by it, the last matching rule deciding, as in Seatbelt. A construct the
  supervisor does not read refuses the chat; it is never passed over.
- **The supervisor performs what it judges.** It is the wrap's own process and the chat's
  parent. The chat's filter hands it every call a path or an address decides: opening, making,
  moving, linking, removing and changing the mode of a file, and connecting a socket. It walks
  the path one name at a time from folders it holds, judges where the walk arrived as the
  kernel names it, makes the call itself, and hands the chat the result or the descriptor. The
  kernel never goes on with the chat's own call.
- **Landlock holds both sides.** The chat opens, writes and connects nothing by itself, binds
  no port, signals no process outside the sandbox and reaches no abstract socket made outside
  it; it may run programs. The supervisor is held to what the profile could ever let a chat
  write and reach.
- **The filter refuses what would go round the supervisor**: the calls of another ABI; a change
  of a file's owner or extended attributes; datagram, raw and packet sockets;
  TCP Fast Open; io_uring and `openat2`; the kernel's key store; opening a file by handle; and a
  seccomp listener of the chat's own. A process the chat starts inherits all of it.
- **Every class of section 5 holds**, the later-code names at any depth of every folder a chat
  writes included, and so does egress: a wrapped chat connects to its own proxy's ports on this
  machine and its hook socket, and to nothing else. The credential store (class 1) is held on
  Linux for these harnesses: the wrap reaches no socket of the session bus. A Claude Code chat
  on Linux keeps Claude Code's own sandbox, as before.
- **The floor too.** A project that has not turned the full sandbox on starts Codex and opencode
  chats at the floor inside the same wrap, as on macOS, with the later-code names of a Linux
  session held as macOS's are. So a Codex chat there starts only on a profile for sandboxed
  chats (#2037), and a chat at the floor does not run over ACP.
- **Fail closed.** The wrap needs Landlock ABI 6 (Linux 6.12) or later. In a project with
  `mode = "on"`, a kernel without it starts no wrapped chat, and the refusal names the version
  and the ways on: a newer kernel, a Claude Code profile, or, where policy allows, starting the
  chat without the sandbox from the new-chat picker. At the floor such a chat starts without it,
  and its tab says why.
- **The entry point** is the hidden `purlis _sandbox` command, which a wrapped chat's line
  starts with. It is not shown in help and is not a public command line.
- **The seccomp notification calls** are the workspace's third audited `unsafe` block
  (`purlis_linux_sandbox::linux::notify::raw`), by the operator's ruling of 2026-10-16: one
  more small, isolated, audited module, and no new C dependency.
