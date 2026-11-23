### Fixed

- **A chat's push through the forge route to a large repo is no longer refused for taking too long
  to read the base.** Every push fetched the repo's whole history again and a large one passed the
  120-second limit (403, "git could not fetch the base from the forge"). purlis now keeps an
  object cache of its own for each forge repo, in its data folder (so it survives a restart), and
  each push fetches only what changed. The first push to a large repo is refused at once, saying
  purlis is still reading it for the first time, while the cache fills in the background; try the
  push again in a few minutes and it goes through. All the fetches of one push share four minutes,
  so a refusal always reaches the chat, and a fetch that does run out of time says it took longer
  than its limit. A damaged cache is made again, and a repo too large to cache is refused with
  that reason.
