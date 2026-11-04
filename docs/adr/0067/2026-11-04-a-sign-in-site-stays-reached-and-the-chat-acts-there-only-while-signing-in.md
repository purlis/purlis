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
  provider frame, and each key after the first is judged again. A click, a hover, a drag's drop
  and a scroll are refused where what is drawn at their point is a provider frame, so a frame
  drawn over the element aimed at takes nothing.
- Leaving a provider page is not acting on it: going back, or to another address, goes on. A
  provider page is listed and answered with its origin alone and no title, so the chat can tell
  why a call is refused there and reads nothing of the page. A download that came from the
  provider is not brought into the chat's folder outside a sign-in.
- While one of the chat's calls is carried out, nothing is sent to the provider by an address the
  chat wrote, however it was written (judged as the browser itself reads the address), nor by a
  provider document to the provider itself (a form, a request with a body). What a provider
  document sends elsewhere (a form posted back to the app) goes, and so does whatever the app's
  own pages send to the provider, and everything outside the chat's calls. This holds even where
  a key or a click reaches a provider page that came as the call was made.
- A browser holding a login sends to the login's sites without asking the person, its sign-in
  host included: the person allowed both with the login, and the proxy refuses every other site.
  A token refresh the app's page sends to its provider goes.
- "Shows its form again" is a provider page that shows a password field: its refusal says to log
  in again. Every refusal on the provider says what to do: go back, or call `browser_login` where
  the page asks to sign in.
- A browser opened from kept logins reaches the provider once a login signs in there again: a
  kept login does not record its form's host. `browser_login` on a kept login is how a chat
  begins in it, so this is its first call.
