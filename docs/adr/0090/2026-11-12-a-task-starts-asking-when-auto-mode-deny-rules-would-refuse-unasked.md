# Amended (2026-11-12): a task starts asking when auto mode deny rules would refuse it unasked

Ruled by the operator on 2026-11-12 (#6315), amending
[a task starts in its asking chat's mode, capped at auto](2026-10-29-a-task-starts-in-its-asking-chat-s-mode-capped-at-auto.md):
"Default mode".

Claude Code 2.1.322 asks nobody in place of auto mode's check when that check gives no answer
and the settings hold auto mode deny rules: an entry of `autoMode.soft_deny` or
`autoMode.hard_deny` other than `$defaults`, its own rules. It refuses the call instead, and every
later call that needs the check, for the rest of the session. Its `PermissionDenied` hook fires,
but it says why only in free text, so purlis cannot tell a check that gave no answer from one
that judged the call unsafe without reading the harness's words (ADR 0076). A task in auto mode,
in a tab nobody watches, kept working with its file tools alone, and nobody saw why.

| Asking chat          | Settings hold auto mode deny rules | Task    |
|----------------------|------------------------------------|---------|
| default (asks)       | either                             | default |
| auto                 | no                                 | auto    |
| auto                 | yes                                | default |
| bypass (prompts off) | no                                 | auto    |
| bypass (prompts off) | yes                                | default |

- **Read where Claude Code reads them**: the person's `settings.json` in the chat's Claude Code
  config directory, the managed settings it holds from the server (`remote-settings.json`
  there), and the machine's managed policy files (`managed-settings.json` and its
  `managed-settings.d/*.json`). Never a project's, which Claude Code ignores for these rules
  because a repo controls them. A file that is not there, or does not read, holds no rule
  (`harness::claude::settings_keep_auto_from_a_task`).
- **The task's pane says why** from its start, and how to let tasks run in auto again: remove the
  rules from `autoMode`. It asks through purlis's own asks in the window, as any task at the
  default does.
- **Unchanged.** A profile whose command names its own mode, and a grade's chat, start as
  before: no words are added for either, so nothing is said either. Codex and opencode add
  nothing (`HarnessAdapter::auto_kept_from_a_task` is `None` for them).
- **Not in this amendment.** A `PermissionDenied` hook as a named cause shown for a task ("auto
  mode refused a call") is a follow-up.
