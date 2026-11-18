### Added

- Pool setup shows how long each check took, once it is done, and the step list shows each step's time. A check past 30 seconds says what it is waiting on and why, so a slow line doesn't read as a stuck one (#7191).
- One status bar at the top of the Pool setup tab holds the pool's state (Unreachable, Refused, Host key changed, Runner gone) and the actions for it, with Forget pool… in every state (#7191).
