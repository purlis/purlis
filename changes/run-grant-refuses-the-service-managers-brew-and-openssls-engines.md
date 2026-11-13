### Security

- **A run's notice offers no Allow for the service managers** (`systemctl`, `start-stop-daemon`,
  runit's `sv` and `runsv`, `supervisorctl`, `openrc`, `openrc-run`, `chkconfig`,
  `deb-systemd-helper`). Each starts a program, a unit or a script it is named, or loads a plugin
  its settings name, and `systemctl import-environment` copies a value the run was handed into
  the service manager, for every service started after it. A persona can still declare any of
  them as a tool (#6320).
- **A run's notice offers no Allow for a program reached through a link in an `init.d` folder**,
  even where that link points on out of it, as Upstart's jobs were laid out
  (`/etc/init.d/<job>` to `/lib/init/upstart-job`) (#6321).
- **A run's notice offers no Allow for MySQL Cluster's `ndb_error_reporter` or `ndb_size.pl`**.
  The first hands a shell the file name and the user it is given, and the second puts its host
  into a database address that can name a file of settings the client library reads. A persona
  can still declare either as a tool (#6324).
- **A run's notice offers no Allow for Homebrew's `brew`**, which runs the Ruby or the shell
  command it is handed (`brew ruby -e`, `brew sh --cmd`). A persona can still declare it as a
  tool (#6325).
- **A run's notice offers no Allow for `openssl` with `-engine`, `-provider`, `-provider-path` or
  `-config`, or for its `engine` and `fipsinstall` commands**, each of which loads a library or a
  module it is named. `openssl` is still offered without them, and so is `my_openssl`, the copy
  MySQL's test package ships, with the same caution (#6329).
