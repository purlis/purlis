### Security

- **A Never holds in every mode, and a grant serves only the ask it was given for.** A Never you
  give at an ask (This workspace or Everywhere) now denies that action on that resource whatever
  the template says, `open` included, and a nevers list purlis cannot read denies every action
  until you fix it. A grant covers one rule, one action, one resource and the context it was given
  under (a grant for a push without `force` does not serve a force push), is used only at an ask,
  never lifts a deny, and where two asks decide one action each needs its own grant. The scope
  offered is the narrowest `@keep` among them, capped by an organisation or project scope lock
  (#7029).
