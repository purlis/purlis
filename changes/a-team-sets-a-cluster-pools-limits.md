### Added

- **A team sets a cluster pool's limits** (#4287). A Pool's `spec.limits` sets each limit's highest allowed
  and a default. A default below purlis's holds with no prompt; a higher one is offered, and one press sets
  it. Each person chooses within the highest, a suggestion stops at it, and at the highest the error says
  the team set it and who to ask.
