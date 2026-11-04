# Amended (2026-11-04; the person's ruling of 2026-11-04, #5152): a sign-in site stays reached, and the chat acts there only while signing in

Ruled by the person, amending the amendment of 2026-11-03
(`2026-11-03-a-sign-in-site-is-reached-only-while-signing-in.md`). A site whose app sends the
browser back to its sign-in provider straight after the login (an OpenID Connect code exchange, a
silent re-auth, a token refresh) broke once the provider's host was cut off.

- **Both hosts stay reachable.** A logged-in browser keeps reaching the login's page's site and
  the host its form is on (the form's host the amendment of 2026-11-03 records), so the app's own
  redirects, hidden frames and token refreshes to the provider work.
- **The chat can't act on the provider's pages outside a sign-in.** Opening a page there,
  clicking, typing, pressing, selecting, dragging, uploading and reading (snapshot, find, wait,
  screenshot) are refused, each with the reason. During a sign-in that `browser_login` runs, the
  provider's pages work as before.
- **When the provider needs the person again** (its session ended and it shows its form), the
  chat is told to call `browser_login` again, which starts a new sign-in under its grant.
- **The `browser_login` answer lists the hosts** the browser reaches, and what the chat may do
  on each.

Every other site stays refused, and `browser_new` is still the way to any other site. The
sign-in itself (when it opens and ends) is as the amendment of 2026-11-03 decided; it now bounds
what the chat may do on the provider, not what the browser reaches.

*Decided in implementation (in the pull request):*

- An act is judged in the browser's driver at the moment it is made, by the frame it acts on and
  every frame around it: each frame's address and its document's origin, so a frame that takes
  its origin from the page around it, or a sandboxed one, is judged as what it is. The driver
  asks the browser's binding whether the agent is kept off that site now; it never judges by the
  address the chat names alone. An address the chat names on the provider is refused as well.
- A page of the login's own site that holds a frame of the provider is read without that frame:
  it is left out of every snapshot, find and wait, and painted over in a screenshot. The page
  itself is not refused, since an app may keep such a frame for as long as it is open.
- Keys go where the focus is: they are refused where the focused element is in a provider frame,
  where the page is the provider's, or where the focus cannot be told and the page holds a
  provider frame.
- Leaving a provider page is not acting on it: going back, or to another address, goes on. The
  browser's pages are listed as they are, a provider page with its address and title, so the chat
  can tell why a call is refused there.
- While the agent drives, what a provider document sends to the provider itself (a form, a
  request with a body) outside a sign-in is not sent. What it sends elsewhere (a form posted back
  to the app) goes, and so does whatever the app's own pages send to the provider. This holds
  even where a key or a click reaches a provider page that came as the act was made.
- "Shows its form again" is a provider page that shows a password field: its refusal says to log
  in again. Every refusal on the provider says what to do: go back, or call `browser_login` where
  the page asks to sign in.
- A browser opened from kept logins reaches the provider once a login signs in there again: a
  kept login does not record its form's host. `browser_login` on a kept login is how a chat
  begins in it, so this is its first call.
