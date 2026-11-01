### Changed

- **A cluster runner's pod now has limits**: 4Gi of memory and, for its disk, its home's size and
  2Gi. `perPerson.quota` gains `memoryLimit` (24Gi) and `ephemeralStorage` (10Gi). A runner
  namespace made from now on enforces the restricted Pod Security Standard (#3844, #3824).
