# Amendment, 2026-11-24: the sandbox level's reading, the escape denies as a step of their own, and the plane root held against rename (M75 slice 11, #7020)

Decision 6 makes `[sandbox]` a clean cut and the 2026-11-16 amendment to ADR 0125 makes
`[policy] sandbox` the level's one key. Slice 11 builds both, and settles the points the spec
marked **proposed**:

- **`strict` is the floor where `[policy] sandbox` names no level.** The Q308 code answered
  `Template::Strict => None` and left the level to `[sandbox] mode`; with that table gone, `strict`
  has a level of its own, the floor (Q318 point 9). `[policy] sandbox` sets the level apart from
  any template, as `[sandbox] mode` did.
- **The key takes a level, never a mode.** `off`, `floor` and `full` read; `log`, `enforce` and
  any other word break the file, and a broken file runs the full sandbox (decision 7).
- **The `read` and `write` forbids' mode follows the level and the template.** Enforced at
  `floor` and `full` (so `watch`, which runs at the floor, enforces them); at `off`, off under
  `open` and `log` under any other template (Q320b R2-2).
- **`[sandbox] policy = "never"` reads as a moved key**: a Project lock on the `sandbox` family,
  which freezes its minimum level at `full` where nothing says otherwise. A lower level may raise
  the level, never lower it. `[sandbox] mode` reads as the level it names, `"off"` or `full`
  (`never` refuses the `off`), and a `[policy] sandbox` beside either wins. Each says so in a
  Notice naming the new spelling. Your own file's `[sandbox]` is not read as a tightening.
- **The escape denies are a step of their own** (spec §3.5): the floor's process denies and the
  session-bus and mach-service `dial`s apply whenever the level is `floor` or `full`, take the
  level and nothing else, and are lifted by no family's mode, lock or rule (`watch` never logs
  them, `processes = "off"` never lifts one). **A rule that names one is refused at load**: an
  `exec` on one of those `Command`s or a `dial` on one of those `Socket`s, permit or forbid.
- **At the level `off`, `network`, `ports` and `processes` are not in force.** The core says so
  (`Loaded::not_in_force_for`); Settings' and doctor's rows read it in a later change.
- **The plane root's own folder entry is held against rename** at `floor` and `full`, in the
  list a start compiles the sandbox from, so a chat cannot `mv` its plane away and link the old
  path to it. Where a sandbox cannot hold a rename, or at `off`, it is a declared gap (slice 37).

What this slice leaves in place, for the slices that follow: the legacy readers of `[sandbox]`
in `sandbox::team` and `enforcement::Settings` still run beside the new reading until slice 13
and slice 22 retire them, and the app records `context.sandbox` as `floor` for a sandboxed chat
until the host records the level a start applied.
