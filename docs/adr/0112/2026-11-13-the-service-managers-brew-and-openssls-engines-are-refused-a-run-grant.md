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
  out of reach from here. It is filed as #6557 rather than left unsaid.

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

## Homebrew's brew (#6325)

`brew ruby -e <text>` `exec`s Homebrew's Ruby with the text as a script, and `brew ruby
<file>` with the file (`Library/Homebrew/dev-cmd/ruby.rb`, github.com/Homebrew/brew, 4.4.0);
`brew sh --cmd <command>` has the preferred shell run the command (`dev-cmd/sh.rb`); and
`brew <name>` runs the `brew-<name>` or `brew-<name>.rb` the search path finds
(`Commands.external_cmd_path`, `Library/Homebrew/commands.rb:84-88`). It is refused whole.
`brew services start mysql`, the usual way to start the server on macOS, goes with it.

## openssl's engines, providers and configuration modules, and my_openssl (#6329)

`openssl` stays offered, with the general-purpose caution, as it is a usual reason to hand a
chat a key; only its forms that load a library are refused, by a row in `command_flags`.
Checked in github.com/openssl/openssl at `openssl-3.0.15`:

- **The parser.** `opt_next` (`apps/lib/opt.c:848-885`) takes a flag as `-name` or `--name`,
  matched to its whole name with `strcmp`, its value after `=` or in the next word, and stops
  at `--` or the first word without a dash. The row reads each flag the same way, as a whole word
  whatever its case, so a short option (`enc -e`) is never taken for the start of a long one, and
  it reads on past the first operand, which is wider than openssl and only takes away an Allow.
- **`-engine <id>`**, in nearly every command, and `req -keygen_engine` and
  `s_client -ssl_client_engine`: an id no engine has is handed to the dynamic engine as
  `SO_PATH` and `LOAD`ed, a shared library by its path (`try_load_engine`,
  `apps/lib/engine.c:28-38`).
- **`-provider <name>`** loads a provider module, which a path names as well as a name does,
  and **`-provider-path <folder>`** sets the folder it is loaded from (`apps/include/opt.h:290-291`,
  `app_provider_load` and `opt_provider_path`, `apps/lib/app_provider.c:33-37, 61-65`).
- **`-config <file>`**: `ca`, `req`, `ts` and `srp` (`app_load_modules`, `apps/lib/apps.c:400-417`)
  and `cms`, `smime`, `genpkey` and `pkeyutl` (`app_load_config_modules`) load the modules the
  file names, its `providers` and `engines` sections among them, so a file the chat wrote names
  the library. `fipsinstall -config` does the same (`apps/fipsinstall.c:206`). `-config` is
  refused in every command, the ones that only read it too.
- **The `engine` command** loads the engines it is named and runs the control commands its
  `-pre` and `-post` name, `SO_PATH:<library>` and `LOAD` among them (`apps/engine.c`), and
  **`fipsinstall`** sets the provider search path to its `-module`'s folder and loads the FIPS
  provider from it (`apps/fipsinstall.c:400-410, 500`). Both are refused as the first operand.

**`my_openssl`**, the bundled openssl MySQL's test RPM ships under its own name
(`%{_bindir}/my_openssl`, `packaging/rpm-oel/mysql.spec.in:1611`; the bundled build's `openssl`
copied, as `cmake/ssl.cmake:185-186` says, mysql-server 8.4.3), is read by the same row, and is
on the general-purpose list, which matches a program's exact name only, so its notice carries
openssl's caution too.

## The review of this amendment's pull request

The adversarial review of #6556 found more, each checked against its source:

- **openssl finds a command by its first eight characters and a hash of the whole name**
  (`function_cmp` is `strncmp(a->name, b->name, 8)`, `function_hash` hashes the whole name,
  `apps/openssl.c:439-447`, openssl 3.0.15). A word that starts `fipsinst` and has
  `fipsinstall`'s hash runs `fipsinstall`, and the review found such words, so a refused command
  of eight characters or more is refused by its first eight. `engine` is shorter, and is matched
  whole. `speed` loads a `-config` file's modules too, which the row already refuses.
- **systemd's programs that run the command after them**, v257: `systemd-cat`,
  `systemd-inhibit` and `systemd-socket-activate` `execvp` it (`src/journal/cat.c:177`,
  `src/login/inhibit.c:318`, `src/socket-activate/socket-activate.c:211`), `systemd-notify
  --exec` runs it (`src/notify/notify.c:151, 476`), and `systemd-dissect --with` runs a command
  in the image it mounts (`src/dissect/dissect.c`). `busctl call` reaches any method on the
  bus, the manager's `StartTransientUnit` among them, which is how the refused `systemd-run`
  runs a command. All refused.
- **The supervision suites**: s6's `s6-supervise <folder>` runs the folder's `./run`
  (`src/supervision/s6-supervise.c`, github.com/skarnet/s6), and `s6-svscan` supervises each
  folder in the one it is named, as runit's `runsvdir` starts `runsv` on each
  (`runsv()`, `src/runsvdir.c`). All refused. daemontools' `svscan` and `supervise` are surely
  the same, but their source is not on GitHub, so they are filed (#6559) rather than refused
  unread.
- **Beside `brew`**: `apt-get` and `apt` take any setting with `-o` or `-c`
  (`apt-private/private-cmndline.cc:418-419`, github.com/Debian/apt, 2.9.8), and
  `Dir::Bin::Methods::<scheme>` names the program a download `execv`s as its method
  (`apt-pkg/acquire-worker.cc:96-155`), as an ordinary user for `download`. `composer exec`
  runs the command it is handed (`src/Composer/Command/ExecCommand.php`,
  github.com/composer/composer). All refused.

The review also said a bare `openssl` 1.1.1 reads its commands from its input, which a granted
run is handed. It is not: a run under a grant has no stdin (`Stdio::null()` where
`under_a_grant`, `crates/purlis-core/src/secrets/brokered.rs`), so nothing is read, and openssl
alone stays offered.

The review's second round found more, each checked in source:

- **openssl's engine URI.** At start openssl registers a store loader for the
  `org.openssl.engine:` scheme (`setup_engine_loader`, `apps/openssl.c`). A key or a file named
  `org.openssl.engine:<id>:<key>`, wherever one is read (`-in`, `-key`, `storeutl`, `-CAstore`),
  loads the engine by its id (`ENGINE_by_id`, `apps/lib/engine_loader.c`). An id no engine has
  goes to the dynamic engine with `DIR_LOAD` (`crypto/engine/eng_list.c:418-423`), which takes
  a path, so no refused flag is needed (openssl 3.0.15). The row refuses that text anywhere in any
  word, whatever its case, and, as the review's third round found, after a `--` too, where
  `storeutl` takes its URI (`apps/storeutl.c:258-263`) and `verify` its certificates.
- **rpm**: `-E <expr>` expands a macro (`lib/poptALL.c:128-131`,
  github.com/rpm-software-management/rpm, 4.20.0), and `%(…)` has a shell run what it holds
  (`rpmio/macro.c`). Every rpm tool that shares those options is refused: `rpm`, `rpmbuild`,
  `rpmspec`, `rpmquery`, `rpmverify`, `rpmkeys`, `rpmsign` and `rpmdb`.
- **dpkg**: `--force-not-root --pre-invoke=<cmd>` has `system()` run the command
  (`src/main/main.c:397`, dpkg 1.22.11), and **`dpkg-buildpackage --hook-init=<cmd>`** runs its
  hooks (`scripts/dpkg-buildpackage.pl`). Both refused.
- **`setpriv`** `execvp`s what follows it, with no privilege needed (`sys-utils/setpriv.c:1077`,
  util-linux 2.40.2), and so do **s6's chain loaders** `s6-setuidgid`, `s6-softlimit` and
  `s6-applyuidgid`. All refused.
- **`coredumpctl debug --debugger=<program>`** runs the program it is named
  (`src/coredump/coredumpctl.c`, systemd v257). Refused.

Its leads that were not checked (`aptitude`, `run-parts`, `capsh`, `cpanm`, `dnf`) are filed as
#6564.
