### Added

- **A pool on another machine stops and asks when its machine changes.** When the machine's SSH
  host key, the CA its TLS files hold or its Docker engine is no longer the one you trusted, the
  pool starts no runner, opens no new link and sends no vault value there, and links already
  open stay open. A Notice names the pool, what changed and both fingerprints, with Check and
  trust again, which shows what the machine shows now and keeps only what you confirm, and
  Remove pool…, which forgets the pool here without contacting that machine. After you trust a
  new engine, Make the runner again makes the pool's runner on it. `purlis pool doctor <pool>`
  says the same and exits 1.
