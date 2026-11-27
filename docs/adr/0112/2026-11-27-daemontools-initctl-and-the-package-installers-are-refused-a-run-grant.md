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
reading this could not do. The programs of these packages left offered beside them are left so
on their manuals too, as running nothing they are handed: daemontools' `svc`, `svok` and `svstat`
(and s6's `s6-svc`, `s6-svok` and `s6-svstat`), which talk to a supervisor already running, and
its filters `readproctitle`, `tai64n` and `tai64nlocal`; and Upstart's `shutdown`, `reboot`,
`halt` and `poweroff` (the last three run its fixed `/sbin/shutdown`), `runlevel`, which reads,
and `initctl2dot`, which draws a graph. From debianutils, beside `run-parts`: `which`,
`tempfile`, `savelog`, `ischroot`, `add-shell`, `remove-shell`, `update-shells`, and
`installkernel`, which runs `run-parts` only over the fixed `/etc/kernel` hook folders a chat
cannot write (filed on #7833 all the same). From libcap, beside `capsh`: `getcap`, `setcap`,
`getpcaps` and `captree`. Reading each tool in source, to cite its lines or to give an Allow back,
is filed as #7825.

## daemontools (#6559)

- **`supervise <folder>`** changes to the folder and runs its `./run`, again whenever it exits.
- **`svscan [folder]`** starts a `supervise` on each folder in the one it is named (the current
  one by default), as runit's `runsvdir` and s6's `s6-svscan` do; those were refused on
  2026-11-13.
- **`multilog`**'s `!processor` action, given on its command line, has a shell run the
  processor on each log file it finishes.
- Its chain loaders **`setuidgid`**, **`envuidgid`**, **`softlimit`**, **`setlock`**,
  **`pgrphack`** and **`fghack`** each run the program after them, as its `envdir` (refused
  since #5833) and s6's chain loaders (2026-11-13) do.

`svc`, `svok` and `svstat` are left offered: they talk to a `supervise` already running, over
the files in its `supervise` folder (`control`, `ok`, `status`), and run nothing themselves.
`readproctitle`, `tai64n` and `tai64nlocal` only read and filter.

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
  `initctl`'s name and need no entry of their own. Its `shutdown`, `reboot`, `halt`, `poweroff`,
  `runlevel` and `initctl2dot` stay offered, as running nothing they are handed.

Its kin, refused with it:

- **Upstart's `init`**, also shipped as **`upstart`**, runs as a session init with `--user`, and
  `--confdir <folder>` has it run the jobs of the folder it is named.
- **`init` is just as often a link to `systemd`**, the service manager itself, whose `--user`
  instance starts the units its search path holds. A granted run's home is its own, so no unit a
  chat wrote is known to reach it; it is refused all the same, on the conservative side, as the
  program every refused `systemctl` verb is carried out by. `init` is refused by the name it was
  found by, and `systemd` by its own.
  The first part of a name counts as one of its forms (`emacs-nox`), so every `systemd-<name>`
  program is refused along with it.

## The review of this amendment's pull request

The first review of #7828 found kin of these tools that were still offered. Each is refused on
what it is for, on the same footing as the rest, and is in #7825 too:

- **Programs that run the command after them**, as `setuidgid` and `capsh` do: `runuser`,
  `gosu`, `prlimit`, `chrt`, `runcon`, `setarch`, `numactl`, `bwrap`, `proot`, `unbuffer`,
  `sg`, and `run-one` with its `run-this-one` and `keep-one-running`.
- **Loggers that run a `!processor` they are handed**, as `multilog` does: runit's `svlogd`
  reads it from a log folder's `config` file, and s6's `s6-log` from its command line. Neither
  name reaches a row by its forms.
- **`anacron`**, which runs the jobs of an anacrontab it is named (`-t`), with a spool folder of
  its own (`-S`) and no privilege needed.
- **The Perl installers and runners beside `cpanm`**: `cpanp` (whose `!` evaluates Perl),
  `carmel` and `plenv` (`exec` runs a command), `prove` (runs a distribution's tests, or what
  `--exec` names) and `dzil` (`dzil run` runs a command in the built distribution).
- **The package managers beside `dnf`**: `zypper <name>` runs the `zypper-<name>` its search path
  finds, as `brew <name>` does; `makepkg` runs a `PKGBUILD`; MacPorts' `port` runs a `Portfile`;
  and `microdnf` and `tdnf` take settings the way `dnf` does.

Tests now pin the names that, the first review said, reach a row by their forms (`aptitude-curses`,
`dnf-3`, `upstart-local-bridge`, `upstart-event-bridge`), and a link named `start` that points to
`initctl`. Each was seen to fail with its entry taken off the row.

The second review of #7828 found more, each refused on the same footing:

- **systemd's programs that run a command and are not named `systemd-<name>`**:
  `machinectl shell`, `homectl with <user> -- <cmd>`, `userdbctl ssh-authorized-keys --chain
  <cmd>`, `varlinkctl call exec:<path>` and `portablectl attach --now` of an image a chat wrote.
- **Upstart's `telinit`**, a program of its own and not a link to `initctl`: `telinit -e
  KEY=VALUE` puts the value into the environment of the jobs the runlevel event starts. On
  sysvinit and systemd it is a link to a program that is refused already.
- **s6's programs that run the program after them** and reach no row by a form of their name:
  `s6-svlisten`, `s6-notifyoncheck`, `s6-ftrig-listen`, `s6-ipcserver`, `s6-ipcserverd`,
  `s6-ipcserver-socketbinder`, `s6-ipcclient`, `s6-fdholder-retrieve` and `s6-sudod`.
  `s6-svlisten1` and `s6-ftrig-listen1` reach the first and third without their version.
  `s6-svc`, `s6-svok` and `s6-svstat` run nothing and stay offered.

It also found that 2026-11-13's test of a link through `init.d` named its target `upstart-job`,
which the `upstart` row now refuses by its first part. The test passed whether or not the
`init.d` check ran. Its target is now named for no row, and the test was seen to fail with the
`init.d` check turned off.

The third review of #7828 found the long tail of the same families, each refused on the same
footing:

- **s6's `s6-ipcserver-access`, `s6-connlimit`, `s6-fdholder-getdump` and `s6-permafailon`**,
  which run the program after them.
- **OpenRC's `supervise-daemon`**, which runs the program it is named. It reached daemontools'
  `supervise` row by its first part, but only by chance, since the two are of different
  families, so it has a row of its own.
- **`fakeroot`, `fakechroot` and `eatmydata`**, which run the command after them.
- **Debian's package builders**: `debuild`, `dh`, `pbuilder`, `sbuild` and `gbp` run a package's
  `debian/rules`, as `dpkg-buildpackage` does. The `dh_<name>` helpers have no form that reaches
  `dh` (`name_forms` splits on `-`, not `_`) and are left for #7833.
- **Perl's test and app runners beside `prove` and `dzil`**: `yath`, `cover` (whose `-test` runs
  `make test`), `milla`, `minil` and `plackup` (whose `-e` runs Perl).
- **The RPM and MacPorts kin**: `mock` runs its configuration as Python, `portindex` evaluates
  every `Portfile` as Tcl, and `spectool` and `rpmlint` parse a spec whose `%(…)` a shell runs,
  as `rpmspec` does.
- **systemd's `kernel-install`, `ukify` and `mkosi`**, which run the plugins, tools or build
  scripts of a folder they are named.

The fourth review of #7828 found more of the same families, each refused on the same footing:

- **Programs that run the command after them**: macOS's `taskpolicy`, util-linux's `choom` and
  `uclampset`,
  moreutils' `chronic`, `ifne`, `lckdo` and `zrun`, `cpulimit`, `nocache`, `schedtool`,
  `dbus-run-session`, `xvfb-run`, `faketime`, `torsocks`, `proxychains` (and `proxychains4`, by
  the name without its version),
  `trickle`, and s6's `s6-maximumtime`. Also s6-networking's `s6-tcpserver`, `s6-tcpserver-access`,
  `s6-tcpserver-socketbinder`, `s6-tcpclient`, `s6-tlsserver`, `s6-tlsclient`, `s6-tlsd` and
  `s6-tlsc`, which are twins of the `s6-ipc*` programs.
- **`rotatelogs`**, whose `-p` runs a program on each new log file.
- **The package builders beside `debuild`**: `pdebuild`, `cowbuilder`, `qemubuilder`, `dgit`,
  `fedpkg`, `centpkg`, `rhpkg` and `tito`. Also `debconf`, which runs the command after it, and
  MacPorts' `daemondo`, which runs its `--start-cmd`. `deb-systemd-invoke`, which has
  `systemctl` act on a unit, reaches Python's `invoke` row by its last part, but only by chance,
  so it has a row of its own (round 12), pinned by a unit test.
- **Perl's servers and runners beside `plackup`**: `starman`, `starlet`, `morbo`, `hypnotoad`,
  `mojo` (whose `eval` runs Perl), `start_server` and `mbtiny`.
- **`s6-svlink` and `s6-rc-update`**, which hand a running s6 a service folder or a compiled
  database that a chat may have written.

A unit test now pins `supervise-daemon`'s own row, which the earlier test could not tell from
the `supervise` row.

The fifth and sixth reviews of #7828 found the utilities beside `dnf` and `yum`
(dnf-plugins-core and yum-utils): `yumdownloader`, `repoquery`, `reposync`, `repotrack`,
`repoclosure`, `debuginfo-install`, `needs-restarting`, `package-cleanup`, `repodiff`,
`repomanage`, `repo-graph` and `find-repos-of-install`. They take `-c` and `--setopt` and load
plugins as `dnf` and `yum` do, and no form of their names reaches either row. Only
`find-repos-of-install` was refused before, and that by chance, through its first part, `find`.
All are refused, each by a row of its own. So is daemontools' `svscanboot`, which runs
`svscan`.

Other families the reviews found still offered are outside these issues, among them `tini`,
`pm2`, `supervisord`, ucspi-tcp, execline, `logrotate`, `pacman`, `apk` and macOS's `installer`.
Each round's leads are filed on #7833, its body and its comments.

## What the name forms cost

A name is judged by its forms as well (`name_forms`): its first part, its last part, its stem before
a `.`, itself without a version, and each of those without a GNU `g` or `gnu`. So these entries
refuse more than they name. The extra cost is an Allow and nothing else. Some of the programs caught
rightly stay refused, and some may be harmless. These are examples, not a complete list:

- every `systemd-<name>`. Some run code (`systemd-analyze unit-shell` runs a shell in a unit's
  namespace) and others are thought harmless (`systemd-escape`, `systemd-path`,
  `systemd-creds`).
- every `init-<name>` and `<name>-init`. `cloud-init` stays refused rightly, since it runs the
  `bootcmd` and `runcmd` of the file it is handed.
- every `yum-<name>`, `dnf-<name>` and `zypper-<name>`. `yum-config-manager` stays refused
  rightly, since it loads plugins through `-c` and `--setopt` as `yum` does.
- every `cpan-<name>`. `cpan-upload` is thought to run nothing, though a chat may be handed a
  PAUSE password for it, and `cpan-outdated` to only read.
- every `dh-<name>`, `gbp-<name>`, `mock-<name>` and `debconf-<name>` (`debconf-show`), and
  every `port-<name>` and `<name>-port`, along with the other first and last parts of the names
  refused here.
- ast-grep's `sg`, a different program with the same name as shadow's `sg`. It is still offered
  under the name `ast-grep`. Modular's `mojo` has the same clash with Mojolicious's `mojo`.

The forms also reach rows that were there before: `systemd` reaches `systemd-run`, `systemd-cat`,
`garb-systemd` and the other `systemd-` and `-systemd` rows, and `supervise`, `svscan`,
`setuidgid` and `softlimit` reach their `s6-` twins. Their tests pass through either row, so a
unit test pins each as a row of its own, as it does `supervise-daemon` and
`find-repos-of-install`.

A form that reaches a program of another family only by chance (OpenRC's `supervise-daemon`
meeting daemontools' `supervise`, `find-repos-of-install` meeting `find`, `deb-systemd-invoke`
meeting Python's `invoke`) is not what keeps that program refused: each has a row of its own and
a unit test. A form within one family, such as `yum-config-manager` and the other `yum-<name>`
programs reaching `yum`, or `aptitude-curses` reaching `aptitude`, is what refuses them, and a
give-back under #7825 keeps it for the ones that run code.

Which to give back, and how to do it without giving back the ones that run code, is part of
#7825.
