### Fixed

- **Use the new address on a moved repo's Notice no longer leaves pushes going to the old
  address.** Where every push URL of `origin` names the old place, it goes with `origin`, so
  pushes go where the repo now fetches from. Where one names another place, every push URL is
  left as it is, and the Notice says where the repo pushes (#2117).
