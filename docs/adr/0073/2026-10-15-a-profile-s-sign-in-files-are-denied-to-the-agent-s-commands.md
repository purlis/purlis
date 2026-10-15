# Amended 2026-10-15: a profile's sign-in files are denied to the agent's commands (#2037)

From the operator's sandbox round of 2026-10-14 (Q46 to Q54), as ADR 0095 §5 records it: a
profile's credential files are denied to the agent's commands wherever the harness runs its
commands apart from itself, and otherwise the profile page says commands can read them.

- **The sign-in files of each harness purlis ships** are a fixed list in purlis's code
  (`sandbox::signin::credential_files`), not declaration data: Claude Code's `.credentials.json`
  in its config folder, Codex's `auth.json` in its home, and opencode's `auth.json` and
  `mcp-auth.json` in `opencode` under its data folder. The folders are the chat's own profile's:
  those its folder variables name in the chat's environment, and the harness's own defaults
  where they name none.
- **Each compiler denies them** to the agent's commands, reading and writing, with the
  human-powers class, where the harness runs its commands apart from itself. Claude Code's
  sandbox holds the commands its Bash tool runs, and a deny rule its Read and Edit tools, while
  its own process reads its sign-in, so its compiler denies them. Codex and opencode run whole
  inside purlis's wrap with their commands, so their wraps cannot.
- **Claude Code answers `kept_from_commands` no**, decided from its sandbox as purlis compiles
  it, not by probing a running one: on macOS purlis cannot keep its sign-in from the commands it
  runs.
- **The profile page** (Settings › You › Profiles) says, on each profile whose harness does not
  answer yes, that the commands its chats run can read its sign-in, with the declaration's
  reason.
