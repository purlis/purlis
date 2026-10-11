### Fixed

- **A branch whose reads stayed busy is watched again.** When the reader was busy for every
  retry, or paused the branch, the explorer stopped following that branch's changes and said
  nothing. Its open folders now say "Changes on disk are not shown yet", and purlis asks for the
  branch again once the pause is over (otherwise after about thirty seconds). Once the branch is
  watched, its changes are read again, so a file written in the meantime is still marked
  (#1745).
