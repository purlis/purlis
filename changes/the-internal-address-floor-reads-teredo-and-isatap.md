### Security

- **A runner's internal-address floor reads Teredo and ISATAP addresses.** A Teredo address is
  judged as the server and the client IPv4 addresses it carries, and an ISATAP address as the
  IPv4 address it carries and as itself. `sandbox.relayed-internal` has to list each internal
  one as that IPv4 address, and a range that takes in Teredo addresses is refused (#2678).
