# Amended (2026-11-05; the person's rulings on the hold and on the redirect): the hold follows related windows, and a redirect goes on by itself only to where the sign-in began

Amends [A filled page holds its browser to its site](2026-11-04-a-filled-page-holds-its-browser-to-its-site.md)
and [A filled page leaves by its server's redirect](2026-11-05-a-filled-page-leaves-by-its-servers-redirect.md).

- **The hold follows related windows.** A window is related to the filled page where a script in
  one can reach the other: it opened the page, the page opened it, or either holds of a window
  that did. A window opened with no such link isn't related.
- **A fill is refused while a related window is on the field's site**, or on no site, as a blank
  one is: it could read the field. The answer says to open the form in a page of its own.
- **Any other related window closes when the page is done with the value**, by loading another
  document, an error page among them, or by closing. A window opened from then on closes too. The
  hold ends only once they are gone, and the chat's next answer says they closed.
  Closing was chosen over keeping the hold for as long as they live: a chat has no call that closes
  one window, so such a hold could outlast the work, and the browser would stay held with no way
  on but ending it.
- **What the hold doesn't stop.** It can't stop a script on the approved site using that site's own
  storage or workers: such a script can keep what it reads there and send it from the same site
  once the hold is over. Approving a key for a site trusts the scripts that site serves. A related
  window that comes to that site during the hold can read the field too: it reaches only what the
  page reaches, and closes with the rest.
- **A redirect goes on by itself only to the site the chain leading to this sign-in began from**,
  in place of a site the browser was on before the fill. The chain begins at the last load chosen
  by an address, written or gone back to, that showed a document, and an act that addresses a page
  and comes to nothing leaves no chain, so the next redirect asks. What a page does on from there, a link
  or a server's redirect, stays in that chain, a window takes its opener's, and a site the chain
  only passed through is no part of its beginning. Any other site asks the person once, naming the
  site.
