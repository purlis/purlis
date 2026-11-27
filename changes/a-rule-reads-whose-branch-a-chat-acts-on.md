### Added

- **A policy rule can read whose branch, port or browser a chat acts on** (M75, #7748). On
  `read`, `write`, `commit`, `push`, `update`, `discard`, `merge`, `reach`, `listen`, `browse`, a
  browser's new `close` and every forge operation, `context.owner` is always present: `"self"`,
  `"task"`, `"asker"`, `"other"` or `"none"`. purlis computes it from records only the app seals,
  so a chat cannot make itself an owner by editing a file: a record it changed or deleted reads as
  `"none"`, no other chat can take its place, and the window shows a notice. It reads `"none"`
  wherever purlis holds no record yet, which today is everywhere: the guards that name what a chat
  acts on come next. **Write `context.owner != "self"`**, never `== "other"`, which does not fire
  on `"none"`. A sandbox rule on `read` or `write` can't name `owner`. `context.made_by` on forge
  operations is now always present too, reading `"none"` until it is filled.
