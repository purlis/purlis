# Amendment, 2026-10-16: the browser engine's manifest is a static signed file beside the updater

**Ruled by the operator on 2026-10-16** (#2073): *"The background fetch is listed in the
network record (ADR 0083)."*

§9 names the purlis-hosted reads a run without an account makes: the signed updater's and the
static signed files beside it. The browser engine's manifest and its Sigstore bundle
(ADR 0042, amended the same day) are two more such files, published as assets of purlis's
`engine-manifest` prerelease. They are reads of files and carry no identifier.

- **Listed as the updater's.** Each request for them is a line of the network log with the
  feature `updater`, as the updater's own reads are, and a purlis line keeps its path whole.
- **The engine's archive is listed as the browser engine's.** Chrome for Testing's archive of
  the version a trusted manifest names, from Chrome for Testing's bucket, is a third party's
  file: its line has the feature `browser-engine`, the engine's first download included.
- **Every request, redirects included.** A redirect is a line of its own, with its host and
  status, and a line says the answer came only once the whole answer was read.
- **Only where the engine is used.** The session host reads the manifest in the background
  only once the pinned engine has been used on its machine, and before a browser starts on an
  engine behind past its warning. A machine that never runs a browser on the pinned engine
  reads neither.
