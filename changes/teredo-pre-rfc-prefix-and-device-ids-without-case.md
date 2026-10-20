### Security

- **A runner's internal-address floor reads Teredo's pre-RFC prefix.** An address under
  `3ffe:831f::/32` is judged as the server and the client IPv4 addresses it carries, as one
  under `2001::/32` is, and a `sandbox.relayed-internal` range that takes it in is refused
  (#2741).

### Fixed

- **A runner's device id is compared without case.** Two spellings of one device id that
  differ only in case are one id when purlis keeps a runner (#2741).
