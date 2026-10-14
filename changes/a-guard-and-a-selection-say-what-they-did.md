### Fixed

- **A `git` variable set on the line turns the clone guard's sparse-checkout reading off.** Any
  `GIT_*` assignment or export on the line now counts as configuration: the paths a sparse
  checkout leaves out are counted again (qw387).
- **`purlis workspace use` and `create --use` say when nothing was selected.** Where the plane's
  state folder refuses the pointer, the command fails and says so, instead of announcing a
  selection that was not kept (qw387).
- **A `cd` whose `&&` chain ends before a branch move in the plane root is told how to join
  it**, as a `cd` joined by `;` already was (qw387).
