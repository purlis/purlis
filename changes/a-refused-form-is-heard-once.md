### Changed

- **A screen reader hears a refused form once.** When a form will not send, because a field is not
  right or because purlis refused what it was sent, a screen reader says it once, as a summary such
  as "Email: That does not look like an email." or "2 fields to fix: Name, Port.", and the keyboard
  goes to the first field to fix. Each field still says why under its box, as its description. A
  field's error is no longer an interrupting alert as well: it is said politely after what you
  type, and no longer talks over the other fields (#3291).
