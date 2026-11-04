# Amended (2026-11-03; the person's ruling of 2026-11-03): a sign-in site is reached only while signing in

*Amended on 2026-11-04 by the person's ruling in #5152: both hosts stay reachable, and the chat acts on the form's host only while signing in (`2026-11-04-a-sign-in-site-stays-reached-and-the-chat-acts-there-only-while-signing-in.md`).*

Ruled by the person, narrowing the amendment of the same day that records a login's form host
(`2026-11-03-a-browser-login-records-the-host-of-its-form.md`). A browser logged in through a
login whose form is on another host reaches that host only during the sign-in.

- **The form's host is reachable from the moment the browser is held to the login until the
  browser is back on the login's page.** From then on the logged-in browser reaches the page's
  site alone, and every connection to the form's host is ended.
- **Another login goes through the same steps again**: it is judged as any login is, and opens a
  sign-in of its own. A browser is never held to the form's host for longer than its sign-in.

*Decided in implementation (in the pull request):*

- "Back on the page" is the page purlis opened for the login showing a document on the page's own
  origin, never another tab. That page showing a document anywhere else but the form's host (a
  chat that sends it elsewhere, a host the browser doesn't reach) ends the sign-in too.
- The sign-in also ends when the login does not go through and the agent has the browser back,
  and two minutes after it began whatever happened, so a site that never sends the browser back
  holds nothing open. A sign-in the person takes over is theirs to finish within those minutes.
- The browser reaches no third host during a sign-in: a provider that sends it on to another
  host does not sign in this way.
- A browser signs in on one form's host: a later login of its page whose form is on another host
  is refused, as the earlier amendment's binding did.
- A browser held to the login's page already (logged in before, or opened from kept logins)
  reaches the form's host again from the moment a new login begins, so its page can send it to
  the form. A login whose form is on another host keeps its signed-in browser like any other
  (#5014), reopened held to its page's site alone; reopened, its page counts as signed in only
  once it has stayed on its own origin, asking for nothing, for a fifth of the login's wait.
