# Harnesses: wire formats, per-session routing and their own sign-ins

Research, 2026-11-14, for #6580 (part of map #6578, *Any model on any harness*). Facts only.
This note decides no policy: the map's grilling tickets do. Legal readings are out of scope here.
#6578 says a qualified review comes before anything they gate.

It refreshes research 09 §1.1 (`purlis-plane/docs/program/research/09-model-gateway.md`,
2026-09-29) and adds each harness's own subscription sign-ins and what breaks behind a model it
was not built for. purlis's own adapters were read from `main` at `bcd9681ba6`.

## How this was read, and what it could not reach

The session that wrote this had **no web access** besides the GitHub API: no WebFetch or WebSearch
tool, and `curl` was refused. So every vendor page (`code.claude.com`, `developers.openai.com`,
`opencode.ai`, `ampcode.com`, `cursor.com`, `docs.github.com`, `antigravity.google`) was out of
reach. What was read instead:

- **Each open-source harness's own repository** through `gh api`: its docs folder, its source
  and its changelog or releases. The repo is the source the docs site is built from (opencode,
  Gemini CLI, Goose, Aider), or it is the code itself (Codex, Crush). The commits read:

  | Repo | Branch @ commit | Latest release |
  |---|---|---|
  | `openai/codex` | `main` @ `e8f5d2bb55` | `rust-v0.169.0`, 2026-11-13 |
  | `anomalyco/opencode` (was `sst/opencode`) | `dev` @ `83960f8a56` | `v1.19.5`, 2026-10-28 |
  | `charmbracelet/crush` | `main` @ `4eb49b9eac` | `v0.101.0`, 2026-11-12 |
  | `google-gemini/gemini-cli` | `main` @ `4ed06ed0dd` | `v0.68.0`, 2026-11-10 |
  | `aaif-goose/goose` (was `block/goose`) | `main` @ `2365bc5e89` | `v1.59.0`, 2026-11-10 |
  | `Aider-AI/aider` | `main` @ `5dc9490bb3` | `v0.86.0`, 2025-08-09 (last push 2026-05-22) |
  | `github/copilot-cli` (README and changelog only; the CLI is closed) | `main` @ `a0d26c24ba` | `1.0.111`, 2026-11-13 |
  | `anthropics/claude-code` (changelog only; the CLI is closed) | `main` @ `96a90e8299` | `2.1.325` |

- **Claude Code 2.1.325, installed on this machine**: `claude --help`, `claude auth --help` and
  `claude auth login --help`.

**Marks.** Each claim carries one:

- **P**: primary, read on 2026-11-14 at the commit above.
- **C**: carried from research 09 (2026-09-29, itself primary unless it said otherwise). It was
  **not re-verified**, because the page that holds it was out of reach.
- **U**: unverified. No primary source was reachable.

opencode's `dev` branch documents **V2** (`services/site/src/docs/content/`), which is not
released. Its v1 docs, which describe the released `v1.19.5`, sit beside them
(`services/site/src/docs/v1/content/`). Both are cited, and the difference matters (§4).

## 1. Harness × wire format matrix (as of 2026-11-14)

Legend: **N** native · **✓** through config · **~** works, with documented caveats · **✗** not
offered · **?** not verified.

| Harness | Anthropic Messages | OpenAI Responses | OpenAI Chat Completions | Gemini generateContent | Cloud-native (Bedrock / Vertex / Azure) | Custom base URL |
|---|---|---|---|---|---|---|
| **Claude Code** | **N** (C) | ✗ (C) | ✗ (C) | ✗ (C) | Bedrock, Bedrock Mantle, Vertex, Foundry, Claude Platform on AWS (P [cc1]) | ✓ `ANTHROPIC_BASE_URL` (P [cc2]) |
| **Codex** | ✗ (P [cx1]) | **N**, the only `wire_api`; also Responses over WebSocket (P [cx1]) | ✗, `wire_api = "chat"` is a hard error (P [cx1]) | ✗ (P [cx1]) | built in: `amazon-bedrock` (Mantle, OpenAI-compatible), `amazon-bedrock-runtime`, `gcp` (Vertex) (P [cx2]) | ✓ `model_providers.<id>.base_url`; `openai_base_url` (P [cx1], [cx2]) |
| **opencode** | ✓ `anthropic`, `anthropic-compatible` (P [oc1]) | ✓ `openai/responses`, `openai-compatible/responses` (P [oc1]) | ✓ `openai/chat`, `openai-compatible` (P [oc1]) | ✓ `google`, `google-vertex/gemini` (P [oc1]) | ✓ `amazon-bedrock` (Converse, Mantle), `google-vertex`, `azure` (P [oc1]) | ✓ V2: `providers.<id>.settings.baseURL`; v1: `provider.<id>.options.baseURL` (P [oc1], [oc2]) |
| **Crush** | ✓ type `messages` (P [cr1]) | ✓ type `responses` (P [cr1]) | ✓ type `completions` (P [cr1]) | ✓ via `GEMINI_API_KEY`, Vertex (P [cr2]) | Bedrock, Vertex (Gemini), Azure OpenAI (P [cr2]) | ✓ `provider add <id> --base-url` (P [cr1]) |
| **Gemini CLI** | ✗ (P [gm1]) | ✗ | ✗ | **N** (P [gm1]) | Vertex (P [gm2]) | ✓ `GOOGLE_GEMINI_BASE_URL`, `GOOGLE_VERTEX_BASE_URL`, **now documented**; HTTPS unless loopback (P [gm1]) |
| **Goose** | ✓ engine `anthropic` (P [gs1]) | ~ its OpenAI provider's `OPENAI_STORE` "persist[s] the generated Responses API response"; which models use it was not read (P [gs1]) | ✓ engine `openai`, default path `v1/chat/completions` (P [gs1]) | ✓ Google provider (P [gs1]) | Bedrock, Vertex, Azure AI Foundry, SageMaker (P [gs1], [gs4]) | ✓ custom provider JSON; `OPENAI_HOST`/`OPENAI_BASE_PATH` (P [gs1]) |
| **Aider** | ✓ through LiteLLM (C) | ? | ✓ `OPENAI_API_BASE` + `openai/<model>` (P [ad1]) | ✓ through LiteLLM (C) | through LiteLLM (C) | ✓ `--openai-api-base` (P [ad2]) |
| **Copilot CLI** | ✓ BYOK provider type `anthropic` (P [cp1]) | ✓ type `openai`, Responses wire API, WebSocket transport (P [cp1]) | ✓ "OpenAI-compatible chat-completions providers" (P [cp1]) | ? | Azure (type `azure`) (P [cp1]) | ✓ BYOK `COPILOT_PROVIDER_*` or `providers.json` (P [cp1]) |
| **Amp** | ✓ Custom URL (C) | ✓ (C) | ✓ (C) | ✓ (C) | ? | ✓ Custom URL connection (C) |
| **Cursor CLI** | ✗ (C) | ✗ (C) | ✗ (C) | ✗ (C) | ✗ (C) | ✗: all traffic goes through Cursor's servers (C) |
| **Antigravity CLI** (`agy`) | ? | ? | ? | ? | Google Cloud (P [gm3], its own README only) | ? |

**What changed since 2026-09-29:**

- **opencode V2 drops the AI SDK** for its own provider packages, `@opencode/ai/providers/*`,
  backed by its own protocol layer (`packages/ai/src/protocols/`: `anthropic-messages`,
  `openai-responses`, `openai-chat`, `openai-freeform`, `gemini`, `bedrock-converse` and more).
  Its config moves from `provider.<id>.npm` / `options.baseURL` to `providers.<id>.package` /
  `settings.baseURL` (P [oc1]). v1.19.5 still uses `@ai-sdk/*` packages (P [oc2]).
- **Crush** renames its provider types to the wire formats (`responses`, `completions`,
  `messages`; the legacy `openai`, `openai-compat` and `anthropic` are migrated). Its config is
  now a `crushrc`, Bash with Crush builtins; the JSON format is deprecated (P [cr1]).
- **Gemini CLI** documents `GOOGLE_GEMINI_BASE_URL` and `GOOGLE_VERTEX_BASE_URL` (research 09 said
  undocumented) (P [gm1]). Its README now says it is meant for Gemini Code Assist Standard /
  Enterprise only (§3).
- **Copilot CLI** has full BYOK: provider types `openai`, `azure`, `anthropic`, and local Ollama
  (P [cp1]). Research 09 did not cover it.
- **Codex** adds built-in Bedrock (Mantle and Runtime) and Vertex (`gcp`) providers, and a
  `capabilities` table for custom Responses providers (P [cx2]).
- **Goose** moved to `aaif-goose/goose` and deprecated its CLI pass-through providers
  (`claude-code`, `codex`, `gemini-cli`, `gemini_oauth`) in favour of ACP providers
  (`claude-acp`, `codex-acp`) (P [gs2]).

## 2. Per-session routing: base URL, model, key, headers

The question for each: **can one launch be pointed at another endpoint without touching the
person's global config?**

| Harness | Base URL | Model | Key | Extra headers | Config location / inline config | One launch only? |
|---|---|---|---|---|---|---|
| **Claude Code** | `ANTHROPIC_BASE_URL` (P [cc2]) | `--model` (P [cc3]); `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL` (P [cc2]) | `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`; `apiKeyHelper` through `--settings` (P [cc3]) | `ANTHROPIC_CUSTOM_HEADERS` (C; also named by opencode's Claude Agent doc, P [oc3]) | `CLAUDE_CONFIG_DIR`; `--settings <file-or-json>` (P [cc3]) | **Yes**, env and flags. Managed `allowedProviders` can restrict the routes a machine may use (P [cc1]) |
| **Codex** | `-c model_providers.<id>.base_url=…` with `-c model_provider=<id>` (P [cx2]) | `-m/--model` (P, purlis's flag list [pu6]) | `env_key` names the variable; `experimental_bearer_token`; command-backed `auth`; AWS SigV4 (P [cx2]) | `http_headers`, `env_http_headers` (P [cx2]) | `CODEX_HOME`; `-c key=value`; `--profile` (P [pu6]) | **Yes**: `-c` overrides per launch. Built-in providers cannot be overridden, only extended (P [cx2]) |
| **opencode** | `providers.<id>.settings.baseURL` (V2) / `provider.<id>.options.baseURL` (v1) (P [oc1], [oc2]) | `--model provider/model`; `"model"` in config (P [oc1]) | `env` list on the provider, `{env:NAME}` substitution; `/connect` keys go to `~/.local/share/opencode/auth.json` (P [oc1], [oc2]) | `providers.<id>.headers`; `body` merged into requests (P [oc1]) | `OPENCODE_CONFIG_CONTENT` (inline, above project config in v1), `OPENCODE_CONFIG`, `OPENCODE_CONFIG_DIR` (P [oc4]). V2's core still reads `OPENCODE_CONFIG_CONTENT` (P, `packages/core/src/config.ts`) | **Yes**, inline config. Managed config and macOS MDM outrank it (P [oc4]) |
| **Crush** | `provider add … --base-url` in a `crushrc` (P [cr1]) | `model add` and the picker (P [cr1]) | `--api-key "$VAR"`; `$VAR` and `$(command)` expansion; well-known `*_API_KEY` variables (P [cr1], [cr2]) | `--extra-header` (P [cr1]) | `./.crushrc` first, then global; `CRUSH_GLOBAL_CONFIG`, `CRUSH_GLOBAL_DATA`, `CRUSH_DATA_DIR` (P [cr1]) | **Yes**, by pointing `CRUSH_GLOBAL_CONFIG` at a generated file. No inline-config variable was found (P [cr1]) |
| **Gemini CLI** | `GOOGLE_GEMINI_BASE_URL` (API-key auth), `GOOGLE_VERTEX_BASE_URL` (Vertex) (P [gm1]) | `-m/--model`, `GEMINI_MODEL` (P [gm1]) | `GEMINI_API_KEY`, `GOOGLE_API_KEY` (P [gm1]) | none for model requests found; `headers` exists for MCP servers and OTLP only (P [gm1]) | `GEMINI_CLI_HOME` (root of `.gemini`) (P [gm1]) | **Yes**, env |
| **Goose** | custom provider JSON, or `OPENAI_HOST` / `OPENAI_BASE_PATH` (P [gs1]) | `GOOSE_MODEL` (P [gs3]) | provider variables; system keyring, or `~/.config/goose/secrets.yaml` when it is off (`GOOSE_DISABLE_KEYRING`) (P [gs3]) | `OPENAI_CUSTOM_HEADERS`; custom-provider headers (P [gs1]) | `GOOSE_PROVIDER`, `GOOSE_PROVIDER__TYPE`, `GOOSE_PROVIDER__HOST`… (P [gs3]) | **Yes**, env |
| **Aider** | `--openai-api-base` / `OPENAI_API_BASE` (P [ad1], [ad2]) | `--model` (P [ad2]) | `--openai-api-key`, `--anthropic-api-key`, `--api-key provider=key` (P [ad2]) | `--set-env` for LiteLLM variables (P [ad2]) | `-c` config file, `--env-file`, `--model-settings-file` (P [ad2]) | **Yes**, flags |
| **Copilot CLI** | `COPILOT_PROVIDER_*` (the base-URL variable's exact name was not read: U) (P [cp1]) | `--model`, `COPILOT_MODEL`, `COPILOT_PROVIDER_MODEL_ID`, `COPILOT_PROVIDER_WIRE_MODEL` (P [cp1]) | `COPILOT_PROVIDER_API_KEY`, `…_BEARER_TOKEN`, `…_API_KEY_COMMAND` (P [cp1]) | a custom credential header (P [cp1]) | `COPILOT_HOME`; `providers.json`, or the file `COPILOT_PROVIDERS_CONFIG` names. `COPILOT_PROVIDER_*` beats a saved `providers.json` unless `COPILOT_PROVIDERS_CONFIG` is set (P [cp1]) | **Yes**, env |
| **Amp** | Custom URL connection (C) | per mode (C) | in Amp's settings (C) | ? | `amp config model-providers` (C) | **Not documented** (C) |
| **Cursor CLI** | ✗ | `--model` among Cursor-hosted models (C) | ✗ | ✗ | ✗ | **No** (C) |

## 3. Each harness's own subscription sign-ins

Only sign-ins the harness offers **through its own flow**. A key typed into a harness's own
prompt is still its login (ADR 0087 §2), and that is noted where it applies. "Official" means
the harness's maintainers ship and document it; "plugin" means a third party.

| Harness | Plan | Flow | Where the credential is kept | Official? | Mark |
|---|---|---|---|---|---|
| **Claude Code** | Claude Pro / Max / Team / Enterprise | `claude auth login --claudeai` (default), `/login`; `--sso`; `claude setup-token` for a long-lived token | `CLAUDE_CONFIG_DIR` (purlis: `claude.toml` `[folders]`); macOS keychain (C) | Yes | P [cc3], [cc4] |
| | Anthropic Console (API billing) | `claude auth login --console` | same | Yes | P [cc3] |
| | Claude apps gateway (an org's own gateway) | `/login` against the gateway | same | Yes | P [cc4] |
| **Codex** | ChatGPT plan | `codex login` (browser on a local server), `--device-auth`, access-token login | `CODEX_HOME/auth.json` by default; `cli_auth_credentials_store` = `file` / `keyring` / `auto` / `ephemeral` | Yes | P [cx3], [cx4] |
| | API key | `codex login --with-api-key` (stdin) | same | Yes | P [cx3] |
| **opencode** (v1.19.5) | ChatGPT Plus / Pro | `/connect` → OpenAI → "ChatGPT Plus/Pro" (browser OAuth); CLI `opencode auth login` | `~/.local/share/opencode/auth.json` | Yes, built in | P [oc2] |
| | GitHub Copilot (incl. Enterprise) | `/connect` → GitHub Copilot → device flow | same | Yes, built in | P [oc2], [oc1] |
| | GitLab Duo | listed as a zero-setup subscription | same | Yes, built in | P [oc2] |
| | OpenCode Zen / Go | console key pasted into `/connect` (a key, not OAuth) | same | Yes, opencode's own | P [oc1], [oc5] |
| | Claude Pro / Max | **removed** from the bundle as of 1.3.0. The docs say "Anthropic explicitly prohibits this"; third-party plugins exist | — | No | P [oc2] |
| **opencode** (V2, `dev`, unreleased) | ChatGPT Plus / Pro | `/connect` → OpenAI → "Sign in with ChatGPT" or legacy Codex sign-ins. Context is capped at Codex's defaults (500k / 272k) unless `-opencode.provider.openai-context-caps` | location-scoped store (path not read) | Yes | P [oc1] |
| | GitHub Copilot | device flow; model list fetched from the account after login | — | Yes | P [oc1] |
| | **Claude Agent** (Claude plan through Claude Code) | `claude auth login`, then `/connect` → Claude Agent or `opencode auth login claude-agent`. opencode **runs Claude Code ≥ 2.1.248**, which calls the model; opencode's tools replace Claude Code's. Page marked `noindex` | Claude Code's own (opencode does not hold it) | Yes, opencode's | P [oc3] |
| **Crush** | Charm Hyper (Crush's own provider) | `crush login hyper` | written to Crush's global scope (`login.go:101`); the path is inferred as `~/.local/share/crush/crush.json` | Yes | P [cr3]; path inferred from [cr1] |
| | GitHub Copilot | `crush login copilot` | same | Yes | P [cr3] |
| | ChatGPT | `crush login openai` (`chatgpt`) | same | Yes | P [cr3] |
| | xAI Grok | `crush login grok` (`xai`) | same | Yes | P [cr3] |
| **Gemini CLI** | Sign in with Google: Code Assist Standard / Enterprise, Code Assist for GitHub | browser sign-in from `gemini`; credentials "cached locally" (`GEMINI_CLI_HOME/.gemini`) | Yes | P [gm2], [gm3] |
| | Google AI Pro / Ultra, free tier | **moved to Antigravity CLI on 2026-06-18**, per the README. The auth page still tells Pro / Ultra subscribers to sign in with Google: the two primary pages disagree | — | — | P [gm3] vs [gm2] |
| **Goose** | ChatGPT Plus / Pro | provider `chatgpt_codex`, browser OAuth | system keyring (secrets) | Yes | P [gs1], [gs3] |
| | GitHub Copilot | provider `github_copilot`, device flow | system keyring | Yes | P [gs1] |
| | Claude plan, ChatGPT plan through the vendor's own CLI | ACP providers `claude-acp`, `codex-acp`: Goose drives Claude Code or Codex over ACP; the CLI pass-throughs (`claude-code`, `codex`, `cursor-agent`, `gemini-cli`) are deprecated | the wrapped CLI's own | Yes, Goose's; ACP adapters by `agentclientprotocol` | P [gs1], [gs2] |
| | others in source: `xai_oauth`, `kimicode`, `copilot_acp`, `amp_acp`, `pi_acp` | not read beyond file names | — | — | P (file list, `crates/goose/src/providers/`) |
| **Aider** | none of its own. Copilot works by pasting an `oauth_token` copied from a JetBrains IDE's `~/.config/github-copilot/apps.json` as `OPENAI_API_KEY` | — | — | No: borrowed from another client | P [ad3] |
| **Copilot CLI** | GitHub Copilot | `/login` or `copilot login` (device flow); `GH_TOKEN` / `GITHUB_TOKEN` / `COPILOT_GITHUB_TOKEN` | OS keychain ("waits up to 2 minutes for keychain unlock") | Yes | P [cp2], [cp1] |
| **Amp** | ChatGPT; 𝕏 Premium+ / SuperGrok; **no Claude plan** | connections in Amp's settings | ? | Yes | C |
| **Cursor CLI** | Cursor plan | Cursor login | ? | Yes | C |
| **Antigravity CLI** | free, Google AI Pro / Ultra | ? | ? | Yes | U (named by [gm3]) |

**Who carries a Claude plan today.** Claude Code itself. Plus two harnesses that **run Claude Code
as the model client** and leave its sign-in with Claude Code: opencode V2's Claude Agent and
Goose's `claude-acp`. None of them reads the Claude credential. No harness found ships a native
Claude-plan OAuth: opencode removed its plugin in 1.3.0. Whether running Claude Code beneath
another harness is within Anthropic's terms is a legal reading, not this note's (#6578).

## 4. What breaks behind a non-native model

| Harness | Tool calls | Reasoning items | Prompt caching | Model discovery / limits | Mark |
|---|---|---|---|---|---|
| **Claude Code** | A gateway must forward `anthropic-beta` / `anthropic-version` unchanged and stream without buffering (C). Anthropic "doesn't support routing Claude Code to non-Claude models" (C) | thinking signatures, adaptive thinking (C) | `cache_control` must pass through, or every turn bills as uncached (C) | Discovery reads `GET /v1/models`, keeping only ids containing `claude` / `anthropic` (C). `CLAUDE_CODE_MAX_CONTEXT_TOKENS` now sets the window for unrecognized `claude-*` ids; a fix for "many third-party gateways … stopping with a context length error on every turn" landed in 2.1.325 (P [cc4]). Auto mode behind a gateway needs `CLAUDE_CODE_AUTO_MODE_SERVER` (P [cc4]) | mixed |
| **Codex** | Custom Responses providers get `capabilities` overrides (`cyber_access_programs`, `external_web_access`, `remote_compaction`), all off by default except on OpenAI (P [cx5]). Freeform `apply_patch` custom tools and `previous_response_id` are rejected by many non-OpenAI upstreams ([secondary], from research 09) | Responses reasoning items: same secondary report | remote compaction off unless declared (P [cx5]) | A custom provider is listed from its own OpenAI-compatible `/models`; the Codex catalog format is separate and only on ChatGPT hosts (P [cx6], [cx2]) | P + secondary |
| **opencode** | `requireAssistantAfterTool` for endpoints that reject tool → user order (Mistral by default) (P [oc6]) | `reasoningField` / `requireReasoning` for Chat-format reasoning (DeepSeek on by default); `supportsThinkingBlockBinding`, `supportsEffortUpdates` (P [oc6]) | Breakpoints are placed automatically only on routes known to need them. Claude behind an OpenAI-compatible proxy (LiteLLM) needs `compatibility.cacheFormat: "anthropic"`; `supportsPromptCacheKey` (P [oc6]) | models.dev catalog; local discovery for Ollama (`/api/tags`, `/api/show`, `completion` capability), LM Studio, vLLM; `maxTokensField`; `streamUsage` (P [oc1], [oc6]). Model options are provider-specific and may be rejected (P [oc6]) | P |
| **Crush** | type must match the API ("make sure to choose the right one") (P [cr1]) | ? | price fields for cache create / hit are declared per model (P [cr1]) | Catwalk catalogue; a custom model needs `--context-window` and `--default-max-tokens` by hand (P [cr1]) | P |
| **Gemini CLI** | ? | ? | ? | README: "Not optimized for Gemini 4+" (P [gm3]). Hang on an unknown model; env not passed into its sandbox (C) | P + C |
| **Goose** | `GOOSE_TOOLSHIM` for models that emit text tool calls, with an interpreter model (Ollama default) (P [gs3]) | reasoning captured from `reasoning_content` (Kimi), Gemini thinking blocks (P [gs1]) | ? | `goose configure` cannot take a custom model name; edit `GOOSE_MODEL` (P [gs1]) | P |
| **Aider** | none needed: text edit formats chosen per model (P [ad4]) | ? | ? | `--list-models`, `--model-metadata-file` (P [ad2]) | P |
| **Copilot CLI** | fixes to Anthropic thinking-mode mismatches and image handling on BYOK (P [cp1]) | "adaptive-thinking mismatches" on BYOK Anthropic, fixed (P [cp1]) | ? | quota display hidden while a BYOK model is selected (P [cp1]) | P |
| **Amp / Cursor CLI / Antigravity** | ? | ? | ? | ? | U |

## 5. What purlis's adapters do today (`main` @ `bcd9681ba6`)

- **Sign-in kinds** are `subscription`, `api-key`, `cloud`, `local`
  (`crates/purlis-core/src/harness_declaration.rs:188-199`). A subscription is "signed in through
  the harness's own flow … purlis never reads, copies, stores or relays it" (`:189-191`). [pu1]
- **Declared plans.** `claude.toml` lists `subscription`, `api-key`, `cloud`
  (`harness_declaration/claude.toml:33-38`). `codex.toml` adds `local` (`codex.toml:32-35`).
  `opencode.toml` lists the plans `openai` (ChatGPT) and `github-copilot`
  (`opencode.toml:35-44`). **A plan naming `anthropic` is refused in any declaration but
  Claude Code's** (`harness_declaration.rs:1333-1337`). [pu2]
- **Handed variables** per harness and kind (`crates/purlis-core/src/profilesignin.rs:142-180`):
  Claude Code's `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` and its Bedrock, Vertex and Foundry
  variables; Codex's `OPENAI_API_KEY` / `CODEX_API_KEY` and `AZURE_OPENAI_API_KEY`; opencode's
  five key variables and its AWS / GCP / Azure ones. Nothing is handed for any other harness
  (`_ => &[]`). [pu3]
- **No base URL is set by purlis yet.** A chat on an `api-key` profile "waits for the key proxy
  and starts nothing" (`crates/purlis-core/tests/it/a_chat_on_a_profile_starts_as_its_sign_in_says.rs:142`),
  pending #822 (open). ADR 0087 §7 (V36) names the targets: `ANTHROPIC_BASE_URL`, Codex's
  `model_providers.<id>.base_url`, opencode's `baseURL`. [pu4]
- **Local models**: purlis probes Ollama (`127.0.0.1:11434/api/tags`) and LM Studio
  (`127.0.0.1:1234/v1/models`) and names opencode as the harness that runs on them
  (`crates/purlis-core/src/noharness.rs:99-118`). [pu5]
- **Codex**: a chat's own `-c` may set `model`, `model_provider`, `model_providers` and the
  display keys, and nothing else (`crates/purlis-core/src/sandbox/codex.rs:835-853`); `CODEX_HOME`
  is the profile's folder and `auth.json` is one of its home files (`sandbox/codex.rs:126,135`).
  [pu6]
- **opencode** is configured through `OPENCODE_CONFIG_CONTENT` (`crates/purlis-core/src/opencode.rs:90`).
  [pu7]

### Where the readings meet the adapters

Facts for the grilling tickets, not decisions:

1. **opencode V2 changes the config shape** an adapter would write: `providers.<id>.settings.baseURL`
   and `package`, not `provider.<id>.options.baseURL` and `npm` (§1). ADR 0087 §7's "opencode's
   `baseURL`" holds for both, but a generated config must know which major version it targets.
2. **opencode's plans** in `opencode.toml` omit **GitLab Duo**, which v1 offers (§3). The
   allow-list keeps it out on purpose (`harness_declaration.rs:228-231`).
3. **opencode V2's Claude Agent and Goose's `claude-acp`** run Claude Code beneath another harness
   on the person's Claude plan. purlis's `anthropic` rule (`:1333`) refuses the *plan* in another
   declaration. It says nothing about a harness that starts Claude Code as its model client.
4. **Crush, Goose and Copilot CLI** each have their own ChatGPT and/or Copilot sign-ins (§3) and
   per-launch routing (§2). None has a purlis declaration or handed variables yet (`profilesignin.rs:180`).
5. **Gemini CLI** is for Code Assist Standard / Enterprise only, per its own README. A person on
   Google AI Pro / Ultra is pointed to Antigravity CLI, which this session could not read.
6. **Cursor CLI** still takes no outside endpoint (C), so ADR 0087 §4's "runs on `login` only"
   stands unless re-read.

## 6. Left unsettled

- **Not re-verified** (vendor sites unreachable from this session): Claude Code's docs pages
  (model discovery filter, "doesn't support … non-Claude models", header forwarding), Codex's
  developers.openai.com pages, Amp, Cursor CLI, Antigravity CLI, and OpenAI's cut-off of Cursor
  (research 09 §7.7). These are worth one pass from a session with WebFetch.
- **Copilot CLI's exact BYOK variable for the base URL and provider type**: `copilot help
  providers-env` names them, and that CLI is not installed here.
- **Where opencode V2 and Crush keep an OAuth token** on disk was not read from source.
- **Codex's non-OpenAI breakage** (freeform tools, reasoning items) is still a secondary reading.
- **Gemini CLI's two pages disagree** on Google AI Pro / Ultra (§3).

## Sources

All read 2026-11-14, at the commits in the table at the top.

- [cc1] `anthropics/claude-code` `CHANGELOG.md`: `allowedProviders` (Anthropic API, custom
  endpoint, Bedrock, Mantle, Vertex AI, Foundry, Claude Platform on AWS, Cloud gateway).
- [cc2] the same changelog: `ANTHROPIC_BASE_URL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `CLAUDE_CODE_MAX_CONTEXT_TOKENS`.
- [cc3] `claude --help`, `claude auth --help`, `claude auth login --help`, Claude Code 2.1.325.
- [cc4] the same changelog, 2.1.325 and earlier entries: gateway fixes, `setup-token`, Claude apps gateway `/login`, `CLAUDE_CODE_AUTO_MODE_SERVER`.
- [cx1] `openai/codex` `codex-rs/model-provider-info/src/lib.rs`: `WireApi` (Responses only), `CHAT_WIRE_API_REMOVED_ERROR`, `OLLAMA_CHAT_PROVIDER_REMOVED_ERROR`, `supports_websockets`.
- [cx2] the same file: `ModelProviderInfo` fields, `built_in_model_providers`, `merge_configured_model_providers`, `model_catalog_url`.
- [cx3] `codex-rs/cli/src/login.rs`: ChatGPT browser login, `--device-auth`, API-key and access-token login.
- [cx4] `codex-rs/config/src/types.rs`: `AuthCredentialsStoreMode`.
- [cx5] `codex-rs/model-provider-info/src/capabilities.rs`.
- [cx6] `codex-rs/model-provider/src/models_endpoint.rs`.
- [oc1] `anomalyco/opencode` `services/site/src/docs/content/providers.mdx` (V2).
- [oc2] `services/site/src/docs/v1/content/providers.mdx` (v1).
- [oc3] `services/site/src/docs/content/claude-agent.mdx` (V2, `noindex`).
- [oc4] `services/site/src/docs/v1/content/config.mdx` (precedence) and `content/config.mdx` (V2).
- [oc5] `services/site/src/docs/content/console/go.mdx`.
- [oc6] `services/site/src/docs/content/models.mdx`: Reasoning, Prompt caching, Compatibility, Caveats.
- [cr1] `charmbracelet/crush` `README.md`: Configuration, Custom Providers.
- [cr2] the same, API Keys table.
- [cr3] `internal/cmd/login.go`: platforms `hyper`, `copilot`, `openai` (`chatgpt`), `grok` (`xai`).
- [gm1] `google-gemini/gemini-cli` `docs/reference/configuration.md`: environment variables, flags.
- [gm2] `docs/get-started/authentication.mdx`.
- [gm3] `README.md`: the status banner (Code Assist only; Antigravity CLI since 2026-06-18).
- [gs1] `aaif-goose/goose` `documentation/docs/getting-started/providers.md`.
- [gs2] `documentation/docs/guides/cli-providers.md`: deprecation of the CLI pass-throughs.
- [gs3] `documentation/docs/guides/environment-variables.md`.
- [gs4] `crates/goose/src/providers/` (file list).
- [ad1] `Aider-AI/aider` `aider/website/docs/llms/openai-compat.md`.
- [ad2] `aider/website/docs/config/options.md`.
- [ad3] `aider/website/docs/llms/github.md`.
- [ad4] `aider/website/docs/more/edit-formats.md`.
- [cp1] `github/copilot-cli` `changelog.md` (1.0.111, 2026-11-13, and earlier): BYOK, `COPILOT_PROVIDER_*`, `providers.json`, `--acp`, keychain.
- [cp2] `github/copilot-cli` `README.md`: `/login`, `GH_TOKEN` / `GITHUB_TOKEN`, `COPILOT_GITHUB_TOKEN`.
- [pu1]–[pu7] this repository at `bcd9681ba6`, paths as cited in §5.
- Research 09: `purlis-plane/docs/program/research/09-model-gateway.md` (2026-09-29), for every **C**.
