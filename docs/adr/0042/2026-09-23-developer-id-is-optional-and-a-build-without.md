# Amendment, 2026-09-23: Developer ID is optional, and a build without it is ad-hoc signed

§3 said *a published macOS build is signed with a Developer ID Application certificate*, and
`release.yml`'s preflight enforced it: with `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`
and `APPLE_SIGNING_IDENTITY` unset, every publish was refused before the build. That check ran
before twenty minutes of building, which was the right shape and is why it blocked everything:
on a repository with the minisign key set, the public key committed and the `dev` release
created, **nothing could publish on any channel**, because the one remaining requirement costs
an Apple Developer account that purlis does not have and is not getting.

**The sentence changes. A published macOS build is signed; whether Apple can name the signer is
optional.** When the three secrets are set the bundle is Developer ID signed exactly as above.
When none of them is, it is **ad-hoc** signed.

The reason this is a small decision and not a retreat is the separation §3 already draws.
**Apple signing is about Gatekeeper; the updater's trust is minisign.** Nothing in the update
chain consults Apple: the app fetches a manifest over TLS, verifies a minisign signature made
by the operator's key over the artifact's exact bytes, checks the version out of the signature's
trusted comment (`requireSignedVersion`), and only then unpacks. An ad-hoc bundle changes none
of that. What it costs is one dialog on one first install from a browser. What requiring it
cost was the entire milestone.

**Ad-hoc is not a weaker way of being unsigned — it is the floor.** On Apple Silicon a Mach-O
with no signature at all does not execute, so every arm64 build is signed by someone, and the
only question is by whom. There is a trap here worth writing down, because it is the obvious
implementation and it is wrong: leaving `APPLE_SIGNING_IDENTITY` unset does *not* produce an
ad-hoc bundle. The bundler's `keychain()` returns `None` for an absent identity and then signs
nothing, which leaves each executable carrying the linker's own ad-hoc signature while the
*bundle* has no seal and no `_CodeSignature` — a state `codesign --verify --deep --strict`
rejects outright. So the workflow always names an identity, `-` when it has nothing better,
and the bundler runs `codesign --force -s -`. It must be the bundler's own signing step rather
than a `codesign` call bolted on afterwards, because the signature has to be inside
`charter.app.tar.gz` before that file is minisigned; anything signed later would not be the
bytes the updater verifies.

**What was rejected, again:** making the updater's minisign key optional in the same breath.
It is not the same trade. Apple's signature protects a first install; minisign protects every
update after it, and an updater without verification is a remote-code-execution channel. The
preflight still refuses a publish with no `TAURI_SIGNING_PRIVATE_KEY` or a placeholder pubkey,
and `release-manifest` still refuses over the actual bytes.

## What the check had to become

`codesign --verify` on a bundle answers *is this signed*, and that is now the wrong question in
both directions. An ad-hoc bundle must not pass a check that was asking for Developer ID — but
equally, a run that *had* all three secrets and somehow produced an ad-hoc bundle must not pass
either, which is precisely the set-but-wrong-certificate case §3 introduced the check for. So
the signing mode is decided once, in the `plan` job, carried to the build, and asserted by
name: `Authority=Developer ID Application:` for one, `Signature=adhoc` **and**
`TeamIdentifier=not set` for the other. `--verify --deep --strict` runs in both modes, because
it is the one thing that separates a sealed ad-hoc bundle from an unsealed one — both print
`Signature=adhoc`.

A **half-set** certificate is now refused, and that is new. It used to be caught by the
all-three requirement; without a case of its own it would read as *no certificate* and quietly
ad-hoc sign a run that was trying to use a real one.

## App Management: the question this record left open, now measured

§3 said the same-team rule was *Apple's documented behaviour and was not measured here*. Ad-hoc
has no team at all, so the worry was concrete: if an ad-hoc app cannot replace its own bundle
without App Management permission, every update would raise a permission prompt, and the
operator needed that answer before relying on it rather than after.

**Measured on macOS 26.2, Apple Silicon, 2026-09-23.** A replica of `tauri-plugin-updater`
2.12.0's macOS `install_inner` was compiled into a small ad-hoc-signed `.app`, installed in
`/Applications`, and launched through LaunchServices with `open` — so the process replacing the
bundle *is* the bundle, which is the only configuration in which the self-update exemption is
in play, and so that its TCC identity is its own rather than a terminal's. It then did what the
plugin does: extract into `$TMPDIR`, `rename` the installed bundle aside, `remove_dir_all`,
`rename` the new tree into place.

| | result |
|---|---|
| `rename` of its own bundle, new tree ad-hoc signed with a **different** cdhash | **succeeded**, no prompt, no `EPERM` |
| App Management permission | **never requested**; `tccutil` afterwards reported no entry for the app at all |
| `com.apple.quarantine` set on the old bundle, then replaced | **gone** — only `com.apple.provenance` on the new tree |
| the replaced bundle, relaunched through LaunchServices | **launched**, and updated itself again |
| `spctl -a -t exec` on the ad-hoc bundle | `rejected` — unchanged, and this is the first-install cost, not an update cost |

**The control matters more than the result.** In the same process, immediately before the
rename, the probe tried to read `~/Library/Application Support/com.apple.TCC/TCC.db` and was
**denied**. TCC was enforcing against that process identity, so the rename passing is the
system's answer and not a grant inherited from whatever launched it.

This also settles something §3 got backwards by omission. §3's quarantine table was *already*
measured on an ad-hoc probe — it says so, and lists "a Developer ID signature" under what was
not tested. So the quarantine finding carries to this decision directly; it is the **Developer
ID** path that remains the unmeasured one, not this one.

## What was not measured, stated so nobody has to rediscover it

- **The first-install dialog for an ad-hoc bundle.** Launching a quarantined ad-hoc app was not
  attempted: it would put a Gatekeeper dialog on the operator's screen. The quarantine row
  above was obtained by marking the bundle quarantined while it was already running, which is
  the same filesystem state without the dialog. So it is **not established** that an ad-hoc
  build shows the same *"purlis" Not Opened* / **Open Anyway** flow as an un-notarized
  Developer ID one; macOS has a blunter *"is damaged and can't be opened"* wording for apps it
  cannot attribute. `docs/updating.md` therefore gives `xattr -dr com.apple.quarantine` as the
  instruction that works either way, and does not promise Open Anyway.
- **The real bundle.** The probe is a single small binary in a minimal `.app`, not `charter.app`
  with its WebKit-linked executable and `purlis` sidecar. The syscalls are the same; the tree
  is bigger.
- **A real browser download.** As before, the quarantine attribute was written by hand.
- **Why** it passed. That an ad-hoc bundle is simply not a bundle App Management protects — for
  want of a signature identifying anyone to protect it for — is the likely explanation, and it
  is an inference, not a measurement. It follows that Apple could change this without changing
  anything documented. The first ad-hoc release should be watched for a prompt on its first
  real update, and this section amended if one appears.

## Consequences

- **A first install is rougher, and only a first install.** One `xattr` command, or whatever
  Open Anyway does on the operator's macOS version.
- **Adopting a Developer ID later is not blocked.** The Developer ID path is kept whole, tested
  in the same check, and turned on by setting the three secrets. Moving from ad-hoc to a real
  team is the one-time reinstall any team change is — §3's *keep the team* rule applies from
  the moment there is a team, not before.
- **purlis can publish to the dev channel with the secrets that exist**, which was the point.
