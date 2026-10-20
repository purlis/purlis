# A runner's device id is compared without case, 2026-10-20

This note adds to *The desktop keeps a runner's key once and removes it whole* (#2525) and
changes none of its rulings. Its item is **decided in implementation, to be confirmed**
(D-2741-2).

1. **Two spellings of one device id differing only in case are one id** (#2741). A device id
   is a ULID, and a ULID reads the same in either case. So `runners::keep` compares device ids
   without regard to case: a kept runner answering its own id in another case, with its own
   key and container, is the runner as it was kept, and a runner answering another kept
   runner's id, or the desktop's own, in another case is refused as the same id would be. The
   row keeps the id as it was first answered. *Rejected:* writing every id in upper case as it
   is kept, which would change what an existing row holds and what the runner was paired with.
   *D-2741-2.*
