### Security

- **The cluster operator is built on kube-rs 5.2.0, past RUSTSEC-2026-0600.** kube-client 5.0.0
  with its `openssl-tls` connector trusted OpenSSL's default certificate authorities on top of the
  cluster's own. The operator was never exposed: it builds kube with `rustls-tls`, so the OpenSSL
  connector is not in its binary. The update clears the advisory, and nothing an operator
  installs or configures changes.
