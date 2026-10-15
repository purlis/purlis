# Amended for #2001, the kill switch's authority is the session host's, 2026-10-14

Spec #1974 rules that the kill switch's authority moves into the host, that the command line
stops and only the window re-arms, and that the host reads the switch before starting anything,
including after its own restart (the operator's rulings of 2026-10-14; ADR 0071 is amended to
match). §1, §4, §5 and the amendments before this one are left as accepted, and this note adds to them.
Items 1 and 2 are the spec's protocol additions; items 3 to 5 are **decided in implementation,
to be confirmed** (D-2001-1, D-2001-4 and D-2001-5).

1. **Session protocol 1.5, a minor.** It adds, each recorded in the protocol's fixture:
   `stop_all` (end every program the host runs, in every project, and start none until
   re-armed), `rearm` and `kill_switch` (the switch's state), each answered `kill_switch` with
   whether every agent is stopped and, when the host has something to say of it, `why`; and the
   refusal code `stopped`, which a start gets while every agent is stopped, and a re-arm the
   host could not keep.
2. **FD-27's table gains three rows.** `stop_all` is a stop, so every scope that may `stop` may
   send it; `kill_switch` is a read like `list`; `rearm` is `local-ui`'s alone, and a `rearm`
   from any other scope is refused `not_allowed` in words that say a re-arm is made in purlis's
   window.
3. **The host keeps the switch in its own folder**, `<config>/purlisd/kill-switch`, the folder
   every chat's sandbox denies reading and writing: the word `stopped` or `armed`, `0600`,
   read back only as a regular file of this user's that nobody else may read or write. No file
   is a host never stopped; any other file, or one that cannot be read, holds every agent
   stopped, and the host says why in its log and in each refusal. *D-2001-1, to be confirmed.*
4. **While stopped, the host starts nothing but a shell the window opens** (ADR 0071, ruled
   3): the window, which alone may re-arm, says which shell is the operator's, and refuses
   every other before it asks. A start under way as the switch is thrown is stopped as soon as
   it is listed. *D-2001-4, to be confirmed.*
5. **A re-arm is the host's first, and the files' after.** One the host does not take leaves
   every agent stopped; one the files then do not take stops the host again, and a stop the
   host does not take is sent again at the window's next look at the switch, so the two never
   stay apart towards running. A host with no window beside it hears `purlis stop --all` itself
   once #2190 lands. *D-2001-5, to be confirmed.*
