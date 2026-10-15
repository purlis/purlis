# Amendment, 2026-10-09: a vault's sign-in set-up is held to purlis's windows (#1527)

The guided set-up of a 1Password vault (ADR 0047's amendment of the same date) added six
commands: `vault_setup_accounts`, `vault_setup_begin`, `vault_setup_test`,
`vault_setup_create`, `vault_setup_change` and `vault_setup_cancel`. None answers a secret's
value, so they were first listed as value-free. But they are the other direction of the same
thing: `vault_setup_begin` takes a service-account token from the window, and the others use
the token it holds, or run the 1Password CLI under the person's own desktop-app sign-in. A
dispatcher's ruling for the night of 2026-10-09, flagged to the operator, moves them.

- **They join `vault_values`**, the class whose capability grants only purlis's own windows
  (`main` and `window-[0-9]*`). The class is now "a secret's value, or a vault's sign-in,
  crossing between the window and purlis". `ipc.rs` pins the class's exact list, so a command
  joins it only by amending this record.
- **No link serves them.** They are listed in the session protocol's
  `WINDOW_ONLY_CREDENTIALS`, beside `WINDOW_ONLY`, and `ui::window_only` answers for both: a host
  built with the app's whole command list still refuses them on the UI RPC, and the generated
  `uiRpc.ts` client leaves them out. A credential the person gives a vault reaches purlis
  through the window's own IPC alone.
- **A set-up is held for the window that began it.** Only that window may test, store or
  cancel it, a split window of the same project included. The token it holds goes when that
  window is destroyed or its page reloads, and in any case fifteen minutes after it was given,
  on a timer started then.
