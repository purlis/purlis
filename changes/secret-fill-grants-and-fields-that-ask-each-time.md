### Added

- **Secret fill takes Allow for this chat and Always allow.** Always writes one line naming the
  key and the site in the persona's tools, shown on the notice before you press it, and
  `sandbox.proposed-changes = "off"` holds it back. A key marked `card = true` is offered Allow
  once only, and a key missing from its vault goes to the Secret ask (#5116).
- **A multi-line text box or an editable area of a page asks you each time** before a secret goes
  into it, whatever you allowed before, since text there is often posted (#5120).
