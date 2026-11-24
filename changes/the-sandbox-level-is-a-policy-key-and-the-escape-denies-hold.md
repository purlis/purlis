### Changed

- **The sandbox level is `[policy] sandbox`.** `off`, `floor` or `full` sets how much of a chat
  is wrapped (`strict` is the floor where the key says none), and `log` or `enforce` there is a
  broken file, which runs the full sandbox. `[sandbox] mode` and `[sandbox] policy = "never"` are
  read as moved keys, each with a Notice that names the new spelling: `never` is a lock on the
  sandbox that a workspace or persona can raise but not lower, and it is the full sandbox whatever
  `[policy] sandbox` says. A workspace's or persona's `sandbox =`, a lock's minimum and the
  Organisation's level are read by the policy, and reach a chat's start when the host loads the
  levels, in a following change (M75-11, #7020).

### Security

- **A chat can never start `launchctl`, `osascript` or `open`, or reach the session bus, at the
  floor or the full sandbox**, and no service grant can lift the Launch Services or Apple events
  half. These holds belong to the sandbox level, not to a family's mode, so
  `processes = "off"` and the `watch` template no longer matter to them, and a rule that names one
  is refused when the policy loads (M75-11, #7020).
- **A sandboxed chat cannot rename its plane root.** At the floor and the full sandbox the plane's
  own folder is held where the sandbox can hold it, so a chat cannot move its plane away and link
  the old path to it (M75-11, #7020).
