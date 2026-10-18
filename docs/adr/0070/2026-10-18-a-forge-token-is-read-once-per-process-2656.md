# Amended 2026-10-18 (#2656, delegated; flagged for the operator): a forge token is read once per process

**The problem it closes.** Section 4 says the sign-in token "is read when a request is sent,
set as the `Authorization` header, and dropped." So every caller went to the keyring itself:
each listing of the accounts searched for each item (`Store::exists`), and each request read
the item's value. On macOS either call can make the Keychain ask the person, and with ad-hoc
signed builds it does after every update (ADR 0047, V90). A start with a few projects made
the window, the session host and one credential helper per fetch ask in turn, and the person
was asked 3–5 times for one item.

**Decision.**

- **Each process reads a forge account's item once, and keeps it in its own memory for as long
  as it runs.** The store `accounts::keyring_at` hands out remembers, per process, the value it
  read or wrote and that an item is there. The value is wiped when it is dropped. It is never
  written anywhere, never logged (it is still registered with `applog::keep_out`), and never
  handed to another process or a chat: section 4's rule on who resolves the token is unchanged,
  and a request still holds the token only while it is sent. This replaces "read when a request
  is sent … and dropped" for the keyring read only.
- **Whatever purlis writes or deletes updates the memory first-hand:** a sign-in, a renewal, a
  sign-out and an ending the forge decided. That an item is gone is never remembered, so a
  search or read that finds nothing is asked again.
- **A renewal reads afresh.** Before a browser sign-in's token is renewed, the remembered pair is
  forgotten and the item read again under the refresh lock. Another process that renewed it
  first is then seen, and a spent refresh token is never sent.
- **git's credential helper asks once.** It lists the accounts from the accounts file and reads
  the token, which says itself whether the item is there, without a separate search first.

**What it does not see.** An item deleted or edited in Keychain Access while a process runs is
used until that process ends. The forge still checks the token, so revoking it at the forge
stops it either way. A sign-out in the window reaches the session host through the accounts
file, which every listing reads afresh.

**Not decided here.** A second process (the session host, each credential helper) still reads
the keyring itself, once. Having one process own every Keychain read, with the others asking it
over the link, is left open (#2656). A build that keeps its Keychain access across updates
needs a Developer ID (#2658).
