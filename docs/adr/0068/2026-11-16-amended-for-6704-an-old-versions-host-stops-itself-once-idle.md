# Amended for #6704, an old version's host kept by its machine stops itself once idle, 2026-11-16

D-724-3 gives each installed version a service of its own, enabled, so each version's host comes
back after a restart and settles what it ran. D-724-11 has pruning take back a drained version's
service by name before its folders go. But under a supervisor an old version's host never
drains: systemd or launchd starts it at every boot, and it holds its lock for ever with nothing in
it. So on a supervised runner pruning removed nothing, and every version ever installed kept a
host running. This note says who stops an idle old host. §3, the amendments for #724 and #6700
stay as accepted. Each item was **decided in implementation, to be confirmed** (D-6704-1 to
D-6704-6).

1. **The old host stops itself.** A host started by its service manager (`purlis serve
   --as-a-service`, `Serves::AsARunnersService`) and installed as a version stops serving once,
   for `purlis_host::SUPERSEDED_IDLE_FOR` (a minute) without a break, a newer version is
   installed beside it, nothing runs on it (`Hosting::nothing_running`: no program, start,
   browser, save or held project), and no link from its desktop is open. It says so in its
   journal (`purlis_host::SUPERSEDED`) and exits 0
   (`Hosting::until_superseded_and_idle`, `purlis_host::Superseded`). "A newer version installed
   beside it" is a folder in `<config>/server/` read as a later version, ordered as pruning
   orders them, that holds its binary and whose service the manager starts at boot, as item 6
   says (`runnerinstall::prune::newer_beside`). The host reads its
   own disk. It asks no other host and parses nothing printed, so a change of state has a named
   cause: an act of the host (ADR 0076). The clock starts again whenever any of the three fails,
   so a chat or a link holds the host for as long as it lasts.
   *Rejected:* the new version's install asking the old host whether it is idle and stopping its
   unit by name (`systemctl --user stop`, `launchctl bootout`). The question and the stop are two
   acts with a race between them: a chat started in between is ended. Only the host can decide
   "idle" and stop in one step. It also adds a protocol message to every older host, which
   older hosts would not understand. *Rejected:* enabling only the newest version's service at
   boot. After a restart nobody would settle the chats an old version was running (D-724-8),
   because each version's host record is its own (RR-14 D-723-3). *D-6704-1.*
2. **A clean exit is not restarted, and that is all the supervisor needs.** The unit already has
   `Restart=on-failure` and the agent `KeepAlive {SuccessfulExit: false}` (D-724-4, D-724-7), so
   the host that stopped itself stays stopped until the machine restarts. Then it starts again,
   settles what the boot ended, finds the newer version, and stops itself again a minute later.
   Each service stays enabled, as D-724-3 has it, so that settling still happens. *D-6704-2.*
3. **Only a host kept by its machine stops itself.** A bare `purlis serve` is a container's
   command, whose restart policy may start it again on any exit, or a person's at a terminal. A
   host not installed as a version has nothing newer beside it. Both serve on as before.
   *D-6704-3.*
4. **The next prune removes it, as D-724-11 says.** Its lock is free, nothing answers at its
   socket, and its host record is empty, so it has drained. `prune_in` takes back its unit or
   agent by name while holding its lock, then removes its folders. The old binary's
   `serve --remove` is never run. A version with a live chat keeps its host serving, newer
   versions or not, and pruning keeps it (`kept=<version> serving`). The version before the
   running one is never pruned (`may_go`), so its host comes back at each boot and stops itself
   a minute later, until a later install prunes it. *D-6704-4.*
5. **Prune looks for a host in every runtime folder one may listen in** (the issue's "Also";
   `prune::runtimes_to_look_in`). That is the one this process reads (`runnerhost::runtime_dir`:
   `XDG_RUNTIME_DIR`, else the user manager's), and the user manager's own `/run/user/<uid>`
   wherever that folder exists. A version has drained only once its lock is free and nothing
   answers in each of them. Its lock, socket and link socket are removed from each. A host that
   systemd keeps listens in the user manager's folder, whatever folder the environment prune
   runs in names: a `su` shell keeps the other account's, and a job may name one of its own.
   Before this, prune looked only in the folder named. D-724-12 covered an unset variable, not
   one set elsewhere. Looking in a folder where no host can be costs one failed `connect` and
   cannot keep anything wrongly. *Rejected:* looking in the user manager's folder only where a
   runner service is installed. A host started by hand listens in the folder its own shell
   named, so either folder alone could miss a serving host. *D-6704-5.*
6. **Only a newer version whose service starts at boot takes over.** A newer version counts for
   item 1 only while the machine's manager would start its host at the next boot
   (`runnerhost::version_started_at_boot`): under systemd, the link `systemctl --user enable`
   puts in `default.target.wants/` for its unit, which `disable` takes away; under launchd, its
   agent's file. The host reads it from disk by the service's name, as `remove_version` names it,
   and runs no manager. So a person going back to an older version beside a newer one disables
   the newer one's service, or takes it back with that binary's `serve --remove`, and the older
   host serves on. A newer binary put there with no `serve --install` run takes over nothing
   either, so a supervised host never stops itself in favour of a version nothing supervises.
   *Rejected:* a newer binary alone, as item 1 first had it. A person who installs an older
   version beside a newer one would have the host they wanted stop itself, while the newer host
   served for ever. *Rejected:* asking the manager (`systemctl --user is-enabled`, `launchctl
   print-disabled`) at each look. It runs a program every few seconds for the life of the host,
   and the file is what the manager reads anyway. *Rejected:* the service installed last wins,
   by its file's time. A reinstall that changes nothing does not rewrite the file
   (`write_if_changed`), so the time says nothing about what the person last chose.
   Prune's runtime folders (item 5) are compared as the system resolves them, so one folder
   named two ways (`/var/run/user/<uid>`, where `/var/run` links to `/run`) is looked in once:
   twice, its lock's two `flock`s conflict within prune itself, and every version would be kept
   as serving. *D-6704-6.*
