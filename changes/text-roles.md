### Changed

- **The window's words have roles.** Seven text roles (title, heading, body, secondary, meta,
  label, code) each fix a size, a weight and a colour, with new core tokens for the two weights,
  the two line heights and the one monospace family. Nothing looks different yet: the Inbox's
  rows wear the roles with the sizes and colours they had, and the other surfaces follow one at
  a time. A check now refuses a new font size, weight or text colour written outside them
  (#1920).
