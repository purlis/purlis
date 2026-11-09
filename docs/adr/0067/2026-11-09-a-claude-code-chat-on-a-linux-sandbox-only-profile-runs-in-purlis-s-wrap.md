# Amended 2026-11-09: a Claude Code chat on a Linux sandbox-only profile runs in purlis's wrap

Taken under the operator's standing instruction (the recommended path at every choice), 2026-11-09.
The direction is purlis's own wrap for Claude Code chats on Linux; this is its first step.

1. **On Linux, a sandboxed Claude Code chat on a profile for sandboxed chats only (#2037) runs whole
   inside purlis's wrap**, on a person's own machine as on a runner (amended 2026-10-30): Landlock,
   seccomp and the supervisor, with Claude Code's own sandbox switched off in its `--settings`. It
   needs the kernel and no bubblewrap or socat, and the picker asks the same of that profile.
2. **Held as on a runner, for the same reason.** A sandbox-only profile never starts a chat without
   the sandbox, so what a wrapped chat writes in its folder (a transcript, a memory, a folder marked
   trusted in `.claude.json`) is read only by other wrapped chats: D-88s's residual and no wider, as
   for Codex (D-88q). Its sign-in is read by the harness inside the wrap, so its commands can read
   it too, and Claude Code's declaration says so.
3. **What it asks of the project, as Codex does.** At the full sandbox its model's traffic goes
   through the chat's proxy, so the project's hosts list its model's (`model-providers`).
4. **A profile for every chat is unchanged.** A Claude Code chat on one keeps Claude Code's own
   sandbox, and needs what it always needed. Moving those chats into the wrap asks every such
   profile to become one for sandboxed chats only, and is the next step.
