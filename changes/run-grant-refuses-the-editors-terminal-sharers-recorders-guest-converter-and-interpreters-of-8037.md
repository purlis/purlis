### Security

- **A run's notice offers no Allow for editors with a shell or tasks** (`kak`, `micro`, `vis`,
  `mg`, `jed`, `joe`, `lite-xl`, `pulsar`, `neovide`, `mvim`, `code`, `codium`, `subl`, `zed`,
  `lapce`), **for more terminal sharers** (`upterm`, `sshx`, `tty-share`, `teleconsole`, `wetty`)
  **and recorders** (`ttyrec`, `termtosvg`, `terminalizer`, `t-rec`), **for libguestfs's
  `virt-v2v`**, **or for more interpreters and shells** (`gsc`, `hissp`, `lissp`, `ion`, `oil`).
  Each runs the shell command, the task, the script or the code it is handed. A persona can still
  declare any of them as a tool (#8037).
- **Programs that share a name or a name's form with these lose their Allow with them**: macOS's
  own `vis`, a filter that runs nothing, ZFS's `zed` daemon, and every `<word>-<name>`,
  `<name>-<word>`, `g<name>`, `gnu<name>` and versioned form of the one-word rows, most likely of
  the common and short ones (`code-<word>`, `<word>-mg`, `gion`, for example). Giving the
  harmless ones back is its own change (#7825).
