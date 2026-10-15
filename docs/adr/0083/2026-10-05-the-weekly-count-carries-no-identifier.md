# Amended for OB-17 (2026-10-05): the weekly count carries no identifier

§9 names *"the weekly manifest asset (V13, OB-17)"* among the purlis reads that *"carry no
identifier"*. OB-17 (#688) builds it, and this section records how it keeps that promise.
`docs/updating.md`, *Counting weekly users*, is the operator's account of it.

- **What is sent.** Each release publishes its manifest twice with the same bytes:
  `latest.json` and `latest-weekly.json`, or `dev.json` and `dev-weekly.json`. A machine's first
  update check of each ISO week (UTC) reads the weekly file in place of the manifest. No request
  is added. The request is the updater's ordinary GET: one URL with no query and no template
  variable, and only fixed headers: `Host`, `Accept`, `Accept-Encoding` and the updater
  library's user agent. It is byte for byte the same from every machine. A test runs three machines with their own device ids and holds that.
- **What is kept.** The machine store notes the week (`machine.json`'s `weekly`). The note
  never leaves the device, and every machine that checked that week holds the same value.
- **What is counted.** GitHub's `download_count` of the weekly file, per release. The estimate
  of a channel's weekly users is the sum, over that channel's releases, of the growth of that
  count between two listings a week apart (`weekly-users`, in `crates/release-manifest`).
  purlis publishes no estimate; the raw count is public, as every asset's is (V38). The
  operator keeps the listings.
- **What GitHub sees** is each request's IP address and user agent, as for every update check.
  This is disclosed in `docs/updating.md`. purlis never receives an IP address, and no store
  of purlis's holds one (X50).

## Decided in implementation (OB-17)

- **D-OB17a: no identifier at all, not even a rotating one.** The weekly request carries no
  value of its own. It sends no install id, no device id, no hash, and no salted or per-week
  token. **It cannot be linked across weeks or devices because there is nothing in it to
  link.** Two machines send the same bytes, and one machine sends the same bytes in two weeks.
  What is left to correlate is GitHub's own view of the transport, the IP address and the
  user agent. Every update check already exposes that view, and purlis never sees it.
  Rejected: a per-week random salt over an install id, which is unlinkable across weeks only
  while the salt is secret and still lets a log join a device's requests within a week. Also
  rejected: k-anonymous bucketed ids, which buy deduplication this estimate does not need,
  at the cost of sending something.
- **D-OB17b: the week is claimed before the request and given back without an answer.** A
  machine that cannot note the week is never counted, because otherwise it would be counted at
  every check: no config home, or a platform with no machine store. A request that was never
  sent, or got no answer, including a 404 from a release published before the weekly file existed, gives the
  week back. That check then reads the usual manifest. Rejected: noting the week after the
  answer, which counts a machine with no store at every check.
- **D-OB17c: the opt-out is `DO_NOT_TRACK`.** purlis has no telemetry switch of its own yet:
  OB-12's product telemetry and §7's collection setting are not built. The cross-tool
  `DO_NOT_TRACK` convention is the opt-out that already exists, so purlis honours it. Any value
  but empty or `0` opts out. The weekly count is the updater's (§9), not product telemetry, so
  it is on unless opted out, as V13 rules. A settings switch is a follow-up, made when OB-12's
  switch exists.
- **D-OB17d: any week other than the noted one is due.** A clock that was once wrong ahead
  must not stop the count until that date comes. The cost is a machine whose clock jumps
  between weeks being counted twice, which is a stated bias.
- **D-OB17e: dev's count restarts at every dev build.** Replacing an asset resets its count,
  and dev replaces its manifests at every green `main`. Every workaround publishes a tally
  file, which is a new public format, so none is taken. Dev's count is read as covering the time
  since its last build, and the report prints the asset's upload time. Stable's assets are
  replaced only when a publish is re-run.
