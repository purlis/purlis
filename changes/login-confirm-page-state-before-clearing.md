### Fixed

- A login now waits until every other page of the browser has confirmed it is paused before it clears the login's sites, and checks again after clearing; if a page does not confirm in time, the login stops and says so.
- A login now waits until the pages it held are running again before it answers, and a page that is slow to answer on a busy computer is asked again instead of stopping the login at once.
