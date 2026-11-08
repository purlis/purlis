### Security

- **A run's notice offers no Allow for a command whose flags name a command.** `ssh`, `scp`,
  `sftp`, `rsync`, `kubectl`, `psql`, `mysql`, `mariadb`, `zip` and `wget` are still offered
  Allow once on the exact command, by any name their builds go by (`kubectl.docker`, `psql-16`,
  `wget2`), but not with a flag that names a command or a file that can: among them
  `ssh -o ProxyCommand`, `-o XAuthLocation`, `-F` and `-I`; `scp -S`; `sftp -b`; `rsync -e`,
  `--rsh` and `--rsync-path`; `kubectl --kubeconfig` and kustomize's plugin flags; `psql -f`, `-o`
  and a backslash command in `-c`; `mysql --pager`, `--defaults-file` and a shell command in `-e`;
  `zip -TT`; and `wget --use-askpass`, `-e` and `--config`. The chat is told to leave the flag
  out. `sed`, `aria2c`, `kustomize`, `sshpass`, `autossh`, `mosh`, `lftp` and mysql's
  server-side and shell-like tools get no Allow at all. A persona can still declare any of them
  as a tool (#5799).
