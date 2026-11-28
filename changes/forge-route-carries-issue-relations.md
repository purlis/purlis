### Fixed

- **A chat can add, remove and reorder sub-issues, and add and remove blocking links, on its own
  GitHub repo.** The forge route carries them as it carries an issue's update, and refuses one
  that names an issue of another repo. A request the route doesn't carry now says that only
  purlis's maintainers can change that (#8022).
