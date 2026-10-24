### Changed

- **Purlis now matches its Figma record** (#3177, #3113). The default primary button is filled
  in the accent with white words, and Allow keeps its edge and its lock, so the two never look
  alike. Controls, fields, dialogs and menus draw no hairlines; every md control is 28px tall
  with 8px corners; controls' and dialogs' words are 13px, with 15px and 18px headings; every
  corner is on the record's 7/8/10/12 scale; a chosen activity button takes the record's
  lavender wash and lighter icon, and the update pill's words are lavender. Purlis Classic
  looks as it did.

### Added

- **Theme tokens for a plain control's edge and the filled primary.** `border.control` draws
  the edge of a secondary button, a field and a select; `accent.fill`, `accent.fill-hover` and
  `accent.fill-text` draw the primary; `accent.wash` and `accent.text` draw a chosen activity
  button and words in the accent; `radius.control` is an md control's corner. A theme over
  Purlis Classic that leaves them out keeps drawing as it did: each follows the token it was
  drawn with before.
