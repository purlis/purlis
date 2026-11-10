# A link is followed only on a press's one-time token

**2026-11-10, spec #5948, from the adversarial review of #5978.** Amends §5 of ADR 0122 and the
amendment of 2026-11-09 on `purlis:` links.

purlis's script in the page reported a person's press on a link as an image of
`/.purlis/link?href=…`, and the app acted on any such request whose `href` the template wrote.
The page could make that request itself, with no script at all: `<img src="/.purlis/link?href=…">`
beside a matching `<a>` opened an "always open" host in the browser, brought up the dialog, or
navigated to a `purlis:` place, with nothing pressed.

- **A report is followed only with the one token purlis handed the page's open load.** purlis's
  script opens its load on `/.purlis/link/open/<word>`; purlis answers by handing that load a
  random token through `__purlisLink`, a property the page cannot replace, into the script's
  closure, where the page cannot read it. A trusted press sends the token with the `href`; the
  app follows only an unspent token of the load now open in that webview, spends it, and hands
  the next. A newer load replaces the older, spending its token. `Links::heard` in
  `app/src-tauri/src/artifact_links.rs`.
- **Nothing acts without it**: no always-open host opens, no dialog comes up, and no `purlis:`
  link navigates on a report that carries no valid token.
- **One page follows at most one link per half-second.** A report sooner than that is not
  followed, and its token stays unspent for the person's next press.
- **What the report is built with is taken before the page's script runs** (the event's
  `target`, `closest`, `getAttribute`, an image's `src` setter), so an approved page's script
  cannot read the token on its way out or make a press report another link. A token seen
  afterwards, in the page's resource timings, is already spent.
- **Its runtime half is not yet tested in a real webview.** The token's rules are unit tests; the
  script itself has not run under a test in WebKit, WebKitGTK or WebView2, and a probe of it in
  `tools/artifact-isolation-probe` is still to be written.
