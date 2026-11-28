### Security

- **A run's notice offers no Allow for more supervisors, init systems and command queues**
  (`tini`, `catatonit`, `daemonize`, `pm2`, `supervisord`, `runit`, `dinit`, `shepherd`, perp's,
  more of s6's, the Procfile runners `foreman`, `honcho` and `overmind`, `pueue`, `tsp`, `nq`,
  `logrotate`, `fcrontab`, `incrontab`, `lefthook`, `husky` and others), **or for more chain
  loaders and command runners** (ucspi-tcp's `tcpserver` and its kin, execline's `execlineb`,
  moreutils' `pee` and `vipe`, `run-mailcap`, `nsjail`, `lxc-attach`, `schroot`, `newgrp`,
  `distrobox`, `toolbox`, `snap`, `guix`, `apptainer`, `runc`, `ctr`, `limactl`, `colima`, `ip`,
  `perf`, `gpg-agent`, `dbus-launch`, `startx`, `keyctl`, `opam`, `ccache`, `sccache`, `distcc`,
  `icecc` and others). Each runs the command, script or service it is handed. A persona can still
  declare any of them as a tool (#7833).
- **A run's notice offers no Allow for more package managers and installers** (`pacman`, `apk`,
  macOS's `installer`, `emerge`, `xbps-install`, `opkg`, `pkg`, `yay`, `paru`, `pkcon`, `nala`,
  `gdebi`, `tasksel`, `fink`, `zef`, `unattended-upgrade`, `checkinstall`, `alien`,
  `installkernel`, dnf5's daemon, the rest of yum-utils, `salt-call`, `puppet`, `rex`,
  `needrestart`, `etckeeper` and others), **or for more build, test and packaging tools**
  (every debhelper `dh_<name>`, `debootstrap`, `mmdebstrap`, `autopkgtest`, `piuparts`, `quilt`,
  `uscan`, `dput`, `osc`, `kiwi-ng`, `koji`, `cpan2dist`, PAR's `pp`, `meson`, `scons`, `ctest`,
  `buck2` and others). Each runs a package's scripts, a build or a project's tests. A persona can
  still declare any of them as a tool (#7833).
- **Programs named after these lose their Allow with them**, for example `pkg-config` (by
  `pkg`), every tool built for an `x86_64-` or `i386-` target, and every `<name>-build`,
  `<name>-daemon` and `<name>-compose`. Giving the harmless ones back is its own change (#7825).
