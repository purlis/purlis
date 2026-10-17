# ADR 0086, amended: the Linux cold start gates against main again, on its record (2026-10-17)

**Decided in implementation (#1866, with #1876).** Amends the 2026-10-13 amendment
(`2026-10-13-linux-cold-start-is-held-to-main-measured.md`) only in that the gate it decided gates
again on pull requests; nothing in its rule changes.

From train 87 the gate only reported on pull requests too (#1876): its first runs held the same app
code at 0.98 in one job and 1.20 in another. Its annotations on 524 pull request runs, from
2026-10-14 to 2026-10-17, were then read: 1,572 cases.

- **Against main**: 1,567 cases between 0.80 and 1.10, decided in one pass. Five, in two runs
  (38704550907 on a test-only commit, 38861993257), were past 1.15 in both passes: one build
  steadily slower than the other through the whole job, not a slow launch now and then. The
  commits either side of each were at 1.00. Two false reds in 524 runs.
- **The absolute limit, on the same launches**: the median missed 2 s in 39 cases, on runners
  whose medians went from 838 to 2611 ms.

So the gate gates again on pull requests, as the 2026-10-13 amendment decided. The cause of the steady gap is not
known: each round line now carries both builds' split, and the CPU time and major page faults of
each launch's process tree at its first frame (`processTreeUse`), so the next one says whether a
build read its binary off the disk on every launch, ran more code, or neither.
