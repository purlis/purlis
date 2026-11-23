### Fixed

- A login now waits until every other page of the browser has confirmed it is paused before it clears the login's sites, and checks again after clearing; if a page does not confirm in time, the login stops and says so.
- A login now waits until the pages it held are running again before it answers, and a page that is slow to answer on a busy computer is asked again instead of stopping the login at once.
- A login no longer stops over a blank page that nothing opened and that never confirms it is paused (the engine has nothing to tell it): that page holds nothing, so the login closes it and goes on. A page showing a site is never closed for this: it stops the login, as before.
- A login stopped because a page did not stay paused now says which kind of page it was (blank, site page, popup, extension, devtools or other), whether another page opened it, how many frames it had, which step did not confirm and how long the login waited. It names no address.
