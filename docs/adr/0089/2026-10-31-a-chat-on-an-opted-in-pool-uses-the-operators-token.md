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
8. **A runner of a Pool naming an App's key, or told of a mint port, starts nothing outside the
   sandbox but a shell** (ruled 2026-10-31), saying why, and no chat reads the folder its pod hands
   its link key over in.
9. **The operator mints only for a pod made with the mark** `PURLIS_MINTING_POOL`, set from its birth
   where the Pool names a key (ruled 2026-10-31: every runner it would mint for refuses). A pod's
   containers never change, so this reads what has held since its first moment.

## Added by #4330, #4331 and #4332

10. **A runner is told again at each link** (amends 2): the same words, `--mint-port --again`. It
    keeps any key the port still shows, and takes its desktop's pin only once the port shows that,
    so a roll it followed is not undone. The desktop also reads `mintInCluster` again at each link.
11. **A chat's start waits at most 5 seconds for its repos to reach the Claim** (amends 5), on a pool
    that opts in alone, so a first mint covers them.
12. **A chat's refused mints are paced per chat and tool** (adds to 4): the port from 5 seconds,
    doubling to 5 minutes. A refused run still reaches the desktop's gate and Ask. Once that refused
    it too, the next runs are refused on the runner on the same pace. A run's record says `"from":
    "cluster"` for a token from the operator.
