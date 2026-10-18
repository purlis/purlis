# Amended 2026-10-18 (#2656, delegated; flagged for the operator): a forge token is read once per process

**The problem it closes.** Section 4 says the sign-in token "is read when a request is sent,
set as the `Authorization` header, and dropped." So every caller went to the keyring itself:
each listing of the accounts searched for each item (`Store::exists`), and each request read
the item's value. On macOS either call can make the Keychain ask the person, and with ad-hoc
signed builds it does after every update (ADR 0047, V90). A start with a few projects made
the window, the session host and one credential helper per fetch ask in turn, and the person
was asked 3–5 times for one item.

**Decision.**

- **Each process reads a forge account's item once, and keeps what it holds in its own memory
  for as long as it runs.** For a browser sign-in that is the access token, the refresh token
  and the expiry; for a pasted or imported token, the token. The store `accounts::keyring_at`
  hands out remembers each value it read or wrote, wiped when it is dropped. It is never
  written anywhere, never logged (each value is still registered with `applog::keep_out`), and
  never handed to another process or a chat. Section 4's rule on who resolves the token is
  unchanged, and a request still holds the token only while it is sent. This replaces "read
  when a request is sent … and dropped" for the keyring read only.
- **One read answers both questions.** Where asking whether an item is there would be a
  Keychain search of its own (macOS), the store reads the item instead and keeps its value, so
  a listing and the request after it ask once between them. Elsewhere it asks nothing for a
  listing, as before.
- **Whatever purlis writes or deletes updates the memory first-hand**: a sign-in, a renewal, a
  sign-out and a sign-in the forge ended. Its lock is held across the store's own call, so a
  read racing a delete never puts the deleted token back. That an item is gone is never
  remembered.
- **Three reads go past the memory:**
  - **A renewal** reads the item again under the refresh lock before it renews. A grant
    another process renewed is then seen, and a spent refresh token is never sent.
  - **A sign-out** reads it again under the same lock, so the grant it revokes is the one that
    is current.
  - **A token the forge refuses (HTTP 401)** is forgotten, and the next request reads the item
    again.
- **git's credential helper asks once.** It lists the accounts from the accounts file and reads
  the token, which says itself whether the item is there, without a separate search first.

**What it does not see.** An item deleted or edited in Keychain Access while a process runs is
used until that process ends, or until the forge refuses it. Revoking the token at the forge
stops it either way. A sign-out in the window reaches the session host through the accounts
file, which every listing reads afresh.

**Not decided here.** A second process (the session host, each credential helper) still reads
the keyring itself, once. Whether one process should own every Keychain read, with the others
asking it over a link, is open for the operator's ruling (#2664). Keychain access that lasts
across updates needs a Developer ID (#2658).
