### Security

- **A run's notice offers no Allow for `mysqlaccess`, `mytop`, the table format converter, the
  permission setter or Percona Server's `ps-admin`** (`mariadb-access`,
  `mysql_convert_table_format`, `mariadb-convert-table-format`, `mysql_setpermission`,
  `mariadb-setpermission`). `mysqlaccess` splits its host value into the client's own flags, and
  the other MariaDB tools put theirs into a database address that can name a file of settings
  the client library reads. `ps-admin` runs its configuration file as a shell script. A persona
  can still declare any of them as a tool (#6185).
- **A run's notice offers no Allow for the test suite's runners, RocksDB's hot backup or a
  service's init script** (`mysqlxtest`, `mysqltest_safe_process`, `myrocks_hotbackup`,
  `rcmysql`). The runners run a command their script or their arguments name, and the hot
  backup hands its options to a shell and to ssh. An init script, such as the one `rcmysql`
  links to, starts the service it is for, so a program found in an `init.d` folder is refused
  whatever it is called. A persona can still declare any of them as a tool (#6187).
