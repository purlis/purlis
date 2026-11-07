### Fixed

- **Move into repos/ survives a crash partway.** A move stopped by a crash, a kill or a power cut
  is flagged by the doctor and finished by the next move, which repairs the branch folders it
  left. A chat with no folder holds the move back, as does one opened while it runs; a folder
  named like one in repos/ in another case is reported as beside it; and the move takes only the
  repos its confirmation showed (#5569).
