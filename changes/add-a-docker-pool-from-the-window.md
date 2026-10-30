### Added

- **Add a pool from the window.** *Add a pool…*, in the palette and in Settings › This machine ›
  Pools, asks for the pool's name, its runner image by digest, and its CPUs, memory, processes
  and `/tmp`, each left empty for purlis's default. The image's default is the runner image this
  version of purlis was released with. On the dev channel, where purlis is often behind the
  newest build, it is that build's image, and the dialog says an update of purlis to it is
  waiting. A local build names none, and says so. purlis makes the pool's runner on your Docker,
  confined and held to those limits, and pairs it with this machine. Settings › This machine ›
  Pools is always there: before the first pool it says what a pool is, and then it lists each
  pool with Remove. A runner that answers with another key than the one it was paired with is
  said in a Notice with Remove (#2679).
