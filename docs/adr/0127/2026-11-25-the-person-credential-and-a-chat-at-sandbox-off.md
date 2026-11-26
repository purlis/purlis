# Amendment, 2026-11-25: the credential does not bind a chat at sandbox level `off`, and the state folder `$PURLIS_HOME` names is held whole (#7605)

The decision's sentence that the plane's `fingerprint.key` is one "no chat's sandbox reads" holds
for every chat that has a sandbox. A chat at sandbox level `off` has none: it runs as the person,
reads the key, and can sign a `PersonWhy` ask. The credential keeps out a chat that has a sandbox,
including one that left its tree by double-fork and `setsid`; it does not keep out a chat the
person has already let run as themselves, and no more is claimed.

Three follow-ups of the same confirm review:

- **One root.** `purlis policy why` signs for the root `plane::resolve` names (`$PURLIS_ROOT`
  first) and knocks at that root's socket. A refusal that is still `PERSON_ONLY` says which plane
  and which state folder this command signed for.
- **`$PURLIS_HOME`'s state folder is held whole**, renamed, moved or removed by no chat, as the two
  default spellings are, in the seatbelt and Linux profiles. Claude Code's sandbox holds only what
  is below a path, so it is not handed the entry, for either.
- **The long-path socket fallback** is a file named `hooks.sock`, which the names held at every
  floor stop any chat creating or linking; a test names that path. No expiry is added to the
  credential.
