# Amended 2026-10-30: the Pools group is shown before the first pool

**The person's ruling of 2026-10-30**, after looking for pools in Settings and finding none: "it's
not so good; better to show, because it announces the feature, but with the palette it's
hidden." It amends item 1 of the amendment of 2026-10-20 (a pool is added and removed from the
window), and spec #2041's story 6 as #2679 built it, **for Settings alone**:

1. **Settings › This machine › Pools is always listed**, and so is its row in the palette
   (*Your settings: Pools*). With no pool kept, it says in plain words what a pool is, and its
   *Add a pool…* opens the dialog the palette's row opens. With pools, it lists them, with the
   same *Add a pool…*.
2. **Devices and the other runner surfaces stay as they were**: offered only once this machine
   keeps a pool.
3. **The palette's row and the dialog are named for what they add**: *Add a pool…*, whose
   dialog, *Add a pool*, starts with its Kind, Docker or Cluster (#3292).
4. **A new pool's image defaults to the runner image its release names**, checked as the pools'
   follow after an update checks it (the amendment of 2026-10-20 on following the desktop's
   version): signed by the updater's key over its line, for the version the manifest names,
   purlis's own image by digest. On the stable channel it is read from this version's own
   release, so a machine with an update waiting still gets its own version's image. On the dev
   channel, whose one manifest is always its newest build's, a machine behind it is offered that
   newer version's image, and the dialog says whose it is and that an update of purlis to it is
   waiting. A local build, with no update channel, or a release whose runner image was not
   built, names none and says so under the Image box, which then needs an image by its digest;
   so does an answer that could not be read or was refused. The core still refuses an image not
   named by its digest.
