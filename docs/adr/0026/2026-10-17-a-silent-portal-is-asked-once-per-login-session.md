# Amendment, 2026-10-17: a silent portal is asked once per login session

Operator ruling on purlis#2432, amending the mechanism of the 2026-09-30 addendum (FR-8).

**What it cost.** On a machine whose session bus can activate a desktop portal that cannot
start, every launch gave the portal its 300 ms budget (`portal::BUDGET`) and then started again
in place without the bus. On the GitHub runner that is 371–374 ms before the app's clock, against
63–65 ms with no bus at all (run 39014455549). The machines that have this portal (CI runners,
containers with desktop packages, a remote X session, a tiling window manager with no desktop
session) have it at every launch, so the cost was paid every time, not once.

**The ruling.** The first launch in a login session asks the portal and waits the 300 ms budget,
as before. When the portal does not answer, later launches in the same login session do not ask
it, and start without the bus at once. The memory resets at the next login, so a portal that
starts working is picked up then.

**How.** The silence is written to `$XDG_RUNTIME_DIR/<identifier>.portal-silent`. That directory
belongs to one login (systemd-logind creates it at login, mode 0700, on tmpfs, and removes it
when the user's last session ends). With lingering on (`loginctl enable-linger`) it outlives a
logout, and so does the memory, until the bus answers or the machine restarts. The file holds
only the address the silence was heard on, empty for the bus the standard lookup finds. A launch
given a different address asks again. The bus that chats are given (`SESSION_BUS_KEPT`) is still
the launch's own address and is never read from the file. Three more rules:

- When the kept bus answers after the launch (`listen_again`), the file is removed, so the next
  launch asks again without waiting for a new login. The window still offers the restart, as
  before.
- With no `$XDG_RUNTIME_DIR`, nothing is remembered and every launch asks, as before.
- Why the launch is without the bus is now said once, by the launch that starts again, at the
  same time the window is told. The launch before the restart says nothing. Its line used to go
  through the app's log, and the log holds its first line until the redaction rules are built.
  That takes about 300 ms in CI's debug build. Behind the 300 ms ask it cost almost nothing, but
  a launch that skips the ask would have paid it instead.
- A silence on the X display's bus, which is found through `dbus-launch` at each launch
  (2026-09-30, charter#746), is not remembered. Remembering it would mean reading the bus that
  chats are given back from a file. That case still pays its budget at every launch.

**The risk the ruling accepts.** A portal that was only slow at the first launch after login (a
cold disk) is skipped for the rest of the session if it never answers the asks after that launch
(5, 15 and 45 s). The run loses the tray, notifications and the single-instance hand-over until the
operator restarts onto the bus from the window, or logs in again. A portal that does answer one
of those asks clears the memory, which limits the cost to the one run.
