# Amendment, 2026-10-09: a record made before the item was pinned (#1542)

**Correcting the line above** that a record made before the 2026-10-09 amendment "names no item
and is not held to one". Such a record is now held to an item at its next read, and how depends
on who chose the item the vault reads:

- **This machine chose it** (its half's own `op-item`, or the default `charter-<name>` when no
  half names one): the read writes that item into the record, quietly, and from then on the
  record is held to it like one made since.
- **The committed half chose it** (the item the merged registry names differs from the one this
  machine's half alone gives): the record is not honoured and is never pinned. The read is
  refused with a sentence that points to the vault's tab, where the token is given again on the
  settings shown.

The two are told apart by the item each view reads, never by which keys are present. This keeps
the rule above as written: a committed entry can neither create a record nor change one.
