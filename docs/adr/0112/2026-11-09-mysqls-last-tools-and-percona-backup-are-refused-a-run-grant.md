# Amended 2026-11-09: mysql's last tools and Percona's backup tools are refused a Run grant

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
  `xbcloud_osenv`, `xbcrypt`) are mariabackup's kin, and reached no row at all.
- **Every `wsrep_sst_` script**, by that prefix, as git's own programs are by `git-`: each is a
  shell script the server runs to hand a node its state, which runs the transfer program its
  arguments name, and releases add new ones.

A row of their own was not taken: each is server-side, none is a usual reason to hand a chat a
key, and a row would have to follow every program each one runs. A person can still declare any
of them as a tool.
