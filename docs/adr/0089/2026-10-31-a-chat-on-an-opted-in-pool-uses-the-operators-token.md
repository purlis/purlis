# Amended 2026-10-31: a chat on an opted-in pool uses the operator's token

**Built by #4207** on the person's rulings of 2026-10-30 on #3807 (Q1, Q2) and of 2026-10-31. It adds
to the amendment of 2026-10-31 on the mint port, and keeps everything else in it.

1. **A Claim's status says where its runner asks**: `status.mint`, `address` and `operatorKey`
   (ruled 2026-10-31), only for a vouched runner on a Pool that opts in. It decides nothing.
2. **A runner is told these as it is paired**, by its desktop: the status's address, and the key the
   desktop pinned, whatever key the status names. A runner needs no way to the Kubernetes API.
3. **A runner holds the port to keys, not to a name or an authority**: the TLS 1.3 handshake's
   signature must be a held key's. The port shows the current key alone during a roll, and the
   runner follows by the current key's word, as a desktop's pin does, holding both until the port
   shows the next.
4. **A mint tool's run on such a runner asks the operator first, and nobody else** (Q1). A refusal, an
   unreachable port or a runner told of nothing hands the line to the desktop, whose gate and Ask
   stand as on any pool: a refused mint never becomes a token without the person.
5. **The desktop adds the repos of each workspace it sends to the Claim's `spec.forge.repos`** (Q2),
   and takes none away while the runner lives, since the runner's other chats use them. A token
   covers the Claim's repos, not one chat's.
6. **A runner keeps its token in memory and revokes it as the chat ends**, as a desktop does (#3808).
7. **The operator may `get` its own mint Service, by name**, for the address, in the Role that
   otherwise reads the keys the chart names, and only where it names one.
8. **A runner told of a mint port starts nothing outside the sandbox but a shell** (ruled
   2026-10-31), saying why, and no chat reads the folder its pod hands its link key over in.
