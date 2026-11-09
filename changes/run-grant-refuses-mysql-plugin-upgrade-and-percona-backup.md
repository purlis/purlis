### Security

- **A run's notice offers no Allow for `mysql_plugin`, `mysql_upgrade`, Percona's backup tools or
  the server's state transfer scripts** (`mariadb-plugin`, `mariadb-upgrade`, `xtrabackup`,
  `innobackupex`, `xbstream`, `xbcloud`, `xbcrypt` and every `wsrep_sst_` script): each runs a
  server, a client or a program its flags name. A persona can still declare one as a tool (#5931).
