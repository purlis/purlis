# Amended (2026-10-29): a task starts in its asking chat's mode, capped at auto

Ruled by the operator on 2026-10-29 (#3787), amending decision 20 of the 2026-10-07 grill
([amended and accepted](2026-10-07-amended-and-accepted.md), §3: "the persona chat never
inherits the bypass", and "it never takes the asking chat's per-chat grants, its opt-out or its
permission mode"). A task always started in its harness's default, asking mode. A chat in auto
mode therefore started tasks that stopped on their own tool prompts in tabs nobody watched.

| Asking chat          | Task    |
|----------------------|---------|
| default (asks)       | default |
| auto                 | auto    |
| bypass (prompts off) | auto    |

- **Capped at auto, never bypass.** Auto switches nothing off: the harness's classifier still
  judges every tool call, and purlis's sandbox and guards stand. A mode that is not
  classifier-backed (Claude Code's `dontAsk`, `acceptEdits`, `plan`, or any word purlis does not
  know) starts the task in its harness's default. "The persona chat never inherits the bypass"
  stands as written, and so does the refusal of a profile whose command switches the prompts
  off, whoever named it (`dispatchunattended::bypass_refusal`).
- **The app's own record, never a line the chat sends.** The asking chat's mode is the last one
  its own harness named in a hook payload's `permission_mode`, read from the chat's hook
  reports as the board takes them: not a sub-agent's, and only about the conversation the board
  holds for the chat (`dispatchmode::of_its_own_harness`, `Heard`). Unlike the unattended mark
  it goes both ways: a chat the person switches out of auto starts its next task asking. A chat
  whose harness never named a mode starts its tasks asking. #3822 hardens the source of those
  reports, as ruled to follow this change.
- **One neutral level, one adapter per harness** (`dispatchmode::Level`). Claude Code reads its
  modes by 2.1.312's own table (`auto` is reported as `auto`) and starts a task in auto with
  `--permission-mode auto`, unless the profile's command names a mode of its own.
  Codex and opencode read nothing and add nothing: Codex names `bypassPermissions` for every
  chat, and neither documents a classifier-backed mode, so their tasks keep their default.
- **A profile's own mode is the person's declaration, and wins** (the dispatcher's decision in
  review, as decision 20 already treats profiles). A task on a profile whose command names
  `--permission-mode` starts in that mode, with nothing added. `dontAsk` there is allowed, as it
  is stricter than the default: it refuses what is not allowed ahead, and asks nobody. A
  profile that switches the prompts off stays refused.
- **A grade's chat is never started above its grade's mode**: no words are added for it,
  whatever level its record holds.
- **A task only.** A handoff's chat is the person's to read, and starts in its default.
- **Never on disk.** The level rides the task's record in the app's memory and is not written to
  the plane's record of its chats, so a chat brought back from that record after a restart
  starts asking: conservative, and recorded as such.
- **Unchanged.** Decision 20's grant rules for an unattended chat: it dispatches only under a
  standing grant, a missing grant is a refusal and not a Notice, and with no sandbox it
  dispatches to no other persona. A task in auto is not unattended by this: the asking chat's
  own mark decides how its dispatches are answered, and the task's own harness reports decide
  its own.
