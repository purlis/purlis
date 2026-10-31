# Amended 2026-10-28: a grade can follow each edit, once the person turns it on for a skill

Ticket #3238 (SK-58), from spec #2762 decision 22 ("optionally after each edit, never on a
schedule"). It adds to §4, whose last paragraph left the opt-in to this ticket.

**It is off for every skill until the person turns it on, for that skill, on this machine, from
a grade's plan.** The plan shows the chats a grade runs and the dollar limit it is held to, and
"Grade after each edit", a press of its own beside Grade that starts no grade, keeps exactly
those as what the person accepted, with the profile and model picked, the trigger tests by digest
and the version compared with (SK-57), in this machine's state
(`<state>/skill-grade-after-edit.json`, device-bound, which no chat writes and no push carries).
It is turned on only from a plan whose consent still holds, checked as Grade's press checks it.

**A saved edit grades within what was accepted, and never past it.** Once ten seconds pass with no
other save of the skill, its grade starts on the accepted profile and model, held to the token cap
and dollar limit of a grade pressed by hand. Before it starts, the plan is read again: where a
grade of it would now run more chats than were accepted (a trigger test added, the previous
version's arm arriving) or may spend more (a cap raised, a dearer model), or would run other trigger tests or compare
with another version (a fixture or a commit a chat could make), nothing starts, the
opt-in waits, and the skill's tab says why until the person sees the cost again and turns it on
again. A copy an edit graded already is not graded again, so one save never grades twice and a
burst of saves grades once.

**A second edit while such a grade runs stops it**, what ran is scored and recorded, and the next
grade starts after it. A save that comes while the edit's grade is being read and started stops it
too, and the copy is graded again once the saves go quiet. Only a grade an edit of the same copy
(its level and name) started is stopped. A grade pressed by hand is never stopped by an edit; the
edit's grade waits for it to end, and so does one whose start another grade took the project
from under. Each change of the opt-in's file holds a lock beside it, so turning it off is never
undone by a grade keeping what it graded.

**Only the window's save of the skill's editor counts as an edit.** A save from `purlis skill
edit`, a chat's proposed change, a pull or any other write to the folder grades nothing: a chat
can write files and run the command line, and neither may spend the person's account. Nothing
grades on a timer. Turning it off stops a grade an edit started.

**The record says it**: `after_edit` on the row of a grade an edit started, and the Grades panel
and the scorecard say "after an edit".

## Rejected

- **A key in `charter.local.toml`.** A hand-written key would turn spending on without the person
  ever seeing what it costs; the opt-in holds a consent, so it is written from the plan alone.
- **Per workspace.** The ticket names a skill, and a skill's grade is what its plan shows; a
  workspace-wide switch would accept the cost of skills whose plan was never shown.
- **Watching the folder for changes.** A chat writes files; a watch would let it start grades.
