# Amended (2026-11-05, spec #5100, built from #5237; the person's ruling of that day): the person marks a key as no 2FA seed

A plain fill refuses a value that looks like a 2FA seed (#5118), and the look can't be told from
some real values: a 16-letter app password, say. The person can now say a key's value is no seed,
so it fills as a value, on the same notice and grants as any key. A chat can't make the mark, and an
`otpauth` address is refused whatever the mark says.

- **Where the mark is made.** In the window alone, from the key's menu in the vault's tab, after a
  question that says what it lets through. One window command makes it, served on no link. There
  is no CLI verb, session protocol method, chat tool or settings key for it.
- **Where it lives.** `<state>/vaults/-no-seed.json` (`docs/plane-format.md`). Every chat's sandbox
  denies that folder both ways, at every level, so a chat neither writes nor reads it. A settings
  file a chat proposes is never read for a mark.
- **What it is.** A seal of the value under a key derived from the plane's key, which those
  sandboxes deny too, so a row purlis didn't write counts for nothing. The seal covers the value,
  so a new value under the key is unmarked, even one changed outside purlis. A rename keeps the
  mark.
- **An address.** A value holding `otpauth://` or `otpauth-migration://` anywhere, in any case, is
  refused before the mark is read, and can't be marked. A value with text before its address was
  no seed by its look until now, and is refused too.
- **The refusal says where.** It names the vault's tab, the row's menu and the words on the row, for
  the chat to pass on.

*Decided in implementation (D-5237-1 to D-5237-6 in the pull request).*
