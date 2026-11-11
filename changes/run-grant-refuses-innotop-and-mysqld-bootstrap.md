### Security

- **A run's notice offers no Allow for `innotop`, `mariadb-report`, the server's systemd helpers
  or Percona XtraDB Cluster's cluster checks** (`mysqlreport`, `mysqld_bootstrap`,
  `mysqld_pre_systemd`, `mysql-systemd`, `clustercheck`, `pyclustercheck`). innotop loads the
  Perl plugins of the folder its configuration file names, and `mariadb-report` hands its
  `--outfile` to a shell. The systemd helpers start or initialize the server, and the cluster
  checks run the client with a file of settings they are handed. A persona can still declare
  any of them as a tool (#6109).
