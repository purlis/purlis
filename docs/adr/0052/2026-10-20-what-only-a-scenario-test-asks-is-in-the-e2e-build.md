# Amendment, 2026-10-20: what only a scenario test asks is in the `e2e` build alone

Some commands exist for the scenario tests and for nothing in the UI. The first is
`native_window_drawn` (#2328, #2760): it reads back the appearance and the colour the platform
draws the window in, so a spec can check that the theme reached the native window. A build
anyone is given has no use for it (#2836).

- **A third list in `ipc_commands.rs`: `e2e_only`.** It is no class of the window's. Its
  commands are compiled, registered and allowed only under the app's `e2e` cargo feature,
  the one the scenario run builds with (`--features e2e`). No shipping build turns that feature
  on; `tools/release-features.mjs` fails a release build line that passes any feature flag.
- **Granted as value-free in the `e2e` build.** `build.rs` adds the list's names to the
  `value-free` set when Cargo builds the feature (`CARGO_FEATURE_E2E`). In every other build no
  permission names them, so no capability can grant them, and the handler does not register
  them.
- **Never in the TypeScript.** `bindings.ts` and `uiRpc.ts` are generated from the other
  builds' commands, and the `e2e` build does not write them at its start. The specs call these
  commands by name. A command moves to this list when nothing in the UI calls it.
- **Still the window's alone.** Each is on the session protocol's `WINDOW_ONLY` list, so a link
  is never served one.
- **The tests.** `ipc.rs` pins the list, checks that none of it is in a class of the window's,
  and checks, in a build without the feature, that no command of it is registered, has a
  permission, appears in the bindings, or passes the real ACL from the main window.
