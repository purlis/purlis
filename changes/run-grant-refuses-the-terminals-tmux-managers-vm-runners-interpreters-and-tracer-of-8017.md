### Security

- **A run's notice offers no Allow for more terminals** (`rio`, `contour`, `cool-retro-term`,
  `sakura`, `guake`, `tilda`, `yakuake`, `Eterm`, `aterm`, `mlterm`, `roxterm`, `cosmic-term`,
  `blackbox-terminal`, `deepin-terminal`, `hyper`, `tabby`, `warp-terminal`), **for more tmux
  session managers** (`teamocil`, `smug`, `sesh`, `tmuxifier`, `mux`), **for more VM and
  container runners** (`virt-customize`, `virt-sysprep`, `guestfish`, `virt-install`, `crictl`,
  `youki`, `firecracker`, `vfkit`, `krunvm`, `hyperkit`, `xhyve`, `lxd`, `VBoxHeadless`,
  `prlsrvctl`), **for more interpreters** (`newlisp`, `picolisp`, `gxi`, `gerbil`, `csc`,
  `murex`, `amm`, `ammonite`, `cs`, `coursier`) **or for macOS's `ktrace`**. Each runs the
  command, the layout, the guest or the code it is handed. A persona can still declare any of them
  as a tool (#8017).
- **Programs that share a name or a name's form with these lose their Allow with them**, for
  example TabbyML's `tabby` server, Project Contour's `contour`, the C# compiler `csc`, LXD's
  `lxd-*` programs (by `lxd`), and every `<name>-<word>`, `<word>-<name>` and `g<name>` of the
  short rows `cs`, `amm`, `mux`, `rio` and `smug`. Giving the harmless ones back is its own change
  (#7825).
