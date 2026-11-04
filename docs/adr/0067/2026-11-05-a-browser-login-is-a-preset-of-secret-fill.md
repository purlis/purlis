# Amended (2026-11-05, spec #5100, built from #5121; the person's ruling SF-7 on #5096): a browser login is a preset of secret fill

Ruled by the person. Until now every login the person allowed from its notice opened a new browser,
so the session moved away from the browser they were watching and logged-in browsers piled up. The
reason it did, that nothing the chat did in another browser is there when the password goes in, is
overruled by SF-2 and SF-7: where the browser went before doesn't matter, as the value is kept out of
all the chat reads, in any browser.

- **A login is a fill of the user name, a fill of the password and a press of submit, under the same
  rules** as any secret fill.
- **It runs in the browser the chat names, or in its current one.**
- **A browser that already holds a session of the same login answers "already logged in".**
- **A new browser is opened only to keep a login for later reuse**, which still needs a browser that
  did nothing before. The answer and the person's view say why, and offer to close the browser left
  behind.

The form-host rules (#4995), the replace-site ask (#4996) and the sign-in reach rules (#5099, as
amended by #5152) stay as they are.

*Decided in implementation (D-5121-1 to D-5121-9 in the pull request):*

- "Holds a session" is judged by two things: a login that answered in that browser, as the same login
  now (identity whole; the site where the call names one; nothing policy forbids since), and the page
  the browser shows asking for no sign-in, read by purlis with no vault opened, at session protocol
  1.73, which a host before refuses, and then the login goes as it always did. A page that asks is
  logged in again, so a provider that wants a new sign-in (#5152) still gets one.
- A browser that tried a login before is treated as one opened from its kept store: where the login's
  page asks for nothing, nothing is filled.
- "To keep a login" is the login whose kept store is there to open. A login with no store yet goes
  into the named or current browser, and where that browser did something first the answer says its
  session isn't kept and how to keep it (`browser_new`, then `browser_login`). The chat, not purlis,
  moves to a new browser for that.
- A browser `browser_new` just opened is what the chat asked for: a login goes there, not back to one
  holding it.
- A login's value still goes in by typing where the field has the focus, with its checks either side,
  since a login's page listens for typing. A page that changes as it goes in ends the browser, which is
  stricter than a fill's refusal. Its hold stays the browser's life (#5099), stricter than a fill's.
- A login that needs another site than the one a page holding a filled secret holds the browser to is
  refused at once, saying why.
- A login into a browser the person has taken over is refused before any vault is read or anyone is
  asked (SF-10).
