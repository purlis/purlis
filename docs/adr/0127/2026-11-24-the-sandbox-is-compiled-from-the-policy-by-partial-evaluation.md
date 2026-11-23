# Amendment, 2026-11-24: the sandbox is compiled from the policy by Cedar's partial evaluation, and a sandbox rule stays inside a subset (M75-22, #7031)

Spec §11's choices were proposed, with no ruling behind them. Slice 22 builds them, and records
what building them found.

- **One function compiles `read` and `write` per chat at start** with `PolicySet::tpe`: the chat
  and the start's context are known, the resource is an unknown `Path`. The pin is
  `cedar-policy =4.14.1` with `tpe`. A rule folds to `true`, `false`, an error or a residual.
- **The subset.** A residual is `true`, `false`, `resource in E`, `resource == E`, a
  disjunction of those (`E` a Path, Workspace, Repo, Branch or a named `PathSet`), or the
  guarded cross-workspace form. Partial evaluation writes `a != b` as `!(a == b)`, so the
  compile reads both. Anything else is refused at the chat's start, naming the rule.
- **Beyond §11.2's list:** the guarded `resource has workspace && resource.workspace ==
  principal.workspace` is admitted too, compiled as `resource in Workspace::"…"`. A project's
  permit for its own workspace's files is written that way (the `regulated` sample project), and
  a read allow-list under deny-by-default cannot be written without it. A named `PathSet` is
  admitted as `E` for the same reason: the shipped default's `sandbox-secret-stores` names one.
- **Refused at load:** a rule on a sandbox action (one that names `read`, `write`, the
  `sandbox` family, or no action) that names `context.attended` or `context.harness_mode` or uses
  `like`. A fixed profile cannot follow them, and a hook fallback would leave shell commands
  unguarded. This check reads the rule's text and runs on every load; the residual subset is
  checked at the start, and by `Policy::sandbox_problems` before one.
- **`@ask` on a Path** compiles to a deny in the profile plus a Block, which the person's Allow
  lifts.
- **One path resolver** (`policy::resolver`) serves the compile and the hook. A `Path`'s
  ancestors are every folder above it (the root included), its owning workspace, the repo whose
  clone holds it, the branch folder it is in, and the named sets that list it or a folder above
  it. It reads no file and runs nothing.
- **The start event records the compiled rule set**: each rule's `@id` and the shape of its terms
  (`always`, `in workspace billing`, `in path`), never a path.
- **Reads** are an allow-list under deny-by-default on Seatbelt and Landlock. Claude Code's
  sandbox takes deny paths only, so its reads are "everything but the forbids", a declared gap
  that slice 37 lists.
