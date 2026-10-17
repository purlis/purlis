### Security

- **A runner starts a chat's confinement only from a plan that fits it.** A plan the desktop
  relays must put the chat's ssh route in the runner's own folder for the project. It may give
  the chat only the services on the runner that the runner's own sandbox for the project gives.
  The chat reaches an internal address only where the runner's machine policy lists it under
  the new `sandbox.relayed-internal` key. An internal address is a private, cluster or
  link-local one, the runner's own, or one on a network the runner is on. That holds whether
  the plan names the address or a name that resolves to it, and it is checked again at every
  connection. Any other plan is refused with a sentence saying what does not fit, and nothing
  starts (#2445).
