### Security

- **A run's notice offers no Allow for `innotop`, `mysqld_bootstrap` or `mysqld_pre_systemd`.**
  innotop loads the Perl plugins of the folder its configuration file names. Percona XtraDB
  Cluster's `mysqld_bootstrap` has the service manager start the server, and
  `mysqld_pre_systemd` runs the server to initialize it. A persona can still declare any of them
  as a tool (#6109).
