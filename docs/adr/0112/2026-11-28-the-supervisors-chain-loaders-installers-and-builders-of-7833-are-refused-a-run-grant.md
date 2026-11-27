# Amended 2026-11-28: the supervisors, chain loaders, installers and builders of #7833 are refused a Run grant

Decided under the operator's standing order to take the recommended path (#7833), recorded here
as delegated decisions. It follows *daemontools, Upstart's initctl, aptitude, run-parts, capsh,
cpanm and dnf are refused a Run grant* (2026-11-27), whose review, rounds 2 to 22, filed these
leads on #7833.

## Not read in source, but for one, and why each is refused all the same

As in the amendment before this one, the source of these tools was out of reach: from the
sandbox that did it, `gh` reaches only `purlis/purlis`, and a plain fetch of any other host was
refused. Of the tools refused here, only `pp`, macOS's `installer` and `newgrp`, and Homebrew's
`gpg-agent` are on the machine. `pp` is Perl, and it was read where macOS keeps it; the others
are compiled programs, judged by their manuals.

- **`pp`** is PAR::Packer. In macOS's copy (`/System/Library/Perl/Extras/5.34/PAR/Packer.pm`,
  version 1.050), `-x` is handed to Module::ScanDeps as `execute` (`:760-774`), and ScanDeps runs
  the files it scans when `execute` is true (`Module/ScanDeps.pm:114-118`). So `pp -x` runs the
  script it packs.

Every other tool is refused whole, on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its
manual describes it, and not on a line of its source. That is the conservative side: a refusal
only takes away an Allow on a run's notice, and a persona can still declare any of them as a
tool in a reviewed line. Reading each tool in source, to cite its lines or give an Allow back, is
#7825.

## A refusal of the whole tool, not a row of `command_flags`

Some of these tools run code only through one subcommand or flag: `perf record`, `snap run`,
`pkcon install`, `keyctl session`, `ip netns exec`, `gpg-agent --daemon`, `opam exec`, `quilt
shell`, `transactional-update run`. Each is refused whole all the same. For each, more of its
forms run code than #7833 names (`perf stat`, `trace` and `script`; `snap install`'s hooks;
`pkcon update` and `install-local`; `ip vrf exec` and `-batch`; `gpg-agent`'s
`--pinentry-program`), and without the source no parser could be written as the tool reads its
own words, which is what a `command_flags` row needs. None of them is the usual reason a chat is
handed a key, as `ssh` and `psql` are.

## What is refused

- **Supervisors, init systems, queues and schedulers**: `tini`, `catatonit`, `daemonize` and
  libslack's `daemon` run the program after them; `pm2` and `supervisord` the scripts and
  `[program:x]` commands they are handed; `startpar` the init scripts it is named; runit's
  `runit` and `runsvchdir`; `dinit`, `dinitctl`, GNU Shepherd's `shepherd` and `herd`, and
  OpenRC's old `runscript`; perp's `perpd`, `perpboot` and `tinylog`; s6's `s6-svscanctl`,
  `s6-instance-create`, `s6-sudoc`, `s6-notifywhenup`, `s6-fdholder-daemon` (with
  `s6-fdholderd`, which runs nothing itself and is kept with it on the conservative side) and
  `s6-linux-init-shutdownd`, which runs the `rc.shutdown` of the folder its `-c` names; the
  Procfile runners `overmind`, `foreman`, `honcho`, `hivemind` and `goreman`; the queues
  `pueue`, `pueued`, `tsp` and `nq`; `ubic-admin`, `reflex`, s6-overlay's `with-contenv`,
  cloud-init's `cloud-init-per`, SUSE's `transactional-update`, `fcrontab`, `incrontab`,
  `logrotate`, `lefthook` and `husky`.
- **Chain loaders and command runners**, each of which runs the command, the program or the shell
  after it, or one its settings name: ucspi-tcp's `tcpserver`, `tcpclient`, `recordio` and
  `argv0`; busybox's `tcpsvd`; s6-networking's `s6-tcpserverd`, `s6-ucspitlsc`, `s6-ucspitlsd`,
  `s6-tcpserver4-socketbinder` and `s6-tcpserver6-socketbinder`; execline's `execlineb`,
  `fdmove`, `redirfd`, `background`, `foreground`, `pipeline`, `if`, `ifelse`, `emptyenv` and
  `forx`; perp's `runuid`, `runlimit`, `runlock` and `runenv`; moreutils' `pee`, `mispipe`,
  `vipe` and `vidir`; sensible-utils' `sensible-editor`, `sensible-pager` and
  `sensible-browser`; mime-support's `run-mailcap`, `see`, `edit`, `print` and `compose`
  (Debian's are links to `run-mailcap`, judged by it, and `view` already had a row); `nsjail`,
  `minijail0`, `cgexec`, `lxc-attach`, `lxc-execute`, `schroot`, `dchroot`, `newgrp`, SELinux's
  `newrole` and `sandbox`, `distrobox`, `toolbox`, `snap`, `guix`, `apptainer`, `singularity`,
  `runc`, `crun`, `ctr`, `limactl`, `colima` and iproute2's `ip`; `setarch`'s names `linux32`,
  `linux64`, `i386`, `x86_64` and `uname26`, where they are files of their own; util-linux's
  `coresched`, `enosys`, `setpgid` and `scriptlive`; `catchsegv`, Yocto's `pseudo`,
  `datefudge`, `limitcpu`, `torify`, `tsocks`, `autoexpect`, `cronic`, `sem`, `dtach`,
  `abduco`, `perf`, `gpg-agent`, `dbus-launch`, `startx`, `xinit`, `keyctl`, `opam`,
  `xvfb-maybe`, and run-one's `run-one-constantly`, `run-one-until-success` and
  `run-one-until-failure` where they are files of their own; and the compiler wrappers `ccache`,
  `sccache`, `distcc`, its `pump`, and `icecc`.
- **Package managers and installers**, each of which runs a package's scripts, a build script or
  plugins: `pacman`, `apk`, macOS's `installer`, `emerge`, `ebuild`, `xbps-install`, `opkg`,
  FreeBSD's `pkg`, `slackpkg`, `installpkg`, `yay`, `paru`, `urpmi`, `pkgin`, `pkg_add`,
  `pkcon`, the apt front-ends `nala`, `gdebi`, `wajig`, `aptdcon` and `tasksel`, `fink`, Raku's
  `zef` and `rakubrew`, `unattended-upgrade`, `debi`, `debpkg`, `checkinstall`, `alien` and
  `installkernel`; dnf5's `dnf5daemon-server` and `dnf5daemon-client` and yum-utils' `yumdb`,
  `repo-rss`, `verifytree`, `show-installed` and `show-changed-rco`, which load plugins as `yum`
  does; `salt-call`, `puppet` and `rex`, which run a state or a manifest; `needrestart`, whose
  `-c` file is Perl it evaluates, and `etckeeper`, which runs its hook folders and the `vcs`
  command it is handed. The amendment before this one had called `installkernel` harmless, as
  running `run-parts` over the fixed `/etc/kernel` only; it is refused here on the
  conservative side, because some builds of it hand over to `kernel-install`, which the
  amendment before this one refuses.
- **Build, test and packaging tools**, each of which runs a package's build system, tests,
  maintainer scripts, hooks or the code it is pointed at: Debian's `dh_<name>` helpers, by a
  `dh_` stem beside `git-`, `wsrep_sst_` and `pt-` (`name_forms` splits on `-`, not `_`);
  `debootstrap`, `mmdebstrap`, `autopkgtest`, `piuparts`, `mk-build-deps`, `equivs-build`,
  `quilt`, `debspawn`, `uscan`, `cvs-buildpackage`, `svn-buildpackage`, `dput` and `dupload`;
  `rpkg`, `mockchain`, openSUSE's `osc`, obs-build's `build`, `kiwi` and `kiwi-ng`, and `koji`,
  which loads plugins from the path its settings name; MacPorts' `portmirror`; `cpan2dist`,
  `cpan2deb`, `cpanspec`, `cpan2rpm` and `pinto`; PAR's `pp`; `plx`; the PSGI servers `twiggy`
  and `corona`; and `meson`, `scons`, `ctest` and `buck2`.

Three leads were already refused by a form within their own family, which a give-back under
#7825 keeps: `debconf-apt-progress` by `debconf`, and ubuntu-dev-tools' `pbuilder-dist` and
`cowbuilder-dist` by `pbuilder` and `cowbuilder`. The `run_grants` tests name them, so that a
give-back that drops them fails.

## Left offered

These programs of the same packages run nothing they are handed, by their manuals, and stay
offered: s6's `s6-svperms`, `s6-svdt` and `s6-svwait`, which talk to a supervisor already running;
`s6-ioconnect`, which only copies between descriptors; `s6-rc-compile`, which writes a database
and runs nothing (the refused `s6-rc`, `s6-rc-init` and `s6-rc-update` are what hand one to s6);
and `s6-linux-init-maker`, `s6-linux-init-shutdown`, `-hpr`, `-nuke`, `-umountall` and
`-logouthookd`, which write a folder, ask the running `s6-linux-init-shutdownd` to act, or stop,
unmount and clean up. `copr-cli` submits builds that run on Copr's builders and loads nothing
locally, so it stays offered too. runit's `runit-init` is reached by the new `runit` row, its own
family's, and runs nothing it is handed: a cost of the forms, below.

## A chance reach, by a row of its own

As the amendment before this one ruled, a program that another family's row reaches only by
chance has a row of its own and a unit test. #7833 named ten: REFUSED_NAMES' `rc` reached
`s6-rc`, `update-rc.d`, `invoke-rc.d`, `rc-service` and `rc-status`; the `service` row reached
daemontools' `update-service`; `envdir`, `setsid` and `svscanboot` reached `s6-envdir`,
`s6-setsid` and `s6-svscanboot`; and the `node` prefix reached `nodemon`. Each now has a row of
its own. The new rows also reach rows that were there before: `tcpserver` and `tcpclient` reach
`s6-tcpserver` and `s6-tcpclient`, and `daemon` reaches `start-stop-daemon` and
`supervise-daemon`. A unit test pins all four. Among the new rows, `build` reaches `equivs-build`,
`daemon` reaches `s6-fdholder-daemon`, and `debi` reaches `gdebi` without its GNU `g`; each has a
row of its own as well. `kiwi` reaches `kiwi-ng` within its own family.

## What the name forms cost

These rows refuse more than they name, through their first and last parts and the rest of
`name_forms`. The extra cost is an Allow and nothing else. Short and common words cost the most.
These are examples, not a complete list:

- `pkg` reaches `pkg-config`, which runs nothing it is handed.
- `x86_64` and `i386` reach every tool built for a target named that way, by its first part
  (`x86_64-linux-gnu-objdump`, `i386-elf-ld`), and so do `linux32` and `linux64` for their own
  names.
- `build`, `daemon`, `edit`, `print`, `see`, `compose`, `if`, `pipeline`, `background`,
  `foreground`, `sandbox`, `toolbox`, `installer`, `snap`, `perf`, `ip`, `sem`, `pp`, `rex`,
  `herd` and `pump` reach every `<word>-<name>` and `<name>-<word>`. For example,
  `podman-compose` and `docker-compose` reach `compose`, and `docker-compose` is refused by its
  own family's `docker` row as well.
- `runit` reaches `runit-init`, and `kiwi` reaches every `kiwi-<name>`.
- the `dh_` stem refuses every debhelper helper, and some of them run nothing
  (`dh_installdocs`, `dh_link`).

Which of these to give back, and how to do it without giving back the ones that run code, is
part of #7825.

## Other families

The leads found beside #7833's own list, outside the families it names, are filed as #7964 rather
than added here: execline's other programs, terminals and multiplexers that run a command, remote
and VM runners, cron daemons, interpreters and runtimes, and tracers.
