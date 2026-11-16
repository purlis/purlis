### Security

- **A sandboxed chat's proxies read only a host a URL client sends.** The egress proxy and the
  browser reach proxy, on their HTTP and SOCKS ports, now refuse a request whose host is not a
  name of dot-separated labels of letters, digits, `-` and `_` (none empty), a dotted IPv4
  address or a bracketed IPv6 address:
  `1.2.3.4<tab>.github.com`, `evil.test@x.example.com` and `0x7f.1` were read as names, so a
  listed `*.github.com` matched requests curl and Node never make. The name dialled was always
  the one judged, so none reached another host. Found by the parsers' new fuzz targets (T1.1-47,
  #6894).
- **A plain request to a bracketed IPv6 host with no port is judged by its host.** `http://[::1]/` is sent
  with `Host: [::1]`, which the proxies split at the colon inside the brackets and answered
  `403` whatever the policy said; they now read it as its target's host and judge it as any other.
