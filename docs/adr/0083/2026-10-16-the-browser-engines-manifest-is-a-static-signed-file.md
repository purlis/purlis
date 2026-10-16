# Amendment, 2026-10-16: the browser engine's manifest is a static signed file beside the updater

**Ruled by the operator on 2026-10-16** (#2073): *"The background fetch is listed in the
network record (ADR 0083)."*

§9 names the Charter-hosted reads a run without an account makes: the signed updater's and the
static signed files beside it. The browser engine's manifest and its Sigstore bundle
(ADR 0042, amended the same day) are two more such files, published as assets of purlis's
`engine-manifest` prerelease. They are reads of files and carry no identifier.

- **Listed as the updater's.** Each read is a line of the network log with the feature
  `updater`, as the updater's own reads are, and is a purlis line, its path kept whole. No
  new feature or field is added to the log.
- **Only where the engine is used.** The session host reads them in the background only once
  the pinned engine has been used on its machine, and before a browser starts on an engine
  behind past its warning. A machine that never runs a browser on the pinned engine reads
  neither.
- **The archive a newer manifest names** is downloaded from Chrome for Testing's own bucket, a
  third party, and listed the same way, as the engine's first download is.
