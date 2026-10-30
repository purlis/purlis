# Amended 2026-10-30: a Claude Code chat on a runner runs in purlis's wrap

Ruled by the operator on 2026-10-30 (#4083). A person's real Docker pool refused every Claude Code
chat: the backend check asked for bubblewrap and socat, which the runner image does not carry, and
Claude Code's own sandbox needs a user namespace that a pod or a default container does not give.

1. **On a runner, a Claude Code chat runs whole inside purlis's Linux wrap** (Landlock, seccomp
   and the supervisor), as Codex and opencode chats do. Claude Code's own sandbox is switched off
   there (`"sandbox": {"enabled": false}` in its `--settings`). The runner image stays without
   bubblewrap and socat. A runner whose kernel cannot apply the wrap refuses the chat in words, as
   for Codex.
2. **A runner is a Linux machine whose session host is paired with a desktop.** The start reads
   that from the host's own store, which no chat reads or writes. On a person's own Linux machine
   nothing changes: a Claude Code chat keeps its own sandbox, and needs what it always needed.
3. **It is held at least as Codex is.** Its config folder is written only where a turn writes, at
   the full sandbox and at the floor, and only purlis writes `.claude.json`, before the wrap,
   as it writes Codex's config. A chat never writes its settings, hooks, skills, commands, agents
   or plugins. Its Read, Edit and web tools keep the rules compiled for them. Its traffic, its
   model's included, goes through the chat's proxy, whichever version it is.
4. **What being wrapped whole gives up, as Codex does.** The harness and its commands are held to
   one wrap, so a command can read the sign-in the harness reads (`[sign_in] kept_from_commands`,
   ADR 0073). The chats on one profile share what a turn writes (D-88s).
5. **Its commands run unasked**, as they do under its own sandbox, whose
   `autoAllowBashIfSandboxed` is off with it: `Bash` is allowed in its `--settings`, and a deny or
   an ask still outranks that.
6. **A folder trusted for a chat.** A chat on a runner cannot save Claude Code's answer to its
   trust question, and would be asked at every start, so purlis marks the chat's folder trusted
   in `.claude.json` before the wrap. Project settings are screened as before.

Proven by live probes: the program Claude Code ships, armed by a runner's own host, in the runner
parity suite (`a_real_claude_code_chat_on_a_runner_is_held_as_a_codex_chat_is`), and the wrap's
profile applied by this build's `purlis _sandbox` (`a_claude_code_chat_on_a_runner_holds_as_they_do`).
