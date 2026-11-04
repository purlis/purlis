# Amended (2026-11-04, spec #5100, built from #5118 and #5119; rulings SF-4, SF-8, SF-10 and SF-11 on #5096): a fill's 2FA code, its mark, and a browser taken over

This builds on the amendment of the same day that records the secret fill. It adds three parts.

- **A 2FA code from a stored seed** (SF-4, SF-11). A chat asks for a code with `code` on
  `browser_fill_secret`, naming the key that holds the seed. The seed is an `otpauth://totp/`
  address with its own algorithm, digits and period, or the bare base32 secret, read as six
  digits of SHA-1 every 30 seconds. The notice says it is a code, and the person approves the
  seed for one site as they would any key, so Allow for this chat and Always allow hold it too.
  Only once it is allowed does purlis read the seed and work the code out (TOTP, RFC 6238), on
  the vault's side, at the moment of filling. Only the code goes to the browser. A counter seed
  (`hotp`) is refused, since purlis keeps no count. A value taken as a seed, an `otpauth://`
  address or a bare secret of the lengths authenticator apps are given, is refused as a plain
  value too, so a seed never goes into a page.
- **What the person sees** (SF-8). The browser's view marks each key filled into the page it
  shows "secret: <key>", beside the page's site, while that page is on the site the key went to.
  The mark is heard as the key and the site, and its hint names the field the page labels. It is
  drawn by purlis in the view's bar, outside the page, as the site's chip is (#2024), so a page
  can neither fake a mark nor move one. The chat's line in the browser's timeline names the key,
  the field and the site. Neither ever shows a value.
- **A browser the person has taken over** (SF-10) is filled nothing, and the chat is told why.
  The check comes before the field is read, again before the value is read, and once more before
  the fill. The session host also refuses any act of the agent's
  while the person holds the lease.

*Decided in implementation (D-5118-1 to D-5119-4 in the pull request):* the code is made in
purlis-core, beside the vault read that the Allow unlocks, so the engine and the session protocol
are unchanged. The notice for a code and the notice for its value are separate notices, whose
presses never stand for each other. The mark follows the page's site, not its document: the
page-scoped hold (#5117) is where "the page holding the value" is known.
