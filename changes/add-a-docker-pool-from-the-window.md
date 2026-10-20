### Added

- **Add a Docker pool from the window.** *Add a Docker pool…* in the palette asks for the pool's
  name, its runner image by digest, and its CPUs, memory, processes and `/tmp`, each left empty
  for purlis's default. purlis makes the pool's runner on your Docker, confined and held to those
  limits, and pairs it with this machine. The pools are listed in Settings › This machine ›
  Pools, which appears once one is added, each with Remove. A runner that answers with another
  key than the one it was paired with is said in a Notice with Remove (#2679).
