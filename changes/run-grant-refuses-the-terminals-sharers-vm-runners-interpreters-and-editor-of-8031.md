### Security

- **A run's notice offers no Allow for more terminals** (`uxterm`, `lxterm`, `koi8rxterm`,
  `pterm`, `termite`, `kmscon`, `zutty`, `mrxvt`, `wterm`, `waveterm`, `extraterm`), **for
  terminal sharers and recorders** (`ttyd`, `gotty`, `asciinema`, `vhs`) **or for more VM and
  sandbox runners** (`virt-builder`, `virt-rescue`, `runsc`, `kata-runtime`, `cloud-hypervisor`,
  `bhyve`). Each runs the command it is handed, in a window, a shared terminal, a recording or a
  guest. A persona can still declare any of them as a tool (#8031).
- **A run's notice offers no Allow for more interpreters and installers** (`gxc`,
  `chicken-install`, `hy`) **or for Helix** (`hx`, `helix`), whose `:sh` runs a shell command.
  A persona can still declare any of them as a tool (#8031).
- **Programs that share a form of the short names lose their Allow with them**: every
  `<word>-<name>`, `<name>-<word>` and `g<name>` of `hy`, `hx` and `vhs`, and a hex viewer also
  called `hx`. Giving the harmless ones back is its own change (#7825).
