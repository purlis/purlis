# The shapes of a subscription-shaped credential (spike T1a-18, R-711)

This answers design §6.2 item 32 (gap C-17): what a Claude plan sign-in and a ChatGPT plan sign-in
look like when they reach the key proxy where the run's token belongs. T1a-28's classifier
implements the rules below. `samples.json` holds one synthetic sample per shape and per negative,
and `tools/subscription-shapes.test.mjs` (`subscription_shape_fixtures_classify_as_recorded`)
checks each label against these rules. Every value in the samples is made up. No real credential
was read, printed or used.

**Read:** 2026-11-15, on macOS (Darwin 27.2.0).

**Sources:**

- **Claude Code 2.1.325** (the version the 1a spikes pin): the strings of the installed binary
  (`strings -n 6`), quoted below.
- **Codex CLI 0.169.0** (`@openai/codex`, `codex-darwin-arm64`): the strings of its native binary,
  which is built from the open-source `openai/codex` repository (`codex-rs/login`).
- Vendor web documentation could not be fetched from this sandbox. The binaries' own code is the
  primary source for both rules.

## The verdict

A request is **subscription-shaped** when any credential header carries a value matching C1 or C2.
The credential headers are the ones the run's token can arrive in:

- `Authorization`, when its scheme is `Bearer` in any case (RFC 9110 §11.1). The value is the
  token after the scheme and its spaces, trimmed.
- `x-api-key`, the whole value, trimmed.

Any other header, and any other scheme (`Basic …`), is not read. The check runs before the token
check (design §3.3), and a request with a plan token beside the run's token is still
subscription-shaped.

### C1: a Claude plan sign-in (label `claude-plan`)

**Rule:** the credential value starts with `sk-ant-oat` or `sk-ant-ort`.

- **Header:** `Authorization: Bearer <token>`. On a plan, Claude Code passes the OAuth access token
  as the SDK's `authToken` with `apiKey: null`, and the SDK sends it as a bearer:
  `function aPa(e){if(e.isSubscriber)return{apiKey:null,authToken:e.oauthAccessToken??null};return{apiKey:e.apiKey,authToken:null}}`.
  The same token, from `claude setup-token`, arrives through `CLAUDE_CODE_OAUTH_TOKEN`.
- **Form:** `sk-ant-oat01-` followed by URL-safe base64 characters. Claude Code's own test for one
  is a prefix: `J$.isAnthropicOAuthToken = (e)=>e.startsWith("sk-ant-oat")`. Its secret scanner
  names the full form, and the refresh token's: `anthropic-oauth-token`,
  `\b(sk-ant-(?:oat|ort)\d{2}-[\w-]{20,})`. A refresh token is only sent to the token endpoint
  (`/v1/oauth/token`), but if one arrives it is still a plan credential, so C1 takes both.
- **Marker header:** `anthropic-beta` carrying `oauth-2025-04-20` (the binary's constant
  `Hu="oauth-2025-04-20"`, beside the OAuth scopes). It comes with the token, so the rule does not
  need it, and it is never a credential on its own: a request carrying it with the run's token is
  not subscription-shaped.
- **Not measured on the wire.** The ticket asks for a local recording server. This sandbox refuses
  to bind a local port (`bind: Operation not permitted`), so the header was read from the binary,
  not seen. To see it, run a local recorder that prints a credential only when it equals the
  synthetic one, where binding is allowed, with Claude Code started as:
  `CLAUDE_CODE_OAUTH_TOKEN=<synthetic sk-ant-oat01-…> ANTHROPIC_BASE_URL=http://127.0.0.1:<port>`
  with a throwaway `HOME` and `CLAUDE_CONFIG_DIR`. T1a-28's real-binary test can confirm it.

### C2: a ChatGPT plan sign-in (label `chatgpt-plan`)

**Rule:** the credential value is a JWT, meaning three dot-separated base64url segments, the first
decoding to a JSON object with a string `alg` and the second to a JSON object, whose payload
carries an object under the claim `https://api.openai.com/auth`. The signature is not checked:
this is a shape, not a verification.

- **Header:** `Authorization: Bearer <access_token>`, and with it the account header
  `chatgpt-account-id: <account id>`, both seen in Codex's request code (`codex-cli` and
  `chatgpt-account-id` together, more than once). Codex sends these to
  `https://chatgpt.com/backend-api/codex`.
- **Form:** the tokens come from `https://auth.openai.com/oauth/token` (`id_token`,
  `access_token`, `refresh_token`). Codex reads the account and plan from the JWT payload's
  `https://api.openai.com/auth` object (`chatgpt_account_id`, `chatgpt_plan_type`,
  `chatgpt_user_id`), and fails with
  *"JWT payload missing expected 'https://api.openai.com/auth' object"* when it is absent.
- **Account header:** it accompanies the token, so the rule does not need it. On its own it carries
  no credential (open question Q3).

### The negatives (label `not-subscription`)

These never match, and the samples hold each:

| Rule | Never subscription-shaped | Why it can't match |
|---|---|---|
| N1 | An Anthropic API key, in `x-api-key` or as a bearer: `sk-ant-api03-` + 93 characters + `AA`; an Admin key, `sk-ant-admin01-…AA` (the binary's scanner: `anthropic-api-key`, `anthropic-admin-api-key`) | Its prefix is `sk-ant-api` or `sk-ant-admin`, never `sk-ant-oat`/`ort`, and it has no dots |
| N2 | The run's token: 32 random bytes (T1a-05, R-063), sent as `Authorization: Bearer` from `ANTHROPIC_AUTH_TOKEN` | Hex can never start with `sk-`, and has no dots. See Q1 for base64url |
| N3 | No credential header, an empty `Authorization`, `Bearer` with no token, an empty `x-api-key` | No value to match |
| N4 | Garbled values: another scheme, the prefix not at the start, a JWT without OpenAI's claim, three segments that aren't JSON, Claude Code's session-ingress token (`sk-ant-si-…`, a remote session's JWT), an OpenAI API key (`sk-proj-…T3BlbkFJ…`) | Not C1's prefix, and not a JWT with the claim |

A request that matches neither rule goes on to the token check, so a credential this spike does
not name is still refused there, only without the Notice. That is why every open question below
proposes leaving the case out of C1 and C2: the safe failure is the token refusal.

## Open questions for the orchestrator

- **Q1. The run's token's encoding.** T1a-05 fixes 32 random bytes but not how they are written.
  As hex (or standard base64, whose alphabet has no `-`), the token can never match C1 or C2. As
  unpadded base64url it could, in principle, begin with `sk-ant-oat` (one in 64¹⁰). Proposed:
  T1a-05 writes the token as lowercase hex, so N2 holds by construction. The samples carry both
  encodings.
- **Q2. A claude.ai web session key.** The binary also knows `sk-ant-sid…`, a claude.ai browser
  session key, which the code read sends only as `Cookie: sessionKey=…` (with
  `X-Organization-Uuid`) to Claude Code's own claude.ai endpoints, not to Messages. Proposed: leave it out of C1, since a cookie is not where the run's
  token belongs (the cookie is stripped as a header anyway).
- **Q3. The ChatGPT account header alone.** `chatgpt-account-id` without a ChatGPT JWT carries no
  credential. Proposed: it does not classify.
- **Q4. Whether the ChatGPT access token itself carries the claim.** The binary shows Codex reading
  `https://api.openai.com/auth` from a JWT payload, but not whether that is the `access_token` it
  sends or only the `id_token`. If the access token lacks the claim, C2 misses it and the request
  falls to the token refusal without the Notice. That fails safe. A wider rule (any JWT whose `iss`
  is `https://auth.openai.com`) would catch it. Proposed: keep C2 narrow until a ChatGPT
  access token's claims are read from Codex's source (`codex-rs/login`) or a recording.
- **Q5. Codex's other sign-ins.** Codex 0.169.0 also has Agent Identity tokens
  (`CODEX_ACCESS_TOKEN`) and personal access tokens (`PersonalAccessTokenMetadata`), whose forms the
  binary doesn't show. Proposed: out of C2 until measured. They fall to the token refusal.
