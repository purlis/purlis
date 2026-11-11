# Amended 2026-11-11: innotop and Percona XtraDB Cluster's systemd helpers are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6109, from the
reviews of #5994), recorded here as delegated decisions. It follows *mydumper, Percona Toolkit,
gh-ost, Galera's helpers and the embedded clients are refused a Run grant* (2026-11-10), which
left these leads to #6109. Each was checked against the tool's own source, not recalled:

- **`innotop`**, which Debian ships with MariaDB's client. Its `-c` names the configuration
  file it reads. That file's `plugin_dir` names a folder, and innotop `require`s each `.pm` file
  there that the file's `[plugins]` section lists (`load_config_plugins` in `innotop`,
  github.com/innotop/innotop). Its name starts with neither `mysql` nor `mariadb`, so no row of
  `command_flags` read it, and a run's notice offered it Allow once.
- **Percona XtraDB Cluster's `mysqld_bootstrap`** has the service manager start the server's
  unit with `--wsrep-new-cluster` added to its options (`scripts/systemd/mysqld_bootstrap.in`,
  github.com/percona/percona-xtradb-cluster), as `galera_new_cluster` does.
  **`mysqld_pre_systemd`**, installed beside it, runs the server's `--initialize` with what its
  settings name (`scripts/systemd/mysqld_pre_systemd.in`). Both reached the mysql tools' row by
  their `mysql` prefix, and that row reads nothing either does.

All three are refused whole, as the rest of the family's server-side tools are. A persona can
still declare any of them as a tool.

The third lead, MySQL Enterprise Backup's `mysqlbackup --exec-when-locked`, was not verified
here: its documentation is published only on dev.mysql.com, which this task's sandbox could not
reach. It is left to #6182, and `mysqlbackup` keeps the mysql tools' row until then.
