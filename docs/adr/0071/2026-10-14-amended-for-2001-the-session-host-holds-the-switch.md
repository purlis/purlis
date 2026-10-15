# Amended for #2001, the session host holds the switch, 2026-10-14

Spec #1974 rules that the kill switch's authority moves into the session host of ADR 0068, that
the command line stops and only the window re-arms, and that the host reads the switch before it
starts anything, including after its own restart (the operator's rulings of 2026-10-14). While
chats run in the host (the start-time switch of ADR 0068's amendment for #1998), the items below
marked so are **decided in implementation, to be confirmed** (D-2001-1, D-2001-4 and D-2001-5,
as in ADR 0068's amendment for #2001):

- **The host is the authority over every program it runs.** It keeps the switch in its own
  folder, `<config>/purlisd/kill-switch`, which every chat's sandbox denies reading and writing,
  and reads it as it starts, before it serves anyone. A state it cannot read holds every agent
  stopped. *D-2001-1, to be confirmed.*
- **The window's switch is the host's client.** Stop all on the title bar, and `purlis stop
  --all` once the window hears it, reach the host as `stop_all`; an app that finds its host
  stopped holds every agent stopped with it, and a host that finds the app stopped is told.
  A host with no window beside it hears `purlis stop --all` itself once #2190 lands, which
  #646 and #655 wait on.
- **Only the window re-arms.** A re-arm goes to the host first, over the window's `local-ui`
  link, and to the two files after; one the host does not take leaves every agent stopped, and
  one the files do not take stops the host again, or, failing that, at the window's next
  look at its host. The host refuses `rearm` from every other scope, in words saying where a
  re-arm is made. *D-2001-5, to be confirmed.*
- **The operator's shell still opens** while stopped (ruled 3): the host lets a shell the window
  opens through. *D-2001-4, to be confirmed.*

With the start-time switch off, nothing here changes. The two files of this record stay how
`purlis stop --all` throws the switch from any terminal, with no app and no host running.
