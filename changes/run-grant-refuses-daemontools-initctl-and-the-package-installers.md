### Security

- **A run's notice offers no Allow for daemontools' `svscan`, `supervise` or `multilog`, or for
  its `setuidgid`, `envuidgid`, `softlimit`, `setlock`, `pgrphack` and `fghack`**. `supervise`
  runs a folder's `run` script and `svscan` starts one on each folder it finds, `multilog` has
  a shell run the processor it is named, and the rest run the program after them. A persona can
  still declare any of them as a tool (#6559).
- **A run's notice offers no Allow for `aptitude`, `run-parts`, `capsh`, `cpanm`, `cpan`, `cpm`,
  `carton`, `dnf` or `yum`**. `aptitude` runs the download program a setting names, as
  `apt-get` does; `run-parts` runs every program in a folder; `capsh` hands a shell the command
  after `--`; the Perl installers run a package's build script, and `carton exec` the command
  it is handed; and `dnf` and `yum` load plugins from a folder a setting names. A persona can
  still declare any of them as a tool (#6564).
- **A run's notice offers no Allow for Upstart's `initctl`, or for `init`, `upstart` or
  `systemd`**. `initctl set-env` puts a value the run was handed into the environment of every
  job started after it, and `initctl start` starts a job. Upstart's `init` runs the jobs of a
  folder it is named, and `init` is as often `systemd`, the service manager. Every
  `systemd-<name>` program, `systemd-creds` and `systemd-analyze` among them, and every
  `init-<name>` loses its Allow with them. A persona can still declare any of them as a tool
  (#6557).
- **A run's notice offers no Allow for the programs beside these that run the command after
  them** (`runuser`, `gosu`, `prlimit`, `chrt`, `runcon`, `setarch`, `numactl`, `bwrap`, `proot`,
  `unbuffer`, `sg`, `run-one`, `run-this-one`, `keep-one-running`), **for the loggers `svlogd`
  and `s6-log`, for `anacron`, or for `cpanp`, `carmel`, `plenv`, `prove`, `dzil`, `zypper`,
  `makepkg`, `port`, `microdnf` and `tdnf`**, each of which runs a command, a script or a plugin
  it is handed. A persona can still declare any of them as a tool (#6559, #6564).
