# Built: the browser sign-in to gitlab.com (#730, 2026-10-13)

§1's and §3's PKCE flow is written for gitlab.com, in `purlis_core::forge::pkce` with the
listener in `purlis_core::forge::loopback`, and offered as **Sign in with GitLab** beside the
token sign-in. The operator registered the application on 2026-10-13 (#726): not confidential,
scope `api`, the three loopback redirects. Within this record, these were settled while
building it:

- **The redirect's path is `/purlis/signin`**, the rename's spelling of §1's `/charter/signin`
  (#726, corrected 2026-10-13, before anything was registered or listened). §1 above now says
  so. The ports are 47823, 47824 and 47825.
- **With all three ports in use the sign-in stops and says so**, naming them and the token
  sign-in. It never falls back to another port: GitLab refuses a redirect address the
  application did not register, and a port picked at run time is one nobody reviewed.
- **The listener binds `127.0.0.1` only, for one sign-in.** It drops a peer that is not a
  loopback address unanswered. It takes a `GET` of the registered path whose `Host` is its own
  address and whose `state` is this sign-in's, compared in constant time, answers it, and is
  closed. So a second request is never read. Anything else gets a fixed refusal and changes
  nothing, and after 32 of those the sign-in stops. It also closes on Cancel and after five
  minutes.
- **Every answer is one of three fixed pages.** Nothing a request said is put in an answer, an
  error or a log: not the query, not a header. The page a finished sign-in shows says "You can
  return to purlis".
- **Why another program on the machine gains nothing from the listener.** A program that takes
  a port first only moves purlis to the next, and the browser comes back to the port purlis
  bound. One that calls the listener has no `state`. One that reads the code on its way back
  cannot spend it: GitLab hands a token for a code only with the verifier, which never leaves
  purlis's process except in the one exchange, over HTTPS. The application has no secret, so
  nothing else stands in for the verifier. On macOS a sandboxed chat's profile gives it no
  listening socket and no loopback port but its proxy's (ADR 0067). That is a second bar, not
  the one this rests on: the reasoning above holds for a chat with no sandbox too.
- **What the listener does not stop:** any program the person runs can start a sign-in of its
  own with purlis's public client id, as "What this costs" says of every public client. What
  stands between that program and a token is the forge's own page in the person's browser.
- **The browser is opened by the host process, not by the page.** The window's call names the
  forge and the host, and its answer is the account. The authorization address, which holds the
  `state`, the code and the token never cross to the window.
- **Only gitlab.com signs in this way.** A self-managed GitLab has no registration and keeps
  the token sign-in (§4, #1810 Q10). The per-host registration of §1's table is later work.
- **A `glab` login made through glab's own OAuth sign-in is still not imported** (the import's
  section above): that token is glab's application's, not purlis's.
- **The refresh of §3** is one writer for both forges, built with the GitHub sign-in (#729).
  GitLab's is `grant_type=refresh_token` with the client id and no secret, and GitLab answers
  with a new refresh token each time.
- **Sign-out revokes at gitlab.com** (§3), in `forge/revoke.rs`: one form to `/oauth/revoke`
  with the client id and the token (the refresh token where the item holds one; GitLab keeps a
  grant's two tokens as one record), and no secret. The sign-out on this machine happens
  first and never waits on GitLab's yes: when GitLab cannot be asked or will not, the person is
  signed out here all the same, and the row says so and where on gitlab.com to end the token.
  A pasted or imported token is never revoked, nor one on a host with no registration.
  **GitHub is not asked**, as §3 says: ending an app's grant there is authenticated with the
  app's client secret, and the registration has none. A sign-out waits for a refresh in
  flight, so a renewed pair is never written back for an account just signed out.
- **A test build signs in through no browser**, and revokes at no forge. The flow's stand-ins are a scripted listener,
  browser and token address; one case runs the real listener over this machine's loopback
  against a stand-in token address. Nothing in a test reaches gitlab.com.
