# Amended for #2530, the internal-address floor reads NAT64 and ranges, 2026-10-18

This note adds to the amendment for #2445 (a relayed plan fits the runner) and changes none of
its rulings. Its items 1 and 2 stand, with what follows. Each item is **decided in
implementation, to be confirmed** (D-2530-1 to D-2530-4).

1. **An IPv6 address under a NAT64 prefix is judged as the IPv4 address it carries, wherever
   the prefix puts it.** RFC 6052 places the IPv4 address after a prefix of 32, 40, 48, 56, 64
   or 96 bits, skipping bits 64 to 71. Two kinds of prefix are read:
   - NAT64's local-use prefix, `64:ff9b:1::/48` (RFC 8215), may be used at any of those lengths
     from `/48`. An address in it is read at each of them, and it is internal where any reading
     is. A reading in `0.0.0.0/8` is left out, because no NAT64 translates one.
   - The runner's own NAT64 prefixes are read from the runner, as its resolver answers
     `ipv4only.arpa` (RFC 7050), and each is read at the length that answer shows. A reading is
     kept five minutes.

   The well-known forms keep their one reading. *Rejected:* reading the local-use prefix as a
   `/96` alone, which misses an address its NAT64 put at another length. *Rejected:* trusting a
   reading only where the bits RFC 6052 leaves zero are zero, because a translator that ignores
   them reaches the address all the same. *D-2530-1.*
2. **`sandbox.relayed-internal` takes ranges as well as hosts.** A range is written
   `10.96.0.0/12` or `[fd00:10:96::/112]` (or `fd00:10:96::/112` with no port), each with an
   optional `:port`. It carries every address in it, on that port where it names one, and never
   a name. A range is written as its network, so one with bits set past its prefix is refused,
   and the refusal spells the network. A range is also refused where it takes in anything no
   host may be:
   - this machine (`0/8` or `127/8`);
   - a link-local, broadcast or multicast address;
   - a cloud metadata address;
   - an IPv6 form that carries an IPv4 address (mapped, SIIT, compatible, NAT64 or 6to4).

   So `0.0.0.0/0` is never an entry. *D-2530-2.*
3. **One bad entry is refused alone.** An entry of `relayed-internal` that is neither a host nor
   a range, or is not text, carries nothing and is said in a sentence naming it. The rest of the
   list and every other lock of the file stand as written. Two places say the sentences:
   - the runner's refusal of a plan that lists an internal address its policy does not list;
   - `purlis doctor`'s `sandbox relayed internal` row, a warning.

   A `relayed-internal` that is not a list still refuses the whole file. An entry left out can
   only carry less, but a value of the wrong shape says nothing purlis can read. *D-2530-3.*
4. **An interface's mask is read as the address alone where it says more than one network
   could be.** That covers a mask of `/0`, one shorter than `/8` (`/16` for IPv6), one of the
   other family, and none at all. `hosts::network_of` is the one place that rule is read, and
   each case has a test. *D-2530-4.*
