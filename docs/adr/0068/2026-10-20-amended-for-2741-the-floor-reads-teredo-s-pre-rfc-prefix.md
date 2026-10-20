# Amended for #2741, the floor reads Teredo's pre-RFC prefix, 2026-10-20

This note adds to the amendment for #2678 and changes none of its rulings. Its item is
**decided in implementation, to be confirmed** (D-2741-1).

1. **A Teredo address under `3ffe:831f::/32` is read as one under `2001::/32` is.** Teredo
   clients used `3ffe:831f::/32` before RFC 4380 gave it `2001::/32`, and the address carries
   its server and its client the same way under either. The floor judges it as itself and as
   each IPv4 address it carries, `sandbox.relayed-internal` has to list each internal one as
   that IPv4 address, and a range that takes in `3ffe:831f::/32` is refused, as one that takes
   in `2001::/32` is. Where purlis judges a browser's reach, an address under it is not public,
   as a Teredo address under `2001::/32` is not. *Rejected:* judging it public where both
   IPv4 addresses it carries are, since the prefix was a 6bone block, returned with the rest
   of `3ffe::/16` in 2006 (RFC 3701), and a Teredo address under `2001::/32` is not public
   either. *D-2741-1.*
