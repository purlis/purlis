# Amended 2026-11-10: every sandboxed Claude Code chat on Linux runs in purlis's wrap

Taken under the operator's standing instruction (the recommended path at every choice), 2026-11-10.
The next step of the amendment of 2026-11-09, which wrapped a Claude Code chat on a sandbox-only
profile.

1. **On Linux, every sandboxed Claude Code chat runs whole inside purlis's wrap**, on a person's own
   machine as on a runner: Landlock, seccomp and the supervisor, with Claude Code's own sandbox
   switched off. Claude Code's own sandbox (bubblewrap and socat) is no longer used on Linux; macOS
   is unchanged.
2. **It starts only on a profile for sandboxed chats only, as a sandboxed Codex chat does
   (D-88q).** A sandboxed Claude Code chat on a profile for every chat on a person's own Linux
   machine is refused before anything starts, and the refusal says the way out. On a runner a
   Claude Code profile is for wrapped chats only already (amended 2026-10-30), so nothing changes
   there. A chat started without the sandbox runs on the person's own profiles as before.
3. **The window guides the person to one.** The refused start leaves one Ask per project and
   harness, *Claude Code needs a sandbox-only profile here*, beside Codex's: in plain words it says
   the sandboxed chat keeps its own sign-in apart from theirs, and the three steps — Make one,
   sign in in the shell it opens, start the chat again. Make one makes `claude-sandbox` (then
   `claude-sandbox-2`, …) with nothing imported and opens Claude Code's own sign-in in a shell
   tab. Settings' *For sandboxed chats only* row says the same rule.
4. **A held folder's lift is read from the chat's own record.** Whether a refused write of a held
   folder may be lifted for a chat turns on whether its harness runs whole inside purlis's wrap,
   as its start recorded it (`Confines::whole`), not on what this machine would start now.
5. **Values purlis puts in a run's environment.** With every sandboxed Claude Code chat on Linux
   wrapped whole, a value purlis itself puts in a chat's or a run's environment (a minted
   `GH_TOKEN`, a forge tool's host variables, an approved MCP server's `sets`) is held inside the
   wrap with the harness, as for Codex and opencode. The `--env` refusal of 2026-11-09 stays for a
   chat whose record says it runs on Claude Code's own sandbox.
6. **What it costs.** A person on Linux makes and signs in one more profile before their first
   sandboxed Claude Code chat, and a sandboxed chat does not read their own Claude Code settings,
   transcripts or memory. Chats on that profile share its folder (D-88s's residual, as for Codex).
