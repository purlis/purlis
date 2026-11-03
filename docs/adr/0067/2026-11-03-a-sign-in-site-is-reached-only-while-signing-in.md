# Amended (2026-11-03; the person's ruling of 2026-11-03): a sign-in site is reached only while signing in

Ruled by the person, narrowing the amendment of the same day that records a login's form host
(`2026-11-03-a-browser-login-records-the-host-of-its-form.md`). A browser logged in through a
login whose form is on another host reaches that host only during the sign-in.

- **The form's host is reachable from the moment the browser is held to the login until the
  browser is back on the login's page.** From then on the logged-in browser reaches the page's
  site alone, and every connection to the form's host is ended.
- **Another login goes through the same steps again**: it is judged as any login is, and opens a
  sign-in of its own. A browser is never held to the form's host for longer than its sign-in.

*Decided in implementation (in the pull request):* "back on the page" is the page purlis opened
for the login showing a document on the page's own origin, never another tab; the sign-in also
ends when the login does not go through and the agent has the browser back, and two minutes after
it began whatever happened, so a site that sends the browser elsewhere, or never back, holds
nothing open; a sign-in the person takes over is theirs to finish within those two minutes.
