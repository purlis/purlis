# Amended 2026-11-10: mydumper, Percona Toolkit, gh-ost, Galera's helpers and the embedded clients are refused a Run grant

Decided under the operator's standing order to take the recommended path (D-5994-1, D-5994-2,
#5994, from the reviews of #5990), recorded here as delegated decisions. It follows *mysql's
server-side tools and Percona's backup tools are refused a Run grant* (2026-11-09), which left
these to #5994.

The tools of MySQL's and MariaDB's family that #5931 left are refused whole, with no notice:

- **`mydumper` and `myloader`** run a command on each file they write or read (mydumper's
  `--exec` and `--exec-per-thread`, myloader's `--exec-per-thread`), and either may be named in
  a defaults file that the command line never shows.
- **Every Percona Toolkit tool**, by its `pt-` prefix, as git's own programs are by `git-` and
  the state transfer scripts by `wsrep_sst_`. Many run code their flags name:
  `pt-online-schema-change --plugin` loads a Perl file, `pt-stalk --plugin` sources a shell
  file, `pt-kill --execute-command` runs a command, and `pt-query-digest --filter` evaluates
  Perl. Any flag may come from a configuration file the command line never shows
  (`--config`, `~/.percona-toolkit.conf`). Releases add new tools.
- **gh-ost** (D-5994-2, from the review): its `--hooks-path` names a folder whose `gh-ost-on-*`
  programs it runs.
- **Galera's helpers.** `galera_recovery` has a shell evaluate its arguments into the server's
  command line, so they can name the server's plugins (`--plugin-dir`, `--early-plugin-load`)
  or run a command of their own. `galera_new_cluster` has the service manager restart whatever
  unit it names. Percona XtraDB Cluster's `garbd` runs the scripts its `--recv-script` and
  `--post-recv-script` name, and every `garbd` reads the file its `--cfg` names. `garb-systemd`
  runs `garbd`.
- **The client and its test built with the server embedded** (`mysql_embedded`,
  `mariadb-embedded`, `mysql_client_test_embedded`, `mariadb-client-test-embedded`). What their
  `--server-arg` hands the server (`--plugin-load`, `--plugin-dir`) loads code from a file it
  names.

These are used client-side with a key, unlike #5931's, so a row of their own was weighed. None
was taken. A row would have to read every flag that names a command or a plugin, and every
defaults or configuration file a tool reads, across a family that grows with each release. The
client itself (`mysql`, `mariadb`), `mysqldump` and the rest of the clients stay offered, with
only their forms that name a command refused. A persona can still declare any of the refused
tools as a tool.
