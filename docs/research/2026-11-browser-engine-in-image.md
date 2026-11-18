# Browser engine in the runner image, or downloaded on first use

Research for the map "Remote you can trust". Question: build the browser engine into the runner
image, or download it the first time a browser is used (today: up to 1 GiB, on a runner's first
browser)? No product code changed.

**How to read the labels.** **V** = verified in this repo at the cited line. **E** = a fact about
the outside world, which must come from its owner's own page. **E-unverified** = an outside fact
this session could not fetch (see "What this research could not do"); the owner's page to check
is named. Nothing marked E-unverified is a finding.

## What this research could not do

Every outside request from this session (`curl` from the shell, and a research sub-agent with web
tools) was refused by the session's permission classifier. So every outside fact below is
**E-unverified**: the primary page that settles it is named, and the recommendation is written so
that it holds under either answer, or says which answer it needs. Someone with network access
should close the list in "To verify" before the decision is ruled.

## What the code does today (V)

- **The engine is Chrome for Testing (CfT), pinned.** `crates/browser-engine/engine-pin.json`
  names version 157.0.8103.38, issued 2026-11-09, with a URL and SHA-256 for `mac-arm64`,
  `mac-x64`, `linux64` and `linux-arm64`, all on
  `storage.googleapis.com/chrome-for-testing-public/`.
- **The pin is the only proof.** "Chrome for Testing publishes no signature or digest of its
  own, so the pin is the only proof there is" (`install.rs:1-12` doc comment). An archive runs
  only after its SHA-256 matched the trusted manifest's, before a byte was unpacked; a mismatch is
  deleted (same comment). The download is capped at `MAX_ARCHIVE_BYTES` = 1 GiB, "several times
  Chrome for Testing's" (`install.rs:55`).
- **Two manifests can be trusted:** the one the build ships (trusted because it is inside the
  updater-verified build, ADR 0042), or a newer one whose Sigstore bundle verifies offline against
  a trusted root shipped in the build, from the identity
  `…/.github/workflows/engine-manifest.yml@refs/heads/main` (ADR 0042 amendment 2026-10-16).
  `engine-manifest.yml` re-reads CfT's last-known-good stable version daily (cron `17 5 * * *`),
  downloads each platform's archive, records its SHA-256 and signs keyless with cosign.
- **The engine ages.** It is "behind" from the issue time of the newest trusted manifest: warning
  at 7 days, refused with "Update now" at 21 days unless the person allowed an outdated engine
  (`install.rs:33-36`, `:493-494`). A host checks for a newer manifest 5 minutes after it starts
  and every 6 hours (`purlis-core/src/browsers.rs:196-199`), **only once the pinned engine has
  been used on that machine** (ADR 0042 amendment).
- **The image carries the engine's libraries and a font, not the engine** (`runner-image/Dockerfile`
  header comment and the `apt-get install` line; `docs/runner-image.md` "What a browser on a
  runner needs"). A team's image must add the libraries itself (same section).
- **Image size and build.** About 530 MB, of which `gh` and `glab` are about 90 MB (amd64) and
  85 MB (arm64) (`docs/runner-image.md:123`). Ubuntu 26.04, pinned by digest; packages are "the
  archive's current ones at build time, so every build of `main` carries the security fixes of
  that day" (Dockerfile header). Built once per architecture on a native runner, no emulation
  (`runner-image.yml` header). The release's image is built and signed per release; a build for
  any branch is pushed nowhere (`runner-image.yml`, `on:` comment).
- **Where a download lands:** `<data>/engines/chrome-for-testing` in the runner's home. The audit
  records that "a runner downloads again if its home is new" and that a stopped runner's home is
  kept (ADR 0089 amendment 2026-11-02) (audit B5).
- **A cold first page today** has the engine download as one serial step of a start that holds a
  window-wide mutex (audit section C, steps 3, 6, 7; `app/src-tauri/src/browsers.rs:1303`). The
  start may take up to 3720 s (`A_START_TAKES_AT_MOST`, `browsers.rs:243-244`), a figure whose
  stated rationale is the first download's size (`browsers.rs:239-242`). No step is timed
  anywhere (audit E), so no measured cold-start figure exists.
- **The download needs egress.** A runner reaches `storage.googleapis.com` for the archive
  (the manifest reads reach `github.com`). A runner with a tight reach (a policy, a network
  policy) cannot fetch it. `docs/runner-image.md:160` already lists this kind of reach for the
  harnesses. An engine in the image removes this need for the first start, not for updates.

## The comparison

### 1. Image size and pull time

- **V:** the image is about 530 MB today. A baked engine adds the unpacked engine, per
  architecture.
- **E-unverified:** CfT's linux64 zip is on the order of 150 to 200 MB compressed and 500 to 600 MB
  unpacked, as the author remembers it. The 1 GiB cap is a safety bound, not the size. Confirm with
  the `Content-Length` of the pinned URLs above and `du` of an unpack.
- **Reasoning, not a measurement:** a baked engine roughly doubles the image to about 0.7 to 1.1 GB
  on the wire, paid by **every** runner of the pool, including the ones that never open a
  browser. Layers pull in parallel and are shared by digest, so a layer that does not change
  between image builds is pulled once per node, not per runner (OCI image spec; E-unverified,
  check docs.docker.com "storage drivers" and the OCI image-spec "layer" section). A layer
  placed before the `purlis` binary layer stays cache-stable across purlis releases.
- Downloaded on use, the cost moves from every runner to the runners that open a browser, and is
  paid once per home, not once per image pull.

### 2. How often the image must change as the engine gets security releases

- **V:** the engine already has an update path that does not need an image: the daily signed
  manifest and the 6-hourly check. The image today has no engine to update, so Chrome's cadence
  does not touch it.
- **E-unverified:** Chrome stable ships a new major about every four weeks and security fixes
  between, commonly weekly (chromereleases.googleblog.com; chromiumdash release schedule).
  Confirm from those pages.
- **A baked engine ages with the image, and the code treats age as a fault.** At 21 days past the
  newest trusted manifest the engine is refused (V). Baked at build time, the engine is as old as
  the image's build. A runner started from a month-old image would therefore either (a) need the
  run-time update path anyway, or (b) refuse to start a browser. So baking does **not** remove
  the download path; it only makes the common case skip it, and the image would need rebuilding at
  least every 3 weeks to be useful unaided. Images are built per release today, not on a
  schedule (`runner-image.yml`).
- **A team's image** would have to do the same: today it adds only libraries
  (`runner-image/team-example.Dockerfile`, `docs/runner-image.md`). Making the engine part of the
  contract is a new obligation on every team.

### 3. Pinning and signature check, build time versus run time

- **Run time (today, V):** pin + SHA-256 before unpack, newer manifests by offline Sigstore
  verification against a root shipped in the build, fail closed, every request in the network log
  (`install.rs` doc comment; ADR 0042 amendment; ADR 0083 amendment 2026-10-16). One mechanism,
  the same on a laptop and on a runner.
- **Build time:** the Dockerfile already has the standard pattern, for `gh` and `glab`:
  `ADD --checksum=sha256:…` in a `FROM scratch` stage, a pin by hand per architecture
  (Dockerfile lines 20-30). The same would pin the CfT zip, taking the URL and SHA-256 from
  `engine-pin.json`. It is checked at build, and what the image carries is then trusted by the
  image's own signature and provenance attestation (`docs/runner-image.md:12-19`, release job).
- **What changes:** two pins to keep equal (the Dockerfile's and the crate's), or one generated
  from the other. `tools/runner-image-contract.mjs` could check they agree (a new check, not
  written). The host's run-time check of an engine found in the image would also have to treat it
  as trusted by digest (the host re-hashes an installed engine today: `installed_digest`,
  `install.rs:918`), otherwise baked and downloaded engines disagree on what "installed" means.
- **Neither route verifies a vendor signature,** because CfT publishes none (V, above). Both
  rest on purlis's own pin. Build time is verified once by the build; run time is verified by the
  host on each install.

### 4. Licence and redistribution (Chrome, Chrome for Testing, Chromium)

All E-unverified. The question is whether purlis may **put the engine in a public image**
(`ghcr.io/purlis/purlis-runner`, public, no sign-in, V `docs/runner-image.md:9-12`), which makes
purlis a redistributor, where today the person's own runner downloads it from Google.

- **Chromium:** open source under a BSD-style licence plus third-party licences of its bundled
  components. Check `chromium.googlesource.com/chromium/src/+/main/LICENSE` and the build's
  `about:credits`. Builds with proprietary codecs (H.264, AAC) carry patent-licence questions the
  source licence does not settle.
- **Google Chrome (branded):** under the Chrome Terms of Service, which restrict redistribution
  (check `google.com/chrome/terms`). Debian/Ubuntu images of `google-chrome-stable` are the
  usual example; purlis does not use that build.
- **Chrome for Testing:** a Google-built binary for automation. Which of the two licences it falls
  under, and whether the project invites redistribution, is **the fact this decision most
  needs**. Primary pages: the `GoogleChromeLabs/chrome-for-testing` README and `LICENSE`, and the
  licence file inside the zip. **Do not rule for baking until this is read.** Baking converts a
  user-side download into an act of redistribution by the project; the download route leaves that
  with Google and the person.
- **Chromium from the distribution** is the alternative that has a clear answer, but on Ubuntu
  the `chromium-browser` package is understood to be a snap (E-unverified; check
  packages.ubuntu.com for 26.04), which does not run in a container image without snapd; Debian's
  `chromium` is a real package for amd64 and arm64 (check packages.debian.org). Using it would
  replace the pin with the distro's version and drop the daily manifest design (a different
  decision, not assessed here).

### 5. Multi-arch (amd64 and arm64)

- **V:** the pin has `linux64` and `linux-arm64`, and `Platform` maps `x86_64` and `aarch64`
  Linux to them (`install.rs:93-94`). The image is built once per architecture on a native
  runner (`runner-image.yml`), so a per-architecture engine layer fits: the existing
  `forge-programs-${TARGETARCH}` stage is the template.
- **E-unverified:** CfT publishing `linux-arm64` for the stable channel; the pin in this repo
  names it at 157.0.8103.38, so it exists at least for that version. Whether it is in every
  last-known-good release is for googlechromelabs.github.io/chrome-for-testing to confirm.
- Both routes handle both architectures. Baking doubles the build's download and the registry's
  stored layers (one per architecture).

### 6. Effect on a cold first page

- **V:** today a cold first browser on a fresh runner pays: the manifest read (up to 60 s, only if
  the engine is behind), the download (up to 3600 s), the unpack and hash, then launch (`FIRST_ANSWER`
  60 s) and navigation settle (audit C, steps 6 to 11). The audit says the dominant step is
  **U** (unmeasured).
- **Baked:** steps 6 and 7 become a local hash check, or nothing. What remains is launch and
  settle. The first browser's time stops depending on the runner's network, its reach rules and
  Google's storage.
- **Not changed by baking:** the window-wide `starting` mutex (audit top problem 2), the 62-minute
  ceiling (problem 3), the missing timing (problem 4). Baking removes the worst case of
  problem 5 but not those; and a pull that is 0.2 to 0.5 GB larger delays **runner creation**
  instead, for every runner.
- **A reused or stopped runner** already keeps its home and its engine (ADR 0089 amendment
  2026-11-02), so for it baking buys nothing. The gain is for runners made fresh and for
  minutes, which is exactly the case ADR 0089 §1 says is meant.

## Options

| Option | Cold first browser | Image | Freshness | Pin and proof | Open risk |
|---|---|---|---|---|---|
| **A. Today:** download on first use | Slowest; needs egress to Google storage | Smallest | Always as new as the daily manifest | One path, offline Sigstore | The 1 GiB / 62-minute worst case |
| **B. Bake the engine; keep the run-time updater** | Fast while the image is under 21 days old | About 0.2 to 0.5 GB larger (E-unverified), per architecture | Image age is the cap; the updater covers the rest | Two pins (build and crate) kept equal | Redistribution terms (item 4); rebuild cadence |
| **C. Bake and drop the run-time download** | Fast | As B | Only as fresh as the image: refused at 21 days | Build pin only | Dead engine in an old image; breaks the manifest design; no |
| **D. Warm start without baking:** a pool-level engine volume, or download at runner creation (not on first browser) | Fast if the volume is warm | Smallest | As A | As A | New moving part; a shared volume across runners is a new store (needs a `**Tier:**` in `docs/plane-format.md`) |

## Recommendation

Keep downloading at run time as the only engine path, and do not bake, **until the licence
question for Chrome for Testing is read from its owner and shown to allow redistribution in a
public image.** Whether the engine is in the image does not change the run-time updater, the pin,
or the 21-day rule; it only changes who pays the first download. A baked engine is as old as its
image, and the code already refuses an engine 21 days behind, so baking (option B) needs the
updater anyway and an image rebuilt at least every three weeks. What it buys is a fast first
page for a fresh runner and no dependence on reaching Google's storage. If that is what the
"Remote you can trust" map needs, do B, not C, with the engine in its own layer per architecture,
pinned from `engine-pin.json` by one generated source, and a contract check that the two agree.
Fix the cold first page's other causes first, since baking leaves the window-wide mutex, the
62-minute ceiling and the missing timing untouched: first add the timing (audit E) so the
download's share of a cold start is measured, not assumed.

## The facts the recommendation depends on

1. CfT publishes no signature or digest, so the pin is the only proof (V `install.rs`). Baking
   does not change this.
2. An engine more than 21 days behind its newest trusted manifest is refused (V `install.rs:494`),
   so a baked engine needs the run-time updater and a fresh-enough image.
3. The updater starts only once the pinned engine has been used on the machine (V ADR 0042
   amendment 2026-10-16); a baked engine must count as "used", or the check never starts. To be
   confirmed in the code before B is built.
4. The runner image is public and signed per release, not on a schedule (V `runner-image.yml`,
   `docs/runner-image.md:9-19`), and is about 530 MB today (V `:123`).
5. The pin names `linux64` and `linux-arm64` and the image is built natively per architecture
   (V), so multi-arch is not a blocker.
6. Nothing is timed in the start path (V audit E), so the download's share of a cold start is
   unmeasured.
7. **E-unverified, decisive for B:** that CfT's licence allows redistribution in a public image.

## To verify (needs network)

| # | Fact | Primary source |
|---|---|---|
| 1 | CfT licence and redistribution terms; licence file inside the zip | github.com/GoogleChromeLabs/chrome-for-testing (README, LICENSE); the zip |
| 2 | Chrome ToS on redistribution, for contrast | google.com/chrome/terms |
| 3 | Chromium licence and codecs note | chromium.googlesource.com/chromium/src (LICENSE, `about:credits`) |
| 4 | Compressed and unpacked size of `linux64` and `linux-arm64` | `Content-Length` of the two pinned URLs in `engine-pin.json`; unpack |
| 5 | Stable release and security-fix cadence | chromereleases.googleblog.com; chromiumdash release schedule |
| 6 | `linux-arm64` in every last-known-good-versions-with-downloads entry | googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json |
| 7 | Ubuntu 26.04 `chromium` is a snap; Debian `chromium` for amd64 and arm64 | packages.ubuntu.com; packages.debian.org |
| 8 | Layer sharing and parallel pull | OCI image-spec "layer"; docs.docker.com storage drivers |
| 9 | Whether a baked engine counts as "used" for the updater's start condition | `crates/purlis-core/src/browsers.rs` `keep_current` (local code read) |
