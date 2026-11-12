### Security

- **A run's notice offers no Allow for `mysqlaccess`, `mytop`, the table format converter, the
  permission setter or Percona Server's `ps-admin`** (`mariadb-access`,
  `mysql_convert_table_format`, `mariadb-convert-table-format`, `mysql_setpermission`,
  `mariadb-setpermission`). `mysqlaccess` splits its host value into the client's own flags, and
  the other MariaDB tools put theirs into a database address that can name a file of settings
  the client library reads. `ps-admin` runs its configuration file as a shell script. A persona
  can still declare any of them as a tool (#6185).
