# Amended 2026-11-12: the tools whose host reaches the client's flags or a DBI address, and ps-admin, are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6185, from the
reviews of #6109), recorded here as delegated decisions. It follows *innotop, mariadb-report,
the Perl log readers, the server's systemd helpers and PXC's cluster checks are refused a Run
grant* (2026-11-11), which left the `mysqlaccess` and `mytop` leads to #6185. Each was checked
against the tool's own source, not recalled:

- **`mysqlaccess`**, and **`mariadb-access`**, the name Debian's MariaDB client ships it by
  (`debian/mariadb-client.install`; `mysqlaccess` is in `mariadb-client-compat`). Its `Connect`
  appends `--host=` and its `--rhost` (`-H`) value to the client's command line, then hands the
  client that line split on white space through `open3`, and its dump of the grant tables does
  the same to `mariadb-dump` (`scripts/mysqlaccess.sh`, github.com/MariaDB/server, 11.4). A host
  of `x --plugin-dir=<folder> --default-auth=<name>` is then three of the client's arguments,
  and the client loads an authentication plugin from the folder it names: the form the client's
  own row refuses. Its prefix took it to the mysql tools' row, which reads no `-H`.
- **`mytop`**, in the same package: it joins its `--host` value into
  `DBI:MariaDB:database=…;mariadb_read_default_group=mytop;host=<host>` (or `DBI:mysql:…`), and
  its `--db` value into the same address (`scripts/mytop.sh`). DBD::MariaDB's `parse_dsn` and
  DBD::mysql's `_OdbcParse` split the address at each `;` and take every `name=value` part as an
  attribute of the connection (github.com/perl5-dbi/DBD-MariaDB, `lib/DBD/MariaDB.pm`;
  github.com/perl5-dbi/DBD-mysql, `lib/DBD/mysql.pm`), and `mariadb_read_default_file` hands the
  client library a file of settings to read (`mysql_options(…, MYSQL_READ_DEFAULT_FILE, …)`,
  DBD::MariaDB's `dbdimp.c`). Such a file can name a plugin folder and an authentication plugin,
  as `--defaults-extra-file` can. Its name starts with neither `mysql` nor `mariadb`, so no row
  read it.
- **`mysql_convert_table_format` and `mysql_setpermission`**, and `mariadb-convert-table-format`
  and `mariadb-setpermission`, as the client package ships them: each joins its host value
  unquoted into a DBI address the same way (`DBI:MariaDB:$opt_database:${opt_host}` in
  `scripts/mysql_convert_table_format.sh`, `host=$sqlhost` in `scripts/mysql_setpermission.sh`),
  and their `--socket` values too. Their prefix took them to the mysql tools' row, which reads
  none of the client's flags in their commands, because they are not there.
- **Percona Server's `ps-admin`**, which its packages ship in `usr/bin`
  (`build-ps/debian/percona-server-server.install`, `build-ps/percona-server.spec`,
  github.com/percona/percona-server, 8.0; Percona XtraDB Cluster's spec deletes it). Its
  `-c`/`--config-file` is `source`d as a shell file, which alone runs whatever the file says, and
  its `--user` and `--host` values are expanded unquoted into the client's command line
  (`${MYSQL_CLIENT_BIN} … -u ${USER} ${PASSWORD} ${SOCKET} ${HOST} ${PORT}`,
  `scripts/ps-admin.sh`), where they split into flags. Its name starts with neither `mysql` nor
  `mariadb`, so no row read it.

All are refused whole, as the rest of the family's tools that run what they are handed are. A
persona can still declare any of them as a tool.

Found harmless: **`ps_mysqld_helper`**, shipped beside `ps-admin`
(`build-ps/debian/percona-server-server.install`), reads no argument at all
(`scripts/ps_mysqld_helper.sh`): it checks transparent huge pages and jemalloc against the
fixed settings file `/etc/default/mysql` or `/etc/sysconfig/mysql`. It keeps no row and needs
none.
