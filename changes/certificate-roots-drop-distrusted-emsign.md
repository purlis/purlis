### Security

- **Certificate checks reject the distrusted emSign intermediates.** purlis's own HTTPS (engine
  updates, the updater) uses `rustls-webpki` 0.103.16 and `webpki-roots` /
  `webpki-root-certs` 1.0.10 (RUSTSEC-2026-0381, -0382, -0383).
