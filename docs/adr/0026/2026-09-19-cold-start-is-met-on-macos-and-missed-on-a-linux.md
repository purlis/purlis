# Amendment, 2026-09-19: cold start is met on macOS and missed on a Linux whose portal cannot start

The cold-start row above was measured on macOS only. On Linux the same app took 25 s to reach
its own `setup` (purlis#24), twelve times the limit, and nothing recorded it. This
amendment records the miss on Linux, names its cause, and says which milestone owns it.

**How it was measured.** On GitHub's `ubuntu-24.04` runner, purlis at `f93129b`, a debug
build (`npx tauri build --debug --no-bundle`) under `xvfb-run`, in an empty plane. The runner
was used because it is exactly the kind of machine the issue is about. Each condition was
launched three times, timing to the window's first frame (`CHARTER_BENCH_LOG`) and to the
startup markers (`CHARTER_LAUNCH_LOG`). To name the wait, the app was also attached with `gdb`
during it and the session bus was watched with `dbus-monitor` while it started. A debug build
on a shared runner is not comparable with the release build on the operator's machine, so the
fast rows below show the order of magnitude, not a Linux figure to hold the limit against. The
slow row is two timeouts and does not depend on the build.

| The session bus | First frame, 3 launches | |
| --- | --- | --- |
| A working one (`dbus-run-session`) | 706 ms, 702 ms; 2898 ms for the first launch, the only one cold on disk | met |
| **The runner's own: present, with an activatable desktop portal that cannot start** | **30.5 s, 30.5 s, 26.4 s** | **missed** |
| None reachable (`DBUS_SESSION_BUS_ADDRESS` pointing at nothing) | 482 ms, 463 ms, 526 ms | met |

**The cause is two waits, and purlis makes neither call.**

1. **25.0 s inside `tauri::Builder::build()`.** Tauri's event loop (tao 0.35) creates a
   `GtkApplication` and registers it. At startup GTK 3 looks for a session manager
   (`org.gnome.SessionManager`, then `org.xfce.SessionManager`, both with auto-start off, and
   both answered at once with "no such name"). With neither found, it creates a proxy for
   `org.freedesktop.portal.Desktop` **with auto-start on**. On the runner that name is
   activatable, so the bus starts `xdg-desktop-portal`. The portal then waits for its GTK
   backend, `xdg-desktop-portal-gtk`, which cannot start without a desktop session. The app's
   `StartServiceByName` got no reply until D-Bus's default 25-second timeout ran out: sent at
   `…455.4427`, next traffic at `…480.4486` on the bus monitor. The backtrace during the wait
   is `g_dbus_proxy_new_sync` ← GTK ← `g_application_register` ←
   `tao::…::EventLoop::new_gtk` ← `tauri::Builder::build` ← `charter_app_lib::run`.
2. **5.0 s while the window is created**, before the app's `setup` runs. WebKitGTK asks the
   same portal for `org.freedesktop.appearance` / `color-scheme` and gives up on its own
   5-second timeout. Run 3 above skipped most of this wait, and its first frame was sooner by
   the same amount.

**The issue's suspect, `tauri-plugin-single-instance`, is cleared.** Its
`RequestName("dev.charter.app.SingleInstance")` is on the bus at `…480.4545`, after the wait
and answered at once. **"Without a session D-Bus" is the wrong description:** a machine with
no session bus at all starts in under 0.6 s, because every bus call fails immediately. The miss
needs a session bus that is up **and** a desktop portal that is installed and activatable but
cannot come up. CI runners, containers with desktop packages, and a remote X session without a
desktop all fit that description. A full Linux desktop, where the portal is already running,
would answer straight away. That follows from the trace and was not measured on a desktop.

**Why it is not fixed now.** The wait belongs to GTK and xdg-desktop-portal, reached through
tao, and purlis cannot switch the proxy's auto-start off. The only lever inside the app is
process-wide: give the app no session bus when the portal looks dead. That would also remove
the three things purlis uses the bus for, and one of them is a guarantee the core depends on.
The single-instance name is what makes it safe for `hookwire` to remove a stale hook socket at
startup (purlis `crates/purlis-core/src/hookwire.rs`: "there is no second live app whose
socket this could be"). Notifications and the tray would go as well. Trading a startup delay
for two live apps on one plane makes the product worse, not faster.

**The decision.** The 2 s cold-start limit is **met on macOS** (p50 370 ms, above) and
**missed on a Linux whose session bus has a desktop portal that cannot start** (26–31 s), with
the cause named above and tracked in
[purlis#24](https://github.com/diazoxide/charter/issues/24). It belongs to **M4**,
where Linux becomes a platform purlis ships for, alongside the rest of the Linux desktop
integration. At that point either tao or GTK can register without the portal proxy's
auto-start, or purlis finds a narrower lever than removing the whole bus. The limit itself
stays at 2 s: an operator waiting half a minute for a window is noticing, and the number is
right. M1 is macOS, and nothing in M1 depends on this.

The scenario job keeps running the Linux relaunch test under `dbus-run-session` (purlis#23). That is a Linux desktop with a working bus, and passing it proves nothing about this
miss.

**Addendum, 2026-09-20: the miss is no longer silent.** Everything above stands — the trade is
still the wrong one to take and the fix is still M4's. What has changed is the part purlis
owns. The worst of a 26-second start was that nothing said anything: no window, on some
desktops not even an icon, and the only record of the wait was a marker behind an environment
variable that nobody sets before they have a reason to. purlis#108 makes the wait visible
on the two channels that exist before there is a window. At the 2 s limit the app writes one
line to standard error naming what it is waiting for, which is what a terminal launch, a
`.desktop` file's journal and a CI log have; and when the window does arrive it says on screen
how long the start took and why, which is the only channel an operator who clicked an icon has.
The cause is named on Linux alone and hedged even there, because a slow Linux start can also be
a cold disk.

This changes the caveat, not the row: **cold start is still missed** in this configuration. An
operator who is told why they are waiting is still waiting.

**Addendum, 2026-09-30: met, by starting without the bus when the portal is silent.** The
trade above was refused for one reason: the single-instance name was what made it safe for
`hookwire` to remove a stale hook socket, and a launch with no bus has no name. That reason is
now answered on its own terms, and the trade is taken (purlis#24, program map FR-8).

- **Ask first, for 300 ms.** Before `tauri::Builder::build()`, the app asks the session bus to
  start `org.freedesktop.portal.Desktop` (`StartServiceByName`, the call GTK's proxy makes). A
  running portal gets "already running" at once, and one that nothing can start gets
  `ServiceUnknown` at once. Only a portal the bus is still trying to start gets no answer, and
  that is the 25 s wait. After 300 ms of silence the app `exec`s itself, with the same
  process, binary and arguments, and with `DBUS_SESSION_BUS_ADDRESS` pointing at nothing. It
  says on standard error what the run loses: the tray, notifications, and handing a second
  launch to this one (`portal.rs`).
- **One purlis per user, with or without the bus.** On Linux the app also holds an `flock`
  on `$XDG_RUNTIME_DIR/<identifier>.lock` from the start of `setup`. The single-instance
  plugin has already handed an ordinary second launch over by then, so only a launch the bus
  could not hand over reaches the lock, and that launch is refused. The lock is released at
  `Exit`, before a restart to update starts the new process (`instance.rs`). This makes
  `hookwire`'s promise true on every Linux launch. Before, it was not true on a machine with
  no bus at all.
- **X11 with no session bus is the same case.** With no address and no
  `$XDG_RUNTIME_DIR/bus`, GIO autolaunches a bus of its own through `dbus-launch`, and on that
  bus the portal can be activated again. The app pins the address to nothing there as well.
  There is no bus for it to lose, since the plugins find none by the standard lookup.

Measured in a Linux container (Ubuntu 24.04, debug build, Xvfb). The session bus there had an
activatable portal whose service never takes its name, which is the issue's machine in
miniature. Five launches each, to the first frame:

| Build and case | Launch to first frame |
| --- | --- |
| before (`main` at `dd60d3d`), X without a window manager | no first frame within 30 s |
| after, X without a window manager | p50 775 ms, worst 1244 ms |
| after, i3 under X11, on the same bus | p50 654 ms, worst 877 ms |
| after, i3 under X11, no session bus | p50 385 ms, worst 481 ms |

CI now runs the same three cases on the GitHub runner, whose own bus is the machine the issue
was traced on, and fails when a p50 passes 2 s (`tools/coldstart-linux.sh`, in the `app` job).
GNOME Wayland and KDE are not measured there, because a hosted runner has neither. On a full
desktop the portal is already running and answers at once. That follows from the trace and is
still not measured.

What remains to fix later: a portal that answers the question and then stalls is not caught,
and on a machine where the bus is silent, a second launch is refused instead of being handed
over. `purlisd` (FD-5) removes the second cost, because the hook socket is no longer the
app's.

**Addendum, 2026-09-30 (charter#746), superseding the addendum above's ruling to pin the
address when no bus is named: X11 without a bus starts on the display's bus.** The
addendum above pinned the address to nothing when no bus was named, on the reasoning that GIO's
autolaunched bus would make the portal activatable again. Measured in a container (Ubuntu
24.04, Xvfb, i3, `xdg-desktop-portal` and its GTK backend installed, no systemd user session),
that bus does not hang: `dbus-launch --autolaunch` answered in 11 ms and the portal it activated
came up in 114–202 ms. The pin cost a notification daemon started from the window manager's
config, which lives on that same bus. So the app now does what GIO does, `dbus-launch
--autolaunch=<machine id>`, asks the portal there, and starts again on that bus when it
answers. zbus, which the tray, notifications and single-instance use, never autolaunches, so
starting again with the address named is what lets them reach it. Only when there is no bus to
be had, or its portal is silent too, does the app start without one.

The bound is two budgets, not one: `dbus-launch` gets 300 ms and the portal on its bus another
300 ms, so a launch with no bus named spends at most about 600 ms here before it goes on. Each
wait runs on a thread of its own with a timeout, so neither can hang the launch. When the
portal on the autolaunched bus is silent too, the `dbus-daemon` that `dbus-launch` started stays
running after the app starts without it, as GIO leaves it; it belongs to the X display and ends
with it.

Two more things changed with it. The bus is healthy and only the portal is silent, so
everything the app starts gets the bus the app was given, not the dead address
(`SESSION_BUS_KEPT`): chats through `charter_core::chatenv::inherited`, and the app's own
`git`, `gh` and `purlis` through `forklock::spawn`, which every program the app starts goes
through. The app's own process cannot be given it back without setting its environment, so a
keyring vault's reveal and copy from the window do not work for that run, and the window says
so. And a run without the bus says so in the window, asks the bus again after 5, 15 and 45 s
until it answers, and offers a restart onto it when the portal does. It never restarts on its
own, because a restart ends every chat: a chat that could be mid-turn is named and asked about
first, as Restart to update does.
