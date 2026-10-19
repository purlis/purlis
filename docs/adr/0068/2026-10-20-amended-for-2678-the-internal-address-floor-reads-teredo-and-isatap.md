# Amended for #2678, the internal-address floor reads Teredo and ISATAP, 2026-10-20

This note adds to the amendment for #2530 and changes none of its rulings. Each item is
**decided in implementation, to be confirmed** (D-2678-1 to D-2678-3).

1. **A Teredo or ISATAP address is judged as itself and as each IPv4 address it carries.**
   A Teredo address (`2001::/32`, RFC 4380) carries its server's IPv4 address in bits 32 to 63
   and its client's, every bit flipped, in the last 32. An ISATAP address (RFC 5214) carries
   one in the last 32 bits behind a `0:5efe` or `200:5efe` interface identifier, under any
   prefix; the group bit (`100:5efe`, `300:5efe`) is read too. Such an address is internal,
   refused or a metadata address where it is as itself or where any IPv4 address it carries
   is. Unlike the NAT64, mapped, compatible, SIIT and 6to4 forms, the IPv6 address itself
   keeps its own judgement, since an ISATAP address may sit under a unique-local prefix.
   The same reading holds where purlis judges a browser's reach, so an ISATAP address is
   public only where the IPv4 address it carries is public too. *D-2678-1.*
2. **What the policy lists is judged against each internal IPv4 address such an address
   carries, and against the IPv6 address where it is internal as itself.** Each of them has to
   be covered: an IPv4 entry or range covers the IPv4 address, and the name asked for covers
   what it resolves to, as for NAT64. An IPv6 range around a Teredo or ISATAP address covers
   none of the IPv4 addresses it carries. *D-2678-2.*
3. **A range that takes in Teredo addresses is refused; a range is not refused for taking in
   ISATAP ones.** Every IPv6 network of `/64` or wider takes in every ISATAP interface
   identifier, so refusing those would refuse every useful IPv6 range. Item 2 is what holds
   instead: a range never carries an ISATAP address by itself. *Rejected:* refusing a range
   longer than `/64` that takes in an ISATAP identifier, because it would refuse one sort of
   range and not the wider ones that take in the same addresses. *D-2678-3.*
