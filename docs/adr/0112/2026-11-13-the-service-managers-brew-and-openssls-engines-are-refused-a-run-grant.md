# Amended 2026-11-13: the service managers, a link that passes through init.d, MySQL Cluster's reporters, brew and openssl's engines and providers are refused a Run grant

Decided under the operator's standing order to take the recommended path (#6320, #6321, #6322,
#6324, #6325 and #6329, from the reviews of #6318), recorded here as delegated decisions. It
follows *the tools whose host reaches the client's flags or a DBI address … are refused a Run
grant* (2026-11-12), which filed these leads. Each was checked against the tool's own source,
not recalled.

## The service managers (#6320)

- **`systemctl`**. `systemctl link <path>` makes a unit of the file it is named, outside the
  unit search path, and `systemctl start` then runs its `ExecStart`; `systemctl edit` runs an
  editor; and `systemctl import-environment <VAR>` copies the value of a variable the run was
  handed into the service manager's own environment (`verb_import_environment`, the
  `SetEnvironment` call), where `show-environment` reads it back and every unit started after
  it is given it (`src/systemctl/systemctl.c`'s verb table and
  `src/systemctl/systemctl-set-environment.c`, github.com/systemd/systemd, v257). `launchctl`
  is refused for the same reason already.
- **`start-stop-daemon`**, dpkg's: `--start` `execv`s the program its `--startas` names, or its
  `--exec` (`utils/start-stop-daemon.c`, dpkg 1.22.11, the `exec` and `startas` options and the
  `execv(startas, argv)` it ends in).

The issue's comment raised six more, each checked too:

- **runit's `sv`** takes a service as a name under `$SVDIR` or as a path (any name starting with
  `/` or `.`, or ending in `/`, is a folder), and runs that folder's `./check` script for
  `check`, `start` and the rest when it waits (`checkscript`, `src/sv.c`,
  github.com/void-linux/runit). **`runsv <folder>`** runs the folder's `./run`
  (`src/runsv.c`). Both refused; `chpst` is refused already.
- **`supervisorctl`**: its `-c` file's `[ctlplugin:<name>]` sections name a
  `supervisor.ctl_factory`, which it imports and calls with the section's other settings
  (`get_plugins` and `import_spec`, `supervisor/options.py`, and `supervisorctl.py`'s
  `Controller`, github.com/Supervisor/supervisor, 4.2.5). Refused.
- **OpenRC's `openrc`**: `--service <name>` resolves the name, which may be an absolute path,
  and `execv`s it (`src/openrc/rc.c`, with `rc_service_resolve` in `src/librc/librc.c`, which
  returns a name starting with `/` as it is, github.com/OpenRC/openrc, 0.55). **`openrc-run`**,
  the interpreter of an OpenRC script, has `openrc-run.sh` source the script it is named
  (`RC_SERVICE="$1"` and `sourcex "$RC_SERVICE"`, `sh/openrc-run.sh.in`). Both refused;
  `rc-service` and `rc-update` are refused already by their forms.
- **`chkconfig`** hands `systemctl` the unit it is named, `systemctl <verb> <name>.service`
  (`forwardSystemd`, `chkconfig.c`, github.com/fedora-sysv/chkconfig, 1.30), and
  **`deb-systemd-helper`** runs `systemctl … preset <unit>` on the unit it is named
  (`script/deb-systemd-helper`, salsa.debian.org/debian/init-system-helpers). `systemctl` is
  refused, so both are.
- **`initctl`**, Upstart's, could not be checked: its source (git.launchpad.net/upstart) was
  out of reach from here. It is filed as an issue of its own rather than left unsaid.

All are refused whole, on `NOT_FOR_A_RUN_GRANT`. A persona can still declare any of them as a
tool.

## An init script reached through a link that points on out of init.d (#6321)

The 2026-11-12 amendment refused a program whose path, as its links resolve, sits in an
`init.d` folder. It judged only that last path, so a link that lands in `init.d` on a name that
is itself a link pointing on out of it was not caught: Ubuntu's Upstart jobs were laid out so,
`/etc/init.d/<job> -> /lib/init/upstart-job`, and the script that runs is named for no service.
Whether any package still ships that layout was not settled, so the check is made whole rather
than argued: following a program's links now reports the folder of every link on the way
(`held_outside`'s walk), and a Run grant refuses the program where the path it was found at,
the path it runs as, or any link on the way sits in a folder named `init.d`. The program is
judged as it was found for the grant, with the folders a chat may write left out, so the links
judged are the ones on the way to what runs. A program whose links never sit in `init.d` is
offered as before. `/etc/rc.d/<name>` (BSD, Slackware) is not taken in: no package is known to
link it onto the search path, and its folder's name is a common one.

## RocksDB's ldb and sst_dump, found harmless (#6322)

RocksDB's `ldb` and `sst_dump` ship as `mariadb-ldb` and `sst_dump` in Debian's
`mariadb-plugin-rocksdb` (`MYSQL_ADD_EXECUTABLE` in `storage/rocksdb/CMakeLists.txt`,
github.com/MariaDB/server, 11.4), as `mysql_ldb` in Percona's, and as `ldb` beside RocksDB's
own. Checked at the RocksDB that MariaDB 11.4 pins (`storage/rocksdb/rocksdb` at
`79f08d7ffa6d34d9ca3357777bcb335884a56cfb`, github.com/facebook/rocksdb):

- `--env_uri` and `--fs_uri` (and `--backup_env_uri`, `--backup_fs_uri`) are handed to
  `Env::CreateFromUri` (`tools/ldb_cmd.cc:303`, `tools/sst_dump_tool.cc:352`), which makes the
  object through the object registry (`env/env.cc:713`). Neither the registry
  (`utilities/object_registry.cc`), the options loader (`options/customizable.cc`,
  `options/options_parser.cc`, `utilities/options/options_util.cc`) nor `env/env.cc` calls
  `LoadLibrary` or `dlopen`; the one `dlopen` is `PosixEnv::LoadLibrary`
  (`env/env_posix.cc:247-268`), which none of the tools' paths reach.
- Neither `tools/ldb_cmd.cc`, `tools/ldb_tool.cc` nor `tools/sst_dump_tool.cc` calls `system`,
  `popen` or an `exec`.
- MariaDB's `mariadb-ldb` is `tools/mysql_ldb.cc`: `MY_INIT`, MyRocks's comparator, and
  `LDBTool::Run`; it reads no defaults file.

What they do is read and write the RocksDB folder their `--db` names, as the arguments the
person reads say. They keep no row of their own: `mariadb-ldb` and `mysql_ldb` reach the mysql
tools' row by their prefix, which none of their usual commands trips, and tests pin all four
names as offered.

## MySQL Cluster's ndb_error_reporter and ndb_size.pl (#6324)

Both ship in MySQL's packages built with the cluster (`%{_bindir}/ndb_error_reporter` and
`%{_bindir}/ndb_size.pl`, `packaging/rpm-sles/mysql.spec.in:1068, 1085`, under `%if
0%{?cluster}`; installed by `storage/ndb/tools/CMakeLists.txt`, github.com/mysql/mysql-server,
8.0.40). Checked again in their source:

- **`ndb_error_reporter`** takes the config file's name as its first argument and joins it
  unquoted into `$ndb_config --config-file=$config_file …`, which `system()` and backticks hand
  a shell (`storage/ndb/tools/ndb_error_reporter.pl:79-122`); a file the chat names `x;<cmd>`
  passes its `stat`. Its second argument, the scp user, is joined into
  `scp $scp_options $config_scp_user$host:…`, which `system($cmd)` runs (`:87-90, 156-163`).
- **`ndb_size.pl`** splits its `--hostname` at `:` and joins the host and its `--socket` into
  `DBI:mysql:host=$host;…;mysql_socket=$socket` (`storage/ndb/tools/ndb_size.pl:466-476`), which
  DBD::mysql splits at each `:` or `;` into attributes, `mysql_read_default_file` among them:
  the case `mytop` was refused for on 2026-11-12. Its `--loadqueries` file is opened with `<`
  only.

Neither name starts with `mysql` or `mariadb`, so no row read them. Both are refused whole.
