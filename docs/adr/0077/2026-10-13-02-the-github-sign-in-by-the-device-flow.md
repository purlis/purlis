# Built: the GitHub sign-in by the device flow, and the refresh (#729, 2026-10-13)

§1's registration table and §3's device flow and refresh are written. The operator registered
both applications on 2026-10-13 and recorded the client ids on #726. Within this record, these
were settled while building it:

- **The loopback path is `/purlis/signin`**, the rename's spelling of §1's `/charter/signin`
  (#726, corrected 2026-10-13, before anything was registered). github.com's callback is
  registered as `http://127.0.0.1/purlis/signin` and never used; gitlab.com's three are
  `http://127.0.0.1:<47823–47825>/purlis/signin`.
- **The registration table** is `forge/registration.rs`: host, kind, client id and flow. It
  answers for a whole host name and nothing else, so neither client id is ever sent to another
  host. The scopes a registration asks for are the ones a pasted token needs (§2 as amended).
- **The keyring item holds the pair as one value.** For a `device` or `pkce` account the item
  `purlis/@forge/<host>/<id>` is one JSON object: the access token, the refresh token and the
  access token's expiry, so §3's "one write" is one write. A pasted or imported token's item
  stays the token as text. Every reader gets the access token through one function, whichever
  way the account was signed in.
- **A sign-in in the browser takes over a token account of the same login.** One account is one
  forge, one host and one login (§6), so signing in with GitHub as a login that has a pasted
  token on github.com signs that account in by the device flow, under the same id.
- **A granted token's scopes are the grant's own.** The forge's token answer lists them, so a
  GitLab grant is not looked up as a personal access token. A grant without a scope §2 names is
  refused, naming it.
- **The verification page must be the forge's own.** An answer that names a page that is not
  HTTPS on the registration's host is refused: the window never sends a person, with a code to
  type, anywhere else. This narrows what §4 of the 2026-10-12 amendment calls the known risk.
- **The browser is offered, not opened.** The dialog shows the code first, with a button that
  opens the forge's page and one that copies the code, so the person reads the code before the
  browser takes the front. §3's "opens the address in the system browser" is that button.
- **A refused refresh keeps the account listed, signed out** (§3, §6), and deletes its keyring
  item. The window is told at once and draws a Notice naming the account, with Sign in again;
  the request that needed the token fails with the same sentence. A forge that could not be
  reached is not a refusal: the token is used while it lasts, and the sign-in is kept. Nor is
  an answer that says nothing about the grant (a rate limit, a proxy's refusal on the way,
  OAuth's `temporarily_unavailable` or `server_error`): only an OAuth `error` other than
  those, or a bare 400 or 401 from the token address, ends a sign-in.
- **The window refreshes for its git too.** The credential helper is another process, so it
  never refreshes (§3's one writer). The window renews a token that is about to expire before
  it starts the git that will ask the helper.
- **An account on a host with no compiled registration is never refreshed against another
  host's endpoint.** §6's per-host client id is not written yet; until it is, such an account's
  token is used while it lasts and then signs out.
- **The three audit actions** stay FW-14's, as before. A GHES still takes a token only (§5 of
  the amendment).
