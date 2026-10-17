### Security

- **A runner starts a chat's confinement only from a plan that fits it.** A plan the desktop
  relays must put the chat's ssh route in the runner's own folder for the project. It may give
  the chat only the services on the runner that the runner's own sandbox for the project gives.
  The chat reaches a private, cluster or link-local address, or one of the runner's own
  addresses, only where the runner's policy lists it in `sandbox.hosts`. That holds whether the
  plan names the address or a name that resolves to it, and it is checked again at every
  connect. Any other plan is refused with a sentence saying what does not fit, and nothing
  starts (#2445).
