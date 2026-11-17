### Security

- A project's `[policy]` is read from the plane root's last commit as it was written: a replace
  ref, a graft or a work tree named in the repository's own config no longer changes what is read
  (M75, #7013).
