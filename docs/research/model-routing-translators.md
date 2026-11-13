# Open-source wire-format translators for Claude Code and Codex

Research, 2026-11-14, for #6583. Facts only. This document decides no policy; decision #6584
(does purlis translate, or does the person run a translator that purlis only points at) is ruled
on afterwards.

**The question.** Claude Code speaks Anthropic Messages (`POST /v1/messages`) to whatever
`ANTHROPIC_BASE_URL` names. Codex speaks only the OpenAI Responses API (`POST /v1/responses`):
`wire_api = "chat"` is now a hard error. Which open-source programs sit between either harness and
a model that speaks another format, how faithfully do they translate, and could a person run one
locally for purlis to point at?

**How this was read.** Every reading is dated 2026-11-14. Repositories, licence files, releases,
commits and issues were read through the GitHub API (`gh`, and unauthenticated `api.github.com`
for `maximhq/bifrost`, whose organisation's IP allow list refuses an authenticated token). Vendor
docs were read from their source repositories on GitHub (`BerriAI/litellm-docs`,
`ollama/ollama/docs`, `vllm-project/vllm/docs`, `lmstudio-ai/docs`, `maximhq/bifrost/docs`). Web
pages off GitHub could not be fetched in this session (no web tool; `curl` to other hosts was
refused), so `code.claude.com` and `developers.openai.com` were **not** read. Claims that rest on
those pages are marked **[unverified: docs site not read]**; a later pass from a chat with web
access is to verify them and post the result on #6583.

**Marks.** Every claim carries one:

- **P** (primary): read on the date above at the cited source: a repo's code, docs, LICENSE,
  release list, commit list or issue tracker.
- **I** (inferred): follows from P facts but was not read directly.
- **U** (unverified): from the brief or memory, not confirmed at a primary source in this session.
- **[unverified: docs site not read]**: rests on a docs site off GitHub that could not be fetched
  here; to be verified by the later pass.

No secondary source (blog, forum, aggregator) is used as fact.

"Open issues" means issues only, not pull requests (`is:issue is:open` search). "Releases in the
window" counts GitHub releases published 2026-05-14 to 2026-11-14. "Fidelity" means whether a
field survives the round trip: Anthropic `tool_use`/`tool_result`, SSE event shapes and `ping`,
`cache_control`, `thinking` blocks and their `signature`, Responses reasoning items,
`encrypted_content` and `previous_response_id`.

## Summary

| Translator | Inbound APIs it exposes | Upstream formats it emits | Tool calls | Streaming | Prompt caching | Reasoning | Licence | Health (6 months) | A person can run it locally |
|---|---|---|---|---|---|---|---|---|---|
| **LiteLLM proxy** (`BerriAI/litellm`) | `/v1/messages`, `/v1/responses`, `/v1/chat/completions`, more | Native Anthropic, Bedrock, Vertex, Azure; every other provider through Chat Completions, or Responses for `openai/*` **P** | Mapped to function calls; tested green only for basic tool use on Azure OpenAI **P** | Anthropic SSE rebuilt from provider chunks; native `ping` handling only on the native path **P** | `cache_control` dropped unless the target consumes it; `metadata.user_id` → `prompt_cache_key` for OpenAI **P** | `thinking` → `reasoning.effort` by budget; a `thinking` block sent to OpenAI becomes `output_text` **P** | MIT, except `litellm/enterprise/` under a commercial licence **P** | 246 releases; 414 commit authors; 1,494 open issues; two compromised releases (Mar, Oct 2026) **P** | Yes: pip or Docker, one YAML file, binds `0.0.0.0:4000` by default **P** |
| **Claude Code Router** (`musistudio/claude-code-router`) + its engine `The-NeXT-AI/ai-gateway` | `/v1/messages`, `/v1/chat/completions`, `/v1/responses`, Gemini `generateContent`, Gemini Interactions **P** | Anthropic Messages, OpenAI Chat, OpenAI Responses, Gemini **P** | Converted; open bugs on deferred tools and tool IDs **P** | Converted; synthesises `ping` on converted Anthropic streams **P** | Open bugs: cache usage dropped or double-counted **P** | Signature carried between Gemini `thought_signature` and Anthropic blocks; open bugs on Responses reasoning **P** | MIT (both repos) **P** | 27 releases (v3.0.0 → v3.1.3), last 2026-10-09; 502 of 626 commits by one person; 956 open issues **P** | Yes: npm CLI, desktop app or Docker; binds `127.0.0.1:3456` **P** |
| **Bifrost** (`maximhq/bifrost`) | OpenAI (`/openai`), Anthropic (`/anthropic`), GenAI (`/genai`) drop-in routes **P** | 23+ providers **P** | Translated; "target model must support the tool calls Codex needs" **P** | Translated; Codex must use HTTP, not Responses WebSocket **P** | Session affinity on `x-claude-code-session-id`; open caching bugs **P** | Open bugs on thinking and reasoning summaries **P** | Apache-2.0; enterprise features outside the open build (**I**) **P** | ≥100 tagged releases in the last four weeks; 640 open issues **P** | Yes: `npx` or Docker, port 8080 **P** |
| **new-api** (`QuantumNous/new-api`) | `/v1/chat/completions`, `/v1/responses`, `/v1/messages`, Gemini **P** | The same four, converted by RelayKit **P** | Responses custom and namespace tools turned into function tools and restored **P** | Converted **P** | Not stated **U** | Effort and thinking budget carried between steps **P** | AGPL-3.0 with Section 7 additional terms **P** | 45 releases (v1.0.0-rc.7 → rc.51); 122 commit authors; 781 open issues **P** | Yes, but it is a multi-user panel with a database (SQLite default) and an admin wizard **P** |
| **Agent Router**, formerly Envoy AI Gateway (`theagentrouter/agent-router`) | OpenAI APIs; Anthropic `/v1/messages` **P** | Anthropic → Anthropic, Bedrock, Vertex, or OpenAI Chat; Responses → OpenAI Responses only **P** | Translated **P** | Translated **I** | `cache_control` mapped where both sides have it **P** | Anthropic `thinking` config mapped to the OpenAI schema's thinking union **P** | Apache-2.0 **P** | 9 releases (v0.7.0 → v1.2.1); 95 commit authors; 205 open issues **P** | Yes: `aigw run`, port 1975; built for Kubernetes **P** |
| **Portkey gateway** (`Portkey-AI/gateway`) | `/v1/messages` (main), `/v1/responses` **P** | On `main`, `/v1/messages` only to providers that host Claude; a Messages → Chat adapter only on the `2.0.0` pre-release branch **P** | Adapter on `2.0.0` only **P** | Adapter on `2.0.0` only **P** | Not handled in the `2.0.0` Messages adapter **P** | Only `output_config.effort` mapped in the `2.0.0` Messages adapter **P** | MIT **P** | Last release 2026-01-12; 7 commits by 2 people since May **P** | Yes: `npx`, port 8787 **P** |
| **Small Claude Code proxies** (`1rgs/…`, `fuergaosi233/…`, `y-router`) | `/v1/messages` | OpenAI Chat (and Gemini through LiteLLM for `1rgs`) **P** | Basic | Basic | Not stated | Not stated | `1rgs`: no licence file; `fuergaosi233`: MIT; `y-router`: MIT, archived **P** | One commit or none since May **P** | Yes, from source **P** |
| **Responses → Chat shims** for Codex | `/v1/responses` | OpenAI Chat **P** | – | – | – | – | Mostly MIT or none **P** | Largest found has 94 stars **P** | Yes, from source |

Kong's open-source AI proxy and CLIProxyAPI are covered below; neither is a fit for this
question (Kong OSS has no Anthropic inbound; CLIProxyAPI is built around subscription logins).

## LiteLLM proxy

**What it is.** A Python proxy and SDK. `/v1/messages` is "call all your LLM APIs in the
Anthropic `v1/messages` format", with "All LiteLLM supported providers" listed **P**
(`litellm-docs/docs/anthropic_unified/index.md`).

**How a Messages request is routed.** The architecture note draws the split **P**
(`litellm/llms/anthropic/messages/architecture.md`):

- `anthropic`, `azure_ai`, `bedrock` invoke and `vertex_ai` go to their native Messages
  endpoints.
- "Other providers" go through `LiteLLMAnthropicMessagesAdapter`:
  `translate_anthropic_to_openai` → `litellm.completion` → `translate_openai_response_to_anthropic`.
- For `openai/*` models the request goes to the Responses API instead. A request with
  `stop_sequences` goes through Chat Completions (`docs/anthropic_unified/messages_to_responses_mapping.md`) **P**.

**Fidelity on the Messages → Responses path** (`messages_to_responses_mapping.md`) **P**:

- `tool_use` → top-level `function_call`; `tool_result` → `function_call_output`. A web-search tool
  → `web_search_preview`; every other tool → a `function` tool.
- `thinking` → `reasoning`. `budget_tokens` maps to an effort level (≥10000 high, ≥5000 medium,
  ≥2000 low, else minimal), and `summary` is always `"detailed"`.
- An assistant `thinking` block in history becomes `output_text` inside the assistant message.
  The page says nothing about its `signature`.
- On the way back, each reasoning summary becomes a `thinking` block. Only `input_tokens` and
  `output_tokens` are mapped into `usage`.
- `top_k` is "Dropped silently". `system` blocks that are not text "are ignored".
- `metadata.user_id` becomes both `user` and `prompt_cache_key` (truncated to 64 characters).
- The page never mentions `cache_control`, `signature`, streaming events or `ping`.

**Prompt caching on the Chat Completions path.** `_add_cache_control_if_applicable` copies a
block's `cache_control` only when `keep_cache_control` is set or `target_consumes_cache_control(model)`
is true (`litellm/llms/anthropic/messages/adapters/transformation.py`, read at `HEAD`) **P**. For
any other target it is not sent **I**.

**Streaming.** The native iterator recognises "whole `ping` keepalive frames" in Anthropic's
own stream (`native/streaming_iterator.py`) **P**. The two adapter iterators (Chat and Responses)
contain no `ping` handling (word search on both files) **P**. Whether a translated stream ever
emits `ping` was not verified **U**.

**Beta headers.** LiteLLM does not pass `anthropic-beta` through unchanged. "Only headers present
in the mapping with non-null values are forwarded", from `anthropic_beta_headers_config.json`
(`docs/tutorials/claude_code_beta_headers.md`) **P**.

**What LiteLLM itself tests with Claude Code.** A daily matrix runs the Claude Code CLI against
the newest final release (`docs/claude_code_compatibility.md`) **P**. The snapshot read
(`src/data/compatibility-matrix.json`, generated 2026-10-21 against v1.106.1) has these cells for
the two non-Claude columns **P**:

| Feature | `openai` | `azure_openai` |
|---|---|---|
| Basic messaging, streaming and non-streaming | not tested | pass |
| Tool use; tool use streaming | not tested | pass |
| Prompt caching (5m, 1h) | not tested | not tested |
| Thinking; extended thinking + tool use | not tested | not tested |
| `count_tokens` | not tested | not tested |

Every other column is a Claude model on Anthropic, Bedrock, Vertex or Azure.

**LiteLLM's own advice.** The harness page says Claude Code "fits best on a Claude model group",
and Codex "fits best on an OpenAI reasoning model". With a Claude or Gemini group the gateway
translates, "though Codex-specific features like reasoning summaries may not come through", and
"a group behind translation may ignore" `reasoning_effort` (`docs/harness/claude_code.md`,
`docs/harness/codex.md`) **P**.

**Responses for Codex.** `/v1/responses` is served natively for OpenAI. For "`custom_openai/`,
`baseten/`, `openai_like/` and most other OpenAI-compatible providers" it "bridges `/v1/responses`
through `/v1/chat/completions`" (`docs/response_api.md`) **P**. Session continuity and
`encrypted_content_affinity` route a `previous_response_id` or an `rs_…` item back to the
deployment that made it (same page) **P**. The Codex setup page tells the person to raise
`stream_idle_timeout_ms` to 7,200,000 because "Long agent turns can idle for minutes behind a
gateway" (`docs/proxy/client_setup/codex_cli.md`) **P**.

**Open fidelity bugs (examples)** **P**:

- #48366 Responses chat-completions bridge "silently ignores a `previous_response_id` it cannot
  resolve".
- #43620 Responses bridge "keeps only the last reasoning item when stream=False".
- #46369 `/v1/messages` exposes Gemini `thought_signature` only through a non-standard
  `tool_use.provider_specific_fields`.
- #48401 streaming drops OpenRouter `reasoning_details`-only chunks (Claude signatures, encrypted
  reasoning).
- #51469 guardrails on `/v1/messages` drop Anthropic-only tool fields (`defer_loading`, `strict`,
  `input_examples`).

Open-issue searches on 2026-11-14: "claude code" 122, "thinking signature" 18, "cache_control" 36,
"responses api chat completions bridge" 15 (full-text matches, so they over-count) **P**.

**Licence.** The root `LICENSE` is MIT for everything outside `litellm/enterprise/`. That
directory is under "The BerriAI Enterprise license": production use needs a subscription, though
"you may copy and modify the Software for development and testing purposes" **P**
(`LICENSE`, `litellm/enterprise/LICENSE.md`). The enterprise feature list includes SSO beyond five
users, audit logs, RBAC and per-team guardrails (`docs/enterprise.md`) **P**. None of the
translation code read above sits under `enterprise/` **P**.

**Health.** 246 GitHub releases in the window, including `-dev` and `-rc` tags, several a day
(v1.87.4 on 2026-06-20 to v1.108.8 on 2026-11-13). 14,817 commits by 414 authors; the top six
(4,756, 2,176, 1,064, 1,043, 858, 682 commits) are all accounts named `*-berri`. 1,494 open
issues **P**. Two releases were compromised: `litellm==1.82.7` and `1.82.8` on PyPI on
2026-03-24, live "for about 40 minutes"; and v1.107.0 on 2026-10-21, which "contained compiled
malware which steals credentials", pushed to Docker and PyPI
(`litellm-docs/blog/security_update_march_2026`, `blog/security_incident_oct_21_2026`) **P**.

**Running it yourself.** `litellm --model …` prints "Proxy running on http://0.0.0.0:4000"
(`docs/proxy/quick_start.md`) **P**. Models are named in a `config.yaml`; a `master_key` and
`database_url` are needed only for virtual keys and the admin features (`docs/harness/claude_code.md`
example) **P**. No cloud account is needed beyond the upstream's own **I**.

## Claude Code Router

**What it is.** "A local model gateway and control plane for coding agents", with profiles for
Claude Code, Codex and eight more clients. It "supports OpenAI Chat / Responses, Anthropic
Messages, Gemini Generate Content / Interactions" and many named providers (README) **P**.

**Where the translation lives.** `packages/core` maps a request path to one of five protocols:
`anthropic_messages`, `openai_chat_completions`, `openai_responses`, `gemini_generate_content`,
`gemini_interactions` (`packages/core/src/routing/protocol-endpoints.ts`) **P**. The conversion
itself is the npm package `@the-next-ai/ai-gateway` (`packages/core/package.json`) **P**, from
`The-NeXT-AI/ai-gateway`: MIT, created 2026-06-14, 10 stars, 15 open issues, top contributor
`musistudio` with 79 commits **P**.

**Fidelity** **P**:

- Streaming: on a converted stream to an Anthropic client the gateway sends `event: ping`;
  "every other source gets an SSE comment" (`src/gateway/streaming-conversion.ts`).
- Reasoning: Gemini's `thought_signature` is attached as the `signature` of Anthropic
  `thinking` blocks (`src/adapters/builtins/source/formatters.ts`). When converting to Responses,
  `output_config.effort` maps to `reasoning.effort`, and the source protocol's own `thinking` /
  `output_config` containers are not forwarded (`docs/usage.md`).
- Open bugs, CCR tracker:
  - #1461 `cache_creation_input_tokens` dropped in OpenAI → Anthropic conversion.
  - #1655 3.x reports `input_tokens` including the cached prefix, so Claude Code compacts at about
    half the real context.
  - #1686 Anthropic `thinking` blocks in history break an `openai_responses` upstream (400).
  - #1725 Responses streaming puts reasoning deltas in the wrong content block.
  - #1783 Responses: 400 "array too long" on a reasoning item with content.
  - #1912 `defer_loading` not dropped for upstreams without a deferred tool loader.
  - #1917 an upstream failure after the headers ends the downstream stream with no terminal SSE
    frame.
- Open issues with these words in the title: cache 8, thinking 15, tool 34, stream 11, codex 19,
  responses 11.

**Licence.** MIT (`LICENSE`, `package.json`) **P**. The README carries paid sponsor links to
model resellers **P**.

**Health.** 27 releases, v3.0.0 (2026-06-25) to v3.1.3 (2026-10-09). The last push was
2026-10-10, so nothing has landed for five weeks. 626 commits by 44 authors, 502 by `musistudio`.
956 open issues **P**.

**Running it yourself.** Desktop app, `npm install -g @musistudio/claude-code-router` then
`ccr ui` (Node 22+), or Docker. "The local model gateway listens on `http://127.0.0.1:3456` by
default", with the management UI on `127.0.0.1:3458`. Providers are set up in the UI, which can
also write the agent's own config ("Agent Config … apply the profile") (README) **P**.

## Bifrost

**What it is.** A Go gateway that "unifies access to 23+ providers … through a single
OpenAI-compatible API", with drop-in `/openai`, `/anthropic` and `/genai` base URLs (README) **P**.

**Claude Code.** The model can be mapped to "any model on any provider", for example
`vertex/gemini-3.1-pro`. A `400` naming `context_management` means "the upstream provider behind
Bifrost rejects pre-release Claude fields"; the fix given is `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`.
Bifrost pins a session to one provider and key on `x-claude-code-session-id` "and keeps hitting the
same prompt cache" (`docs/cli-agents/claude-code.mdx`) **P**.

**Codex.** `supports_websockets = false` "is required when Bifrost translates the request to a
provider that does not implement OpenAI's stateful Responses WebSocket protocol". For non-OpenAI
models: "Provider support for streaming, reasoning items, parallel tools, images, and compaction
can also differ", and "The current automated Codex cloud harness exercises the OpenAI provider
path" (`docs/cli-agents/codex-cli.mdx`) **P**.

**Open fidelity bugs (examples)** **P**: #9542 streaming `/anthropic/v1/messages` drops the
`server_tool_use` input, "breaking multi-turn replay (thinking signature 400)"; #9631 Anthropic
Responses reasoning omits the required summary; #9860 `thinking.display` dropped on
`/anthropic/v1/messages`; #9416 injected `cache_control` breakpoints make OpenRouter double-bill
Gemini prompts; #9522 Anthropic-defined tools silently dropped toward Bedrock.

**Licence.** Apache-2.0 (`LICENSE`) **P**. Enterprise features (clustering, adaptive load
balancing, guardrails, custom plugins) are sold separately (README) **P**; the repo has
`ui/app/_fallbacks/enterprise/` and no enterprise source directory, so those features are
presumably outside the public build **I**.

**Health.** The newest 100 releases span 2026-10-19 to 2026-11-10 (per-module tags such as
`transports/v2.5.1`), so cadence is very high. 640 open issues. All-time top contributors
`akshaydeo` (2,847) and `Pratham-Mishra04` (1,133) **P**. A six-month author count was not taken
(unauthenticated rate limit) **U**.

**Running it yourself.** `npx -y @maximhq/bifrost` or `docker run -p 8080:8080 maximhq/bifrost`,
"zero configuration", UI on `localhost:8080` (README) **P**. Its default bind address was not
checked **U**.

## new-api

**What it is.** A Go service built on One API, for sharing model access "across a team" with a
web console, quotas and billing. It exposes `/v1/chat/completions`, `/v1/responses` (also as a
WebSocket), `/v1/messages` and Gemini `generateContent` (README) **P**.

**Fidelity.** "RelayKit provides request, response, and streaming conversion between the four text
protocols … protocol-specific tools and fields may not map exactly" (README) **P**. RelayKit's
own README says Responses custom tools and namespaced tools sent to Chat, Claude or Gemini
upstreams become plain function tools, with the encoding recorded so the response can restore
them; reasoning effort and thinking budget travel between conversion steps
(`relaykit/README.md`, in Chinese) **P**.

**Licence.** AGPL-3.0 with additional terms under Section 7: modified versions must keep an
attribution line and "a visible link to the original project" in any UI. A commercial licence is
offered for those who cannot accept AGPL (README) **P**.

**Health.** 45 releases, v1.0.0-rc.7 (2026-05-19) to rc.51 (2026-11-13). 914 commits by 122
authors, 440 by `Calcium-Ion`. 781 open issues **P**.

**Running it yourself.** `docker run … -p 127.0.0.1:3000:3000` with SQLite, then a setup wizard
that creates an administrator, then channels and a "New API-issued key". Compose adds PostgreSQL
and Redis (README) **P**.

## Agent Router (formerly Envoy AI Gateway)

**What it is.** "Envoy AI Gateway is now Agent Router, an Agentic AI Foundation project. Same
code, same maintainers"; `envoyproxy/ai-gateway` redirects (README) **P**.

**Formats.** The translators in `internal/translator/` include `anthropic_anthropic`,
`anthropic_awsbedrock`, `anthropic_gcpanthropic` and `anthropic_openai` (Anthropic Messages →
OpenAI Chat Completions). For Responses there is only `openai_responses.go`,
`NewResponsesOpenAIToOpenAITranslator` **P**. So Codex can be routed only to an OpenAI-shaped
Responses upstream **I**. `anthropic_helper.go` maps an ephemeral `cache_control` across;
`openai_helper.go` maps Anthropic `thinking` to an OpenAI-schema `ThinkingUnion` **P**.

**Licence.** Apache-2.0 **P**.

**Health.** 9 releases, v0.7.0 (2026-06-06) to v1.2.1 (2026-11-06). 320 commits by 95 authors;
the top six have 12 to 38 each. 205 open issues **P**.

**Running it yourself.** `OPENAI_API_KEY=… aigw run`, then "point any OpenAI-compatible client at
`http://localhost:1975/v1`". The main design is a two-tier Kubernetes gateway (README) **P**.

## Portkey gateway

**What it is.** A TypeScript gateway, `npx @portkey-ai/gateway`, on `http://localhost:8787/v1`
(README) **P**.

**Formats on `main`.** `/v1/messages` and `/v1/messages/count_tokens` are routed
(`src/index.ts`) **P**. Messages implementations exist only for `anthropic`, `azure-ai-inference`,
`bedrock` and `google-vertex-ai`, all of which host Claude (`src/providers/*/messages.ts`) **P**.
So on `main`, `/v1/messages` is not a translator to non-Claude models **I**.

**The `2.0.0` branch.** The README calls it a pre-release that merges "Portkey's core enterprise
gateway" into open source **P**. It has `src/adapters/messages/` (Messages → Chat Completions) and
`src/adapters/responses/` **P**. Its Messages request transform maps `output_config.effort` and
nothing about `thinking` or `cache_control`; the Responses transform keeps `cache_control` on
tools and passes `thinking` through **P**. The branch's last commit is 2026-03-14 **P**.

**Licence.** MIT **P**. Semantic caching is "Available in hosted and enterprise versions"
(README) **P**.

**Health.** Last release v1.15.2 on 2026-01-12. Since 2026-05-14: 7 commits by 2 people; last
push 2026-05-25. 138 open issues **P**.

## Small Claude Code proxies

- **`1rgs/claude-code-proxy`**: "Use Anthropic clients (like Claude Code) with Gemini, OpenAI, or
  direct Anthropic backends … all via LiteLLM" (README). No LICENSE file in the repo, and GitHub
  reports no licence. One commit since May (2026-06-23). 45 open issues **P**.
- **`fuergaosi233/claude-code-proxy`**: Claude API → OpenAI-compatible providers, with
  "Function Calling" and "Streaming Responses" claimed (README). MIT, added 2026-03-12, which is
  also its last commit. 25 open issues **P**.
- **`luohy15/y-router`**: archived. "OpenRouter now provides an official integration for Claude
  Code" (README) **P**.
- **`maxnowack/anthropic-proxy`**: archived, last push 2025-04-28 **P**.

## Responses → Chat Completions shims for Codex

A search for Responses-to-Chat bridges found only small projects: `kartikkabadi/opencode-go-proxy`
(94 stars, MIT, last push 2026-10-15), `oleg-kn/codex-responses-proxy` (1 star, "translates OpenAI
Responses API requests to (vLLM) Chat Completions and back"), `tmdgusya/codex-provider-proxy`
(0 stars) and others with no licence **P**. None has a release record worth the name **I**. The
mature options for Codex → a Chat-only model are LiteLLM's bridge, CCR, Bifrost and new-api, or a
server that speaks Responses itself (next section).

`openai/codex` ships `codex-rs/responses-api-proxy`, but it forwards to OpenAI and dumps
request/response pairs; it does not translate (`codex-rs/responses-api-proxy/README.md`) **P**.

## Also seen, out of scope

- **Kong (OSS).** The `ai-proxy` plugin's `llm_format` is one of `openai`, `bedrock`, `gemini`
  (`kong/plugins/ai-proxy/schema.lua`). No Anthropic inbound in the open-source plugin. Last commit
  under `kong/llm` 2025-10-21 **P**.
- **CLIProxyAPI** (`router-for-me/CLIProxyAPI`, MIT, 58.5k stars). Exposes OpenAI (including
  Responses), Gemini and Claude-compatible APIs over "multiple CLI accounts", that is the CLIs'
  subscription logins (README) **P**. Whether that use is allowed by each vendor's terms was not
  checked **U**.
- **OpenRouter** (hosted, not open source). Has an official Claude Code integration, per the
  `y-router` README **P**; its own docs site was not read **[unverified: docs site not read]**.

## Servers that remove the need for a translator

| Server | Anthropic `/v1/messages` | OpenAI `/v1/responses` | Notes |
|---|---|---|---|
| **Ollama** | Yes, "a subset". Streams every Anthropic event type including `ping`; `thinking` blocks; `tool_choice` and `metadata` not supported; `cache_control` not listed (`docs/api/anthropic-compatibility.mdx`) **P** | Yes, "Only the non-stateful flavor": no `previous_response_id`, no `conversation`. The cloud API also lacks "custom/freeform tool-call replay" (`docs/api/openai-compatibility.mdx`) **P** | Codex's `ollama` provider replaced the removed `ollama-chat` (`codex-rs/model-provider-info/src/lib.rs`) **P** |
| **vLLM** | Yes, `/v1/messages` and `/v1/messages/count_tokens` (`vllm/entrypoints/anthropic/api_router.py`) **P** | Yes; docs for Codex: "vLLM implements the OpenAI-Responses API" (`docs/serving/integrations/codex.md`) **P** | Needs `--enable-auto-tool-choice` and the model's `--tool-call-parser` (`docs/serving/integrations/claude_code.md`) **P** |
| **LM Studio** | Yes, since 0.4.1 (`1_developer/api-changelog.md`) **P** | Yes, since 0.3.29 (2025-10-06), with `previous_response_id` (`1_developer/3_openai-compat/responses.md`) **P** | Codex's deprecation notice names LM Studio for local models (`openai/codex` discussion #7782) **P**. The app's own licence was not checked **U** |
| **llama.cpp server** | Has `tools/server/tests/unit/test_compat_anthropic.py` **P** | Has `test_compat_oai_responses.py` **P** | Support is inferred from test names only **I** |

## What a translator must preserve

### Claude Code

From Claude Code's own CHANGELOG (`anthropics/claude-code/CHANGELOG.md`, read at 2.1.325) **P**:

- **Idle stream deadline.** 2.1.299: "Fixed streams through a custom `ANTHROPIC_BASE_URL` being
  aborted after 180 seconds with no data arriving; they now wait 300 seconds". 2.1.309: "gateways
  … that send only keepalive pings: routes off Anthropic's backend now wait 45 seconds" before the
  waiting hint.
- **Beta headers.** 2.1.314: when "a gateway or cloud provider refuses one of Claude Code's beta
  headers by name: the request is sent again without it". 2.1.310: retries without the
  `effort-2025-11-24` beta or mid-conversation system messages behind gateways that reject them.
  2.1.307: the `prompt-caching-scope` beta is no longer sent behind custom `ANTHROPIC_BASE_URL`
  gateways.
- **Model discovery.** 2.1.129: gateway `/v1/models` discovery became opt-in with
  `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1`. 2.1.223: fixed discovery "hiding Claude models
  registered under provider-prefixed IDs such as `vertex_ai/claude-*`". 2.1.269: discovery
  timeout defaults to 3 s. 2.1.257: a discovered entry without a description reads "From gateway".
- **Model identity.** 2.1.323: behind a gateway, auto mode refuses a model whose name contains
  "haiku" other than `claude-haiku-5-5`. 2.1.314: model IDs spelled with a dot were read as an
  older model with the wrong context window, output limit, effort, thinking mode and cost.
- **Prompt caching.** Several fixes for caching "behind LLM gateways and custom base URLs".

From the brief, **not** re-read here because `code.claude.com` could not be fetched **[unverified: docs site not read]**: the
gateway protocol page (`code.claude.com/docs/en/llm-gateway-protocol`) says to forward
`anthropic-beta` and `anthropic-version` unchanged, stream without buffering, relay `ping`, and
pass `cache_control` through or the conversation bills uncached. It says discovery keeps only ids
containing `claude` or `anthropic` (the 2.1.223 entry above fits that rule **I**). And Anthropic
says it does not support routing Claude Code to non-Claude models through any gateway.

Against that list, LiteLLM filters `anthropic-beta` through an allow list and drops
`cache_control` for targets that do not consume it **P**. Bifrost's own fix for rejected fields
is to turn experimental betas off **P**. A non-Claude model name must still contain `claude` for
discovery to list it **I**.

### Codex

- **Responses only.** `WireApi` has one variant, `Responses`. `"chat"` fails with
  "`wire_api = \"chat\"` is no longer supported" (`codex-rs/model-provider-info/src/lib.rs`) **P**.
  Discussion #7782 (2025-12-09) announced it: "Full removal is slated for early February 2026",
  and "If your organization uses an LLM proxy or gateway … ensure your proxy supports the
  `responses` API" **P**.
- **Freeform tools.** #37825: Code Mode's `exec` is a custom/freeform tool, which "prevents Code
  Mode from working with Responses-compatible providers that support ordinary function calling
  while rejecting `type: "custom"`". #33405: on third-party providers `apply_patch` "is often
  absent from the tool list or cannot be used" **P**.
- **Namespaced tools.** #23186: MCP tools are wrapped as `{"type":"namespace",…}`, which "has no
  equivalent in the Chat-Completions schema"; translators drop them and "The MCP tools never reach
  the model" **P**. #20574: `tool_search` aborts with "unsupported payload" behind Ollama, Bifrost
  or LiteLLM **P**.
- **Reasoning state.** #53713: switching `model_provider` mid-thread fails with
  `invalid_encrypted_content`. #42249: DeepSeek Responses returns 400 "reasoning_text must be
  passed back". #36586: an `encrypted_content` block dropped for custom non-OpenAI providers **P**.
- **Transport.** Providers declare `supports_websockets`; Bifrost tells Codex to turn it off when
  translating **P**.
- The brief's statement that non-OpenAI upstreams reject `previous_response_id` matches Ollama's
  docs ("no `previous_response_id`") **P**; for the other upstreams it was not checked **U**.

## Bearing on #6584

Facts only.

- **What purlis promises today.** ADR 0087 §7: the key proxy "passes the request and its stream
  through byte for byte. It changes nothing else" **P**. ADR 0073 §4 defines **standing in** to
  include "a proxy that edits its traffic", and says "purlis wraps; it never stands in" **P**. A
  translator rewrites every request and every stream event, so it edits the traffic **I**.
- **ADR 0087 §8 already points harnesses at local servers.** It detects Ollama, LM Studio or
  vLLM on loopback and points a harness at it where the harness takes an OpenAI- or
  Anthropic-compatible base URL **P**. All three now speak both `/v1/messages` and `/v1/responses`
  themselves (table above) **P**, so for local models no translator stands between the harness and
  the server **I**.
- **Which translators are mature enough to point at.** On release cadence and contributor count
  (issue response time was not measured), LiteLLM, Bifrost, new-api and Agent Router are actively maintained by teams **P**.
  CCR is active but has one main author and no push for five weeks **P**. Portkey's open-source
  translation is on an unmerged branch last touched in March **P**. The small proxies are dormant
  or archived **P**.
- **Run locally, pointed at by URL.** LiteLLM (pip or Docker, YAML), CCR (npm, desktop or Docker,
  binds loopback), Bifrost (`npx` or Docker) and Portkey (`npx`) each run as one local process
  that a base URL can name **P**. new-api needs an admin wizard and a database; Agent Router is
  built for Kubernetes but has `aigw run` **P**. None needs a cloud account of its own **I**.
  LiteLLM binds `0.0.0.0` by default **P**.
- **Licences.** All candidates are OSI-licensed in the parts that translate: MIT (LiteLLM outside
  `enterprise/`, CCR and its engine, Portkey), Apache-2.0 (Bifrost, Agent Router), AGPL-3.0 with
  Section 7 terms (new-api) **P**. `1rgs/claude-code-proxy` has no licence **P**.
- **Fidelity gaps exist in every translator read.** None documents end-to-end preservation of
  Anthropic `thinking` signatures and `cache_control` toward a non-Anthropic model. LiteLLM's own
  matrix leaves caching and thinking "not tested" for its non-Claude columns. Each tracker has
  open bugs on reasoning state, cache accounting or tool fields **P**. Codex-side, freeform and
  namespaced tools and encrypted reasoning items are known to break behind Chat-shaped upstreams
  **P**. These gaps are a property of the format pairs and would face any translator, purlis's
  included **I**.
- **Supply chain.** LiteLLM shipped malware in two releases this year (March and October) **P**.
  A person running any of these takes on its update stream **I**.

## Unsettled

1. **Claude Code's gateway protocol page** (`code.claude.com/docs/en/llm-gateway-protocol`) was
   not read: the header forwarding rule, `ping` relaying, the `cache_control` billing statement,
   the `claude`/`anthropic` discovery filter, and Anthropic's statement that it does not support
   non-Claude models through a gateway all remain **[unverified: docs site not read]**.
2. **Codex's docs on developers.openai.com** were not read **[unverified: docs site not read]**; the Codex facts here come from its
   source, discussion #7782 and its issue tracker.
3. **Whether LiteLLM's translated streams emit `ping`** (the adapter iterators contain none).
4. **What `target_consumes_cache_control` returns** for each provider in LiteLLM.
5. **Bifrost's default bind address** and six-month contributor count; whether its enterprise
   features are in the public source.
6. **new-api's handling of `cache_control` and thinking signatures.**
7. **Agent Router's streaming conversion** and how it encodes `thinking` blocks toward OpenAI.
8. **llama.cpp server's endpoint support**, inferred from test names only.
9. **LM Studio's app licence.**
10. **Whether CLIProxyAPI's use of subscription logins is allowed** by each vendor's terms.
11. **Open-issue counts by keyword** are full-text matches and over-count real fidelity bugs; no
    issue was triaged by hand.

## Sources

All read 2026-11-14 through the GitHub API unless noted.

- LiteLLM: <https://github.com/BerriAI/litellm> — `LICENSE`, `litellm/enterprise/LICENSE.md`,
  `litellm/llms/anthropic/messages/architecture.md`, `…/adapters/transformation.py`,
  `…/adapters/streaming_iterator.py`, `…/responses_adapters/streaming_iterator.py`,
  `…/native/streaming_iterator.py`; releases; commits since 2026-05-14; issues #43620, #46369,
  #48366, #48401, #51469.
- LiteLLM docs: <https://github.com/BerriAI/litellm-docs> — `docs/anthropic_unified/index.md`,
  `docs/anthropic_unified/messages_to_responses_mapping.md`, `docs/claude_code_compatibility.md`,
  `src/data/compatibility-matrix.json`, `docs/harness/claude_code.md`, `docs/harness/codex.md`,
  `docs/response_api.md`, `docs/proxy/client_setup/codex_cli.md`,
  `docs/tutorials/claude_code_beta_headers.md`, `docs/tutorials/claude_code_gateway_reference.md`,
  `docs/proxy/quick_start.md`, `docs/enterprise.md`, `blog/security_update_march_2026/index.md`,
  `blog/security_incident_oct_21_2026/index.md`.
- Claude Code Router: <https://github.com/musistudio/claude-code-router> — README, `LICENSE`,
  `package.json`, `packages/core/package.json`, `packages/core/src/routing/protocol-endpoints.ts`;
  releases; commits; issues #1461, #1655, #1686, #1725, #1783, #1912, #1917.
- Next AI Gateway: <https://github.com/The-NeXT-AI/ai-gateway> — README, `docs/usage.md`,
  `src/gateway/streaming-conversion.ts`, `src/adapters/builtins/source/formatters.ts`.
- Bifrost: <https://github.com/maximhq/bifrost> — README, `LICENSE`,
  `docs/cli-agents/claude-code.mdx`, `docs/cli-agents/codex-cli.mdx`; releases; contributors;
  issues #9416, #9522, #9542, #9631, #9860 (unauthenticated `api.github.com`).
- new-api: <https://github.com/QuantumNous/new-api> — README, `relaykit/README.md`; releases;
  commits.
- Agent Router: <https://github.com/theagentrouter/agent-router> — README,
  `internal/translator/anthropic_openai.go`, `openai_responses.go`, `anthropic_helper.go`,
  `openai_helper.go`; releases; commits.
- Portkey gateway: <https://github.com/Portkey-AI/gateway> — README, `LICENSE`, `src/index.ts`,
  `src/handlers/messagesHandler.ts`; branch `2.0.0`: `src/adapters/messages/requestTransform.ts`,
  `src/adapters/responses/requestTransform.ts`; releases; commits.
- Small proxies: <https://github.com/1rgs/claude-code-proxy>,
  <https://github.com/fuergaosi233/claude-code-proxy>, <https://github.com/luohy15/y-router>,
  <https://github.com/maxnowack/anthropic-proxy> — READMEs, file lists, commits.
- Shims: GitHub repository search "responses to chat completions" and "codex responses proxy".
- Kong: <https://github.com/Kong/kong> — `kong/plugins/ai-proxy/schema.lua`, commits under
  `kong/llm`.
- CLIProxyAPI: <https://github.com/router-for-me/CLIProxyAPI> — README; releases; contributors.
- Codex: <https://github.com/openai/codex> — `codex-rs/model-provider-info/src/lib.rs`,
  `codex-rs/responses-api-proxy/README.md`; discussion
  <https://github.com/openai/codex/discussions/7782>; issues #20574, #23186, #33405, #36586,
  #37825, #42249, #53713.
- Claude Code: <https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md> (2.1.129 to
  2.1.325 entries cited).
- Ollama: <https://github.com/ollama/ollama> — `docs/api/anthropic-compatibility.mdx`,
  `docs/api/openai-compatibility.mdx`.
- vLLM: <https://github.com/vllm-project/vllm> — `vllm/entrypoints/anthropic/api_router.py`,
  `docs/serving/integrations/claude_code.md`, `docs/serving/integrations/codex.md`.
- LM Studio docs: <https://github.com/lmstudio-ai/docs> — `1_developer/api-changelog.md`,
  `1_developer/3_openai-compat/responses.md`, `1_developer/4_anthropic-compat/index.mdx`.
- llama.cpp: <https://github.com/ggml-org/llama.cpp> — `tools/server/tests/unit/` file names.
- purlis: `docs/adr/0087-a-chats-model-is-one-choice-from-four-sources-and-charter-never-carries-a-harness-login.md`
  §7 and §8; `docs/adr/0073-a-harness-is-declared-as-data-and-a-chat-runs-it-at-one-of-three-levels.md`
  §4 (this tree).
- Not read (fetch refused in this session): <https://code.claude.com/docs/en/llm-gateway-protocol>,
  <https://developers.openai.com/codex>, <https://openrouter.ai/docs>.
