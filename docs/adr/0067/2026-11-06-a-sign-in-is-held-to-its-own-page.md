# Amended (2026-11-06; the person's rulings of 2026-11-06 on sign-ins, and the steward's decisions on them): a sign-in is held to its own page

Amends [A sign-in site stays reached, and the chat acts there only while signing in](2026-11-04-a-sign-in-site-stays-reached-and-the-chat-acts-there-only-while-signing-in.md)
and [A filled page holds its browser to its site](2026-11-04-a-filled-page-holds-its-browser-to-its-site.md).

- **A browser holding a login takes a secret only for its login's sites** (decided by the
  steward). `browser_fill_secret` on any site but the login's page's or its form's is refused
  before anything is looked for or filled, and the answer says to fill it in a browser of its
  own. Such a fill once left the browser reaching nothing until its page was gone.
- **During a sign-in, a page holding a secret filled on the form's site follows that site's own
  server redirect back to the login's page's site by itself** (ruled by the person). Nothing else
  about the hold changes: it still reaches the form's site alone, a link or a script still can't
  take the page away, and any other site still asks or stays refused. Outside a sign-in, the way
  back asks as before, unless the sign-in's chain began on the login's page.
