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
4. **A new pool's image defaults to the runner image this version's release names**, the image
   the pools already follow after an update (the amendment of 2026-10-20 on following the
   desktop's version), read the same way. A build that names none, a development build or a
   release whose runner image was not built, says so under the Image box, which then needs an
   image by its digest. The core still refuses an image not named by its digest.
