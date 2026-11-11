### Security

- **A run's notice offers no Allow for `innotop`, `mariadb-report`, the slow log's summary and
  the row finder, the server's systemd helpers or Percona XtraDB Cluster's cluster checks**
  (`mysqlreport`, `mysqldumpslow`, `mariadb-dumpslow`, `mysql_find_rows`, `mariadb-find-rows`,
  `mysqld_bootstrap`, `mysqld_pre_systemd`, `mysql-systemd`, `clustercheck`, `pyclustercheck`).
  innotop loads the Perl plugins of the folder its configuration file names. `mariadb-report`
  hands its `--outfile` to a shell, and the summary and the row finder run a file name ending in
  `|` as a command. The systemd helpers start or initialize the server, and the cluster checks
  have the client, or its library, read a file of settings they are handed. A persona can still
  declare any of them as a tool (#6109).
