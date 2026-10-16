# Amendment, 2026-10-16: the browser engine's manifest is signed keyless with Sigstore

**Ruled by the operator on 2026-10-16** (#2073), replacing his ruling of 2026-10-15 that a
dedicated minisign key would sign engine manifests. This record's minisign key stays what it
is: purlis's own updates, and nothing else.

The browser engine (#2009) is a pinned Chrome for Testing, kept current apart from purlis's
releases. Each build trusts the manifest it ships (`crates/browser-engine/engine-pin.json`),
because that manifest is inside the build this record's updater verified. A newer manifest is
trusted as follows.

## The ruling

- **A scheduled workflow on `main` of purlis/purlis signs each manifest**:
  `.github/workflows/engine-manifest.yml` reads Chrome for Testing's last-known-good versions,
  downloads each platform's archive, records its SHA-256 in a manifest of the shape the build
  ships, and signs it keyless with cosign through the workflow's own GitHub identity. There is
  no long-lived key to create, store or rotate.
- **The app verifies the Sigstore bundle offline**, against a Sigstore trusted root that ships
  in the app (`crates/browser-engine/sigstore-trusted-root.json`) and is refreshed with each
  purlis release. It accepts only a certificate whose identity is exactly
  `https://github.com/purlis/purlis/.github/workflows/engine-manifest.yml@refs/heads/main`,
  issued by `https://token.actions.githubusercontent.com`.
- **The pin each build carries keeps working with no update ever received.**
- **The background fetch is listed in the network record** (ADR 0083, amended the same day).

## What a bundle must prove

`purlis_browser_engine::sigstore::Sigstore` takes a manifest only when all of these hold:

- the bundle is of the format `cosign sign-blob` makes (v0.3), and every log entry in it
  carries its inclusion proof and a checkpoint (checked by purlis itself, before the rest);
- the bundle's certificate chains to a certificate authority of the shipped root that was
  valid when it signed;
- its identity and issuer are exactly the two above;
- its signature is over exactly the manifest's bytes;
- the signature's entry in a transparency log of the shipped root is in the bundle with its
  inclusion proof and a checkpoint that log signed, under a log key valid when the entry was
  logged, and the entry describes this signature.

A bundle that fails any of them is refused, and the engine in use stays.

**The verifier.** Sigstore's own Rust verifier, `sigstore-verify` (sigstore/sigstore-rust,
Apache-2.0), pinned to one exact version since it is before 1.0, without its default
cryptography backend (aws-lc, whose OpenSSL licence purlis's dependency policy does not
allow) and without an HTTP client. Its cryptography is `ring`, which purlis's TLS already
links, and RustCrypto's `p256` for the one check `ring` does not offer, an ECDSA signature over
a digest, which is how Sigstore checks a signature over a file. Rejected: the older `sigstore`
crate, with a much larger tree, and a verifier of purlis's own, which would be more
security-critical code to keep.

## Where it is published, and how a host takes it

- **Two assets of the `engine-manifest` prerelease** of purlis/purlis:
  `engine-manifest.json` and `engine-manifest.json.sigstore.json`. A prerelease is never
  GitHub's `latest`, so the updater's stable channel never resolves to it. The workflow makes
  the prerelease on its first run and replaces the two assets each day, the bundle first.
- **Each day, and on a push to `main` that changes the workflow or its tool.** Only on
  `main`: never by hand, from a pull request or after another workflow. Before publishing, the
  workflow verifies its own bundle against the root the builds ship, as the identity they
  accept, so a bundle they would refuse is never published. A scheduled run that publishes
  nothing keeps an issue open until one does.
- **The session host reads it in the background** (`purlis_core::browsers`) once the pinned
  engine has been used on its machine: five minutes after it starts and every six hours after.
  A browser that would start on an engine behind past its warning reads it first. A machine
  that never used the pinned engine reads nothing.
- **What supersedes the manifest in use**: a later version, or the same version issued later,
  which says that version is still current, so a daily signature keeps an engine that is
  current from being refused for its age. A manifest naming an earlier version is never taken.
- **The newest manifest read decides.** A received manifest is taken only when it was issued
  after the one in use, whatever version it names. A manifest says which version is current
  as of its issue time, so a host goes by the latest such statement it has read.
- **Bounded reads, no shared wait.** The manifest and its bundle are each read within a
  minute, an engine's archive within an hour, from the request to the last byte. A read never
  holds what a browser's start waits on: only moving an engine into place and keeping an
  update do, one at a time. A check that fails or is refused is written to the host's log,
  and the engine in use stays.

## Rejected

- **A dedicated minisign key** (the 2026-10-15 ruling): a long-lived secret to create, store
  as a CI secret and rotate, for files a workflow identity can sign without one.
- **This record's updater key**: one key for two purposes, so a leak of either use is a leak
  of both.
- **Verifying online, or fetching Sigstore's root through TUF at run time**: a host would
  reach Sigstore's servers on every check. The root ships in the build instead.
