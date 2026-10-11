### Fixed

- **The Curate menu offers the harness's card again under rows that cannot run.** When the
  default profile's harness cannot have a prompt typed in (opencode), every Curate row is off
  with the card's line as its reason, and the card's own row ("What opencode can do here")
  should follow them. It never did: the reason went on past the card's label with advice to
  change the default, and the window links a card only where a reason ends with its label
  (ADR 0072 §3). The advice now comes first and the card's line and label end the reason
  (#1134).
