# Amended 2026-11-13: the service managers, a link that passes through init.d, MySQL Cluster's reporters, brew and openssl's engines and providers are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6320, #6321, #6322,
#6324, #6325 and #6329, from the reviews of #6318), recorded here as delegated decisions. It
follows *the tools whose host reaches the client's flags or a DBI address … are refused a Run
grant* (2026-11-12), which filed these leads. Each was checked against the tool's own source,
not recalled.

## The service managers (#6320)

- **`systemctl`**. `systemctl link <path>` makes a unit of the file it is named, outside the
  unit search path, and `systemctl start` then runs its `ExecStart`; `systemctl edit` runs an
  editor; and `systemctl import-environment <VAR>` copies the value of a variable the run was
  handed into the service manager's own environment (`verb_import_environment`, the
  `SetEnvironment` call), where `show-environment` reads it back and every unit started after
  it is given it (`src/systemctl/systemctl.c`'s verb table and
  `src/systemctl/systemctl-set-environment.c`, github.com/systemd/systemd, v257). `launchctl`
  is refused for the same reason already.
- **`start-stop-daemon`**, dpkg's: `--start` `execv`s the program its `--startas` names, or its
  `--exec` (`utils/start-stop-daemon.c`, dpkg 1.22.11, the `exec` and `startas` options and the
  `execv(startas, argv)` it ends in).

The issue's comment raised six more, each checked too:

- **runit's `sv`** takes a service as a name under `$SVDIR` or as a path (any name starting with
  `/` or `.`, or ending in `/`, is a folder), and runs that folder's `./check` script for
  `check`, `start` and the rest when it waits (`checkscript`, `src/sv.c`,
  github.com/void-linux/runit). **`runsv <folder>`** runs the folder's `./run`
  (`src/runsv.c`). Both refused; `chpst` is refused already.
- **`supervisorctl`**: its `-c` file's `[ctlplugin:<name>]` sections name a
  `supervisor.ctl_factory`, which it imports and calls with the section's other settings
  (`get_plugins` and `import_spec`, `supervisor/options.py`, and `supervisorctl.py`'s
  `Controller`, github.com/Supervisor/supervisor, 4.2.5). Refused.
- **OpenRC's `openrc`**: `--service <name>` resolves the name, which may be an absolute path,
  and `execv`s it (`src/openrc/rc.c`, with `rc_service_resolve` in `src/librc/librc.c`, which
  returns a name starting with `/` as it is, github.com/OpenRC/openrc, 0.55). **`openrc-run`**,
  the interpreter of an OpenRC script, has `openrc-run.sh` source the script it is named
  (`RC_SERVICE="$1"` and `sourcex "$RC_SERVICE"`, `sh/openrc-run.sh.in`). Both refused;
  `rc-service` and `rc-update` are refused already by their forms.
- **`chkconfig`** hands `systemctl` the unit it is named, `systemctl <verb> <name>.service`
  (`forwardSystemd`, `chkconfig.c`, github.com/fedora-sysv/chkconfig, 1.30), and
  **`deb-systemd-helper`** runs `systemctl … preset <unit>` on the unit it is named
  (`script/deb-systemd-helper`, salsa.debian.org/debian/init-system-helpers). `systemctl` is
  refused, so both are.
- **`initctl`**, Upstart's, could not be checked: its source (git.launchpad.net/upstart) was
  out of reach from here. It is filed as an issue of its own rather than left unsaid.

All are refused whole, on `NOT_FOR_A_RUN_GRANT`. A persona can still declare any of them as a
tool.

## An init script reached through a link that points on out of init.d (#6321)

The 2026-11-12 amendment refused a program whose path, as its links resolve, sits in an
`init.d` folder. It judged only that last path, so a link that lands in `init.d` on a name that
is itself a link pointing on out of it was not caught: Ubuntu's Upstart jobs were laid out so,
`/etc/init.d/<job> -> /lib/init/upstart-job`, and the script that runs is named for no service.
Whether any package still ships that layout was not settled, so the check is made whole rather
than argued: following a program's links now reports the folder of every link on the way
(`held_outside`'s walk), and a Run grant refuses the program where the path it was found at,
the path it runs as, or any link on the way sits in a folder named `init.d`. The program is
judged as it was found for the grant, with the folders a chat may write left out, so the links
judged are the ones on the way to what runs. A program whose links never sit in `init.d` is
offered as before. `/etc/rc.d/<name>` (BSD, Slackware) is not taken in: no package is known to
link it onto the search path, and its folder's name is a common one.
