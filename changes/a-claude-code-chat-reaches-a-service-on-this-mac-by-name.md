### Added

- **A Claude Code chat can be allowed a service on this Mac.** Its Notice now offers **Allow
  only for this chat** or **Allow for me on this machine** instead of only Start without the
  sandbox. Its commands reach the port through the chat's proxy by the name
  `this-mac.internal:<port>` (curl, and any client that uses the proxy), or by
  `kubernetes.docker.internal` or `host.docker.internal`, and only while the port is allowed.
  Removing the grant stops new connections at once; one already open ends when the chat
  restarts. For kubectl, **Point kubectl at it…** lists your kubeconfig's contexts and gives the
  chat its own copy of one whose cluster is on this Mac, credentials included, which the chat
  can read; its server is named `https://this-mac.internal:<port>` with the certificate still
  checked. A context elsewhere is listed with why. Your own kubeconfig is never changed (qw384b).
