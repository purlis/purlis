# Amended 2026-11-12: the tools whose host reaches the client's flags or a DBI address, ps-admin, the test runners, RocksDB's hot backup and an init script are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6185 and #6187, from
the reviews of #6109), recorded here as delegated decisions. It follows *innotop, mariadb-report,
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
  DBD::mysql's `_OdbcParse` split the address at each `:` or `;` and take every `name=value` part as an
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

The same reviews raised four more tools (#6187), each checked against its source too:

- **`mysqlxtest`**, the X protocol's test client, which MySQL's test package and Percona XtraDB
  Cluster's test package ship in the programs' folder (`build-ps/percona-xtradb-cluster.spec`,
  github.com/percona/percona-xtradb-cluster, 8.0). It reads a script from its `-f`, or from its
  input without one, whose `-->system <CMD>` and `-->system_in_background <CMD>` run a command
  (`mysql-test/suite/x/r/mysqlxtest_help.result`, github.com/mysql/mysql-server, 8.0), as
  `mysqltest`'s scripts do, which is refused for that already.
- **`mysqltest_safe_process`**, shipped beside it: it takes `[options] -- progname arg1 … argn`
  and `execvp`s the program it is given (`mysql-test/lib/My/SafeProcess/safe_process.cc`). The
  forms a name is judged by do not split at `_`, so it never came to the refused `mysqltest`.
  Both reached the mysql tools' row by their prefix, which reads neither a script nor `--`.
- **`myrocks_hotbackup`**, in Debian's `mariadb-plugin-rocksdb`
  (`debian/mariadb-plugin-rocksdb.install`, MariaDB 11.4; installed by
  `storage/rocksdb/CMakeLists.txt`). In `storage/rocksdb/myrocks_hotbackup.py`, its
  `--extra_wdt_sender_options` is formatted into a command `os.system` runs, its `--destination`
  becomes ssh's first argument through `Popen(remote_cmd.split())`, so `-oProxyCommand=…` gets
  in, and its stream writer runs `os.system(stream_cmd + " " + file_name)` on the names it finds
  under `--datadir`. Its name starts with neither `mysql` nor `mariadb`, so no row read it.
- **`rcmysql`**: Percona XtraDB Cluster's packages built without systemd link
  `/usr/sbin/rcmysql` to `/etc/init.d/mysql`, the server's `mysql.server` script
  (`ln -s %{_sysconfdir}/init.d/mysql $RBR%{_sbindir}/rcmysql`, under `%if 0%{?systemd} == 0`).
  A Run grant judges the program as its links resolve, and the script's file name, `mysql`, is
  the client's, so `rcmysql start` reached the client's row and started the server. `rcmysql`
  is refused by name, and, as the issue asked of a link whose target is an init script, a Run
  grant now also refuses any program whose path, as its links resolve, sits in a folder named
  `init.d` (`/etc/init.d`, `/etc/rc.d/init.d`), whatever its name or its link's: an init script
  starts and stops the service it is named for, and is named as that service is, so its name
  says nothing about what it runs. A declared tool is not judged by its folder; a person who
  declares one chose it.

The review of this amendment's pull request found that MySQL's own SLES packages link
`/usr/sbin/rcmysql` to **`service`** instead (`ln -sf %{_sbindir}/service …/rcmysql`,
`packaging/rpm-sles/mysql.spec.in`, mysql-server 8.0), which a Run grant judges by its target's
name. `service` runs `/etc/init.d/<name>` with the action it is given, or has `systemctl` act on
the unit (`script/service`, Debian's init-system-helpers), so it is refused too, and with it every
`rc*` link to it. `systemctl` itself, `start-stop-daemon`, an init script reached through a link
that points on out of `init.d`, and RocksDB's `ldb` and `sst_dump` were raised but are not
verified here; they are filed as #6320, #6321 and #6322.

The review's second round found **`mysql_keyring_encryption_test`**, which Percona Server's test
package ships with `mysqlxtest` and `mysqltest_safe_process` (`build-ps/percona-server.spec`):
it loads `file://<--component_dir>/<--keyring>` as a component, a shared library
(`components/test/keyring_encryption_test/options.cc` and `components.cc`, percona-server 8.0),
as `mysql_migrate_keyring` does, which is refused for that already. Its prefix took it to the
mysql tools' row, which reads neither flag, so it is refused too. The round's other leads, MySQL
Cluster's `ndb_error_reporter` and `ndb_size.pl`, Homebrew's `brew`, and more service managers,
are filed as #6324 and #6325, and on #6320.
