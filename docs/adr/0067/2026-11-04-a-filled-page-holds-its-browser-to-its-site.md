# Amended (2026-11-04, spec #5100, built from #5117 and #5122; the person's ruling SF-3 and the steward's decisions SF-9 and SF-12 on #5096): a filled page holds its browser to its site

The amendment of the same day that records secret fill set out what happens after a fill (SF-3,
SF-9) and that runners follow the same rules (SF-12). Both are built now.

- **The hold starts just before the value goes in.** Once every check of the fill has passed, the
  browser's reach proxy is held to the field's site (scheme's port included), and every
  connection it carries to any other site is ended. A browser that can't be held fills nothing.
- **It lives as long as the page the field is on.** A new document in that page's main frame
  (a link, a form, a redirect, a reload, the engine's own page for a load that failed) or the
  page's close ends it, and the browser reaches as it did before. A route change that loads no
  document keeps it, and so does a new document in a frame inside the page. A fill refused once
  the hold was made keeps it too, since something of the value may have reached the page.
- **It narrows a login's hold and never widens it, one site at a time.** While a page holding a
  secret lives, a fill on another site is refused; two on the same site each hold until their own
  page is gone.
- **The chat is told why.** An address the chat writes for another site is refused before the
  browser hears of it, naming the site it is held to and what ends the hold; a page's own
  request elsewhere is refused by the proxy, in words of its own.
- **The field gives nothing away while it lives.** Before the value is written, a copy, a cut and
  a drag started on the field are stopped from purlis's own world, whoever starts them, the person
  and the page's scripts included; and the field is marked as one the browser keeps nothing of,
  so a page left and gone back to comes back without the value. The back-forward cache was off
  already.
- **The agent's copies, cuts and drags stay withheld for the browser's life** (D-5115-10), and
  aren't narrowed to the field and the page's life: the value is kept out of what the chat reads
  for the browser's life because a later page may echo it, and a copy from such a page could carry
  it to another site once the hold has ended.
- **On a pool's runner** (SF-12), the notice is answered in this window and warns that the secret
  goes to that runner, naming the pool; no other ask is added, since the person sees each fill.
  The fill and its hold run on the runner, through its link, as a login's do, and nothing is
  filled on a pool a chat ran on without the sandbox, as no login is.

*Decided in implementation:* D-5117-1 to D-5117-8 and D-5122-1 to D-5122-3 in the pull request.
