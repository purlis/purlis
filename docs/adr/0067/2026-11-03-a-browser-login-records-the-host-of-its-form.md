# Amended (2026-11-03, #4995; the person's ruling of 2026-11-03): a browser login records the host of its form

Ruled by the person. A site that signs in on another host sends the browser from its own page
(`https://app.example.com/`) to a login form elsewhere (`https://auth.example.com/…`). The quick
approve of 2026-11-02 recorded the page's origin, so the next login was refused on the form's.

- **Before the person is asked, purlis follows the page to its login form** and records the
  form's origin: the one place the password may be typed. It opens the page in a browser of its
  own, which reaches only what the chat's browser reaches, types nothing, and ends it. Where no
  form shows, or the form is not on https or is on a host that can't be confirmed by reading it,
  nothing is recorded and the chat is told why in plain words.
- **The notice shows both**: the page it opens, and the host the password is typed on, each
  large, with a plain warning where they differ. A site the person types is followed to its own
  form before any press counts.
- **The password is typed on that one origin alone.** Always writes it as the line's `form` key
  (`docs/plane-format.md`), only where it differs from the page's; a line without it keeps its
  line and its Allow.

*Decided in implementation (D-4995-1 to D-4995-5 in the pull request):* from the first fill the
browser is held to the page's site and the form's, so the form can send it back to the page; a
login whose form is on another origin keeps no kept login yet; a login whose browser has gone
back to its page since it was submitted went through.
