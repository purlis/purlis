# Amended 2026-10-09: a task's reads are not asked about (reported in real use)

A task whose brief ran `purlis persona where` and `purlis dispatch list` stopped on Claude
Code's permission prompt in a tab nobody was at, and waited there on the person until they
happened to open it. A task chat is armed by the same code as every other chat the app starts,
so it carried the same allows as the chat that asked for it. The tool `persona_where` was
allowed (V98a) and its command twin was not. (`purlis dispatch list` was allowed already, and
its tool twin joined it the same day, above.)

Decided under delegation and flagged for the operator: the missing twin joins its pair, on the
rules above (an `allow` only, for Claude Code only, an operator's `ask` or `deny` still wins,
and the list is written out): `Bash(purlis persona where)`, in
`harness::claude::READ_ALLOW`. **The whole line, no wildcard**: the command takes no argument a
chat would pass, so the rule matches no line with anything after it, and no other word of
`purlis persona`. That the harness then asks about a call holding the line beside another
command is carried over from the record command's measurement above, not measured again, and
a redirection on the line is among what is still to be measured (D-1444-14).

Nothing else is added. Every chat the app starts carries the same list, so no task holds an
allow its asking chat lacks, and nothing here writes, starts a chat or reaches a credential.

**What a task stopped on a prompt does now.** Pre-allowing what a task routinely reads removes
the common case, not every prompt. For the rest, the prompt is said where the person is, and
answered only where it always was:

- the asking session's tab wears the hand from the moment the prompt is held on the chat's
  permission hook, before the harness says anything else;
- the session's pane carries a Notice naming the task by its whole path, with **Show the
  task**. While purlis holds the prompt on the hook (up to a minute,
  `harness::hooked::HOOK_TIMEOUT`) it also says what the prompt asks (the harness's own line,
  as the needs-you list already shows it: one line, credential shapes masked, drawn as text).
  Past that hold the harness's own prompt decides in the task's pane and the needs-you row
  goes, and the Notice stays, without the prompt's words, for as long as the board says the
  task is stopped on its prompt: from the harness's `Notification` in the middle of a turn
  until the prompt is answered in the window, a tool of the task's own that began after the
  prompt comes back (it was answered in the task's pane, which no hook says; a tool already
  at work beside the asked call is no answer), its turn ends, its next prompt begins, or it
  ends (#1601). A Deny or an Esc in the task's pane ends the turn with no hook that says so,
  so the Notice then stays until the task's next prompt;
- the asking chat's next turn is told in one line, once for each prompt, that the task waits on
  the person, and that only the person answers it, in the task's own tab.

purlis approves no harness prompt in the person's place: the Notice has no Allow, and the
asking chat is told not to answer it or work around it.

**Codex and opencode.** Codex has no per-command allow (its approval policy is one switch for
the session), so nothing can be pre-allowed for it per command, and the one hook it fires when
it stops for approval is `PermissionRequest`, which decides the prompt and which purlis arms
for no Codex chat: only its own pane shows the prompt. opencode can be given
`permission.bash` patterns for one session, but there the more specific pattern beats an
operator's broader `deny` (above), so nothing is added. Its prompt is heard through the shim's
`permission_prompt` notification, which raises the hand and the asking chat's line, without the
prompt's words.
