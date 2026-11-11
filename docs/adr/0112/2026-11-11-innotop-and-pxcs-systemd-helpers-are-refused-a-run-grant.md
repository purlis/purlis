# Amended 2026-11-11: innotop, mariadb-report, the Perl log readers, the server's systemd helpers and PXC's cluster checks are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6109, from the
reviews of #5994), recorded here as delegated decisions. It follows *mydumper, Percona Toolkit,
gh-ost, Galera's helpers and the embedded clients are refused a Run grant* (2026-11-10), which
left these leads to #6109. Each was checked against the tool's own source, not recalled:

- **`innotop`**, which Debian ships in MariaDB's client (`debian/mariadb-client.install`,
  github.com/MariaDB/server). Its `-c` names the configuration file it reads, and without one it
  reads `~/.innotop/innotop.conf`, or `/etc/innotop/innotop.conf` when that is missing. That
  file's `plugin_dir` names a folder (`~/.innotop/plugins` if it names none), and innotop
  `require`s each `.pm` file there that the file's `[plugins]` section lists
  (`load_config_plugins`, in innotop's own source, github.com/innotop/innotop, and in the copy
  Debian ships, `debian/additions/innotop/innotop`). The file's filters and
  columns are Perl it compiles as well. Its name starts with neither `mysql` nor `mariadb`, so no
  row of `command_flags` read it, and a run's notice offered it Allow once.
- **`mariadb-report`**, in the same package, and **`mysqlreport`**, the name its compat links
  give it (`debian/mariadb-client-compat.links`): its `--outfile` value goes unquoted to a shell
  (`` `mv $tmpfile $op{outfile}` ``, `debian/additions/mariadb-report`), and its `--email` pipes
  to `sendmail`. Its `mariadb` prefix took it to the mysql tools' row, which reads neither (the
  review of this amendment's pull request).
- **`mysqldumpslow` and `mysql_find_rows`**, and `mariadb-dumpslow` and `mariadb-find-rows`, by
  which names the same package ships them: both read the files they are named with Perl's `<>`
  (MariaDB's `scripts/mysqldumpslow.sh` and `scripts/mysql_find_rows.sh`), whose open runs a
  name ending in `|` as a shell command. Their prefix took them to the mysql tools' row, which
  reads no operand (the review's third round).
- **The server's systemd helpers.** `mysqld_bootstrap` has the service manager start the
  server's unit with `--wsrep-new-cluster` added to its options
  (`scripts/systemd/mysqld_bootstrap.in`, github.com/percona/percona-xtradb-cluster, a Codership
  script the tree keeps but its build does not install), as `galera_new_cluster` does.
  `mysqld_pre_systemd`, which the server's own build installs (Percona Server's packages ship
  it), runs the server's `--initialize` with what its settings name
  (`scripts/systemd/mysqld_pre_systemd.in`). Percona XtraDB Cluster's packages ship neither, but
  ship **`mysql-systemd`** (`build-ps/rpm/mysql-systemd`), whose `start-pre` runs the server's
  `--initialize` and whose `galera-recovery` has a shell evaluate the server's command line. All
  three reached the mysql tools' row by their `mysql` prefix, and that row reads nothing they do.
- **Percona XtraDB Cluster's health checks.** `clustercheck` hands the client its sixth argument
  (the fourth after a leading `-`) as `--defaults-extra-file`
  (`scripts/clustercheck.sh`), the form the client's own row refuses, and `pyclustercheck`'s
  `--conf` names the file of settings its client library reads. Neither name starts with `mysql`
  or `mariadb`, so no row read them.

All are refused whole, as the rest of the family's server-side tools are. A persona can still
declare any of them as a tool.

The third lead, MySQL Enterprise Backup's `mysqlbackup --exec-when-locked`, was not verified
here: its documentation is published only on dev.mysql.com, which this task's sandbox could not
reach. It is left to #6182, and `mysqlbackup` keeps the mysql tools' row until then. The review
raised two more leads, `mysqlaccess` and `mytop`, whose host values may carry a flag or a setting
into the client. Neither is verified, and both are left to #6185.
