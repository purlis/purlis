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
- **A download at no web address is from the nearest document around it that has an origin**
  (decided by the steward). A `data:` or `blob:` download a blank or srcdoc frame began is from the
  origin that frame's document has, so one inside a page of the form's site is that site's. One
  that still can't be placed, as a sandboxed frame's can't, isn't brought into the chat's folder
  while the browser keeps the chat off a site: the refusal says it can be brought in while
  `browser_login` signs in.
- **A sign-in is held to its own page** (ruled by the person). While a sign-in is open, the chat
  reads and acts on the form's site only on the page `browser_login` ran on and the windows
  related to it (each that opened another, or was opened by one, where a script in one can reach
  the other). Every other tab stays kept off that site as it is outside a sign-in, and so does an
  address the chat opens a new page at.

*Decided in implementation (in the pull request):*

- The browser's driver keeps the page the last login ran on, and asks the browser's binding which
  site a sign-in is open on now; it judges each page by both. Until a login has run in the
  browser, no page is a sign-in's own.
