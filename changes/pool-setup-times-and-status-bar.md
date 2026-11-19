### Added

- The Pool setup step list shows each step's time, the checks it ran added up (#7191).
- One status bar at the top of the Pool setup tab holds the pool's state (Unreachable, Not added, Host key changed, Runner gone) and the actions for it. Forget pool… there asks first, in the tab, what may be left on the machine (the runner's container and home volume, the browsers it ran, the logins given on it), and offers Remove only where the machine answers (#7191).
