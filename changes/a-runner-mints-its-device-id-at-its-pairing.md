### Changed

- A runner mints its device id when it is paired, so two runners started from one image never
  share one. A runner whose home already holds a device id, such as one its image was built
  with, is not paired, and the refusal says why (#2539).
