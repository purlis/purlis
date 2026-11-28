### Security

- **A run's notice offers no Allow for more terminals** (`xfce4-terminal`, `mate-terminal`,
  `terminator`, `tilix`, `foot`, `footclient`, `urxvt`, `rxvt`, `st`, `ghostty`, `kitten`,
  `x-terminal-emulator`, `lxterminal`, `qterminal`, `terminology`, `kgx`, `ptyxis`), **for tmux's
  session runners** (`tmuxinator`, `tmuxp`, `tmate`), **for VM and container runners** (`orb`,
  `prlctl`, `utmctl`, `tart`, `VBoxManage`, `lxc`, `incus`, `virsh`, every `qemu-system-*`, `wsl`)
  **or for tracers** (`stap`, `heaptrack`). Each runs the command it is handed, in a window, a
  pane, a guest or under a trace. A persona can still declare any of them as a tool (#7988).
- **A run's notice offers no Allow for more compilers, shells, interpreters and runtimes**
  (`runhaskell`, `ghci`, `kotlinc`, `scalac`, `groovysh`, `clj`, `iex`, `erlc`, `qjsc`, `jbang`,
  `bb`, `nbb`, `janet`, `fennel`, `moon`, `chez`, `scheme`, `csi`, `gsi`, `ecl`, `ccl`, `abcl`,
  `kawa`, `lfe`, `jython`, `jruby`, `truffleruby`, `graalpy`, `mruby`, `ocamlrun`, `utop`,
  `tclkit`, `expectk`, `fsi`, `scriptcs`, `planck`, `lumo`, `gleam`, `roc`, `odin`, `hare` and
  others), **for more build and version managers** (`rebar`, `nimble`, `shards`, `dub`, `flutter`,
  `mill`, `ghcup`, `n`), **for cron's siblings** (`jobberrunner`, `bcrontab`) **or for
  configuration and proxy runners** (`pyinfra`, `mitmproxy`, `mitmdump`, `mitmweb`,
  `terminal-notifier`). Each runs the code, the build or the script it is handed. A persona can
  still declare any of them as a tool (#7988).
- **Programs that share a form of these names lose their Allow with them**, for example GStreamer's
  `gst-*` programs (by `st`), Google's `gn` (by `n`), QEMU's `qemu-img` and `qemu-nbd` (by
  `qemu`), LXC's `lxc-ls` and `lxc-info` (by `lxc`), and every `<name>-<word>`, `<word>-<name>`
  and `g<name>` of these rows. Giving the harmless ones back is its own change (#7825).
