# Amendment, 2026-09-24: the token move hardened after the #271 review

An adversarial review of the token move (#271) found several ways a chat could still reach a
moved token, or redirect it. The move is kept, with these changes. Each is covered by a
regression test that began as a proof the exploit worked.

- **purlis runs only the `op` it pinned.** A keyring-held identity is handed to the absolute
  `op` resolved from the operator's own PATH when the token was stored, and to that binary's
  code-signing Team identifier, both recorded in the local half. A read verifies the path and the
  team and refuses on a mismatch; it never resolves `op` from the caller's PATH. A chat that drops
  its own `op` on `$PATH` therefore cannot receive the token. The Team id recorded is whatever
  `codesign` reports at store time (trust on first use); AgileBits' published id is named in the
  code for an operator to confirm a fresh install against, not hard-checked. Without new `unsafe`:
  the check shells out to `/usr/bin/codesign`.
- **The keyring item is random per (vault, plane), and the binding is pinned.** The item is
  `charter/@identity/<random id>`, the id stored in the local half — so a chat cannot name another
  plane's item, and two planes that bind the same variable never share one. The local record also
  fixes the `env` map, the op-vault and the account the move was made against; the mark is honoured
  only while the vault's effective binding still equals them. A committed `vaults.json` that changes
  any of them unpins the mark rather than redirecting the token, which closes the earlier hole
  where a commit could pick the keyring item.
- **The whole record is local-only.** `identity` joins `account` in `LOCAL_ONLY_KEYS`, and it is
  read from the local half alone, so a committed entry can neither create a record nor change one.
- **A keys index may name only this vault's own service.** `charter/<vault>/<id>`, checked against
  the vault name — never `charter/@identity/…` or another vault's service — so an index cannot
  point a keyring vault at the identity item and read the token out as a secret.
- **The preferred move is a password box.** The vault tab lets the operator paste the token
  straight into the keyring, so it never enters the app's own environment. The move-from-environment
  path stays for convenience but the tab warns to relaunch, because a same-user process can read
  the app's environment block while the export is still in it.
- **The chat strip is by every declared name, case-insensitively.** No chat is given any `OP_*`
  variable (matched ignoring case) or any identity source a vault declares
  (`registry::identity_vars`), so a vault bound to `PROD_1P_TOKEN` leaks it to no chat either.

**A defence-in-depth gap is left open, tracked separately:** the app has no Tauri command
allow-list, so only the webview CSP stands between a future cross-site-scripting bug and the
reveal/copy commands. Filed as a follow-up. *Closed by ADR 0052 (purlis#276): every app
command is on an allow-list, and reveal and copy are granted to the main window only, by a
capability of their own.*
