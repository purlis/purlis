### Security

- **A runner's internal-address floor reads every NAT64 layout and takes ranges.** An IPv6
  address under NAT64's local-use prefix is read at every length it may be used at, and one
  under the runner's own NAT64 prefix is read where that prefix puts it. The runner reads its
  prefix from its resolver. In either case the address is judged as the IPv4 address it
  carries. `sandbox.relayed-internal` takes ranges such as `10.96.0.0/12`. An entry that is
  not taken is named on its own and leaves the rest of the policy as written (#2530).
