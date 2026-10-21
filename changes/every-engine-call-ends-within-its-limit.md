### Fixed

- **A browser whose engine stops reading is still stopped in time.** Every DevTools call to a
  chat's browser engine ends within its limit, writing included, and stopping a browser ends its
  engine within its grace however the engine behaves (#2123).
