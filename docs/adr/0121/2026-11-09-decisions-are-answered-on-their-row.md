# Amendment, 2026-11-09: decisions are answered on their row

Ruled by the operator on 2026-11-09 (#5998, Q55), after "I never saw the request": a run grant
was listed in Asks, but its options stood behind an **Answer** text link, so its row did not read
as something to act on. This reverses [ADR 0119](../0119-the-inbox-is-the-persons-own-in-three-views-on-the-right-bar.md)
decision 7 for decisions.

- **A Blocking ask that is a decision shows its options on its row in Asks**, with a question as
  its title ("Allow curl to use vault team?"): a permission, a run grant, a sandbox's host,
  folder or service, a browser's site, and a dispatch's grant, budget or loop.
- **Its row is laid out as a chat pane's band lays out a sandbox ask**, from the same way-out
  component: the narrowest grant (Allow once, Allow only for this chat), then **Other scopes…**,
  which opens the wider grants under the line, then the refusal (Don't allow, Keep blocked, Deny).
  A set of answers that are not scopes of one another (a harness question's choices, a browser's
  sites) is drawn whole, in the order its source offers it.
- **The optional "Why keep it blocked?" box opens only after Keep blocked** is pressed, and never
  takes the keyboard: a second press of Keep blocked sends the refusal, with what was typed or
  with nothing.
- **Only an ask that needs typing keeps its form behind Answer**: a question's reply, a secret's
  value, a run's value its vault lacks. Answer is a visible button, not a text link, and the row
  says what is needed ("Needs a value for SENTRY_INTAKE_TOKEN").
- **The settle guard (#1695) holds as before**: every grant on the row, and each under Other
  scopes…, does nothing when pressed just after the row was drawn or moved, and says why; a
  refusal is never held. Each grant keeps its consent marks
  ([ADR 0097](../0097-a-theme-is-bounded-data-and-may-set-every-token.md) §6).
