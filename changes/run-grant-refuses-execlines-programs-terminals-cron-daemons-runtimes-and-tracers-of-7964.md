### Security

- **A run's notice offers no Allow for the rest of execline's programs** (`backtick`, `export`,
  `importas`, `forstdin`, `trap`, `piperw`, `heredoc`, `define`, `case`, `envfile`, `posix-cd`
  and the others), **for more chain loaders** (ucspi-tcp's `rblsmtpd` and `fixcrio`, busybox's
  `udpsvd`, s6-networking's `s6-tcpserver4d` and `s6-tcpserver6d`), **for terminals and
  multiplexers** (`xterm`, `kitty`, `alacritty`, `wezterm`, `gnome-terminal`, `konsole`,
  `zellij`, `byobu`), **for remote and VM runners and configuration agents** (`lima`,
  `multipass`, `orbctl`, `pssh`, `pdsh`, `clush`, `chef-client`, `chef-solo`, `cf-agent`,
  `buildkite-agent`, `fly`, `drone`, `garden`) **or for cron daemons and runners** (`fcron`,
  `bcron`, `dcron`, `crond`, `supercronic`, `ofelia`, `mcron`, `jobber`, `snooze`, runwhen's
  programs, `cronitor`). Each runs the command, the table or the pipeline it is handed. A persona
  can still declare any of them as a tool (#7964).
- **A run's notice offers no Allow for more interpreters and runtimes** (`kotlin`, `scala`,
  `groovy`, `clojure`, `elixir`, `erl`, `ghc`, `runghc`, `crystal`, `nim`, `dart`, `tcc`, `mono`,
  `wasmtime`, `wasmer`, `qjs`, `jsc`, `d8`, `rhino`, `jjs`, `frida` and others), **for tracers**
  (`bpftrace`, `trace-cmd`, `uftrace`, `sysdig`, `cset`), **for more build and version managers
  and compiler wrappers** (`rebar3`, `sdk`, `nvm`, `bundler`, `esy`, `ocamlfind`, `air`, `modd`,
  `watchman`, `ccmake`, `cpack`, `buck`, `muon`, `samu`, `icerun`, `iceccd`, `distccd`) **or for
  more package managers** (`upgradepkg`, `xbps-reconfigure`, `pikaur`, `trizen`, `aura`,
  `pamac`, `yaourt`, `pacstrap`, `arch-chroot`). Each runs the code, the build or the package
  scripts it is handed. A persona can still declare any of them as a tool (#7964).
- **Programs named after these lose their Allow with them**, for example fly.io's `fly` (by
  Concourse's), and every `<name>-<word>` and `<word>-<name>` of short rows such as `case`,
  `wait`, `boot` and `v`. Giving the harmless ones back is its own change (#7825).
