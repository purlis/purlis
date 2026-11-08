### Security

- **A run's notice offers no Allow for a command whose flags name a command.** `ssh`, `scp`,
  `sftp`, `rsync`, `kubectl`, `psql`, `zip` and `wget` are still offered Allow once on the exact
  command, but not with `ssh -o ProxyCommand` (or `LocalCommand`, `KnownHostsCommand`,
  `PKCS11Provider`), `-F` or `-I`; `scp -S`, `-D` or `-F`; `sftp -b`, `-S`, `-D` or `-F`;
  `rsync -e`, `--rsh`, `--rsync-path`, `-M` or `--daemon`; `kubectl --kubeconfig` or kustomize's
  plugin flags; `psql -f`, `-o` or a backslash command in `-c`; `zip -TT`; or `wget
  --use-askpass`, `-e` or `--config`. The chat is told to leave the flag out. `sed`, `aria2c` and
  `kustomize` get no Allow at all. A persona can still declare any of them as a tool (#5799).
