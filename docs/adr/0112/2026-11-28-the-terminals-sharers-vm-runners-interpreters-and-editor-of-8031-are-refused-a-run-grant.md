# Amended 2026-11-28: the terminals, sharers, VM runners, interpreters and editor of #8031 are refused a Run grant

Decided under the operator's standing order to take the recommended path (#8031), recorded here
as delegated decisions (delegated: recommended path). It follows the amendment for #8017
(#8033, open when this was written), whose closing filed these families as #8031. Nothing here
depends on #8033's rows: each comparison below names a row that was on `main` before either.

## Not read in source, and why each is refused all the same

None of the programs #8031 names has a file of that name on the search path of the machine that
did this (`/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`, `/usr/local/bin`, `/opt/homebrew/bin`,
`~/.cargo/bin` and the rest of its 1,750 program names), no macOS copy of any of them was found,
and none of them is in the machine's Cargo registry (asciinema 3 and Helix are Rust). Their
upstream source was out of reach: from the sandbox that did it, `gh` reaches only
`purlis/purlis`, and a plain fetch of any other host is refused. So every one is refused whole,
on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its manual describes it, and not on a line
of its source. That is the conservative side (delegated: recommended path): a refusal only takes
away an Allow on a run's notice, and a persona can still declare any of them as a tool in a
reviewed line. Reading each in source, to cite its lines or give an Allow back, is #7825.

Before any row went in, a probe of `not_for_a_run_grant` (a port of `name_forms` over
`NOT_FOR_A_RUN_GRANT`, `REFUSED_NAMES` and `REFUSED_PREFIXES`, with #8033's rows applied) found
every name #8031 lists still offered: none was refused already by another row's form. macOS's
other tracers, `dtruss`, `dtrace`, `xctrace` and `leaks`, are refused already, as #8031 says.

## A refusal of the whole tool, not a row of `command_flags`

#8031 names some of these by the one form that runs code: `asciinema rec -c`, `virt-builder
--run-command` and Helix's `:sh`. Each is refused whole all the same (delegated: recommended
path). A `command_flags` row needs a parser written as the tool reads its own words, and none of
their sources was in reach to write one from. For each, more than the named form runs code:
`asciinema rec` without `-c` starts the shell its settings name, `virt-builder` also runs the
scripts its `--firstboot` and `--run` name, and Helix runs a shell command by `:pipe`,
`:insert-output` and its key bindings as well as by `:sh`.

## On `NOT_FOR_A_RUN_GRANT`, not `REFUSED_NAMES`

`gxc`, `chicken-install` and `hy` run code they are handed, as `REFUSED_NAMES`' interpreters do.
They are refused on `NOT_FOR_A_RUN_GRANT` all the same (delegated: recommended path), as #7964's
runtimes were: it takes away a run's Allow, which is what #8031 asks for, and leaves a persona
able to declare one in a reviewed line. Moving one to `REFUSED_NAMES` is its own decision.

## What is refused

- **Terminals**, as `xterm` and `kitty` are: xterm's wrappers `uxterm`, `lxterm` and
  `koi8rxterm` `exec` xterm with what they are handed; PuTTY's `pterm`, `termite`, `kmscon` (a
  terminal on the kernel's console), `zutty`, `mrxvt` and `wterm` run the command they are handed
  in the window they open; and `waveterm` and `extraterm` run the commands their blocks and
  sessions are handed. No form of the wrappers reaches the `xterm` row: `name_forms` splits only
  on `-`, and none of them has one.
- **Terminal sharers and recorders**: `ttyd` and `gotty` run the command after them and serve
  its terminal over HTTP; `asciinema rec -c` runs the command it records; and `vhs` runs the
  commands of a tape file.
- **VM and sandbox runners**, as `lima` and `multipass` are: libguestfs's
  `virt-builder` runs the command its `--run-command` names in the image it builds, and
  `virt-rescue` opens a shell in a guest's disk; gVisor's `runsc` and `kata-runtime` are OCI
  runtimes, which run a container's process as `runc` does; and `cloud-hypervisor` and FreeBSD's
  `bhyve` boot the guest they are handed.
- **Interpreters and installers**: Gerbil's compiler `gxc` runs a module's macros as it compiles
  it, CHICKEN's `chicken-install` runs an egg's setup as it installs it, and Hy's `hy` runs Lisp
  as Python.
- **An editor with a shell**, as `vim` and `emacs` are: Helix's `hx`, and `helix`, the name some
  systems install it as (delegated: recommended path; #8031 names `hx` only, and the same program
  under its other name would be offered without it).

## Left offered

No program #8031 names is left offered. Programs of the same kinds that #8031 does not name stay
offered until they are checked, and are filed below.

## A chance reach, by a row of its own

As the amendments before this one ruled, a row that another family's row reaches only by chance
keeps a row of its own and a unit test. None of #8031's names was reached already, none of the
new rows reaches a row that was there before (#8033's among them), and none reaches another new
row, so no unit test pins one.

## What the name forms cost

These rows refuse more than they name, through their first and last parts and the rest of
`name_forms`. The extra cost is an Allow and nothing else. A probe of `not_for_a_run_grant` over
the machine's 1,750 program names, before and after the rows went in, found none newly refused,
so #7825 has nothing from this machine to add. Elsewhere, the short rows cost the most. These are
examples, not a complete list:

- Every one-word row (`ttyd`, `gotty`, `runsc`, `bhyve`, `pterm`, `wterm`, `zutty`, `gxc` and
  the rest) reaches every `<word>-<name>` and `<name>-<word>`, and every `g<name>` and
  `gnu<name>`, as the forms strip a GNU prefix from the name they check, never from a row. The
  short ones, `hy`, `hx` and `vhs`, are the likeliest to meet another program's name this way.
- `hx` is also the name of a hex viewer, which runs nothing it is handed, and is refused with
  Helix by its name.
- A row with a dash in it (`virt-builder`, `kata-runtime`, `chicken-install`) is reached only by
  its own name, its stem before a `.`, its versions, and its name with a `g` or `gnu` in front.

Which of these to give back, and how to do it without giving back the ones that run code, is
part of #7825.

## Other families

The leads found beside #8031's own list, each still offered, are filed as #8037 rather than added
here: editors with a shell or tasks (`kak`, `micro`, `vis`, `mg`, `jed`, `joe`, `lite-xl`,
`pulsar`, `neovide`, `mvim`, `code`, `codium`, `subl`, `zed`, `lapce`), more terminal sharers
(`upterm`, `sshx`, `tty-share`, `teleconsole`, `wetty`) and recorders (`ttyrec`, `termtosvg`,
`terminalizer`, `t-rec`), libguestfs's `virt-v2v`, and more interpreters and shells (Gambit's
`gsc`, `hissp`, `ion`, `oil`).
