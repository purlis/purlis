# Amended 2026-11-01: a runner is told again at a link, and a refused mint is paced

**Built by #4330, #4331 and #4332.** It adds to the amendment of 2026-10-31 on a chat using the
operator's token, and keeps everything else in it.

1. **A runner is told again at each link** (amends its item 2): the same words, `--mint-port --again`.
   It keeps any key the port still shows, takes back no key it moved on from, and takes its desktop's
   pin only once the port shows that, so a roll it followed is not undone. The desktop also reads
   `mintInCluster` again at each link.
2. **A chat's first start waits at most 5 seconds for its repos to reach the Claim** (amends its item
   5), on a pool that opts in alone, so a first mint covers them.
3. **A chat's refused mints are paced per chat and tool** (adds to its item 4): the port from 5 seconds,
   doubling to 5 minutes. A refused run still reaches the desktop's gate and Ask. Once that refused
   it too, the next runs are refused on the runner on the same pace. A run's record says `"from":
   "cluster"` for a token from the operator.
