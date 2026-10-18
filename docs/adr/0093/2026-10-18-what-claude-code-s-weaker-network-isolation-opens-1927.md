# Checked 2026-10-18: what Claude Code's `enableWeakerNetworkIsolation` opens (#1927)

Since R1, every sandboxed Claude Code chat starts with `enableWeakerNetworkIsolation: true`
unless the project or policy turns certificate checks off. purlis's own profile for Codex and
opencode opens one service for the same purpose:
`(allow mach-lookup (global-name "com.apple.trustd.agent"))`. The question was whether Claude
Code's switch opens more than that. It was checked on 2026-10-18 against Claude Code 2.1.302
(`crates/purlis-core/src/sandbox/claude.rs` was read from 2.1.291). Nothing built changes.

- **In the sandbox profile: the same one service, and nothing more.** Claude Code's macOS
  profile builder adds exactly two lines for the switch, a comment and
  `(allow mach-lookup (global-name "com.apple.trustd.agent"))`. It adds no other rule, no
  environment variable and no change to the egress proxy. A Claude Code chat and a wrapped
  Codex or opencode chat therefore have the same certificate check, and the residual R1
  accepted (trustd's own requests for OCSP, CRL and AIA, which do not pass the proxy) is the
  same on every harness.
- **Outside the profile, Claude Code counts the switch as weakened isolation.** Its device
  tools, which serve this machine's files to a linked cloud session, refuse on a sandbox
  configured with the switch ("configured with a setting that weakens its isolation"), as they
  do for `allowLocalBinding`, `allowMachLookup` and `enableWeakerNestedSandbox`. So a
  purlis-sandboxed Claude Code chat with certificate checks on does not serve Claude Code's
  device tools. purlis neither offers nor depends on them. A project that needs them turns
  certificate checks off (`[sandbox].certificate-checks = false`), and loses `gh` and the
  other Go tools, as R1 describes.
- **Recheck this when the supported Claude Code version moves.** The source for each claim
  above is the profile builder and settings schema in the Claude Code binary, read as text. A
  version that widens what the switch opens, for example a second service or an exception in
  the proxy, would change what R1 accepted and should come back to the operator.
