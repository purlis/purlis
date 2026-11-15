# Amended for #724, a runner's host kept running by its machine, 2026-11-15

§3 gives the session host's supervision: the window, or a service manager once something keeps
the host running (FD-17, amended for #655 the same day). A runner has no window. `purlis serve`
on one already lived until it was ended (`purlis_host::lifetime`), but nothing kept it: started
over SSH it went with the sign-in that started it, and with every restart of the machine. RR-15
hands it to the machine's own service manager. §3 and the amendments above stay as accepted, and
this note adds to them. Each item was **decided in implementation, to be confirmed** (D-724-1 to
D-724-14).

1. **The machine's service manager keeps a runner's host, never purlis.** On Linux with a
   systemd user manager, a user unit, with **linger**, without which the user manager and every
   unit in it stop at the last sign-out and start only at the next sign-in. On macOS, a
   LaunchAgent. In a container, or on Linux with no user manager, nothing: `purlis serve
   --foreground` is the container's command and its restart policy keeps it up. A container that
   boots systemd has a user manager like any machine, and is treated as one.
   (`purlis_core::runnerhost`.) *Rejected:* a purlis watchdog, or a double fork and `setsid`.
   Neither survives a restart, and a watchdog is the custom tooling a standard one replaces.
   *Rejected:* a system unit or a LaunchDaemon. They need an administrator for every install,
   and run the host outside the account's own session. *D-724-1.*
2. **`purlis serve --install`, `--remove` and `--foreground`, on the runner's own binary.**
   `--install` writes the unit or agent, turns linger on, and starts the host; `--remove` stops
   it, with its chats, and removes it; `--foreground` is `purlis serve` as it always was. Bare
   `purlis serve` stays the foreground host, since a Docker pool's runner and a cluster's run it
   as their command. The desktop's install (RR-1, #720) runs `<config>/server/<ver>/purlis serve
   --install` over the pinned route after placing a version. *Rejected:* bare `purlis serve`
   installing itself where it can. It would change every runner image's command under it.
   *D-724-2.*
3. **One service per installed version**: `purlis-runner-v<hash>.service` and
   `dev.purlis.runner.v<hash>`, `<hash>` the one the version's socket carries
   (`hostsocket::version_key`, RR-14 D-723-17). Installing a new version beside an old one never
   rewrites, restarts or stops the old one's, so its chats drain where they run. Each stays
   enabled, so after a restart each version's host comes back and settles what it ran. A
   `purlis` not installed as a version has the one `purlis-runner.service`. None is ever the
   desktop's `purlis-host.service` or `dev.purlis.host`. *Rejected:* one unit rewritten for the
   newest version, which ends the old version's chats as it restarts. *Rejected:* a template unit
   `purlis-runner@<ver>.service`: a version may hold `+`, which a unit's instance may not, and
   the config folder it would name differs per machine. Removing a drained version's service is
   left with RR-14's pruning of its folder. *D-724-3.*
4. **The unit**: `WantedBy=default.target`, which a user manager reaches with no desktop, and
   none of FD-17's `graphical-session.target`; `Restart=on-failure`, `RestartSec=5`, and FD-17's
   breaker, `StartLimitBurst=3` within `StartLimitIntervalSec=300`. `ExecStart` is the version's
   `purlis` with `serve --as-a-service`, each word quoted as FD-17 quotes them. Installing again
   writes it only where it differs, and `enable --now` leaves a running host running. *D-724-4.*
5. **Linger is turned on by the account itself, only once its unit is enabled, and turned off
   again only where purlis turned it on** (amended by the review of #6699). `loginctl
   enable-linger` runs after `enable --now` succeeds, so a failed install leaves no linger
   behind. logind allows it to an account signed in over SSH where its policy does (polkit's
   `set-self-linger`, which Ubuntu and Fedora allow an active session). `--install` says plainly
   which it was: *purlis turned on linger for <user> … (undo: loginctl disable-linger <user>)*,
   or *Linger was already on for <user>*. Where purlis turned it on, it notes so in
   `<config>/purlisd/linger-turned-on-by-purlis`, and the `--remove` that takes back the
   account's last runner service runs `loginctl disable-linger` and deletes the note; a linger
   the account had before purlis is never turned off. Where the machine refuses, the host is
   installed all the same (it runs while the account is signed in, and starts at its next
   sign-in), and purlis names the one command to ask an administrator for, `sudo loginctl
   enable-linger <user>`. Linger is read from the file logind keeps for it,
   `/var/lib/systemd/linger/<user>`. *D-724-5.*
6. **A host the manager starts takes another of its version already serving as a success**
   (`--as-a-service`, `Serves::AsALoginItem`, FD-17's rule for its login item). So a host started
   by hand, or by RR-14's test, never trips the unit's breaker by exiting 3 three times.
   *D-724-6.*
7. **On macOS the agent is loaded now, and after a restart only at a sign-in at the Mac's
   screen** (amended by the review of #6699). It is bootstrapped into `gui/<uid>` where the
   account is signed in at the screen and `user/<uid>` otherwise, and restarted after an exit
   that was not a success. After a restart launchd loads an agent in `~/Library/LaunchAgents`
   only for a GUI sign-in, so `--install` and doctor say exactly that, and give no advice about
   automatic login. Starting before anyone signs in needs a LaunchDaemon, which only an
   administrator may install. What the host says goes to `serve.log` in its own folder. CI
   cannot run this: it is left for an operator's check on a real Mac (#6701). *D-724-7.*
8. **A stop or a shutdown ends the host and its chats, and the next host keeps them as
   records.** systemd ends the unit's whole control group (its default `KillMode`), so a restart
   of the machine ends every chat with it; the host noted each in its host record (#2007), and
   the version's host that starts after the restart settles each as ended by its crash before it
   serves anyone, which a harness chat's run keeps in the event log
   (`left_by_its_last_host`). That is how chats "come back after the reboot as records".
   *Left:* hearing `SIGTERM` as a quit, so a shutdown's end is kept as a stop rather than a
   crash. *D-724-8.*
9. **Doctor checks it on a runner only**: three rows, `runner host service` (installed and
   enabled; in a container *unknown*, a warning, since purlis cannot see the container's restart
   policy from inside it), `runner host after sign-out` (linger, on Linux) and `runner host`
   (something answers at the version's socket, found in the user manager's runtime folder with
   the config home install pinned). A runner is a `purlis` installed as a version, or one whose
   service is installed. A desktop's doctor, and the preflight at every chat's start, have none
   of them, so no recorded answer moves. *D-724-9.*
10. **CI proves it on a machine that boots systemd** (`tests/ssh-runner/systemd.Dockerfile`):
    privileged, with a cgroup namespace of its own and `/run` on tmpfs, sshd with `UsePAM` so a
    sign-in is a logind session, and polkitd so the account may turn its own linger on. Every
    SSH connection of the account is cut and logind says it lingers with no session, and the
    host and both chats still run; `docker restart` is the restart (systemd shuts down on
    `SIGRTMIN+3` and boots again), and the host comes back with nothing run over SSH.
    *Rejected:* simulating the restart by running the unit's command in a fresh container, which
    the ticket allowed where systemd could not run: it would prove the command, not the unit,
    linger or the boot. *D-724-10.*
11. **Pruning a drained version takes back its service first, by name** (ruled in the review of
    #6702; `runnerinstall::prune_in`, `runnerhost::remove_version`). It happens while that
    version's lock is held, before any of its folders go, so the manager can never start a host
    of it again from a binary that is gone. The unit or agent is named from the version alone
    (`hostsocket::version_key`). The old binary's `serve --remove` is never run: an older one may
    not know the flag, and it is about to be deleted. Where the service could not be removed, the
    version is kept whole and said as one that could not be removed. Under a supervisor an old
    version's host comes back at every boot and holds its lock, so it is never found drained;
    who stops an idle old host is #6704. *D-724-11.*
12. **Every runner purlis that talks to a supervised host reads the runtime folder it uses**
    (amended by the review of #6699). Over SSH, pam_systemd gives
    `XDG_RUNTIME_DIR=/run/user/<uid>`, the same as the user manager gives a unit. Where the
    variable is unset (a `sudo -u` shell, a job started with an emptied environment), `--install`,
    `--remove`, doctor and prune would have looked in `/tmp/purlis-<uid>` and found no user
    manager: the keeper read as none, `--remove` and prune's removal by name returned as done
    with the unit still enabled, and prune could take a serving version for a drained one. Each
    now reads `/run/user/<uid>` where that is there (`runnerhost::runtime_dir`), and gives it to
    `systemctl --user` as `XDG_RUNTIME_DIR`. And where no manager is reachable but the unit or
    agent file is there, removal fails rather than passing for done, so prune keeps that
    version. *D-724-12.*
13. **The config home install resolved is pinned in the service** (review of #6699): the unit's
    `Environment="PURLIS_CONFIG_HOME=<config home>"` and the agent's `EnvironmentVariables`. The
    user manager and launchd start a unit with their own environment, not the SSH session's, so
    a runner whose config home is set by `$PURLIS_CONFIG_HOME` or `$XDG_CONFIG_HOME` would
    otherwise get a host reading another store, not finding itself installed as its version, and
    listening at another socket. *D-724-13.*
14. **`--install` checks that the unit stayed running** (review of #6699). After `enable --now`
    and linger it waits two seconds and asks `systemctl --user is-active`. Active: systemd keeps
    the host. Not active, but a host answers at the version's socket: one started by hand holds
    the place, so the manager's host found it serving and stopped (D-724-6). `--install` says so
    and does not claim systemd keeps it: systemd starts the next host once that one stops. Not
    active and nothing answering: a failure, with the two commands that say why
    (`systemctl --user status`, `journalctl --user -u`). *D-724-14.*
