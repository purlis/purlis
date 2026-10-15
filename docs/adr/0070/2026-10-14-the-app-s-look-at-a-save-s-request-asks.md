# Amended 2026-10-14 (the operator's ruling): the app's look at a save's request asks as the window

Section 4 and ruling 5 keep the person's purlis sign-in for a call a person in the window is
waiting on, and a trigger never takes it. One job is ruled an exception: **the app's own look
at a `pr-merge` save's open request** (ADR 0051). It asks as a save pressed in the window
would, with nobody waiting on it (`planegit::look_caller`, the one place the look's caller is
chosen): the person's sign-in where the account has one and the CLI transport is not chosen
for it, and the forge's own CLI otherwise. The look is admitted as background work.

Two things ask as the window with nobody waiting on them: this look, and the refresh of a
workspace's request states when the person brings it forward (`panels.rs`, from before this
amendment). A test names both and fails on a third. Every other call that runs with nobody at
the keyboard is a trigger's: an automatic save's own calls, and a look made anywhere but the
app.

With no sign-in at all, the look merges nothing: the request waits, says a sign-in is needed,
and the window offers one (`forge::signin::ready` is asked when the look got no answer).

The seam gains one read, `Requests::annotations`: what one failed check found (ADR 0051,
amended the same day).
