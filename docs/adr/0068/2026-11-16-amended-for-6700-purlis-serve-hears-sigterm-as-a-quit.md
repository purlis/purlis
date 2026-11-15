# Amended for #6700, `purlis serve` hears SIGTERM as a quit, 2026-11-16

D-724-8 (the amendment for #724, item 8) has systemd end a runner's host's whole control group
at a stop or a shutdown, and the next host settle each chat from the host record as ended by its
crash. The record was right, but a planned restart read as a crash in every chat's history. It
left hearing `SIGTERM` as a quit for later. This note takes that up. §3, the amendment for #724
and the rest of D-724-8 stay as accepted. Each item was **decided in implementation, to be
confirmed** (D-6700-1 to D-6700-4).

1. **`purlis serve` hears `SIGTERM` as a quit.** systemd sends it at a stop, `purlis serve
   --remove` and a shutdown, and launchd sends it at a bootout. The host stops serving and ends
   each program as a quit ends it (`Stopped::Quit`, its run ended `stopped` for `quit`). It
   empties its host record and exits 0. It says `told to stop (SIGTERM)` on standard error, which
   is the unit's journal or the agent's `serve.log` (`purlis_host::TOLD_TO_STOP`). The handler is
   installed before the host says it is ready, so a `SIGTERM` sent the moment it serves is heard.
   This covers bare `purlis serve` as well as `purlis serve --as-a-service`, because a
   container's runtime and a person at a terminal stop it the same way (`Serves::Alone`,
   `Serves::AsARunnersService`, `Serves::hears_sigterm_as_a_quit`). **A host killed outright
   (`SIGKILL`, a power loss) is still settled as a crash** by the next host of its version, as
   D-724-8 says. *Rejected:* a pause that keeps the chats running across the stop. A shutdown
   ends their processes anyway, and only a quit's record is true of them. *D-6700-1.*
2. **The window's own host and the desktop's login item (FD-17) keep the signal's default.** A
   window quits its own host over the session protocol (`quit`), never by a signal, and
   `tools/relaunch.mjs` ends a window's hosts by `SIGTERM` because it wants a sudden end. The
   login item's `SIGTERM` comes from the window's breaker (`Manager::hold`) after the host kept
   failing, or from a sign-out. A crash's record is no less true there, and the window has its
   own way to quit the host. So `Serves::AsALoginItem` is now its own case, apart from a
   runner's service, which takes another host already serving as a success in the same way
   (D-724-6). *D-6700-2.*
3. **The stop reaches the host first.** The unit has `KillMode=mixed`, so `SIGTERM` goes to the
   main process only. Once the host has exited, or `TimeoutStopSec=30` has passed, systemd sends
   `SIGKILL` to whatever is left in the control group. Before this, systemd's default
   (`control-group`) sent `SIGTERM` to every process in the group at once. A chat could then end
   by its own signal before the host marked it as a quit, and an interactive shell ignores
   `SIGTERM` anyway. The LaunchAgent has `ExitTimeOut` 30. launchd already sends `SIGTERM` to the
   job alone at a bootout, and kills its process group only after the job exits. The host waits
   at most `purlis_host::ENDS_WITHIN` (five seconds) for its programs, so thirty seconds leaves
   room for a slow machine (`runnerhost::STOPS_WITHIN_SECONDS`). A unit or agent already
   installed is written again by the next `--install`, which rewrites a unit that differs.
   *D-6700-3.*
4. **What this means for pruning (#6688, #6705) and the login item.** `runnerinstall::prune`
   counts a version as live while its host record names a start of this boot whose process is
   still the one noted. A host that heard `SIGTERM` has emptied its record, so its version is
   drained once its lock is free, as after any quit. A host killed outright leaves starts whose
   processes died with it, or with the boot, and they keep nothing, as before. FD-17's login item
   is unchanged: a `SIGTERM` still leaves its record for the next host, which settles it as a
   crash. CI's `an_ssh_runner_host_outlives_the_sign_out_and_a_restart` now reads the boot that
   the restart ended (every journal on its disk merged, `journalctl -m`, so the fixture keeps its journal there). It checks
   that boot for the quit, and checks that the next boot settles nothing as a crash. *D-6700-4.*
