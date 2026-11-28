# Amended 2026-11-28: the terminals, VM runners, compilers, interpreters and builders of #7988 are refused a Run grant

Decided under the operator's standing order to take the recommended path (#7988), recorded here
as delegated decisions (delegated: recommended path). It follows *execline's programs,
terminals, cron daemons, runtimes and tracers of #7964 are refused a Run grant* (2026-11-28),
whose closing filed these families as #7988, and whose review added the leads in #7988's
comment.

## Not read in source, and why each is refused all the same

None of the programs #7988 and its comment name has a file of that name on the search path of
the machine that did this (`/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`, `/usr/local/bin`,
`/opt/homebrew/bin` and `/opt/homebrew/sbin`), and no macOS copy of any of them was found to
read. Their upstream source was out of reach: from the sandbox that did it, `gh` reaches only
`purlis/purlis`, and a plain fetch of any other host was refused. So every one is refused whole,
on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its manual describes it, and not on a line
of its source. That is the conservative side (delegated: recommended path): a refusal only takes
away an Allow on a run's notice, and a persona can still declare any of them as a tool in a
reviewed line. Reading each in source, to cite its lines or give an Allow back, is #7825.

Before any row went in, a probe of `not_for_a_run_grant` found every name #7988 lists still
offered: none was refused already by `REFUSED_NAMES`, `REFUSED_PREFIXES` or another row's form.

## A refusal of the whole tool, not a row of `command_flags`

#7988 names several of these by the one form that runs code: `kotlinc -script`, `ghci -e`,
`prlctl exec`, `utmctl exec`, `tart exec`, `VBoxManage guestcontrol run`, `lxc exec`, `incus
exec`, `stap -c`, `ghcup run`, `n exec`, `mitmproxy -s` and `terminal-notifier -execute`. Each is
refused whole all the same (delegated: recommended path). A `command_flags` row needs a parser
written as the tool reads its own words, and none of their sources was in reach to write one
from. For most, more forms than the one named run code as well: `stap -g` embeds C, a
terminal's settings name the shell it starts, `virsh` reaches a guest's agent by more than one
command, `mitmproxy` loads add-ons from its settings, and `ghcup` and `n` install what they
fetch. None of them is the usual reason a chat is handed a key, as `ssh` and `psql` are.

## On `NOT_FOR_A_RUN_GRANT`, not `REFUSED_NAMES`

The compilers, shells, interpreters and runtimes run whatever they are told, as
`REFUSED_NAMES`' interpreters do. They are refused on `NOT_FOR_A_RUN_GRANT` all the same
(delegated: recommended path), as #7964's runtimes were: it takes away a run's Allow, which is
what #7988 asks for, and leaves a persona able to declare one in a reviewed line. Moving one to
`REFUSED_NAMES` is its own decision.

## What is refused

- **Terminals**, as `xterm` and `kitty` are: `xfce4-terminal`, `mate-terminal`, `terminator`,
  `tilix`, `foot`, `urxvt`, `rxvt`, suckless `st`, `ghostty`, `lxterminal`, `qterminal`,
  `terminology`, GNOME Console's `kgx`, and `ptyxis` run the command they are handed in the
  window they open; `footclient` has a `foot` server open one, kitty's `kitten` has the running
  kitty launch one, and Debian's `x-terminal-emulator` is whichever terminal the system names.
- **tmux's session runners and fork**: `tmuxinator` and `tmuxp` run the commands of a YAML file
  in the panes they open, and `tmate` is tmux's fork (#7988's comment).
- **VM and container runners** (with #7988's own OrbStack `orb`): `orb`, `orbctl`'s other name,
  runs a command in its Linux machine; Parallels' `prlctl`, UTM's `utmctl`, `tart` and
  VirtualBox's `VBoxManage` (its row is `vboxmanage`, as every form is lowercased) run one in a
  VM's guest; `lxc` and `incus` in a container; `virsh` drives libvirt's guests; `qemu` reaches
  every `qemu-system-*` by its first part, and the user-mode `qemu-<arch>`, which runs a program
  built for another CPU; and `wsl` runs a command in a WSL distribution.
- **Compilers and shells that run code**: `runhaskell`, `ghci`, `kotlinc`, `scalac` (macros and
  compiler plugins), `groovysh`, `clj`, Elixir's `iex`, `erlc` (parse transforms) and `jbang`,
  which runs the Java file it is handed. QuickJS's `qjsc`, `qjs`'s compiler, runs the C compiler
  to build what it makes and runs none of the code it compiles; it is kept with them on the
  conservative side.
- **Interpreters and runtimes** (#7988's comment): babashka's `bb` and `nbb`, `janet`, `fennel`,
  MoonScript's `moon`, Chez Scheme's `chez`, `scheme` and `petite`, CHICKEN's `csi` (the C#
  REPL's name too, which runs C#), Gambit's `gsi`, `chibi-scheme`, `ecl`, `ccl`, `abcl`, `kawa`,
  `lfe`, `jython`, `jruby`, `truffleruby`, `graalpy`, `mruby`, `ocamlrun`, `utop`, `tclkit`,
  `expectk`, F#'s `fsi`, `scriptcs`, `planck` and `lumo`; and `gleam`, `roc`, `odin` and `hare`,
  whose `run` builds and runs a program.
- **Tracers**, as `strace` and `bpftrace` are: SystemTap's `stap` and `heaptrack` run the command
  after them to trace it.
- **Build and version managers and cron siblings**: rebar 2's `rebar`, `nimble`, Crystal's
  `shards`, D's `dub`, `flutter`, `mill`, `ghcup` and tj's `n` run a project's hooks, a build's
  code or the command they are handed; jobber's `jobberrunner` runs a jobfile's commands, and
  bcron's `bcrontab` installs the table `bcron` runs, as `crontab` does.
- **Configuration, proxy and notifier runners**: `pyinfra` runs a deploy file's Python,
  `mitmproxy`, `mitmdump` and `mitmweb` the script their `-s` names, and `terminal-notifier` the
  shell command its `-execute` names when its notification is clicked.

## Left offered

No program #7988 or its comment names is left offered. Programs of the same kinds that #7988
does not name stay offered until they are checked, and are filed below.

## A chance reach, by a row of its own

As the amendments before this one ruled, a row that another family's row reaches only by chance
keeps a row of its own and a unit test. None of #7988's names was reached already. The new rows
reach four rows that were there before: `lxc` reaches `lxc-attach`, `lxc-execute` and
`lxc-init` by their first part, and `rebar` reaches #7964's `rebar3` by its unversioned form.
Each one's `run_grants` test passes through either row, so two unit tests pin the four rows,
each seen red with its rows taken off. Within a family, `scheme` also reaches `chibi-scheme` by its
last part; both are rows of the interpreters' family, so `chibi-scheme` keeps its own row and
needs no pin.

## What the name forms cost

These rows refuse more than they name, through their first and last parts and the rest of
`name_forms`. The extra cost is an Allow and nothing else. A probe of `not_for_a_run_grant` over
the machine's 1,723 program names, before and after the rows went in, found none newly refused:
none of the new rows' names is on that machine. Elsewhere, short and common words cost the most.
These are examples, not a complete list:

- `st`, `n`, `bb`, `orb`, `moon`, `foot`, `tart`, `mill`, `dub`, `roc`, `hare`, `odin`, `csi`,
  `gsi`, `ecl`, `ccl`, `iex`, `clj` and `scheme` reach every `<word>-<name>` and `<name>-<word>`:
  stlink's `st-flash` and `st-info` by `st`, for example.
- the GNU `g` the forms strip reaches every `g<name>` of the one-word rows: GStreamer's
  `gst-launch-1.0`, `gst-inspect-1.0` and the rest lose their Allow by `st`, and Google's `gn`,
  a build generator, by `n`.
- `qemu` reaches QEMU's disk and image tools, `qemu-img`, `qemu-nbd` and `qemu-io`, which run no
  guest; and `lxc` reaches the rest of LXC's `lxc-*` programs, `lxc-ls` and `lxc-info` among
  them, which list and report.
- `qjsc` is refused with its family on the conservative side, though the one program it runs
  is the C compiler.

Which of these to give back, and how to do it without giving back the ones that run code, is
part of #7825.

## Other families

The leads found beside #7988's own list, each still offered, are filed as #8017 rather than added
here: more terminals (`rio`, `contour`, `guake`, `tilda`, `yakuake`, `sakura`, `tabby`, `hyper` and
others), more tmux session managers (`teamocil`, `smug`, `sesh`, `tmuxifier`, and `mux`, a name
tmuxinator is often installed as), more VM and container runners (`virt-customize`, `virt-install`,
`guestfish`, `crictl`, `youki`, `firecracker`, `vfkit`, `krunvm`, `hyperkit`, `xhyve`, `lxd`,
`VBoxHeadless`), more interpreters (`newlisp`, `picolisp`, Gerbil's `gxi`, CHICKEN's `csc`, `murex`,
Ammonite's `amm`, coursier's `cs`) and macOS's `ktrace`.
