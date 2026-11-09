# Amended 2026-11-09: mysql's server-side tools and Percona's backup tools are refused a Run grant

Decided under the operator's standing order to take the recommended path (D-5931-1, #5931, from
the second adversarial review of #5929), recorded here as a delegated decision. It follows *a Run
grant refuses a flag that names a command* (2026-11-09).

The tools of MySQL's and MariaDB's family that no row read, or read only by some of their
spellings, are refused whole, with no notice, as `mariabackup` and `mysqlhotcopy` are:

- **`mysql_plugin`** (`mariadb-plugin`) starts the server its `-m` or `-b` finds, with the plugins
  of the folder its `-p` names, and `--my-print-defaults` names a program it runs. Its tools' row
  read only the long forms.
- **`mysql_upgrade`** (`mariadb-upgrade`) runs the client and `mysqlcheck` with the files of
  settings and plugins it is handed.
- **Percona's backup tools** (`xtrabackup`, `innobackupex`, `xbstream`, `xbcloud`,
  `xbcloud_osenv`, `xbcrypt`), and MariaDB's `mbstream`, are mariabackup's kin, and reached no
  row at all.
- **The server itself, its safe helper and the router** (`mysqld`, `mariadbd`,
  `mysqld_safe_helper`, `mariadbd-safe-helper`, `mysqlrouter` and its own tools)
  and **`mysql_migrate_keyring`** (from the second review of #5990): `mysqld -b` names the
  folder its plugins load from and `--early-plugin-load` names a plugin, the router's
  configuration names its plugins' folder, and the keyring migration loads components from a
  folder it is told. The safe helper runs the command its arguments name, as the user they
  name. The tools' row read none of those forms.
- **Every `wsrep_sst_` script**, by that prefix, as git's own programs are by `git-`: each is a
  shell script the server runs to hand a node its state, which runs the transfer program its
  arguments name, and releases add new ones.

Client-side kin that run code their flags or files name (`mydumper --exec`, Percona Toolkit's
`pt-*` plugins) and Galera's helpers (`galera_recovery`, `garbd`) are left to #5994.

A row of their own was not taken: each is server-side, none is a usual reason to hand a chat a
key, and a row would have to follow every program each one runs. A person can still declare any
of them as a tool.
