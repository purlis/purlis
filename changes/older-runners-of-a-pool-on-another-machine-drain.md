### Fixed

- **Older runners of a Docker pool on another machine now go once their work is home.** The
  window asked this Mac's own Docker about them, which knew none of them, so their work never
  counted as fetched. A chat recorded without its runner no longer holds back every older runner,
  and a runner kept for a fetch is no longer linked to every few minutes. The window checks a
  pool's older runners all at once, so one slow runner no longer delays the others.
