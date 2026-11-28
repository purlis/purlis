# Amended 2026-11-28: execline's programs, terminals, cron daemons, runtimes and tracers of #7964 are refused a Run grant

Decided under the operator's standing order to take the recommended path (#7964), recorded here
as delegated decisions (delegated: recommended path). It follows *the supervisors, chain loaders,
installers and builders of #7833 are refused a Run grant* (2026-11-28), whose closing filed these
families as #7964, and whose review's round 1 added the siblings in #7964's comment.

## Not read in source, but for one, and why each is refused all the same

As in the amendment before this one, the source of these tools was out of reach: from the
sandbox that did it, `gh` reaches only `purlis/purlis`, and a plain fetch of any other host was
refused. Of the tools refused here, three have a file of that name on the machine's search
path: `bundler`, `jjs` and `wait`; JavaScriptCore's `jsc` is there too, off the search path, in
its framework's `Helpers`. `jjs` is macOS's compiled Java stub and `jsc` a compiled shell, judged
by what they are for. `bundler` and `wait` are scripts, and were read where macOS keeps them:

- **`bundler`** is macOS's `/usr/bin/bundler`, a RubyGems stub that loads Bundler 1.17.2
  (`/System/Library/Frameworks/Ruby.framework/Versions/2.6/usr/lib/ruby/2.6.0/bundler`). Its
  `Dsl#eval_gemfile` evaluates the Gemfile as Ruby (`bundler/dsl.rb:47`, `instance_eval`), which
  every command that reads the bundle does, and `bundler exec` hands its command to
  `Kernel.exec` (`bundler/cli/exec.rb:50-53`). So `bundler` runs the project's code under nearly
  every subcommand, and is refused whole, as `bundle` already is.
- **`wait`** is macOS's `/usr/bin/wait`, FreeBSD's four-line `/bin/sh` script that runs
  ``builtin `echo ${0##*/} | tr …` ${1+"$@"}``: the shell's own `wait` builtin, which runs
  nothing it is handed. It is harmless, and not the program the row is for: execline's `wait`
  waits for the processes it is named, then runs the program after them. One name is both, so
  the row stays, and macOS's `wait` loses its Allow with execline's: a cost, below.

Every other tool is refused whole, on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its
manual describes it, and not on a line of its source. That is the conservative side: a refusal
only takes away an Allow on a run's notice, and a persona can still declare any of them as a tool
in a reviewed line. Reading each tool in source, to cite its lines or give an Allow back, is
#7825.

## A refusal of the whole tool, not a row of `command_flags`

#7964 names several of these by the one form that runs code: `xterm -e`, `alacritty -e`,
`wezterm start`, `gnome-terminal --`, `konsole -e`, `zellij run`, `multipass exec`, `orbctl
run`, `fly execute`, `drone exec`, `cronitor exec`, `ghc -e`, `tcc -run`, `bpftrace -c`,
`trace-cmd record`, `cset proc --exec` and `nvm exec`. Each is refused whole all the same. A
`command_flags` row needs a parser written as the tool reads its own words, and none of their
sources was in reach to write one from. For most, more forms than the one named run code as well
(`ghc`'s Template Haskell in any build, a terminal's settings file naming its shell, `bpftrace`'s
`system()`, `sysdig`'s Lua chisels, `zellij`'s layouts, `wezterm`'s Lua settings). The one read
in source, `bundler`, runs the Gemfile under nearly every subcommand, so it has no one form to
refuse. None of them is the usual reason a chat is handed a key, as `ssh` and `psql` are.

## On `NOT_FOR_A_RUN_GRANT`, not `REFUSED_NAMES`

The interpreters and runtimes (`kotlin`, `scala`, `groovy`, `clojure`, `elixir`, `erl`, `ghc`,
`dart`, `mono`, the WebAssembly runtimes, the JavaScript engines and the rest) run whatever
they are told, as `REFUSED_NAMES`' interpreters do. They are refused on `NOT_FOR_A_RUN_GRANT`
all the same (delegated: recommended path): it takes away a run's Allow, which is what #7964
asks for, and leaves a persona able to declare one in a reviewed line, as it can every row of
#7833. Moving one to `REFUSED_NAMES`, which takes it from a persona's declaration too, is its
own decision.

## What is refused

- **execline's other programs**, each a chain loader that runs the program after it once it has
  set a variable, a descriptor, a signal or a loop up: `backtick`, `withstdinas`, `export`,
  `unexport`, `importas`, `elglob`, `forbacktickx`, `forstdin`, `loopwhilex`, `trap`, `tryexec`,
  `piperw`, `fdclose`, `fdblock`, `fdswap`, `fdreserve`, `heredoc`, `multisubstitute`,
  `multidefine`, `define`, `runblock`, `shift`, `wait`, `case`, `envfile`, `getpid`, `getcwd`,
  `posix-cd` and `posix-umask`. `homeof` prints a user's home folder and, by its manual, runs
  nothing; it is refused with them on the conservative side, as `s6-fdholderd` was with its
  family.
- **More chain loaders** (#7964's comment): ucspi-tcp's `rblsmtpd` and `fixcrio`, busybox's
  `udpsvd`, and s6-networking's `s6-tcpserver4d` and `s6-tcpserver6d`, which no form of
  `tcpserver`, `tcpsvd` or `s6-tcpserverd` reaches.
- **Terminals and multiplexers**, as `screen` and `tmux` are: `xterm`, `kitty`, `alacritty`,
  `wezterm`, `gnome-terminal`, `konsole` and `zellij` run the command they are handed in the
  window or pane they open, and `byobu` is `tmux`'s or `screen`'s front-end.
- **Remote and VM runners, agents and configuration runners**: `lima`, `multipass` and
  `orbctl` run a command in a VM; `pssh`, `pdsh` and `clush` on the hosts they are named;
  `chef-client` and `chef-solo` a cookbook's recipes; CFEngine's `cf-agent` and `cfagent` a
  policy's commands; `buildkite-agent` a pipeline's steps; Concourse's `fly` a task; `drone` a
  pipeline; and `garden` a project's actions.
- **Cron daemons and runners**, as `crontab` and `anacron` are: `fcron`, `bcron`, `dcron`,
  cronie's `crond`, `supercronic`, `ofelia`, `mcron` and `jobber` run the commands of a table or
  a file; `snooze` and runwhen's programs (`runwhen`, `rw-add`, `rw-match`, `rw-sleep`, `rw-min`
  and `rw-max`, by the names its documentation gives, refused together on the conservative side)
  run the program after them at a time; and `cronitor` runs the command its `exec` wraps.
- **Interpreters and runtimes**: `kotlin`, `scala`, `groovy`, `clojure`, `boot`, `elixir`,
  `erl`, `escript`, `ghc`, `runghc`, `crystal`, `nim`, `v`, `dart`, `tcc`, `mono`, `wasmtime`,
  `wasmer`, `wasm3`, `qjs`, `jsc`, `d8`, `hermes`, `rhino`, `jjs`, and `frida`, which spawns the
  program it is named and runs a script in it.
- **Tracers**, as `strace` and `ltrace` are: `bpftrace`, `trace-cmd`, `uftrace`, `sysdig` and
  `cset` run the command after them to trace it.
- **Build and version managers, builders and compiler wrappers**: `rebar3`, SDKMAN's `sdk`,
  `nvm`, `bundler`, `esy`, `ocamlfind`, the reloaders `air` and `modd`, and `watchman`, whose
  triggers run commands; CMake's `ccmake` and `cpack` (`cmake` and `ctest` are refused), Buck 1's
  `buck`, `muon` (meson's twin) and `samu` (ninja's); and icecream's `icerun` and `iceccd`, and
  `distccd`, which run the compiler commands they are handed. SDKMAN's `sdk` and `nvm` are
  usually shell functions, which a Run grant never runs; their rows refuse them where they are
  files of their own.
- **Package managers and installers**, as `pacman`, `installpkg` and `xbps-install` are:
  Slackware's `upgradepkg`, `xbps-reconfigure`, the AUR helpers `pikaur`, `trizen`, `aura`,
  `pamac` and `yaourt`, and Arch's `pacstrap`, each of which runs a package's scripts or a
  `PKGBUILD`, and `arch-chroot`, which runs the command after it in a new root.

## Left offered

No program #7964 names is left offered. Among the programs of the same packages that #7964 does
not name, those its new rows do not reach stay offered until they are checked, and are filed
with the other families below.

## A chance reach, by a row of its own

As the amendments before this one ruled, a program that another family's row reaches only by
chance has a row of its own and a unit test. Of the names #7964 lists, one was refused already:
`arch-chroot`, which REFUSED_NAMES' `arch` and `chroot` reach by its first and last parts. It
now has a row of its own, pinned by a unit test seen red first. The new rows also reach one row
that was there before: `buck` reaches #7833's `buck2` by its unversioned form, so `buck2`'s
`run_grants` test passes through either row. A unit test pins `buck2`'s own row, seen red with it
taken off. Among the new rows, `bcron`, `pamac`, `wezterm` and `watchman` reach their own
programs (`bcron-exec`, `pamac-manager`, `wezterm-gui`, `watchman-make`) by their first part,
within their own families.

## What the name forms cost

These rows refuse more than they name, through their first and last parts and the rest of
`name_forms`. The extra cost is an Allow and nothing else. A probe of `not_for_a_run_grant` over
the 1,723 program names on the machine that did this (`/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`,
`/usr/local/bin`, `/opt/homebrew/bin` and `/opt/homebrew/sbin`) found none newly refused beyond
the names of the rows themselves, of which `wait` is the one harmless program. Short and common
words cost the most elsewhere. These are examples, not a complete list:

- `case`, `wait`, `trap`, `shift`, `define`, `export`, `boot`, `air`, `fly`, `sdk`, `nvm`, `v`,
  `nim`, `erl`, `tcc`, `mono`, `dart`, `cset` and `pdsh` reach every `<word>-<name>` and
  `<name>-<word>`: `google-cloud-sdk` and `android-sdk` by `sdk`, `nvm-windows` by `nvm`, and
  pdsh's own `pdsh-mod-*` by `pdsh`.
- the GNU `g` the forms strip reaches every `g<name>` of these rows, and `gv`, GNU's PostScript
  viewer, which runs nothing it is handed, loses its Allow by `v`.
- `fly` is Concourse's, and fly.io's command line, also named `fly`, loses its Allow with it;
  its other name `flyctl` stays offered.
- macOS's `/usr/bin/wait`, read above, runs nothing it is handed and loses its Allow by
  execline's `wait`, the one name on the machine's search path these rows take from a harmless
  program.
- `homeof`, and runwhen's `rw-min` and `rw-max`, are refused with their families on the
  conservative side, though their manuals give them nothing to run.

Which of these to give back, and how to do it without giving back the ones that run code, is
part of #7825.

## Other families

The leads found beside #7964's own list are filed as #7988 rather than added here: more
terminals (kitty's `kitten`, `xfce4-terminal`, `foot`, `ghostty` and others), the tmux session
runners `tmuxinator` and `tmuxp`, OrbStack's `orb`, compilers and shells that run code
(`runhaskell`, `ghci`, `kotlinc`, `scalac`, `iex`, `jbang` and others), rebar 2's `rebar`,
`jobberrunner`, `bcrontab`, `pyinfra` and `mitmproxy`.
