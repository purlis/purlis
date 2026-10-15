# Amendment, 2026-10-05 (V91d): the Notice

The operator's grill of 2026-10-05 (rulings V91a–s, the spec on #1221) names one more house
piece, outside the settings set: **Notice**, in `app/src/Notice.tsx` (NO-1, #1223). A Notice is
a standing line in a project's window about something true now (CONTEXT.md).

**Why a component and not a rule.** About thirty of the window's messages left the operator
reading a problem with nothing to press, each a `<p>` written by hand. A rule that every line
must offer a way out is one a reviewer has to remember; a component whose props **will not
take** a line with no way out is one the compiler remembers. Its props require at least one of
a fix, a link, Copy command or Dismiss, and a stable cause key, which is what a dismissal is
kept by.

**What keeps it from being the layer this record refuses.** It wraps no Radix primitive: it is a
native element with native `<button>`s, drawn under `notice-*` classes in `App.css` in tokens.
Its props are the line's own words (`fixes`, `link`, `copy`, `onDismiss`, `cause`), not a
primitive's. `app/src/Notice.guard.test.ts` fails on a standing line built any other way, and
lists every Notice whose only remedy is Copy command (V91q's debt).

Refusals inside dialogs are not Notices and stay inline errors (V91n). Any further house piece
is still a new amendment to this record.
