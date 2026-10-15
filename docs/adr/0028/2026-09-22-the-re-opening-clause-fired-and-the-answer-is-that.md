# Amendment, 2026-09-22: the re-opening clause fired, and the answer is that the ground has not moved

This record accepts the `stat`-then-open race on one condition, and states the condition rather
than hiding it:

> A confined agent would be in the model, and purlis does not have one. The honest version of the
> third answer is conditional: the race is out of scope *because* nothing in purlis makes an agent
> less privileged than purlis itself. The moment something does … this decision is the first thing
> that has to be re-opened.

**[ADR 0041](../0041-a-plugin-is-a-subprocess-or-charter-has-no-plugins.md) is that moment arriving,
and its own gate item 7 requires this re-opening in the same change as the ruling that triggered
it.** The whole proposition of a plugin boundary is that a plugin is *less* privileged than
purlis. If that were true, the accepted race would stop being an accident nobody can exploit
without already having the operator's shell: it would become a way for a confined principal to
have purlis write purlis's own bytes to a path the plugin chose.

**It is not true, and that is the answer.** The operator ruled on 2026-09-22 to ship the plugin
runtime with **no OS sandbox**. A subprocess runs as the same user, with the same filesystem and
the same ability to `exec`; a plugin can write the plane directly without asking purlis for
anything. So a plugin is not a principal confined below purlis, nothing in purlis makes one, and
the condition this record's third answer rests on still holds. The race stays out of scope, on the
grounds it was always out of scope on, and not because nobody looked.

**What has changed is that the clause is now load-bearing rather than hypothetical.** It has one
named trigger with a date on it, and the trigger is a sandbox rather than a plugin:

- **The day a sandbox lands** — Seatbelt on macOS, seccomp or Landlock on Linux, AppContainer on
  Windows, any of them, for any subprocess purlis starts — the condition is false and this record
  is re-opened for real. Not amended again: re-opened, because the answer changes rather than the
  wording.
- **A plugin runtime without one does not re-open it**, and this amendment is written so that the
  next reader does not have to re-derive that from ADR 0041's honesty paragraph.
- **The `openat` rewrite is still the fix when it comes**, on the terms the section above sets: the
  whole core at once, with the external review decision 16 requires, and not one call site at a
  time.
