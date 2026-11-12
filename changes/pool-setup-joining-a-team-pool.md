### Added

- **Pool setup: join your team's pool on a cluster.** A teammate whose sign-in can't read the
  cluster's install now passes Check the cluster and Install as "Installed by your team", with no
  install buttons, operator image, uninstall or minting commands shown. Where they can't list the
  pools they type the pool's name their admin gave them. A pool's finished Pool step shows the key
  pinned there, for the admin to give teammates.
- **A cluster pool's key is confirmed against the one your admin gave you.** Adding a cluster
  pool, in Pool setup or Settings, asks you to paste the cluster operator key your admin gave you,
  and pins the pool's key only if it is the same; it is shown only then. Only Pool setup on the
  machine that made the signing key in that same setup needs no paste.
- **A changed cluster operator key asks again in Pool setup.** When a cluster pool's Pool names a
  key the pinned one never vouched for, its Pool step shows the pinned key and pins the new one
  only once you paste the key your admin gave you. Your runner stays the one paired.
