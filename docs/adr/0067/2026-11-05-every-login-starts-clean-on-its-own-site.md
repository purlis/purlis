# Amended (2026-11-05, spec #5100, built from #5272; the person's ruling): every login starts clean on its own site

Ruled by the person, widening the ruling of the same day that held for a login allowed on a notice
(`2026-11-05-a-browser-login-is-a-preset-of-secret-fill.md`). A login starts clean on its own site,
whichever way it was allowed.

- **Every login that fills clears all of its own site's stored data in that browser first**: cookies,
  service workers, local storage, IndexedDB and cache. Other sites are untouched. That covers
  declared logins and Always lines, as well as logins allowed on a notice.
- **A browser that already holds the same login answers "already logged in" and clears nothing.**
- **A form that doesn't appear on the clean load leads to a clean new browser, and to nothing else.**
  The answer and the person's view say why, and offer to close the browser left behind.
- **A login follows the same value rules as a fill.**

Secret fill stays as it is, as do the form-host rules (#4995), the replace-site ask (#4996) and the
sign-in reach rules (#5099, as amended by #5152).

*Decided in implementation (D-5272-1 to D-5272-9 in the pull request):*

- A browser that has held nothing has nothing to clear: no act, no input of the person's, no login
  tried, and nothing opened from a kept login. Its first load shows the form, and it's held once one
  shows, so a login page that needs another site loads there.
- Any other browser is held first, then cleared, then loads the login's page. What it would have
  shown before isn't loaded: it would be an earlier session's. So it's held to the login's sites even
  where no form then appears, and the answer says so as it offers to close it.
- Every other page of the browser is held still from before the hold until the login answers, as
  the only thing that could put back what goes. One that won't hold still fills nothing.
- A login that may be held clears only for a form it sees. A page that can't be read, or that went
  elsewhere, says nothing of its session.
- The clean load is in a new tab, since a tab keeps some of what its site stores for its own life, and
  the first tab the login opened goes.
- The browser's HTTP cache has no way to clear one site, and clearing all of it would touch other
  sites. So the login's page loads past it, until the login has answered.
- The clean load's failure is a refusal of its own at session protocol 1.74, not a sentence read back.
  No login goes to a host before 1.74, which would leave a declared login's site as it is or clear less.
- One new browser, once. It tries the login as a first load does, and what it says is the answer, on
  this call or a later one. It opens from nothing kept. The browser left lets go of the store it
  opened from, and a kept login that goes through in the new one keeps its session as any fresh
  browser's does. What policy and the chat's browsing say are asked again before it opens.
- A runner is held to 1.74 by the host that relays to it, as the window's check is of its own host.
- A user name is under the value rules too, as it goes into a page as a fill does.
