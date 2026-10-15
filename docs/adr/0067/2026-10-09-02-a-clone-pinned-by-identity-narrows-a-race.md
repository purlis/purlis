# Noted (2026-10-09, #1415 and #1550): a clone pinned by identity narrows a race, and closes none

This records a limit; it decides nothing new. Brokered git pins a clone's git directory by its
identity, its device and inode (#1415). `gitbroker::checked_repo` takes that identity before it
asks git anything and compares it again when its checks end. Every call in the pinned tree is
refused before git runs if the directory at that path is another one by then. A worktree add
compares it once more after it ran, and fails with a sentence if it changed.

**That narrows the race; it does not close it.** The comparison and git's own open of the path
are two steps. A directory swapped in after a comparison and before git opens the path gets
past it. So does one renamed away and back between two comparisons. Holding a descriptor and
opening beneath it would close the race, and that is the rewrite "ADR 0028, re-opened" above
describes. It is not done one call site at a time.

**The boundary stays the denial of `.git` writes** (section 5), with brokered git reading none of
the person's config. The pin is not counted as a boundary. It makes a replaced git directory
seen in the ordinary case, and a sentence says so.
