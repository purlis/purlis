# Amended 2026-11-27: daemontools, Upstart's initctl, aptitude, run-parts, capsh, cpanm and dnf are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6557, #6559 and
#6564, left over by #6556), recorded here as delegated decisions. It follows *the service
managers, a link that passes through init.d … are refused a Run grant* (2026-11-13), which filed
these leads.

## Not read in source, and why each is refused all the same

The 2026-11-13 amendment checked each tool in its own source and refused none unread. This one
could not: from the sandbox that did it, `gh` reaches only `purlis/purlis` (a chat's forge token
is for its own repo), and a plain fetch of any other host, GitHub's raw files, Launchpad, Debian's
sources and cr.yp.to among them, was refused. No copy of any of these tools is on the machine.

Each tool below is refused whole, on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its
manual describes it, and not on a line of its source. That is the conservative side: a refusal
only takes away an Allow on a run's notice, and a persona can still declare any of them as a tool
in a reviewed line. No tool these issues name is found harmless, since that would need the
reading this could not do; the few left offered beside them (`svc`, `svok`, `svstat`) are left so
on their manuals too, and are in the same follow-up. Reading each in source, to cite its lines or to give an Allow back, is filed as a new
issue (see the pull request).

## daemontools (#6559)

- **`supervise <folder>`** changes to the folder and runs its `./run`, again whenever it exits.
- **`svscan [folder]`** starts a `supervise` on each folder in the one it is named (the current
  one by default), as runit's `runsvdir` and s6's `s6-svscan` do; those were refused on
  2026-11-13.
- **`multilog`**'s `!processor` action has a shell run the processor on each log file it
  finishes.
- Its chain loaders **`setuidgid`**, **`envuidgid`**, **`softlimit`**, **`setlock`**,
  **`pgrphack`** and **`fghack`** each run the program after them, as its `envdir` (refused
  since #5833) and s6's chain loaders (2026-11-13) do.

`svc`, `svok` and `svstat` are left offered: they talk to a `supervise` already running, over its
`supervise/control` and `supervise/status` files, and run nothing themselves.

## aptitude, run-parts, capsh, cpanm and dnf (#6564)

The leads round 2 of #6556's review left unchecked, with the kin each one has:

- **`aptitude`** is built on the same `apt-pkg` as `apt-get`, takes any setting with `-o`, and
  downloads through the method program `Dir::Bin::Methods::<scheme>` names, the reason
  `apt-get` and `apt` were refused on 2026-11-13. A name such as `aptitude-curses` reaches the
  row by its first part.
- **`run-parts <folder>`** runs every program in the folder it is named. That is its purpose.
- **`capsh`** hands the words after `--` to a shell (`capsh -- -c <cmd>`), and `==` runs `capsh`
  again with what follows.
- **`cpanm`** builds the distribution it is named, a folder like `./<folder>` among them, by
  running its `Makefile.PL` or `Build.PL` and then `make`, and so does every install from CPAN.
  **`cpan`** (CPAN.pm's shell) and **`cpm`** install the same way, and **`carton exec`** runs the
  command it is handed. All refused.
- **`dnf`** loads the Python plugins in the folders its `pluginpath` names, which a `--setopt` or
  the `-c` file it is given sets, and **`yum`** does the same. `dnf5` and `dnf-3` reach the row
  by the name without the version.

## Upstart's initctl, and the init daemons (#6557)

The 2026-11-13 amendment filed `initctl` because git.launchpad.net/upstart was out of reach. It
still was, and so were the mirrors tried in its place (Debian's sources, GitHub's copies of
Upstart). As #6557 asked, `initctl` is therefore refused whole on the conservative side:

- **`initctl set-env <VAR>=<value>`** sets a variable in the init daemon's job environment, and
  every job started after it gets the variable. That is the reason
  `systemctl import-environment` was refused.
- **`initctl start <job>`** and **`initctl emit <event>`** start jobs. With `--session` or `--user`
  they talk to a session's own init instead of the system's.
- Upstart's **`start`**, **`stop`**, **`restart`**, **`reload`** and **`status`** are links to
  `initctl`. A program's name is judged again once its links resolve, so they are refused by
  `initctl`'s name and need no entry of their own.

Its kin, refused with it:

- **Upstart's `init`**, also shipped as **`upstart`**, runs as a session init with `--user`, and
  `--confdir <folder>` has it run the jobs of the folder it is named.
- **`init` is just as often a link to `systemd`**, the service manager itself, whose `--user`
  instance starts the units its search path holds. A granted run's home is its own, so no unit a
  chat wrote is known to reach it; it is refused all the same, on the conservative side, as the
  program every refused `systemctl` verb is carried out by. `init` is refused by the name it was
  found by, and `systemd` by its own.
  The first part of a name counts as one of its forms (`emacs-nox`), so every `systemd-<name>`
  program is refused along with it. That costs only an Allow. Giving back the harmless ones
  (`systemd-escape`, `systemd-path`) is a review of its own.

## The review of this amendment's pull request

The first review of #7828 found kin of these tools that were still offered. Each is refused on
what it is for, on the same footing as the rest, and is in the same follow-up:

- **Programs that run the command after them**, as `setuidgid` and `capsh` do: `runuser`,
  `gosu`, `prlimit`, `chrt`, `runcon`, `setarch`, `numactl`, `bwrap`, `proot`, `unbuffer`,
  `sg`, and `run-one` with its `run-this-one` and `keep-one-running`.
- **Loggers whose settings file has a shell run a `!processor`**, as `multilog`'s does: runit's
  `svlogd` and s6's `s6-log`. Neither name reaches a row by its forms.
- **`anacron`**, which runs the jobs of an anacrontab it is named (`-t`), with a spool folder of
  its own (`-S`) and no privilege needed.
- **The Perl installers and runners beside `cpanm`**: `cpanp` (whose `!` evaluates Perl),
  `carmel` and `plenv` (`exec` runs a command), `prove` (runs a distribution's tests, or what
  `--exec` names) and `dzil` (`dzil run` runs a command in the built distribution).
- **The package managers beside `dnf`**: `zypper <name>` runs the `zypper-<name>` its search path
  finds, as `brew <name>` does; `makepkg` runs a `PKGBUILD`; MacPorts' `port` runs a `Portfile`;
  and `microdnf` and `tdnf` take settings the way `dnf` does.

Tests now pin the names the amendment says reach a row by their forms (`aptitude-curses`,
`dnf-3`, `upstart-local-bridge`, `upstart-event-bridge`), and a link named `start` that points
to `initctl`. Each was seen to fail with its entry taken off the row.
