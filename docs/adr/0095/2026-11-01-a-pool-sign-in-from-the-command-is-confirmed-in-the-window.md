# A pool sign-in from the command is confirmed in the window

Amends ADR 0095, by the person's ruling of 2026-11-01.

`purlis profile sign-in <profile> --pool <pool>` asks the running window before it reaches a
runner or its engine, and goes on only on the person's press there. With no window running, or on
any answer but yes, it starts nothing. This holds for Docker pools and cluster pools alike.

The window asks only of a profile and a pool its own files name, for a command reading the same
config home, one question at a time. It takes the question back when the command ends, and after
two minutes, which counts as a no, and the next question then waits a moment. The dialog shows the
code the command's terminal shows, starts on Cancel, and its yes is never the one Return finds and
can be pressed only once nothing has been typed for a moment. The window keeps a profile on a runner
for the command only after that yes.

Sign in on a profile's line in Settings › Pools is already the person's press, so it asks nothing
more.
