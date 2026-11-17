### Security

- A project's `[policy]` is read from the plane root's last commit as it was written: a replace
  ref, a graft or a work tree named in the repository's own config no longer changes what is read,
  and every object read is hashed and checked against its name, so a file rewritten or removed
  inside `.git` reads as strict, never as another text (M75, #7013).
