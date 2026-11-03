### Fixed

- purlis refuses to record a branch folder's base under a name git does not take as a branch, as it already did for an empty one, and leaves the clone's git config untouched. `purlis doctor` names a clone whose config holds `[branch ""]`, which stops every fetch and push in it, with the command that removes it, and a clone whose config git cannot read at all, in git's own words.
