### Security

- **A run's notice offers no Allow for the service managers** (`systemctl`, `start-stop-daemon`,
  runit's `sv` and `runsv`, `supervisorctl`, `openrc`, `openrc-run`, `chkconfig`,
  `deb-systemd-helper`). Each starts a program, a unit or a script it is named, or loads a plugin
  its settings name, and `systemctl import-environment` copies a value the run was handed into
  the service manager, for every service started after it. A persona can still declare any of
  them as a tool (#6320).
