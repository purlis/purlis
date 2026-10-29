# Amended 2026-10-29: a runner installs its profile's pinned harness, and keeps its settings

**Built by #3446 and #3448**, under spec #2041 (*"A profile pins its harness version and the
runner installs exactly it"*; a move sends *"the profile's settings with it"*). It amends the
2026-10-27 amendment's items 1 and 7, which left both to these tickets. Everything here is
**decided in implementation, to be confirmed** (D-3446-1 to D-3446-8 and D-3448-1 to D-3448-9 in
the pull request; D-3448-8 is the steward's). §3 stands: no sign-in is copied between devices.

1. **A profile's settings reach the runner that keeps it** (session protocol 1.50,
   `keep_profile`'s `settings`). At a chat's start or move on a pool, the desktop sends the
   profile's settings files with it: exactly the files a made profile imports
   (`profilestore::ALLOWED`), read from its settings folder as an import reads them, by
   descriptor and following no link, leaving out every hidden file and every file named like a
   credential. The runner keeps them in that profile's folder in place of the ones it kept before,
   and refuses all of them where any path is one an import would not write. A look from
   Settings, a sign-in's watch, and a desktop or runner before 1.50 send none, and the runner's
   settings stay as they are. A profile for sandboxed chats only has purlis's settings, so none
   is sent for it. Settings may carry secrets (an MCP server's environment), so they follow the
   ownership rule vault values follow: **only a runner shown to be the person's own is sent
   any**, by the same check as item 2 of the 2026-10-27 amendment, and nothing is read for any
   other. Item 1's *no setting has a field in what is sent* no longer holds; *no file of the
   desktop's sign-in, and no vault reference, has a field* still does.
   - **No sign-in rides inside a settings file** (D-3448-8, the steward's ruling of 2026-10-29).
     Each harness's own settings file is sent with every environment block removed whole
     (Claude Code's `env`, a Codex server's `env` and its shell policy's `set`, an opencode
     server's `environment`), whatever its variables are called, and with every setting that
     names or runs a credential source (Claude Code's `apiKeyHelper`, `awsCredentialExport`,
     `awsAuthRefresh`, `gcpAuthRefresh` and `otelHeadersHelper`; Codex's bearer tokens, the
     variables that name one, a provider's `env_key` and `auth`; opencode's `apiKey`; and the
     headers any of them sends). A value a chat needs belongs in a vault, and reaches a runner
     chat as vault values do. A settings file that does not parse is refused in words, and
     nothing is sent. The runner removes the same again before it keeps them.
   - **Unlike vault values, settings are kept on the runner.** A vault value crosses once per
     run and is never kept there; a profile's settings are written to the runner's profile
     folder on its home volume, and stay until the next start or move replaces them, or the
     pool's runner is removed with its home volume. Removing the profile on the desktop does not
     yet remove it from its runners.
   - Only a runner of 1.50 or later has the settings read for it, so a settings folder that
     could not be sent refuses no chat on an older one.
2. **A runner chat runs exactly the version its profile pins, installed on the runner.** A
   profile a runner keeps for its desktop with a pin runs that version, never the harness on
   `PATH`. It is installed as a chat first starts on it, and never for a profile no chat starts
   on:
   - from the harness's own release: Claude Code's release bucket, Codex's and opencode's
     GitHub releases, for the runner's platform (Linux, x86-64 or arm64);
   - checked against the SHA-256 its vendor publishes for it (Claude Code's release manifest,
     the digest GitHub publishes for each release asset) before it is unpacked or kept. A
     version whose digest cannot be read, or whose download does not match it, is not installed,
     and the chat does not start and says why;
   - into the runner's data home on its own volume (`<data>/harnesses/<kind>/<version>/`),
     never `/tmp`, which no chat may write.
3. **The desktop's adopted folder carries the desktop's pin on the runner.** It amends item 7's
   *carries no pin*: the runner image has no harness for an adopted folder to follow, so the
   desktop's pin (which follows the desktop's harness, as an adopted folder's pin does) is the
   version the runner installs. An adopted pin is still never a reason to refuse a chat.
