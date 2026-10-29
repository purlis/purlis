# Amended 2026-10-29: a rider is refused for a retry, never asked about (#3785)

Ruled by the operator in a grill on 2026-10-29, superseding D-1444-14 (the amendment of
2026-10-07 for #1444, *What is not measured, and what purlis does about it*). Chats stopped on
"Do you want to proceed?" around dispatch: a hook's `ask` shows the harness's prompt in every
permission mode, auto mode included, so `purlis dispatch list 2>&1 | head -20` stopped a chat
nobody was at.

**The person is asked only when something new reaches someone new. What a chat does with its
own tasks is its own.**

| Group | Subcommands | Asked? |
|---|---|---|
| Look | `list`, `read`, `wait`, `--help` | never |
| Talk to its own task or its own asker | `tell`, `note`, `ask`, `answer`, `report` | never |
| Tidy its own tasks | `close` | never |
| Stop its own task | `cancel` | never, but refused for a task the person has typed in, as `close` is |
| Start something new | `dispatch` (with or without `--wait`), `handoff` | the app's dispatch grant only (unchanged) |
| Beside another command | any | no `ask`: see below |

- **A call that runs a dispatch that only looks, beside anything, gets no hook verdict.** The
  harness's own rules decide it, as they decide any command it has a rule for.
- **A call that runs a dispatch that writes, a handoff or `purlis worktree commit` beside
  another command is refused**, naming the other command and the shape to retry ("run the
  dispatch in a call of its own"), so the chat corrects itself with nobody pressed. The same
  holds for a file-writing redirection on its line, a dispatch brief fed from anything but one
  quoted heredoc, and a handoff that pins its clock. `dispatchguard::rider_refusal`, the last
  arm of the Bash guard, after every refusal of its own reason.
- **`Bash(purlis dispatch close *)` joins `harness::claude::DISPATCH_TASK_ALLOW`.** It was never
  added, so every close prompted. The app refuses a close of a task that has not finished, that
  the person typed in or is looking at, and one in a turn.
- **`dispatch cancel` is refused, in one sentence, for a task the person has typed in**, as
  `close` is. This replaces D-1459-3's cancel that waited for the person's keys.
- Unchanged: the live-substitution refusals for text subcommands (A7e), the helper refusal
  (A7d), and Claude Code being the only harness the rider rule speaks to.

The item of #1471 that asked the operator to confirm D-1444-14 is closed by this ruling. The
measurement it rested on (does a hook's `ask` win over an `allow`) is no longer load-bearing:
purlis no longer answers `ask` for a rider.
