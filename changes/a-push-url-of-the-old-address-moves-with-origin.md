### Fixed

- **Use the new address on a moved repo's Notice no longer leaves pushes going to the old
  address.** Where every push URL of `origin` is its old address, with the same scheme and the
  same user, it goes with `origin`, and the Notice says pushes move to the new address too. Where
  one names another repository, or the same one over SSH or as another user, every push URL is
  left as it is, and the Notice says where the repo pushes (#2117).
