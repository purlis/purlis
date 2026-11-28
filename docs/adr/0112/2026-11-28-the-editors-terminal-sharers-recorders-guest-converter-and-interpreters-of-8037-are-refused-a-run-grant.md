# Amended 2026-11-28: the editors, terminal sharers and recorders, guest's converter and interpreters of #8037 are refused a Run grant

Decided under the operator's standing order to take the recommended path (#8037), recorded here
as delegated decisions (delegated: recommended path). It follows *the terminals, sharers, VM
runners, interpreters and editor of #8031 are refused a Run grant* (2026-11-28, #8038), whose
closing filed these families as #8037. Each comparison below names a row that was on `main`
before either, or one of #8038's.

## Not read in source, and why each is refused all the same

Three of the programs #8037 names have a file of that name on the search path of the machine that
did this (`/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`, `/usr/local/bin`, `/opt/homebrew/bin`,
`~/.cargo/bin` and the rest of its `PATH`, 1,750 names in all): macOS's own `/usr/bin/mg` and
`/usr/bin/vis`, and `/usr/local/bin/zed`, a link to the CLI inside `Zed.app`. All three are compiled
binaries, so each was judged by what it says of itself: `mg(1)` and `vis(1)` on that machine, and
`zed --help`. None of the others is on that machine or in its Cargo registry (Zed, Lapce, Neovide,
`ion`, `t-rec` and `sshx` are Rust). Their upstream source was out of reach: from the sandbox that
did it, `gh` reaches only `purlis/purlis`, and a plain fetch of any other host is refused. So every
one is refused whole, on `NOT_FOR_A_RUN_GRANT`, on what the tool is for, as its manual describes it,
and not on a line of its source. That is the conservative side (delegated: recommended path): a
refusal only takes away an Allow on a run's notice, and a persona can still declare any of them as a
tool in a reviewed line. Reading each in source, to cite its lines or give an Allow back, is #7825.

Before any row went in, a probe of `not_for_a_run_grant` (a unit test over `name_forms`,
`NOT_FOR_A_RUN_GRANT`, `REFUSED_NAMES` and `REFUSED_PREFIXES`, with #8038's rows applied) found
every name #8037 lists still offered: none was refused already by another row's form.

## What a refusal by name costs, for the common words

#8037 asks that `code` and `mg`, common words, and its short rows be weighed before they are
refused by those names. Three questions were put to each: whether the name is a word other tools
use, whether it is on the machine that did this, and whether a narrower `command_flags` row could
be proved from source in reach. One fact answers the first part for every editor here: a Run
grant is a run handed a vault's key, with no stdin and a home of its own. An editor's everyday use,
editing a file at a keyboard, never happens inside one, so taking the Allow away breaks no
everyday use of the editor. What it can cost is another program that shares the name.

- **`code`** (delegated: recommended path: refused whole). A common word, but no other program
  of that name was found, and none is on the machine. Visual Studio Code's `code` opens a folder
  whose tasks, integrated terminal and extensions run code, and `code tunnel` opens the machine
  to a VS Code elsewhere. No narrower row: its macOS CLI is a script inside the app, and the app
  is not on the machine. Its forms reach `code-<word>`: VS Code's own `code-insiders` and
  `code-oss`, and Coder's `code-server`, which serves VS Code with its terminal in a browser.
  They also reach `<word>-code`, `gcode`, `gnucode` and `code2`.
- **`mg`** (delegated: recommended path: refused whole). Short rather than a word, and no
  other program of that name is known. On the machine, as Apple ships it.
  `mg(1)` there lists `shell-command` (`M-!`) and `shell-command-on-region` (`M-|`), and a `-b
  file` that "execute[s] the mg commands found in the specified file", and `-u file`, a startup
  file in place of `~/.mg`. A row refusing `-b` and `-u` might be enough, since a granted run has
  no stdin to type `M-!` into, but proving that takes mg's source, and Apple's copy is a binary
  with OpenBSD's source out of reach. It loses its Allow on every Mac.
- **`micro`** (delegated: recommended path: refused whole). A common word, and also the name of
  the Micro platform's command line, which runs the services it is handed. Neither is on the
  machine. The editor's `run` runs a shell command and its plugins are Lua. No source in reach.
  Its forms reach `micro-<word>`, `<word>-micro` and `gmicro`.
- **`zed`** (delegated: recommended path: refused whole). On the machine. Its `--help` there
  offers `--zed <ZED>`, a "[c]ustom path to Zed.app or the zed binary", which the CLI starts, so it
  runs the program it is handed, and `--dev-container`, which opens the project in a container.
  The editor also has tasks and a terminal. ZFS's event daemon is also called `zed`, and runs the
  scripts in its folder, and so is Brim's `zed` data tool. Neither is on the machine, and both
  are refused by the same row. No source in
  reach. Its forms reach `zed-<word>`, `<word>-zed` and `gzed`.
- **`subl`** (delegated: recommended path: refused whole). Not a common word, and not on the
  machine. Sublime Text's build systems run commands, its plugins are Python, and `subl
  --command` runs one of its commands. No source in reach.
- **`vis`** (delegated: recommended path: refused whole). On the machine, but as a different
  program: macOS's `/usr/bin/vis` is BSD's filter that "display[s] non-printable characters in a
  visual format", by its `vis(1)`, and runs nothing. The editor `vis` runs a shell command by `:!`
  and filters through one by `:|`. A row is a name, and cannot tell the two apart, so the filter
  loses its Allow with the editor's row. No narrower row: no source of either is in reach, and
  `vis(1)` is the filter's. That costs little: a filter of bytes is never the reason
  a run is handed a key. Giving it back by where it is found, and not by its name, is #7825's to
  decide. Its forms reach the editor's own `vis-<word>` helpers, and `gvis`.
- **The other short rows**, `kak`, `jed`, `joe`, `gsc`, `ion` and `oil` (delegated: recommended
  path: refused whole): none is on the machine, and no source is in reach. Three share a name
  with something else: `joe` is also a generator of `.gitignore` files, the Ion window manager's
  `ion3` reaches the `ion` row by its versioned form, and `oil` is an everyday word. Each of those
  loses its Allow with the row. Each row reaches its `<name>-<word>`, `<word>-<name>`,
  `g<name>`, `gnu<name>` and versioned forms. Some of those run code too, such as `kak-lsp`,
  which starts the language servers its settings name, and `jed-script`, which runs S-Lang, so
  refusing them is no cost.

No name here was left offered (delegated: recommended path): for none of them would a refusal break
common everyday use, and each runs a command or code it is handed, by its manual.

## A refusal of the whole tool, not a row of `command_flags`

#8037 names some of these by the one form that runs code: `micro`'s `run`, `vis`'s `:!`,
`virt-v2v --run` and `gsc -e`. Each is refused whole all the same (delegated: recommended path).
A `command_flags` row needs a parser written as the tool reads its own words, and none of their
sources was in reach to write one from (`mg`, above, is the nearest). For each, more than the
named form runs code: `micro` and `vis` run plugins in Lua, `virt-v2v` also takes the first-boot
scripts `virt-customize` does, and `gsc` runs a file's macros as it compiles it.

## On `NOT_FOR_A_RUN_GRANT`, not `REFUSED_NAMES`

`gsc`, `hissp`, `lissp`, `ion` and `oil` run the code they are handed, as `REFUSED_NAMES`'
interpreters and shells do. They are refused on `NOT_FOR_A_RUN_GRANT` all the same (delegated:
recommended path), as #7964's, #7988's and #8017's were: it takes away a run's Allow, which is
what #8037 asks for, and leaves a persona able to declare one in a reviewed line. Moving one to
`REFUSED_NAMES` is its own decision.

## What is refused

- **Editors with a shell or tasks**, as `vim`, `emacs` and #8038's Helix are: Kakoune's `kak`
  runs the shell of its `%sh{}` blocks and the commands its `-e` names; `micro` runs a shell
  command by its `run` and plugins in Lua; `vis` runs one by `:!` and filters through one by
  `:|`; `mg` runs one by
  `shell-command`, and the mg commands of the file its `-b` names; `jed` and `joe` run shell
  commands of their own; `lite-xl` and `pulsar` run plugins in Lua and JavaScript; `neovide` and
  MacVim's `mvim` start `nvim` and `vim`; and `code`, VSCodium's `codium`, Sublime Text's
  `subl`, `zed` and `lapce` each have an integrated terminal, tasks or build systems, and
  extensions that run as they open a folder. `zed`'s `--zed` also names the program it starts.
- **Terminal sharers**: `upterm`, `sshx`, `tty-share`, `teleconsole` and `wetty` run a shell or
  the command they are handed and share its terminal with whoever joins, as #8038's `ttyd` and
  `gotty` do.
- **Terminal recorders**, as #8038's `asciinema` is: `ttyrec`, `termtosvg`, `terminalizer` and
  `t-rec` run the shell or the command they record.
- **A guest's converter**, as `virt-customize` is: libguestfs's `virt-v2v` runs the script its
  `--run` names in the guest it converts.
- **Interpreters and shells**: Gambit's compiler `gsc`, whose `-e` runs code as `gsi`'s does;
  Hissp, whose command line its documentation names `lissp` (delegated: recommended path: #8037
  names `hissp`, the package's name, which is refused too in case a system installs it so, and
  the program by its own name would be offered without its row); and the shells `ion` and
  `oil`. Oils' current names, `ysh` and `oils-for-unix`, are left to #8047; its `osh` is refused
  already.

## Left offered

No program #8037 names is left offered. Programs of the same kinds that #8037 does not name stay
offered until they are checked, and are filed below.

## A chance reach, by a row of its own

As the amendments before this one ruled, a row that another family's row reaches only by chance
keeps a row of its own and a unit test. None of #8037's names was reached already, none of the
new rows reaches a row that was there before (#8038's among them), and none reaches another new
row, so no unit test pins one.

## What the name forms cost

These rows refuse more than they name, through their first and last parts and the rest of
`name_forms` (delegated: recommended path). The extra cost is an Allow and nothing else. A probe of
`not_for_a_run_grant` over the machine's 1,750 program names, before and after the rows went in,
found three newly refused, `mg`, `vis` and `zed`, each a row itself, and nothing reached by a form.
So #7825 has nothing from this machine to add beyond macOS's `vis` filter, weighed above. Elsewhere,
the short rows cost the most. These are examples, not a complete list:

- Every one-word row reaches every `<word>-<name>` and `<name>-<word>`, every `g<name>` and
  `gnu<name>`, as the forms strip a GNU prefix from the name they check, never from a row, and
  its versioned forms. The common words and short rows weighed above (`code`, `mg`, `micro`,
  `vis`, `zed`, `kak`, `jed`, `joe`, `gsc`, `ion`, `oil`) are the likeliest to meet another
  program's name this way.
- A name that is two programs loses its Allow for both: macOS's `vis` filter, ZFS's `zed`
  daemon and Brim's `zed`, the `.gitignore` generator `joe`, and the Ion window manager's
  `ion3`.
- A row with a dash in it (`lite-xl`, `tty-share`, `t-rec`, `virt-v2v`) is reached only by its
  own name, its stem before a `.`, its versions, and its name with a `g` or `gnu` in front.

Which of these to give back, and how to do it without giving back the ones that run code, is
part of #7825.

## Other families

The leads found beside #8037's own list, each still offered by the same probe once these rows
went in, are filed as #8047 rather than added here: more editors with a shell, an eval or tasks
(`emacsclient`, whose `--eval` runs Lisp, `nvi`, `elvis`, `zile`, `qemacs`, `ne`, `dte`, `kate`,
`geany`, `textadept`, `cursor`, `windsurf`, `idea`, `fleet`, `nova`, `bbedit`, `mate`, `vimr`,
`goneovim`), JOE's other names (`jmacs`, `jstar`, `jpico`, `rjoe`), Pulsar's `ppm`, file managers
with a shell (`mc`, `mcedit`, `ranger`, `nnn`, `lf`, `yazi`, `vifm`), more shells (`ysh`,
`oils-for-unix`, `hilbish`, `ngs`), and libguestfs's `virt-v2v-in-place`, which takes `--run` as
`virt-v2v` does and which no form of the `virt-v2v` row reaches.

The review of #8049 found more of the same kinds still offered, and they are filed on #8047 too:
interpreters (`ipython`, `ipython3`, `jupyter`, `bpython`, `ptpython`, `ptipython`, `pry`,
`octave`, `sage`, `maxima`, `swipl`, `gforth`, `pike`, `regina`, and Hy's `hyc` and `hy2py`),
editors (`zeditor`, `sublime_text`, `sublime`, `openvscode-server`, `atom`, `apm`, `lite`,
`pragtical`, `kakoune`, `xjed`, `jove`, `mle`, `uemacs`, `xemacs`, `acme`, `sam`, `ecode`,
`positron`, `void`, `trae`, `kiro`), terminal sharers and recorders (`shellinaboxd`, `xpra`,
`svg-term`, `byzanz-record`, `ttystudio`), libguestfs's `virt-dib`, and the shells `loksh`,
`posh` and `scsh`.
