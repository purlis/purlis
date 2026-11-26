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
  job started after it, and `initctl start` starts a job. `init` and `systemd` run the jobs or
  units of a folder they are named, or of the run's own home. A persona can still declare any of
  them as a tool (#6557).
