# Amended (2026-10-15, #2166; extends the 2026-10-15 amendment's "What the ask says rests only on what purlis verified itself"): the team's off is named only from objects checked against their ids

Decided in implementation (#2166), under delegation.

The ask names the commit that turned the sandbox off, and its author, only where that commit is
in the history of the upstream commit purlis's own fetch found
([2026-10-15-03](2026-10-15-03-the-team-bounds-each-person-s-level-and-its-off.md)). "Read as
written" there means checked against the ids themselves, not taken from the clone's store at its
word:

- **The history.** Every commit on the way from that upstream commit down to the off's commit,
  the off's own included, is read back and its bytes hashed. The way is walked only through the
  parents named by bytes that hash to their commit's id; git's list of the commits on the way is
  only a list of candidates.
- **What the ask says of the commit.** Its author is the one its own checked bytes name. Its
  manifest, and its first parent's, are read through the trees from the commit's root tree down
  to the file and through the file itself, each checked against its id: the commit's own turns
  the sandbox off and its parent's does not.
- An object on the way that is missing or reads as anything else, and a repository whose ids are
  not SHA-1, leave the off local: "This project's sandbox off is local: it is not on the upstream
  purlis fetched."

Checked locally, not through the forge's API: it needs no network, no credentials and no forge of
a given kind, and it reads exactly the objects the ask rests on rather than the whole repository.
