# Amended 2026-11-28: the terminals, tmux session managers, VM runners, interpreters and tracer of #8017 are refused a Run grant

Decided under the operator's standing order to take the recommended path (#8017), recorded here
as delegated decisions (delegated: recommended path). It follows *the terminals, VM runners,
compilers, interpreters and builders of #7988 are refused a Run grant* (2026-11-28), whose
closing filed these families as #8017.

## Not read in source, but for one, and why each is refused all the same

Of the programs #8017 names, one has a file of that name on the search path of the machine that
did this (`/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`, `/usr/local/bin`, `/opt/homebrew/bin` and
the rest of its `PATH`, 1,752 names in all): macOS's own `/usr/bin/ktrace`. It is a compiled
binary, and its source is not published with macOS, so it was judged by its manual,
`ktrace(1)`: `ktrace trace` and `ktrace artrace` each take `-c command [...]`, which "run[s] the
command specified by command and stop[s] tracing when it exits".

The upstream source of every other one was out of reach: from the sandbox that did it, `gh`
reaches only `purlis/purlis`, and a fetch of any other host was refused. So every program here is
refused whole, on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its manual describes it, and
not on a line of its source. That is the conservative side (delegated: recommended path): a
refusal only takes away an Allow on a run's notice, and a persona can still declare any of them
as a tool in a reviewed line. Reading each in source, to cite its lines or give an Allow back, is
#7825.

Before any row went in, a probe of `not_for_a_run_grant`, on `main` with #8019's rows applied,
found every name #8017 lists still offered: none was refused already by `REFUSED_NAMES`,
`REFUSED_PREFIXES` or another row's form.

## A refusal of the whole tool, not a row of `command_flags`

#8017 names several of these by the one form that runs code: `virt-customize --run-command`,
`crictl exec`, `cs launch` and `ktrace trace` with a command. Each is refused whole all the same
(delegated: recommended path). A `command_flags` row needs a parser written as the tool reads its
own words, from source, and none was in reach; `ktrace`'s macOS copy is a binary. For most, more
forms than the one named run code as well: `virt-customize` also has `--firstboot-command` and
`--run`, `ktrace artrace` takes `-c` as `ktrace trace` does, and `cs` installs and runs what it
fetches. None of them is the usual reason a chat is handed a key, as `ssh` and `psql` are.

## On `NOT_FOR_A_RUN_GRANT`, not `REFUSED_NAMES`

The interpreters run whatever they are told, as `REFUSED_NAMES`' do. They are refused on
`NOT_FOR_A_RUN_GRANT` all the same (delegated: recommended path), as #7964's and #7988's were: it
takes away a run's Allow, which is what #8017 asks for, and leaves a persona able to declare one
in a reviewed line.

## What is refused

- **Terminals**, as `xterm` and `kitty` are: `rio`, `contour`, `cool-retro-term`, `sakura`, the
  drop-down terminals `guake`, `tilda` and `yakuake`, Eterm's `eterm` (its program is `Eterm`;
  every form is lowercased), `aterm`, `mlterm`, `roxterm`, COSMIC's `cosmic-term`,
  `blackbox-terminal`, `deepin-terminal`, `hyper`, `tabby` and `warp-terminal` run the command
  they are handed, or the shell their settings name, in the window they open.
- **tmux session managers**, as `tmuxinator` and `tmuxp` are: `teamocil`, `smug`, `sesh` and
  `tmuxifier` run the commands of a layout file in the panes they open, and `mux` is a name
  `tmuxinator` is often installed or aliased as.
- **VM and container runners**, as `lima`, `lxc` and `virsh` are: libguestfs's `virt-customize`
  and `virt-sysprep` run a command in a guest's disk image and `guestfish` runs one there by its
  `command` and `sh`; `virt-install` installs and boots a guest; `crictl exec` runs a command in a
  container, and the runtime `youki` runs one, as `runc` and `crun` do; `firecracker`, `vfkit`,
  `hyperkit` and `xhyve` boot the guest they are handed, and `krunvm` runs a command in a
  micro-VM; `lxd` is the daemon `lxc` drives; and VirtualBox's `VBoxHeadless` (its row is
  `vboxheadless`) starts a VM. Parallels' `prlsrvctl` sets up the server `prlctl` drives; it is
  kept with them on the conservative side.
- **Interpreters**: `newlisp`, `picolisp`, Gerbil's `gxi` and `gerbil`, CHICKEN's compiler `csc`
  (`csi` is refused; `csc` is also the C# compiler's name, whose analyzers and source generators
  run as it compiles), the shell `murex`, Ammonite's `amm` and `ammonite`, and coursier's `cs`
  and `coursier`, whose `launch` runs what it fetches.
- **A tracer**, as `strace` and `bpftrace` are: macOS's `ktrace`.

## Left offered

No program #8017 names is left offered. Programs of the same kinds that #8017 does not name stay
offered until they are checked, and are filed below.

## A chance reach, by a row of its own

As the amendments before this one ruled, a row that another family's row reaches only by chance
keeps a row of its own and a unit test. A probe of `name_forms` over every row, on `main` with
#8019's rows applied, found none: no new row reaches a row that was there before, none that was
there before reaches a new one, and no new row reaches another new one. So no unit pin is added.

## What the name forms cost

These rows refuse more than they name, through their first and last parts and the rest of
`name_forms`. The extra cost is an Allow and nothing else. A probe of `not_for_a_run_grant` over
the machine's 1,752 program names, before and after the rows went in, found one newly refused,
`ktrace`, a row itself, and nothing reached by a form: no collateral on that machine for #7825.
Elsewhere, short and common words cost the most. These are examples, not a complete list:

- the short rows `cs`, `amm`, `mux`, `rio` and `smug` reach every `<word>-<name>`,
  `<name>-<word>` and `g<name>`: `cs` reaches any `cs-<word>` and `gcs`, and `mux` any
  `<word>-mux`, whatever program each is.
- one name is two programs: `tabby` is also TabbyML's model server, `contour` Project Contour's
  Kubernetes ingress controller, `csc` the C# compiler and `hyper` more than one tool's name.
  Each loses its Allow with the terminal or the interpreter.
- `lxd` reaches LXD's own `lxd-*` programs by their first part, `lxd-benchmark` and
  `lxd-to-incus` among them.

Which of these to give back, and how to do it without giving back the ones that run code, is part
of #7825.

## Other families

The leads found beside #8017's own list, each still offered by a probe of `not_for_a_run_grant`
once these rows went in, are filed as #8031 rather than added here: more terminals
(`uxterm`, `lxterm` and `koi8rxterm`, xterm's wrappers; PuTTY's `pterm`, `termite`, `kmscon`,
`zutty`, `mrxvt`, `wterm`, `waveterm`, `extraterm`), terminal sharers and recorders that run the
command they are handed (`ttyd`, `gotty`, `asciinema rec -c`, `vhs`), more VM and sandbox runners
(libguestfs's `virt-builder` and `virt-rescue`, gVisor's `runsc`, `kata-runtime`,
`cloud-hypervisor`, `bhyve`), Gerbil's compiler `gxc`, `chicken-install` (an egg's setup runs as
it installs), Hy's `hy`, and Helix's `hx`, whose `:sh` runs a shell command. macOS's other tracers,
`dtruss`, `dtrace`, `xctrace` and `leaks`, were refused already.
