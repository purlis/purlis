# Amended 2026-10-15: a declaration lists the plans a subscription signs in to (#2039)

**Confirmed by the operator on 2026-10-15**, on #2039's proposal, from the profiles spec (#2030)
story 34 (a new harness needs only its declaration) and story 27 (a subscription a harness may not
carry is never offered).

- **`[sign_in] plans`**: the plans a `subscription` is signed in to, where the harness serves
  several and purlis names the one: a list of `{ provider = "<id>", title = "<words>" }`, the
  provider the harness's own id for it, as its sign-in takes it (lower-case letters, digits, `-`,
  `_` and `.`, starting with a letter or digit, each once), and the title the words a window draws.
  **It is an allow-list**: a provider the harness adds later is not offered until it is declared.
  It is listed only beside a `subscription` kind.
- **A harness that declares `subscription` and no `plans`** signs in to its own vendor's plan
  alone, and no provider is named: Claude Code and Codex.
- **One floor no declaration lifts**: `anthropic` is refused in the `plans` of any declaration but
  Claude Code's own, a project's and this machine's included. Anthropic's terms let no other
  harness carry a Claude plan (ADR 0095 §3), and a project may declare harnesses of its own, so the
  rule is purlis's and is checked when the declaration is read.

opencode's built-in declaration lists ChatGPT (`openai`) and GitHub Copilot (`github-copilot`),
where purlis's code held them before (#2148). The sign-in Ask, a profile's row in Settings and
`purlis profile sign-in --provider` offer what the declaration lists. A local replacement of a
built-in keeps its `plans` as it keeps the rest of `[sign_in]`.
