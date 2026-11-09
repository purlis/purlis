### Security

- **A run's notice offers no Allow for MySQL's and MariaDB's server, router, `mysql_plugin` and
  `mysql_upgrade`, Percona's backup tools or the server's state transfer scripts** (`mysqld`,
  `mariadbd`, `mysqld_safe_helper`, `mariadbd-safe-helper`, `mysqlrouter`,
  `mysqlrouter_plugin_info`, `mysqlrouter_passwd`, `mysqlrouter_keyring`, `mysql_migrate_keyring`,
  `mariadb-plugin`, `mariadb-upgrade`, `xtrabackup`, `innobackupex`, `xbstream`, `xbcloud`,
  `xbcloud_osenv`, `xbcrypt`, MariaDB's `mbstream` and every `wsrep_sst_` script): each runs a
  server, a client, a plugin or a program its flags name. A persona can still declare one as a
  tool (#5931).
