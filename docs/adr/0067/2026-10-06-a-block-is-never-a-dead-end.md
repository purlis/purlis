# Amended (2026-10-06, #1342): a block is never a dead end

The operator's ruling of 2026-10-06 ("go with recommendations", option (a)) amends §1 and §7.
"Only a person turns the sandbox off, for one chat" stays (since 2026-10-15 a person also sets
their own level for their chats, ADR 0096, and the team's policy bounds it: see the section of
that date). A person may also widen one chat's
sandbox, or every chat's, by exactly what a block named:

1. **A grant names one host or one folder to write.** The block's Notice on the chat's tab
   offers **Allow for this chat**, **Always allow** (every chat of this project on this
   machine; or, for a host, everyone in the project, as one of the project's own hosts) and
   **Keep blocked**. What it would allow is shown whole before the press. A folder is never the
   project's: it is a path on one machine. A wildcard host is never proposed from a block: a
   person types one in Settings.
2. **A folder comes only from an allowlist** (2026-10-06, D-1342-10, D-1342-11), judged on
   the folder the kernel would write (D-1342-12): it is resolved first, so a link judges as where
   it points, and only that resolved folder is kept and compiled, and judged again at every
   start (one later swapped for a link is dropped). On macOS names compare case-folded. The
   allowlist is the project tree and a folder the person lists in Settings › Sandbox, on this
   machine only, kept as it resolved when listed and dropped, with a word in Settings, once it
   resolves elsewhere; Settings warns when one holds launch agents, `PATH` folders or a
   harness's own. No temp folder is on it (D-1342-14): no per-chat temp folder is known to
   grant, and a harness's sandboxed temp folder is shared by every session of its user. No
   refusal is ever lifted for a project that happens to live under a refused folder. No cache folder is on it by default: a cache is shared by
   every project, and many hold code another program later runs. On top of it, never a folder on
   `PATH`, a harness's own home or temp root, never `/`, the home folder or a folder above the
   project, and never the classes of §5: vault storage, purlis's integrity state, human powers
   and what later code loads, which get the way that works instead (`purlis secret exec`,
   purlis's own commands, or the person's own change).
   Anything else (much of the machine is shared, or loaded later outside any sandbox) gets no Allow:
   the Notice says why and offers **Start without the sandbox for this chat**, the person's own
   §7 opt-out for that chat's next run, so a block never dead-ends. A local socket gets the same.
   A read is never granted: a chat's reads are denied only for the classes of §5. A granted
   folder that holds a denied path keeps the denial: every compiler writes its denials after its
   grants, and for Claude Code every later-code name is denied again under each granted folder
   by absolute path.
3. **A grant reaches a running chat by restarting it on its conversation** (spike #1347:
   Claude Code pins `--settings`, so nothing reloads a grant live in a terminal chat). The app
   restarts it once its turn has ended, and its first message says what was allowed, so it
   retries without anyone typing.
4. **Every grant and every revoke is audited** (`trust.sandbox.grant`, `trust.sandbox.revoke`),
   before it takes effect: no event log, no grant. Settings › Sandbox › Granted lists each one
   with Revoke (#1348). A policy can lock one out (`hosts::Locks`, #1343).
5. **Allow once** is ruled in: one exact command, run outside the sandbox through the harness's
   own per-command mechanism, with purlis's hook as the only approver, the command shown whole,
   never auto-approved in any mode, classes 1–3 refused, audited, and a policy can turn it off.
   It is a person's per-command opt-out, consistent with §7 narrowed to one command. Not built
   yet; until it is, `allowUnsandboxedCommands` stays `false`.
