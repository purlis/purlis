### Security

- **More programs that run others get no Allow from a run's notice.** `xcodebuild`, `msgexec`,
  `leaks`, `pyenv`, `erb`, `helm`, `socat`, `ncat`, `nc`, `netcat`, `tar` and `bsdtar` each run a
  command a flag, an address or a template names, so a chat's run of one with a vault's key is
  refused at once, with no notice. A persona can still declare one as a tool (#5619).
