# A filled page leaves for another site only by its server's redirect

Amends [A filled page holds its browser to its site](2026-11-04-a-filled-page-holds-its-browser-to-its-site.md).
The person's rulings SF-14 and SF-15 of 2026-11-05 (#5230, on #5096).

A sign-in whose server redirects to another site could never finish after a fill: the hold refused
the redirect, and the page ended on an error page that kept the hold on.

**A redirect from the approved site's own server, after a form sent there, may take the page to
another site.** The driver tells it apart by what the engine says of each request: a main-frame
document request redirected from the fill's site, in a chain that began with a form. Links, scripts
and the chat's navigate begin no such chain, and stay refused.

- It continues by itself to a site this browser was on before the fill, a site that answered one of
  its pages' documents with a redirect among them: the app that sent the person to sign in may show
  nothing first.
- Any other site asks the person once, in the sites' ask. The answer goes with the browser's reach
  (`answered`, 1.74), since a site the reach lists already is asked about too.
- Either must be in the browser's usual reach, and clear of a login held in it.
- An address that holds the value, as written or URL-encoded, is refused, its fragment read too.

**Nothing reaches the destination while the page holding the value lives.** A proxy that goes by
host and port can't tell the redirect from anything else the page sends there, and a page runs until
the next document commits. So the redirect is stopped, the page loads a blank document, what it
sends as it unloads meets the hold, the hold ends, and only then does the browser go to the address
the server named. There is no moment when the proxy carries the destination for a page that holds
the value. Rejected: letting the destination through the proxy for the length of the redirect, with
the driver refusing the page's other requests. The driver doesn't see everything a page can open.

The cost is an entry of history, a request with no referrer and a moment's wait, on a sign-in.

**A filled form posted straight to another site is refused unless the key is approved for it too**:
a line of the persona's or the person's Allow for this chat on that origin. Those origins go with
the fill (`also`, 1.74), and the hold carries them. A refused form is said in the chat's answer and
on a page of purlis's own the person sees, each naming the site and how to approve the key there.
There is no ask per send. A redirect that would send its body on is held to the same rule.

**An error page in the main frame ends the hold.** It holds no value, and the browser must not stay
stuck. A load on another site no longer ends on one while a page holds a value: the driver stops it
and the page stays where it was, with the reason in the answer. So the engine's own page, whose
history entry holds the address of the failed load, only ever stands for an address on the fill's
own site, and the reason the hold outlived that page before is gone.

Browsers on runners behave the same (SF-12): all of it is in the driver and the core the host runs.
