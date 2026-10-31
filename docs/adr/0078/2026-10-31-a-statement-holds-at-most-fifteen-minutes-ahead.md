# Amended 2026-10-31: a statement holds at most fifteen minutes ahead

Amends the line of the 2026-10-28 amendment that says how far ahead a desktop reads a cluster
operator's statement. *Decided in implementation* (D-links-3 in its pull request), accepted by the
program's steward.

1. **Fifteen minutes.** An operator signs a statement for ten minutes, and a desktop allows five
   more for its clock running behind the cluster's. A statement that claims to hold further ahead
   is refused on every link, in words that say to set the clock right. A key roll's statement keeps
   its own limit, seven days and an hour.
2. **A browser login allowed on a cluster runner is bound to its pod's uid too.** A kept claim's
   pod made again asks again for that login. The pairing stays the claim's, as ruled on 2026-10-28:
   nothing is paired again, and the link is not refused.
