# Amended (2026-11-12, #6291; the operator's ruling of 2026-11-12): a login's sign-in starts where the chat names it

Ruled by the operator ("Yes, with limits"), passed on by the chat that dispatched the work. It
amends [A sign-in site stays reached, and the chat acts there only while signing in](2026-11-04-a-sign-in-site-stays-reached-and-the-chat-acts-there-only-while-signing-in.md)
for this one case: there, an address the chat names on the sign-in host is refused.

Until now a login always opened its declared page. A chat could not start an OAuth or OpenID
Connect sign-in with its own request (a `client_id`, a `redirect_uri`, a `state`, a PKCE
challenge). `browser_open` and `browser_navigate` refuse the sign-in host outside a sign-in, and
a suggestion's `site` drops the query and refuses any `%`. So none of these reached the identity
provider.

- **`browser_login` takes `start`**, for a login the persona declares or one the person allowed
  from its notice. It is never taken beside a suggestion (`site`, `vault`, `key`).
- **On the login's own sites alone.** The origin of `start`, as a browser reads the address, is
  exactly the login's page's origin or the origin its form is on.
- **Kept as written.** Its query and fragment, percent-encoding included, are opened as they are,
  in place of the login's page, for that one sign-in.
- **Its way back is the login's page's site.** A `start` whose `redirect_uri`, decoded, is not an
  address on the login's page's origin is refused.
- **Everything else stays as ruled**: the clean start, the fill on the form's origin alone, the
  sign-in held to its own page, its two minutes, and the browser reaching the two hosts alone.
- **Suggested logins are unchanged.** `loginask::site_of` and the page the person confirms on a
  notice stay as the amendment of 2026-11-02 decided.
- **A refused redirect never says its address.** Its `Location` may hold a code. Checked first:
  every refusal of a page leaving names the site alone, and a test now holds that.

*Decided in implementation (in the pull request):*

- `start` is judged in two places: by the window against the login as read, before the browser is
  handed anything, and again by the host as it opens (`purlis_core::loginstart::start_of`). The
  login's page stays the address the browser is held to and counts as back on.
- The address is read by the WHATWG URL parser, as a browser reads it. Its query is read
  form-urlencoded, as an identity provider reads it. Nothing a browser reads otherwise passes: a
  user or password, a backslash, a space or a control character.
- Every `redirect_uri` is judged, under any case or encoding of its name, so a second one an
  identity provider might read instead is judged too. One encoded twice is no address, and is
  refused.
- A `request` or `request_uri` carries a redirect purlis cannot read (RFC 9101, RFC 9126), so a
  `start` with either is refused. This fails closed.
- A `start` with no `redirect_uri` is taken. The provider then sends the browser back to the
  client's registered address, and the browser still reaches the login's two sites alone.
- A refusal names the origin and the parameter, never a value of the query.
- Session protocol 1.86 carries `start` on a login. A host before it would drop the start and
  open the login's page, so no login with a start is sent to one, by the window or by a host that
  relays to a runner.
- A browser that already holds the login answers "already logged in", as before, and opens no
  `start`. A chat that needs a second sign-in opens a new browser for it with `browser_new`.
