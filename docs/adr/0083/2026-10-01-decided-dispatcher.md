# Decided (dispatcher, 2026-10-01)

- **D-0083a:** *"purlis does not emit `charter.on_behalf_of` on the device, which keeps V1 and
  V25 intact. An org member id may be added under org policy once org projects exist, after
  GT-CLOUD. The keyed-pseudonym scheme is dropped."* Applied in §3, §6 and the amendments of ADR
  0066 and ADR 0075.
- **D-0083b:** *"OTLP from a sandboxed chat goes through charter's existing loopback egress proxy
  (ADR 0067 §3). There is no new sandbox allowance. A whole-process sandbox (ADR 0067 §6) states
  that it reaches the receiver only through the proxy."* Applied in §2 and the amendments of ADR
  0067 and ADR 0068, and extended to unsandboxed chats so there is one path.
