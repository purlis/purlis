### Security

- **A run's notice offers no Allow for `mydumper`, `myloader`, Percona Toolkit's `pt-*` tools,
  `gh-ost`, Galera's helpers or the clients with the server embedded** (`galera_recovery`,
  `galera_new_cluster`, `garbd`, `garb-systemd`, `mysql_embedded`, `mariadb-embedded`,
  `mysql_client_test_embedded`, `mariadb-client-test-embedded`). Each runs a command, a script
  or a plugin that its flags or files name. `mysql`, `mysqldump` and the other clients are still
  offered. A persona can still declare any of these as a tool (#5994).
