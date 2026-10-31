# Amended 2026-10-31: a grade can be asked for at a terminal

Ticket #3241 (SK-61), on the operator's ruling of 2026-10-31. It adds to §3 and §4, and to ADR 0068
§5's table of what each scope may do.

**`purlis skill grade <name> --run` runs a grade from a terminal, with the plan and the caps of the
window.** It prints the plan, asks y/N (`--yes` is kept for a script that read the plan), follows
the grade's steps, and prints the scorecard. Inside a chat it is refused. With no session host
running it says how to start one and runs nothing, and with no window holding the project it says
to open it there and runs nothing.

**The grade still runs in the window.** A new power on the session protocol (1.63), `ask_grade`, is
granted to `terminal` alone, since it spends the person's account on their own yes. The host relays
it to one `local-ui` link that holds the project on this host, and that link answers with
`answer_grade` and tells steps with `grade_step`, which only the asking link is pushed. No relaying
host carries any of it.

**The yes consents to what a press consents to.** The window answers the plan with the digest a
press of Grade would be checked against, and the start goes through that same check (`plan_holds`).
The terminal prints its own read of the skill and asks only if the window's digest of what it grades
is the same. Outcome evals stay off unless named (`--eval`).

**A grade with nobody following it stops.** When the asking link ends, Ctrl-C and a closed terminal
included, the host pushes `stop_grade` to the window. When the window goes, the terminal is told the
grade ended.

## Rejected

- **Asking over a chat's hook socket.** A sandboxed chat can reach it, and a line without a token
  on it cannot be told from a terminal's.
- **Moving the runner below the window, so `purlis serve` alone can grade.** That is its own step.
- **Leaving a grade running after its terminal goes.** The window shows it, but a closed terminal
  should not keep spending on a yes given there.
