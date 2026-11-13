# Model providers, routers and local model servers: wire formats and addressing

Research, 2026-11-14, for #6581 (part of map #6578, *Any model on any harness*). Facts only. This
document decides no policy; the person rules on it afterwards. Background on gateways is in
research 09 §2 (`purlis-plane/docs/program/research/09-model-gateway.md`, 2026-09-29).

**How this was read.** This chat had no web access. It had no WebFetch or WebSearch, and `curl` to
vendor docs sites was refused by the harness. So every finding comes from a **first-party source on
GitHub**, read with `gh api` on 2026-11-14 and pinned to the commit read:

- the vendor's own API model: OpenAI's OpenAPI spec, botocore's service JSON (AWS),
  googleapis protos, azure-rest-api-specs, OpenRouter's docs repo and OpenAPI file, xAI's
  OpenAPI and protos;
- the vendor's official SDK: Anthropic, OpenAI, Google GenAI, Mistral, xAI, DashScope;
- for the open-source servers, their source: Ollama, vLLM, llama.cpp, LiteLLM; for LM Studio,
  which is closed source, its docs repo and `lmstudio-js`.

A claim that needs the vendor's docs site to settle is marked **[unverified: docs site not read]**.
A later pass from a chat with web access is to check those marks and post the result on #6581. An
SDK shows what the client *sends*, not what the server *accepts*; where the two may differ, the
cell says so.

**Vocabulary** (map #6578): a *model provider* serves a model under its own API; a *router* is a
model provider that serves many vendors' models behind one API; a *local model server* runs on the
person's own machine or network.

## At a glance

Format codes: **M** Anthropic Messages (`/v1/messages`), **R** OpenAI Responses (`/v1/responses`),
**C** OpenAI Chat Completions (`/v1/chat/completions`), **G** Gemini `generateContent`. **N** marks
a native format that is none of these. Details and citations are in the per-service sections below.
In these two summary tables, a bare *[unverified]* abbreviates *[unverified: docs site not read]*.

| Service | M | R | C | G | Other native | Base URL | Credential header | Model id in |
|---|---|---|---|---|---|---|---|---|
| Anthropic | ✓ | – | [unverified] | – | – | `https://api.anthropic.com` | `Authorization: Bearer` (SDK 2.x, API `2026-10-21`); `x-api-key` (SDK 1.x, `2023-06-01`) + `anthropic-version` | body |
| OpenAI | – | ✓ | ✓ | – | Responses over WebSocket | `https://api.openai.com/v1`, `{us,eu,ae,au,in,jp}.api.openai.com/v1` | `Authorization: Bearer` | body |
| Google Gemini API | – | – | compat `/v1beta/openai/` | ✓ | – | `https://generativelanguage.googleapis.com/{v1,v1beta}` | `x-goog-api-key` (or `?key=`); Bearer on compat | path |
| DeepSeek | ✓ `/anthropic` | – | ✓ | – | – | `https://api.deepseek.com/anthropic`; `/v1` for C | `x-api-key` + `anthropic-version` (M); Bearer for C [unverified] | body |
| Mistral | – | ✓ | ✓ (Mistral-shaped) | – | Conversations/Agents | `api.mistral.ai`, `api.eu.mistral.ai`, `api.us.mistral.ai` | `Authorization: Bearer` | body |
| xAI | ✓ | ✓ | ✓ | – | **gRPC** (`api.x.ai:443`) | `https://api.x.ai/v1` | `Authorization: Bearer` | body |
| Qwen / Alibaba DashScope | ✓ `/apps/anthropic` | – | ✓ `/compatible-mode/v1` | – | DashScope native `/api/v1/services/aigc/…` | `{workspace}.{region}.maas.aliyuncs.com`; legacy `dashscope[-intl].aliyuncs.com` | `Authorization: Bearer` (+ `X-DashScope-SSE: enable` for a native stream) | body |
| AWS Bedrock | via InvokeModel body | [unverified] | [unverified] | – | Converse / ConverseStream; **binary event-stream** | `https://bedrock-runtime.{region}.amazonaws.com` | **SigV4**, or `Authorization: Bearer` (`AWS_BEARER_TOKEN_BEDROCK`) | **path** |
| Google Vertex AI | via `rawPredict` (Claude) | – | ✓ `…/endpoints/openapi` | ✓ | – | `https://{location}-aiplatform.googleapis.com/v1/projects/{p}/locations/{l}/…` | **OAuth token from ADC**; `x-goog-api-key` in express mode | **path** |
| Azure OpenAI / Foundry | ✓ Foundry `/anthropic/` | ✓ | ✓ | – | – | `{endpoint}/openai/v1`; dated `…/deployments/{id}?api-version=` | `api-key`, or **Entra ID** bearer; Foundry Claude `x-api-key` | body (v1), **path** (dated) |
| OpenRouter (router) | ✓ | ✓ | ✓ | – | – | `https://openrouter.ai/api/v1` | `Authorization: Bearer`; `x-api-key` for Anthropic SDKs | body |
| LiteLLM proxy (router) | ✓ | ✓ | ✓ | ✓ | pass-through `/bedrock`, `/vertex_ai`, … | the operator's own host | any of seven headers; `x-litellm-api-key` overrides | body (G: path) |
| LM Studio (local) | ✓ (0.4.1+) | ✓ | ✓ | – | REST `/api/v1`, `/api/v0` | `http://localhost:1234/v1` | none by default; Bearer token (0.4.0+); `x-api-key` on M | body |
| Ollama (local) | ✓ | ✓ (stateless) | ✓ | – | `/api/chat`, `/api/generate` (**NDJSON**) | `http://127.0.0.1:11434` | none locally; Bearer for ollama.com | body |
| vLLM (local) | ✓ | ✓ | ✓ | – | – | `http://<host>:8000/v1` (binds all interfaces by default) | none by default; `Authorization: Bearer` only, even on M | body |
| llama.cpp `llama-server` (local) | ✓ | ✓ | ✓ | – | `/completion`, `/infill` | `http://127.0.0.1:9931/v1` (8080 before 2026-10-09) | none by default; `Authorization` or `X-Api-Key`, `Bearer ` optional | body (ignored in single-model mode [unverified]) |

## Is a data declaration enough?

**The question.** Can one record of data describe each service: base URL, wire format, key header,
model list and usage fields? ADR 0087 §7 sets what that record must serve. The key proxy swaps one
credential header and passes the request and its stream through byte for byte. The host meters
every call it carries.

**Answer: yes for 13 of the 16, once the record grows four fields. No for the three clouds' own
credentials, and no for Bedrock's stream framing.** None of the gaps needs a per-provider
extension. Each is a small closed set that core can own once: three cloud signers (#2151), one
binary stream decoder and one probe rule for local servers. Per service:

| Service | Declaration enough? | Why not, where not |
|---|---|---|
| Anthropic, OpenAI, Gemini API, DeepSeek, Mistral, xAI (HTTP), DashScope, OpenRouter, LiteLLM | **Yes**, with the fields below | A fixed host, a static key in one header and a JSON model list. Regions are a choice of base URL: OpenAI, Mistral and DashScope. Anthropic's region is a body field (`inference_geo`), which the proxy leaves alone. |
| LM Studio, Ollama, vLLM, llama.cpp | **Yes, for addressing.** Not for finding the server or reading its state | The port moves: by user choice, or with llama.cpp's build (9931 against 8080). Whether a model is pulled and loaded is runtime state, and so is its context length, fixed at load. A declaration can carry probes (LM Studio `GET /lmstudio-greeting` → `{"lmstudio":true}`; Ollama `GET /api/version`; llama.cpp and vLLM `GET /health`). Running them and reading the state is core's work. |
| AWS Bedrock | **No** | SigV4 signs the final body, so a proxy that swaps one header cannot carry it. Credentials come through profiles, SSO or STS. The stream is AWS binary event-stream wrapping a second, model-native format, so metering needs a decoder. The model id, often an ARN or inference profile, is URL-encoded into the path. Bedrock API keys (bearer) remove the signing but not the framing. |
| Google Vertex AI | **No** (yes in express mode, for Gemini) | An OAuth access token is minted from ADC and refreshed about every hour. The project and location sit in every path, with the model there too. Claude's `anthropic_version` goes in the body, so an Anthropic Messages harness's request has to be reshaped: that stands in (ADR 0073 §4). |
| Azure OpenAI / Foundry | **Partly**: yes with an `api-key` on the v1 API | Entra ID tokens are minted against two audiences: data, and ARM for listing deployments. The dated API puts a deployment in the path and an `api-version` query that changes the schema. The model list sits behind Azure Resource Manager (subscription, resource group, account), not the data host. |

**What the record must hold beyond the four fields the ticket names.** The research shows each is
needed, and each is still plain data:

1. **The header's scheme, and constant headers.** A header name alone is not enough. The record
   needs the name and value template (`Authorization: Bearer {key}`, `x-api-key: {key}`,
   `x-goog-api-key: {key}`, `api-key: {key}`) plus fixed headers such as `anthropic-version`. The
   same vendor changes the scheme between API versions: Anthropic's SDK sends `x-api-key` under
   `2023-06-01` and `Authorization: Bearer` under `2026-10-21`. llama.cpp and LiteLLM accept
   several headers; vLLM accepts only Bearer, even on its Anthropic route. So the record names one
   header per format, not one per service.
2. **Formats as a list, each with its own path prefix.** One service speaks up to four formats on
   different prefixes. DeepSeek uses `/anthropic`, DashScope `/apps/anthropic` and
   `/compatible-mode/v1`, Gemini `/v1beta/openai/`. A record of one format per service cannot
   describe xAI, DashScope, OpenRouter or any of the four local servers.
3. **Base URL templates.** Regions, workspaces and resources are placeholders:
   `{workspace}.{region}.maas.aliyuncs.com`, `{resource}.services.ai.azure.com`.
4. **A model list described by shape, not only by URL.** The record needs the path, the field
   holding the id and the optional fields for context and capabilities. The shapes differ:
   `data[].id` (OpenAI-style), `models[].name` (Gemini, Ollama `/api/tags`), `ModelInfo`
   (Anthropic, now with `max_input_tokens` and `capabilities`). OpenAI's `Model` has no context
   length, and vLLM's is `max_model_len`. Where the list sits on another host (Bedrock's control
   plane, Azure ARM), the data path ends.

**Usage fields belong to the format, not the service, with additions by service.** A meter reading
streams needs per format:

- **M**: `message_start.message.usage`, then **cumulative** values in `message_delta.usage`
  (overwrite, never add);
- **C**: one chunk with `choices: []` before `[DONE]`;
- **R**: `response.completed`;
- **G**: `usageMetadata` on every chunk (take the last one).

Each service adds its own names: DeepSeek `prompt_cache_hit_tokens`/`prompt_cache_miss_tokens`
[unverified], OpenRouter `usage.cost` and `cost_details`, Ollama native `prompt_eval_count` and
`eval_count`, llama.cpp `timings`. A declaration can map them as field paths.

**One finding matters for metering through a byte-for-byte proxy.** On Chat Completions, usage on a
stream is **opt-in by the client** (`stream_options.include_usage`) at OpenAI, xAI, llama.cpp,
vLLM, LM Studio and Ollama. DashScope's own SDK always sets it because "OpenAI-compatible servers
omit usage from streams unless asked". A proxy that changes nothing in the body cannot set it. So
whether a streamed Chat Completions call can be metered depends on the harness, not on the
declaration. The exceptions:

- OpenRouter always sends usage and has deprecated the opt-in;
- vLLM has `--enable-force-include-usage`;
- Messages, Responses and Gemini always carry usage.

The matrix (MS-2) will need this per harness.

**Streaming quirks a declaration names but core must handle.** None of these is per-provider code;
each is a property of a format or a framing:

- error events inside a 200: Anthropic `event: error`, OpenRouter's top-level `error` chunk and
  `response.failed`;
- SSE comments: OpenRouter `: OPENROUTER PROCESSING`;
- `[DONE]` sent on Chat streams but not on Responses or Messages: llama.cpp;
- NDJSON, not SSE: Ollama native;
- binary event-stream: Bedrock;
- gRPC: xAI native, outside the HTTP proxy altogether;
- reasoning under different names: `reasoning_content`, `reasoning` in Ollama's OpenAI mode,
  `thinking` natively.

## Left unsettled

The cells marked **[unverified: docs site not read]** below are each settled by one vendor docs
page. The ones that bear most on the declaration answer:

- whether Anthropic's API still accepts `x-api-key` and `2023-06-01` under the `2026-10-21` API;
- whether xAI's and DashScope's Anthropic endpoints accept `x-api-key`;
- DeepSeek's Chat Completions base URL and auth: the usage field names come from a converter
  library, not from the live API;
- Bedrock's OpenAI-compatible routes and the `bedrock-2023-05-31` body, seen only in LiteLLM's
  source;
- whether Vertex express mode covers Claude;
- the Azure v1 API's body `model` (deployment name or not), and its content-filter chunks;
- whether Gemini's `/v1beta/openai/` honours `stream_options.include_usage`;
- what each local server's Anthropic route leaves unsupported (LM Studio, llama.cpp);
- LiteLLM's default port;
- one oddity, not looked into: xAI's TypeScript SDK and spec sometimes name the vendor
  "SpaceXSI".


## Model providers (vendor APIs)

### Anthropic

| Aspect | Finding [n] |
|---|---|
| Formats spoken | Native Anthropic Messages: `POST /v1/messages`, `POST /v1/messages/count_tokens`, `/v1/messages/batches*` [6]. The beta surface uses the same `POST /v1/messages` path, switched by the `anthropic-beta` header [17]. No first-party source read here documents an OpenAI-SDK compatibility endpoint: a code search of `anthropics/anthropic-sdk-python` for `openai` finds only internal `_compat.py` modules, and an org-wide search for `"v1/chat/completions"` finds only third-party integrations [22]. OpenAI-compat: **[unverified: docs site not read]**. The SDK also ships separate clients for other hosts (`lib/bedrock`, `lib/vertex`, `lib/google_cloud`, `lib/aws`, `lib/foundry.md`) [22]. |
| Base URL(s) | `https://api.anthropic.com`; overridden by `base_url` kwarg > `ANTHROPIC_BASE_URL` > profile/credentials config [1][3]. Paths carry `/v1/...` themselves [6]. The SDK names no regional host. Data residency is a body field instead: `inference_geo` ("geographic region for inference processing", defaults to the workspace's `default_inference_geo`), reported back as `usage.inference_geo` [19][11]. |
| Key header / required headers | SDK 2.x (API version `2026-10-21`) sends the API key as `Authorization: Bearer <key>`. `auth_token` uses the same header [1][3]. SDK 1.17.0 sent `X-Api-Key: <key>` with `anthropic-version: 2023-06-01` [4]. The 2.0.0 changelog lists "add support for new API version 2026-10-21" as a breaking change [5]. The client-side header check still accepts either `X-Api-Key` or `Authorization` [1][3]. Always sent: `anthropic-version: 2026-10-21` [1][2][3]. Optional: `anthropic-beta: <comma-joined beta names>` [17][18]; `anthropic-workspace-id` (for credentials that span workspaces) and `anthropic-user-profile-id` [17][10]; `anthropic-dangerous-direct-browser-access: true` from browsers [3]. Whether the server still accepts `x-api-key` under 2026-10-21, or `2023-06-01` at all: **[unverified: docs site not read]**. |
| Model list endpoint | `GET /v1/models` (cursor paging: `after_id`/`before_id`, `limit` 1–1000, default 20; page has `has_more`, `first_id`, `last_id`; filter `lifecycle[]`: active/deprecated/retired, where retired models appear only when asked for) and `GET /v1/models/{model_id}` [6][9]. `ModelInfo` fields: `id`, `type`="model", `display_name`, `created_at`, `lifecycle`, `deprecated_at?`, `retires_at?`, `line?`, **`max_input_tokens?`** (context window), **`max_tokens?`** (max for the `max_tokens` param), **`capabilities?`** [7]. `ModelCapabilities`: `batch`, `citations`, `code_execution`, `context_management`, `effort`, `image_input`, `pdf_input`, `server_tools`, `structured_outputs`, `thinking` [8]. |
| Usage fields | `Message.usage` (`Usage`): `input_tokens` (req), `output_tokens` (req), `cache_creation_input_tokens?`, `cache_read_input_tokens?`, `cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}?`, `output_tokens_details.thinking_tokens?` (≤ output_tokens; `output_tokens` stays the billed total), `server_tool_use.{web_search_requests, web_fetch_requests}?`, `service_tier?` (standard/priority/batch), `speed?` (standard/fast), `inference_geo?` [11][13]. Total input = `input_tokens + cache_creation_input_tokens + cache_read_input_tokens` [14]. |
| Usage in a stream | `message_start.message.usage` carries the initial `Usage` [14]. `message_delta.usage` (`MessageDeltaUsage`) carries `output_tokens` (req) and optional `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `server_tool_use`, `output_tokens_details`, each documented as **cumulative** [12]. The SDK accumulator **overwrites** with these values and keeps the `message_start` value when an optional one is absent [15]. |
| Streaming quirks | SSE with `event:` lines. Events: `message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop` [14]. `event: ping` is skipped. Frames with no event name are skipped. `event: error` carries a JSON error body raised as a status error mid-stream (HTTP status already 200) [16][23]. No `[DONE]` sentinel is handled; the stream ends after `message_stop` [16]. The TS SDK yields SSE events with unrecognized names (changelog 0.136.0) [3]. |
| Not capturable by a data declaration | Cumulative-not-delta semantics of `message_delta.usage`, and the absent-means-keep merge rule [12][15]. Per-feature `anthropic-beta` values, comma-joined and per request (the 50+ values in [18]). API-version-dependent auth header (`X-Api-Key` under 2023-06-01 vs `Authorization: Bearer` under 2026-10-21) [4][1]. Retries: `x-should-retry: true/false` overrides; otherwise retry on 408/409/429/≥500; delay from `retry-after-ms` then `retry-after` (seconds or HTTP date); defaults to 2 retries, 0.5–8 s backoff [20][2][3]. Request id comes in the `request-id` response header (`req_…`), and `anthropic-workspace-id` is echoed [21][3]. The base client supports an idempotency header, but it defaults to `None` and the Anthropic client never sets one, so no idempotency key is sent [20][1]. `error` events arrive inside a 200 stream [16]. `inference_geo`/`service_tier`/`speed` are request-body knobs that change billing [19][11]. |

[1] anthropics/anthropic-sdk-python/src/anthropic/_client.py@5139cf2e7d07 — read 2026-11-14
[2] anthropics/anthropic-sdk-python/src/anthropic/_constants.py@5139cf2e7d07 — read 2026-11-14
[3] anthropics/anthropic-sdk-typescript/src/client.ts@590783078cd9 (plus CHANGELOG.md@590783078cd9 for 0.136.0) — read 2026-11-14
[4] anthropics/anthropic-sdk-python/src/anthropic/_client.py@5738a256035e and src/anthropic/_constants.py@5738a256035e (tag v1.17.0); compared with the same files @ecfee6cd504c (tag v2.0.0) — read 2026-11-14
[5] anthropics/anthropic-sdk-python/CHANGELOG.md@5139cf2e7d07 — read 2026-11-14
[6] anthropics/anthropic-sdk-python/api.md@5139cf2e7d07 — read 2026-11-14
[7] anthropics/anthropic-sdk-python/src/anthropic/types/model_info.py@5139cf2e7d07 — read 2026-11-14
[8] anthropics/anthropic-sdk-python/src/anthropic/types/model_capabilities.py@5139cf2e7d07 — read 2026-11-14
[9] anthropics/anthropic-sdk-python/src/anthropic/types/model_list_params.py@5139cf2e7d07 and src/anthropic/pagination.py@5139cf2e7d07 — read 2026-11-14
[10] anthropics/anthropic-sdk-python/src/anthropic/resources/models.py@5139cf2e7d07 — read 2026-11-14
[11] anthropics/anthropic-sdk-python/src/anthropic/types/usage.py@5139cf2e7d07 — read 2026-11-14
[12] anthropics/anthropic-sdk-python/src/anthropic/types/message_delta_usage.py@5139cf2e7d07 — read 2026-11-14
[13] anthropics/anthropic-sdk-python/src/anthropic/types/{server_tool_usage,cache_creation,output_tokens_details}.py@5139cf2e7d07 — read 2026-11-14
[14] anthropics/anthropic-sdk-python/src/anthropic/types/{raw_message_start_event,raw_message_delta_event,raw_message_stream_event}.py@5139cf2e7d07 — read 2026-11-14
[15] anthropics/anthropic-sdk-python/src/anthropic/lib/streaming/_messages.py@5139cf2e7d07 — read 2026-11-14
[16] anthropics/anthropic-sdk-python/src/anthropic/_streaming.py@5139cf2e7d07 — read 2026-11-14
[17] anthropics/anthropic-sdk-python/src/anthropic/resources/beta/messages/messages.py@5139cf2e7d07 — read 2026-11-14
[18] anthropics/anthropic-sdk-python/src/anthropic/types/anthropic_beta_param.py@5139cf2e7d07 — read 2026-11-14
[19] anthropics/anthropic-sdk-python/src/anthropic/types/message_create_params.py@5139cf2e7d07 — read 2026-11-14
[20] anthropics/anthropic-sdk-python/src/anthropic/_base_client.py@5139cf2e7d07 — read 2026-11-14
[21] anthropics/anthropic-sdk-python/src/anthropic/_response.py@5139cf2e7d07 and src/anthropic/_models.py@5139cf2e7d07 — read 2026-11-14
[22] anthropics/anthropic-sdk-python git tree@5139cf2e7d07 (`src/anthropic/lib/*`); GitHub code search `openai repo:anthropics/anthropic-sdk-python` and `"v1/chat/completions" org:anthropics` (hits only in third-party integration repos) — read 2026-11-14
[23] anthropics/anthropic-sdk-typescript/src/core/streaming.ts@590783078cd9 — read 2026-11-14

### OpenAI

| Aspect | Finding [n] |
|---|---|
| Formats spoken | Both native: Responses (`POST /responses`, `GET /responses/{response_id}`) and Chat Completions (`POST /chat/completions`, `GET /chat/completions/{completion_id}`) [1]. Responses can also run over WebSocket: the spec's `response.incomplete` event mentions "Over WebSocket, steering…" [9], and the SDK has a `websocket_base_url` that defaults to the base URL with `wss://` [3]. Spec `info.version` is `2.3.0` [1]. |
| Base URL(s) | `https://api.openai.com/v1` (spec `servers`) [1]. SDK: `base_url` kwarg > `OPENAI_BASE_URL` > default (an mTLS base URL is used when an x509 identity is configured) [4]. `data_residency` names regional hosts: `global`→`https://api.openai.com/v1`, `us`→`https://us.api.openai.com/v1`, `eu`→`https://eu.api.openai.com/v1`, `ae`→`https://ae.api.openai.com/v1`, `au`→`https://au.api.openai.com/v1`, `in`→`https://in.api.openai.com/v1`, `jp`→`https://jp.api.openai.com/v1`. It cannot be combined with `base_url`, `websocket_base_url` or `provider` [2][3]. |
| Key header / required headers | `Authorization: Bearer <key>` (spec security scheme `ApiKeyAuth`: http/bearer; admin key `AdminApiKeyAuth`, also bearer) [1][4]. Optional: `OpenAI-Organization` (env `OPENAI_ORG_ID`), `OpenAI-Project` (env `OPENAI_PROJECT_ID`), each omitted when unset [3]. Key env `OPENAI_API_KEY`; admin `OPENAI_ADMIN_KEY` [3]. No version header. |
| Model list endpoint | `GET /models` → `{object:"list", data: Model[]}`; `GET /models/{model}` [5]. `Model` fields: `id`, `object`="model", `created` (unix s), `owned_by`, `shutdown_date?` (date or null). **No context length, max tokens or capabilities** [5]. |
| Usage fields | Chat `usage` (`CompletionUsage`): `prompt_tokens`, `completion_tokens`, `total_tokens` (req); `prompt_tokens_details.{cached_tokens, cache_write_tokens, audio_tokens, image_tokens, text_tokens}`; `completion_tokens_details.{reasoning_tokens, accepted_prediction_tokens, rejected_prediction_tokens, audio_tokens, text_tokens}` [6]. Responses `response.usage` (`ResponseUsage`, nullable): `input_tokens`, `output_tokens`, `total_tokens`, `input_tokens_details.{cached_tokens, cache_write_tokens, cache_write_tokens_details.{ttl_30m_tokens, ttl_12h_tokens}}`, `output_tokens_details.reasoning_tokens`, all required [7]. |
| Usage in a stream | Chat: only with `stream_options: {"include_usage": true}`. An extra chunk comes before `data: [DONE]` with `choices: []` and the request's whole `usage`; every other chunk has `usage: null`. An interrupted stream may never send it [8]. Responses: `response.completed` (and `response.failed` / `response.incomplete`) carry the full `response` object, whose `usage` is the place to read [9][7]. |
| Streaming quirks | Chat: SSE ending in `data: [DONE]` [8]. The SDK stops at `[DONE]`, and raises when a data frame holds an `error` key [10]. Responses: typed events (`type` + `sequence_number`), e.g. `response.created`, `response.in_progress`, `response.queued`, `response.output_text.delta`, `response.completed`, `response.failed`, `response.incomplete`, and `error` (`code`, `message`, `param`) [9]. The `include_obfuscation` description says streaming delta events get a random-padding `obfuscation` field by default, and `false` turns it off. That field is not in `ChatCompletionStreamResponseDelta`'s schema [8]. Assistants `thread.*` events are told apart by their `event:` line [10]. |
| Not capturable by a data declaration | Usage on a stream is opt-in per request (`stream_options.include_usage`) and can be lost when the stream breaks [8]. Terminal event choice: completed vs failed vs incomplete vs `error` [9]. Regional host chosen by name, with the WebSocket host derived from it [2][3]. Org/project scoping headers [3]. Retries: `x-should-retry: true/false` overrides; otherwise retry on 408/409/429/≥500; delay from `retry-after-ms` then `retry-after` (s or HTTP date), capped by `MAX_RETRY_AFTER_DELAY` = 120 s; defaults to 2 retries, 0.5–8 s backoff [11][12]. Request id comes in the `x-request-id` response header [11]. Idempotency: the base client generates `stainless-python-retry-<uuid>` keys for non-GET requests, but the client's `_idempotency_header` is `None`, so none is sent on these endpoints. The spec names `Idempotency-Key` only on two `/agents/*` POSTs [11][1]. Whether cached and cache-write tokens are subsets of `prompt_tokens`/`input_tokens` is not stated in the field descriptions read [6][7]. |

[1] openai/openai-openapi/openapi.json@deac2b84f855 (`servers`, `security`, `components.securitySchemes`, `info.version`, `paths`) — read 2026-11-14
[2] openai/openai-python/src/openai/_data_residency.py@3dae6090cc31 — read 2026-11-14
[3] openai/openai-python/src/openai/_client.py@3dae6090cc31 — read 2026-11-14
[4] openai/openai-python/src/openai/lib/_client_auth.py@3dae6090cc31 — read 2026-11-14
[5] openai/openai-openapi/openapi.json@deac2b84f855 (`components.schemas.Model`, `ListModelsResponse`, `paths./models`) — read 2026-11-14
[6] openai/openai-openapi/openapi.json@deac2b84f855 (`components.schemas.CompletionUsage`) — read 2026-11-14
[7] openai/openai-openapi/openapi.json@deac2b84f855 (`components.schemas.ResponseUsage`, `CacheWriteTokensDetails`, `Response.usage`) — read 2026-11-14
[8] openai/openai-openapi/openapi.json@deac2b84f855 (`components.schemas.ChatCompletionStreamOptions`, `CreateChatCompletionStreamResponse.usage`) — read 2026-11-14
[9] openai/openai-openapi/openapi.json@deac2b84f855 (`components.schemas.ResponseStreamEvent`, `ResponseCompletedEvent`, `ResponseFailedEvent`, `ResponseIncompleteEvent`, `ResponseErrorEvent`) — read 2026-11-14
[10] openai/openai-python/src/openai/_streaming.py@3dae6090cc31 — read 2026-11-14
[11] openai/openai-python/src/openai/_base_client.py@3dae6090cc31 — read 2026-11-14
[12] openai/openai-python/src/openai/_constants.py@3dae6090cc31 — read 2026-11-14

### Google Gemini API

| Aspect | Finding [n] |
| --- | --- |
| Formats spoken | Native: Gemini `generateContent` / `streamGenerateContent` (REST, JSON, camelCase on the wire) [1][2][5]. Compat: OpenAI Chat Completions at `/v1beta/openai/`, used through the `openai` client for `chat.completions.create` (incl. `reasoning_effort`), `beta.chat.completions.parse`, `embeddings.create`, `batches.create`, `models.list` [4]. OpenAI Responses or Anthropic Messages surface: none found first-party. Vertex AI is a separate surface (`{location}-aiplatform.googleapis.com`, default `v1beta1`), out of scope here [1]. |
| Base URL(s) | `https://generativelanguage.googleapis.com/` + API version [1]. SDK default version is now `v1` (`GOOGLE_AI_API_DEFAULT_VERSION = 'v1'`; env `GOOGLE_GENAI_API_VERSION=beta` selects `v1beta`) [1][3]; cookbook REST samples and SDK recordings use `v1beta` [5][6][7]. Compat base: `https://generativelanguage.googleapis.com/v1beta/openai/` [4]. No regional hosts for the Gemini API in the SDK (regional/multi-regional hosts exist only on the Vertex branch) [1]. |
| Key header / required headers | SDK sends `x-goog-api-key: <key>` and `Content-Type: application/json` [1]. Cookbook REST samples pass the key as `?key=$GEMINI_API_KEY` instead [5][6]. Compat layer: `Authorization: Bearer <key>` via the OpenAI client's `api_key` [4]. |
| Model list endpoint | `GET /{version}/models` with `pageSize`, `pageToken` query [2]; response `{models: [...], nextPageToken}` [2][6]. Model wire fields: `name` (`models/<id>`), `displayName`, `description`, `version`, `inputTokenLimit`, `outputTokenLimit`, `supportedGenerationMethods`, `temperature`, `maxTemperature`, `topP`, `topK`, `thinking` (bool) [2][6]. SDK renames `supportedGenerationMethods` → `supported_actions` [2]. Compat `models.list` returns OpenAI-shaped objects with `id` [4]. |
| Usage fields | `usageMetadata.{promptTokenCount, candidatesTokenCount, totalTokenCount, cachedContentTokenCount, thoughtsTokenCount, toolUsePromptTokenCount}` plus per-modality arrays `promptTokensDetails`, `candidatesTokensDetails`, `cacheTokensDetails`, `toolUsePromptTokensDetails` (`[{modality, tokenCount}]`), `trafficType`, `initialPromptTokenCount` [3]. `totalTokenCount` = prompt + candidates + toolUsePrompt + thoughts [3]. Response also carries `responseId`, `modelVersion` [3]. |
| Usage in a stream | `usageMetadata` on every SSE chunk. Recorded Google AI stream: intermediate chunks carry only `promptTokenCount`/`promptTokensDetails`/`totalTokenCount` (= prompt); the chunk with `finishReason: STOP` adds `candidatesTokenCount` and the full total [7]. Take the last chunk's `usageMetadata`; do not sum. Compat-layer stream usage (`stream_options.include_usage`): **[unverified: docs site not read]**. |
| Streaming quirks | Stream path is `models/{model}:streamGenerateContent?alt=sse` [2][5]; response `Content-Type: text/event-stream`, lines `data: <GenerateContentResponse JSON>` separated by blank lines, no `event:` names, no `[DONE]` sentinel [1][7]. SDK also brace-balances non-`data:` lines because errors arrive as multi-line JSON [1]. Without `alt=sse` the body is a JSON array: **[unverified: docs site not read]**. Reasoning arrives as ordinary `parts` flagged `thought: true` (only when `thinkingConfig.includeThoughts`), with opaque `thoughtSignature` on parts [3]. |
| Not capturable by a data declaration | Version choice is itself a setting (`v1` default vs `v1beta` used by samples) [1][5]; model id is a path segment with a `:method` suffix (`models/{id}:generateContent`), not a body field [2]; model `name` carries a `models/` prefix to strip [6]; usage is cumulative per chunk (last-wins, not additive) [7]; `supportedGenerationMethods` must be filtered to find chat-capable models [6]; thought parts must be split from answer text by a boolean on each part, and `thoughtSignature` must be echoed back on later turns [3]; key may go in header or query [1][5]. |

[1] googleapis/python-genai/google/genai/_api_client.py@e9261111e376 — read 2026-11-14
[2] googleapis/python-genai/google/genai/models.py@e9261111e376 — read 2026-11-14
[3] googleapis/js-genai/src/types.ts@10adc3d0cba3 — read 2026-11-14 (also src/_api_client.ts@10adc3d0cba3 for `GOOGLE_AI_API_DEFAULT_VERSION = 'v1'`)
[4] google-gemini/cookbook/quickstarts/Get_started_OpenAI_Compatibility.ipynb@6c416d766328 — read 2026-11-14
[5] google-gemini/cookbook/quickstarts/rest/Streaming_REST.ipynb@6c416d766328 — read 2026-11-14
[6] google-gemini/cookbook/quickstarts/rest/Models_REST.ipynb@6c416d766328 — read 2026-11-14 (recorded `models.list` output)
[7] googleapis/js-genai/test/system/recordings/AFC_Streaming_Tests_chat_stream_AFC_enabled_Google_AI_chat_stream.json@10adc3d0cba3 — read 2026-11-14 (recorded wire stream, Aug 2025)

### DeepSeek

Source types: [1]–[6] first-party client code (DeepSeek's own agent harness, TypeScript); [7]–[10] first-party serving library (`deepseek-recipe`, which converts requests and responses for DeepSeek models; it shows the wire shapes DeepSeek emits but does not prove what `api.deepseek.com` serves); [11]–[13] READMEs (model repo, integration guides). DeepSeek's API docs site was not reachable.

| Aspect | Finding [n] |
|---|---|
| Formats spoken | **Anthropic Messages**: the official harness speaks only Messages, with "no `protocol` field" [1][2]. **OpenAI Chat Completions**: the model READMEs call the platform "OpenAI-Compatible API" [11], and the harness's earlier Chat Completions adapter targeted `api.deepseek.com` [6]. `deepseek-recipe` also converts **OpenAI Responses** [7], but nothing here shows that `api.deepseek.com` serves Responses (**[unverified: docs site not read]**). |
| Base URL(s) | Messages: `https://api.deepseek.com/anthropic`. The client appends `/v1/messages`, giving `/anthropic/v1/messages`, unless the base already ends in exactly `/v1` [1][3]. Chat Completions: `https://api.deepseek.com/v1` [12] and `https://api.deepseek.com/beta` [13] appear in integration guides; a bare `https://api.deepseek.com` root is **[unverified: docs site not read]**. No regional hosts found. |
| Key header / required headers | Messages: `x-api-key: <key>`, `anthropic-version: 2023-06-01`, and optional `anthropic-beta` (e.g. `files-api-2025-04-14`, `mid-conversation-tool-changes-2026-07-01`) [4][5]. The env var is `DEEPSEEK_API_KEY` [4]. Request-id response headers read: `request-id` / `x-request-id` / `x-deepseek-request-id` [5]. For Chat Completions, `Authorization: Bearer` is **[unverified: docs site not read]** (no first-party code read). |
| Model list endpoint | `GET https://api.deepseek.com/models` (at the origin, not under `/anthropic`), authenticated with `x-api-key` [3][4]. The body has a `data[]` array. Per entry: `id`, `name`, `context_window`, `max_output_tokens`, `input_modalities`, `output_modalities`, `api_capabilities.anthropic_messages{system_prompt_update, tool_update}`, `client_requirements.<product>.min_version`, `client_metadata.<product>.badge` [3]. An entry without `api_capabilities.anthropic_messages` is unusable over Messages [3]. The official listing declares a context window of 1,048,576 for `deepseek-flash` and `deepseek-v4-pro` [8]. |
| Usage fields | Messages: `usage.input_tokens` (excludes cache hits), `cache_read_input_tokens` (= cache hits), `cache_creation_input_tokens` (always 0 in recipe), `output_tokens` [4][9]. Chat Completions: `usage.prompt_tokens`, `completion_tokens`, `total_tokens`, `prompt_tokens_details.cached_tokens`, `completion_tokens_details.reasoning_tokens` (optional), `prompt_cache_hit_tokens`, `prompt_cache_miss_tokens` (= prompt − hit) [10]. These names are confirmed by the recipe's response schema; that the live API uses the same names is inferred, not read. |
| Usage in a stream | Messages: usage arrives in `message_start.message.usage`, then cumulatively in `message_delta.usage` [4]. Chat Completions (recipe): `stream_options.include_usage` only "include[s] a null usage field before the final chunk"; "the final chunk carries usage regardless of this setting" [7a]. The live API's behaviour is **[unverified: docs site not read]**. |
| Streaming quirks | Messages SSE: the event name must equal the JSON `type`; `error` events arrive in-stream; SSE comment lines are treated as heartbeats that keep the idle timer alive [5a]. Thinking arrives as `thinking_delta` / `signature_delta` [4]. Chat Completions: deltas carry `reasoning_content` [10]. The stream ends with `[DONE]` [10]. In thinking mode the API requires `reasoning_content` to be passed back on assistant turns that made tool calls [6]. |
| Not capturable by a data declaration | (a) The base-URL path rule (append `/v1` unless the base ends in exactly `/v1`) [3]. (b) The model list lives at the **origin**, not the format base, and its capability gate is a nested object [3]. (c) `anthropic-beta` flags are needed per feature (Files, tool changes) [5]. (d) Thinking passback of `reasoning_content` on Chat Completions, and signed `thinking` blocks on Messages [6][4]. (e) Messages `input_tokens` excludes cache reads, so the total is input + cache_read + cache_write + output [4][9]. (f) Defaults depend on thinking mode: output tokens default to 8192 without thinking, 65536 with it, 131072 at `max` effort [7]. |

[1] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek/README.md@39163dd30f70 — read 2026-11-14
[2] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek/src/wire-types.ts@39163dd30f70 — read 2026-11-14
[3] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek/src/listing.ts@39163dd30f70 (lines 22–25, 173–229), with src/config.ts:116, src/messages-api.ts and .agents/notes/archived/bug-fix/2026-09-15-messages-v1-base-url.md@39163dd30f70 — read 2026-11-14
[4] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek-api-key/src/index.ts@39163dd30f70 (lines 58, 99) and packages/llm/llm-deepseek/src/translate.ts@39163dd30f70 (lines 35–42, 105–161) — read 2026-11-14
[5] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek/src/adapter.ts@39163dd30f70 (lines 144–149), src/messages-api.ts, src/transport.ts:40@39163dd30f70 — read 2026-11-14
[5a] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek/src/sse.ts@39163dd30f70 — read 2026-11-14
[6] deepseek-ai/deepseek-harness/.agents/notes/archived/bug-fix/2026-08-19-deepseek-reasoning-passback-every-turn.md@39163dd30f70 — read 2026-11-14
[7] deepseek-ai/deepseek-recipe/README.md@935817c66f5d — read 2026-11-14
[7a] deepseek-ai/deepseek-recipe/deepseek-recipe/src/protocol/openai/chat_completion/request/schema.rs@935817c66f5d (lines 160–165) — read 2026-11-14
[8] deepseek-ai/deepseek-harness/packages/llm/llm-deepseek/src/models.ts@39163dd30f70 — read 2026-11-14
[9] deepseek-ai/deepseek-recipe/deepseek-recipe/src/protocol/anthropic/messages/response/schema.rs@935817c66f5d (lines 101–126) — read 2026-11-14
[10] deepseek-ai/deepseek-recipe/deepseek-recipe/src/protocol/openai/chat_completion/response/schema.rs@935817c66f5d (lines 61, 83, 205–243, 270) — read 2026-11-14
[11] deepseek-ai/DeepSeek-V3/README.md@9b4e9788e4a3 (line 225) — read 2026-11-14
[12] deepseek-ai/awesome-deepseek-integration/docs/oai-compatible-copilot/README.md@172dc8b74a0d — read 2026-11-14
[13] deepseek-ai/awesome-deepseek-integration/docs/continue/README.md@172dc8b74a0d (line 28) — read 2026-11-14

### Mistral

| Aspect | Finding [n] |
| --- | --- |
| Formats spoken | Native Mistral Chat Completions, OpenAI-Chat-shaped (`POST /v1/chat/completions`, `choices[].delta`, `usage`) [1][7]. Also a stateless Responses API (`POST /v1/responses`, `POST /v1/responses/input_tokens`) with OpenAI-Responses event names (`response.output_text.delta`, `response.reasoning_text.delta`, `response.completed`) [2][8]. Also Conversations/Agents API (`/v1/conversations`, `#stream` variants) [3], FIM, embeddings, OCR, audio. No Anthropic Messages or Gemini surface found. The SDK does not label either surface "OpenAI-compatible". Separate Azure and GCP SDK packages exist (`packages/azure`, `packages/gcp`) [9]. |
| Base URL(s) | `SERVERS = {global: https://api.mistral.ai, eu: https://api.eu.mistral.ai, us: https://api.us.mistral.ai}`, default `global`; `server_url` override [4]. Paths carry `/v1` [1]. |
| Key header / required headers | `Authorization: Bearer <key>` (Speakeasy security `scheme_type="http"`, `sub_type="bearer"`, `field_name="Authorization"`) [5]. Stream requests send `Accept: text/event-stream` [1]. No other required header found. |
| Model list endpoint | `GET /v1/models` → `{object: "list", data: [...]}`; items discriminated by `type`: `"base"` → BaseModelCard, `"fine-tuned"` → FTModelCard [6]. BaseModelCard: `id`, `object`, `created`, `owned_by`, `name`, `description`, `max_context_length` (SDK default 32768), `aliases`, `deprecation`, `deprecation_replacement_model`, `default_model_temperature`, `internal`, `billing_model_name`, `capabilities`, `audio_*_capabilities` [6]. `capabilities` bools: `completion_chat`, `function_calling`, `reasoning`, `completion_fim`, `fine_tuning`, `vision`, `ocr`, `classification`, `moderation`, `audio`, `audio_transcription`, `audio_transcription_realtime`, `audio_speech`, `unified_resources` [6]. `GET /v1/models/{model_id}` for one [3]. |
| Usage fields | Chat `usage` (UsageInfo): `prompt_tokens`, `completion_tokens`, `total_tokens`, `prompt_audio_seconds`, `service_tier`; model allows extra fields [7]. Responses API `usage`: `input_tokens`, `input_tokens_details.{cached_tokens, cache_write_tokens}`, `output_tokens`, `output_tokens_details.reasoning_tokens`, `total_tokens` [8]. Conversations `usage`: `prompt_tokens`, `completion_tokens`, `total_tokens`, `connector_tokens`, `connectors` [3]. |
| Usage in a stream | Chat: `usage` is an optional field on every CompletionChunk [7]; the SDK's own test models a stream whose last chunk carries `finish_reason` + `usage`, then `data: [DONE]` [10]. No `stream_options` parameter in the SDK's chat call [1]. Responses API: usage on `response.completed`'s `response` [8]. Whether the server ever puts usage on earlier chunks: **[unverified: docs site not read]**. |
| Streaming quirks | SSE; each event's `data` is a CompletionChunk (`CompletionEvent.data`); stream ends with sentinel `data: [DONE]` [1][7]. Delta `content` is either a string or a list of content chunks [7]; reasoning arrives as a `{type: "thinking", thinking: [...]}` chunk inside that list (with a `closed` flag) [7]. Reasoning controlled by `reasoning_effort` and `prompt_mode` (`reasoning`) [1]. |
| Not capturable by a data declaration | Region is a choice among three hosts [4]; model list is a discriminated union (`base` vs `fine-tuned`) and capability bools must be read to pick chat models [6]; `content` polymorphism (string vs chunk list) and thinking chunks need code to split reasoning from text [7]; three usage shapes depending on which API is spoken [3][7][8]; OpenAPI spec itself is not in the repo, only in Speakeasy's registry (`registry.speakeasyapi.dev/mistral-dev/mistral-dev/mistral-openapi:v2`) [9]. |

[1] mistralai/client-python/src/mistralai/client/chat.py@d3da3bd3c63d — read 2026-11-14
[2] mistralai/client-python/src/mistralai/client/responses.py@d3da3bd3c63d — read 2026-11-14
[3] mistralai/client-python/src/mistralai/client/conversations.py@d3da3bd3c63d, src/mistralai/client/models_.py@d3da3bd3c63d, src/mistralai/client/models/conversationusageinfo.py@d3da3bd3c63d — read 2026-11-14
[4] mistralai/client-python/src/mistralai/client/sdkconfiguration.py@d3da3bd3c63d — read 2026-11-14
[5] mistralai/client-python/src/mistralai/client/models/security.py@d3da3bd3c63d — read 2026-11-14
[6] mistralai/client-python/src/mistralai/client/models/{modellist,basemodelcard,modelcapabilities}.py@d3da3bd3c63d — read 2026-11-14
[7] mistralai/client-python/src/mistralai/client/models/{usageinfo,completionchunk,completionevent,deltamessage,thinkchunk}.py@d3da3bd3c63d — read 2026-11-14
[8] mistralai/client-python/src/mistralai/client/models/{createresponseresponse,responsesinputtokensdetails,responsesoutputtokensdetails,responsestextdeltaevent,responsesreasoningtextdeltaevent,responsescompletedevent}.py@d3da3bd3c63d — read 2026-11-14
[9] mistralai/client-python/.speakeasy/workflow.yaml@d3da3bd3c63d — read 2026-11-14
[10] mistralai/client-python/src/mistralai/extra/tests/test_otel_tracing.py@d3da3bd3c63d — read 2026-11-14

### xAI

| Aspect | Finding [n] |
| --- | --- |
| Formats spoken | gRPC (native for the Python SDK): package `xai_api`, service `Chat` with `GetCompletion` (unary) and `GetCompletionChunk` (server stream), plus deferred/stored/compact rpcs; service `Models` [1][2][3]. REST ("xAI's REST API" OpenAPI 1.0.0): OpenAI Chat Completions `POST /v1/chat/completions`, OpenAI Responses `POST /v1/responses`, Anthropic Messages `POST /v1/messages` ("compatible with the Anthropic API"), plus `/v1/complete`, `/v1/completions` [4]. The TypeScript SDK speaks REST Responses over SSE [5][6]. |
| Base URL(s) | gRPC: `api.x.ai` port 443, TLS (`grpc.ssl_channel_credentials`); management API `management-api.x.ai` [7]. REST: `https://api.x.ai/v1` (TS SDK `DEFAULT_BASE_URL`) [5]; OpenAPI declares no `servers` [4]. No regional hosts found in any first-party repo read. |
| Key header / required headers | gRPC metadata `authorization: Bearer <key>` via call credentials; SDK adds `xai-sdk-version`, `xai-sdk-language` metadata (not required) [7]. REST: `Authorization: Bearer <key>` (`bearerAuth`, http bearer), also declared on `/v1/messages` [4][5]; whether `/v1/messages` accepts Anthropic's `x-api-key`/`anthropic-version`: **[unverified: docs site not read]**. TS SDK sends optional `x-client-request-id` [5]. |
| Model list endpoint | gRPC `Models.ListLanguageModels(Empty)` → `{models: [LanguageModel]}`; LanguageModel: `name`, `aliases`, `version`, `input_modalities`, `output_modalities`, prices (`prompt_text_token_price`, `prompt_image_token_price`, `cached_prompt_token_price`, `completion_text_token_price`, `search_price`), `created`, `max_prompt_length` (context length), `system_fingerprint` [2]. REST `GET /v1/models` → `{object, data: [Model]}`; Model: `id`, `aliases`, `created`, `object`, `owned_by`, `context_length`, `capabilities.{reasoning_effort, default_reasoning_effort}`, `pricing`, per-token prices incl. `_long_context` and `long_context_threshold` [4]. REST `GET /v1/language-models` → `{models: [LanguageModel]}` (prices, modalities, `fingerprint`, `version`; no context length field) [4]. |
| Usage fields | gRPC `SamplingUsage`: `completion_tokens`, `reasoning_tokens`, `prompt_tokens`, `total_tokens`, `prompt_text_tokens` (uncached), `cached_prompt_text_tokens`, `prompt_image_tokens`, `num_sources_used` (deprecated), `server_side_tools_used`, `cost_in_usd_ticks` (1 USD = 1e10 ticks) [1]. REST Chat `Usage`: `prompt_tokens`, `completion_tokens`, `total_tokens`, `prompt_tokens_details.{text_tokens, cached_tokens, image_tokens, audio_tokens}`, `completion_tokens_details.{reasoning_tokens, audio_tokens, accepted_prediction_tokens, rejected_prediction_tokens}`, `num_sources_used`, `cost_in_usd_ticks`, `server_side_tool_usage_details` [4]. REST Responses `ModelUsage`: `input_tokens`, `input_tokens_details`, `output_tokens`, `output_tokens_details`, `total_tokens`, `num_sources_used`, `num_server_side_tools_used`, `cost_in_usd_ticks`, `cost_in_nano_usd`, `context_details` [4][8]. Messages `MessageUsage`: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` [4]. |
| Usage in a stream | gRPC: every `GetChatCompletionChunk` has `usage` = running total; "not the final number of tokens billed unless this is the last chunk" [3]. REST Chat: `stream_options.include_usage` adds one extra chunk before `data: [DONE]`; other chunks have `usage: null` [4]. REST Responses: usage on `response.completed` [4][6]. |
| Streaming quirks | gRPC stream: `outputs[]` of `CompletionOutputChunk` with `Delta.{content, reasoning_content, encrypted_content}`; citations only on last chunk [3]. REST SSE ends with `data: [DONE]`; `event:` lines parsed [6]. Responses event types include `ping`, `error`, `response.created/in_progress/completed/failed/incomplete`, `response.output_text.delta`, `response.reasoning_text.delta`, `response.reasoning_summary_text.delta`, function-call and image-gen events [5]. Chat chunk `Delta` has `reasoning_content` [4]. Encrypted reasoning via `use_encrypted_content` (gRPC) / `reasoning.encrypted_content` include (REST) [3][5]. |
| Not capturable by a data declaration | gRPC needs generated stubs and a TLS channel, not a URL + header [7][9]; gRPC usage is cumulative per chunk while REST Chat usage is a single extra chunk only on request [3][4]; four REST usage shapes depending on format [4]; context length lives in different fields per surface (`max_prompt_length` gRPC, `context_length` on `/v1/models`, absent on `/v1/language-models`) [2][4]; cost is reported in ticks (1e-10 USD) or nano-USD and needs conversion [1][8]; REST stores responses by default (TS SDK overrides `store` to false) [5]. |

[1] xai-org/xai-proto/proto/xai/api/v1/usage.proto@7c1372fe5dcf — read 2026-11-14
[2] xai-org/xai-proto/proto/xai/api/v1/models.proto@7c1372fe5dcf — read 2026-11-14
[3] xai-org/xai-proto/proto/xai/api/v1/chat.proto@7c1372fe5dcf — read 2026-11-14
[4] xai-org/xai-sdk-ts/spec/openapi.json@ef80b0189ec4 — read 2026-11-14
[5] xai-org/xai-sdk-ts/src/constants.ts@ef80b0189ec4 and src/http.ts@ef80b0189ec4 — read 2026-11-14
[6] xai-org/xai-sdk-ts/src/sse.ts@ef80b0189ec4 and src/stream.ts@ef80b0189ec4 — read 2026-11-14
[7] xai-org/xai-sdk-python/src/xai_sdk/client.py@884ba42c7440 — read 2026-11-14
[8] xai-org/xai-sdk-ts/src/usage.ts@ef80b0189ec4 — read 2026-11-14
[9] xai-org/xai-proto/README.md@7c1372fe5dcf — read 2026-11-14

### Qwen / Alibaba (DashScope / Model Studio)

Source types: [1]–[7] first-party SDK code (`dashscope-sdk-python` 2.x, including its `legacy_1x` tree); [8]–[10] first-party client code (Qwen Code CLI); [11] a Qwen Code design doc.

| Aspect | Finding [n] |
|---|---|
| Formats spoken | Three protocols on one gateway, each under its own path [1]. **Native DashScope**: `/api/v1` + `/services/aigc/text-generation/generation` (text) or `/services/aigc/multimodal-generation/generation` (multimodal); the body is `{model, input:{messages}, parameters:{result_format:"message", ...}}` [2]. **OpenAI Chat Completions**: `/compatible-mode/v1` + `/chat/completions` [1][3]. **Anthropic Messages**: `/apps/anthropic` + `/v1/messages` [1][4]. OpenAI Responses: not among the SDK's `VALID_PROTOCOLS` (`native`, `openai`, `anthropic`) [1], so **[unverified: docs site not read]**. |
| Base URL(s) | Recommended: a workspace-scoped gateway `https://{workspace}.{region}.maas.aliyuncs.com`. Regions are `cn-beijing` (default), `ap-southeast-1`, `cn-hongkong`, `us-east-1`, `eu-central-1`, `ap-northeast-1` [1]. Legacy shared hosts (still supported, same paths): `dashscope.aliyuncs.com` (Beijing), `dashscope-intl.aliyuncs.com` (Singapore; the SDK also maps US, EU and Tokyo here), `cn-hongkong.dashscope.aliyuncs.com` [1]. Qwen Code's presets list `https://dashscope-us.aliyuncs.com/compatible-mode/v1` for US (Virginia) [8][9], which conflicts with the SDK. WebSocket: `wss://` same host, `/api-ws/v1/inference` and `/api-ws/v1/realtime` [1]. QwenCloud / 千问AI MaaS hosts: `maas.qwencloudapi.com` and `maas.qianwenaiapi.com`, with a `token-plan.` prefix variant [10]. |
| Key header / required headers | `Authorization: Bearer <key>` on every protocol; the SDK's transport sets no `x-api-key`, even for Anthropic [5][4]. Optional `X-DashScope-WorkSpace: <workspace>` [5]. Native streaming requires `X-DashScope-SSE: enable` [2]. Anthropic sends `anthropic-version: 2023-06-01` [4]. Env vars: `DASHSCOPE_API_KEY`, `DASHSCOPE_BASE_URL`, `DASHSCOPE_REGION`, `DASHSCOPE_WORKSPACE` [1]. Optional headers Qwen Code sends: `X-DashScope-CacheControl: enable`, `X-DashScope-UserAgent`, `X-DashScope-AuthType`, `X-DashScope-OssResourceResolve: enable` (for `oss://` media), and `X-DashScope-Source-Config` (usage attribution on QwenCloud hosts) [9][10]. |
| Model list endpoint | Native: `GET {base}/api/v1/models?page_no=&page_size=` returns `output.{models[], page_no, page_size, total}`. Each entry has `model`, `source`, `provider`, `opensource` and **no context length or capabilities** [6]. Compat-mode `GET /compatible-mode/v1/models` is used against the Qwen subscription gateway (`https://api.qwen.ai/compatible-mode/v1`), whose `data[]` may carry `input_modalities` and `context_window_tokens` [11]. Whether DashScope's own compat `/models` returns those fields is **[unverified: docs site not read]**. |
| Usage fields | Native: top-level `usage.input_tokens`, `usage.output_tokens`, `usage.total_tokens`; extra counters such as `image_count` pass through [7]. Native cache and reasoning detail field names: **[unverified: docs site not read]**. Compat: `usage.prompt_tokens`, `completion_tokens`, `prompt_tokens_details.cached_tokens`, `completion_tokens_details.reasoning_tokens` [3]. Anthropic: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` [4]. |
| Usage in a stream | Compat: the SDK always sends `stream_options: {include_usage: true}` because "OpenAI-compatible servers omit usage from streams unless asked" [3]. Native: each chunk's top-level `usage` is parsed per chunk [7]; whether it is cumulative or final-only is **[unverified: docs site not read]**. Anthropic: usage accumulates across events, merged by `dict.update` [4]. |
| Streaming quirks | Native SSE framing uses `id:`, `event:`, `status:` and `data:` lines. `event:error` marks an error and carries the HTTP code in `status:`. Data under `event:done` is skipped. Any other line is "heartbeat" and ignored [12]. `incremental_output: true` gives deltas, `false` gives cumulative snapshots. Only `tts`, `omni` and `qwen-deep-research` models honour `false` on the server. Every other model streams incrementally only, so the SDK keeps the wire incremental and merges on the client [2][13]. Compatible protocols are incremental-only [13]. Compat: assistant `reasoning_content` must be echoed back on later turns for reasoning models [3]. Anthropic: thinking surfaces as `reasoning_content` plus a signature [4]. `[DONE]` on compat: **[unverified: docs site not read]**. |
| Not capturable by a data declaration | (a) Workspace and region make the base URL a template, not a constant [1]. (b) The native path differs by modality (text vs multimodal), and native needs an SSE opt-in header plus a body param (`incremental_output`) [2]. (c) Whether output is incremental or cumulative depends on the model name [13]. (d) The native request and response envelope is not OpenAI (`input`/`parameters`/`output`) [2][7]. (e) The native model list is paged and carries no context window or capabilities [6]. (f) Thinking control is a vendor body field (`enable_thinking`, `thinking_budget`), not standard [2][9]. (g) Explicit prompt caching uses a part-level `cache_control` marker and is ignored on tools [2][9]. (h) The US host is inconsistent between the SDK and Qwen Code [1][8]. |

[1] dashscope/dashscope-sdk-python/dashscope/_config.py@2cb05b1ec87b (lines 1–131) — read 2026-11-14
[2] dashscope/dashscope-sdk-python/dashscope/protocols/native.py@2cb05b1ec87b (lines 97–170) — read 2026-11-14
[3] dashscope/dashscope-sdk-python/dashscope/protocols/openai.py@2cb05b1ec87b (lines 126–150, 180, 214–218) — read 2026-11-14
[4] dashscope/dashscope-sdk-python/dashscope/protocols/anthropic.py@2cb05b1ec87b (lines 1–27, 377–420, 644–645, 767) — read 2026-11-14
[5] dashscope/dashscope-sdk-python/dashscope/transport/http.py@2cb05b1ec87b (lines 38, 76–83) — read 2026-11-14
[6] dashscope/dashscope-sdk-python/dashscope/resources/models.py@2cb05b1ec87b (lines 39–125) — read 2026-11-14
[7] dashscope/dashscope-sdk-python/dashscope/types/common.py@2cb05b1ec87b (lines 101–116) and dashscope/types/completions.py@2cb05b1ec87b (lines 197–245) — read 2026-11-14
[8] QwenLM/qwen-code/packages/core/src/providers/presets/alibaba-standard.ts@6f754be65489 (lines 17–42) — read 2026-11-14
[9] QwenLM/qwen-code/packages/core/src/core/openaiContentGenerator/provider/dashscope.ts@6f754be65489 (lines 188–192, 320–336, 846, 1003–1007) — read 2026-11-14
[10] QwenLM/qwen-code/packages/core/src/core/dashscope-source-config.ts@6f754be65489 — read 2026-11-14
[11] QwenLM/qwen-code/docs/design/qwen-subscription.md@6f754be65489 (lines 36, 46, 83) — read 2026-11-14
[12] dashscope/dashscope-sdk-python/dashscope/legacy_1x/common/utils.py@2cb05b1ec87b (lines 252–283) — read 2026-11-14
[13] dashscope/dashscope-sdk-python/dashscope/resources/chat/_params.py@2cb05b1ec87b and dashscope/resources/chat/completions.py@2cb05b1ec87b (lines 172–197) — read 2026-11-14

## Clouds and routers

*How this was read.* This session had no web fetch. The vendors' HTML doc sites (docs.aws.amazon.com, cloud.google.com, learn.microsoft.com, openrouter.ai) could not be reached. Every reading below is from a machine-readable primary source on GitHub, read with `gh api` on 2026-11-14:

- the vendor's own API model: botocore's service JSON is generated by AWS, googleapis protos by Google, azure-rest-api-specs by Microsoft, and OpenRouter's docs repo and OpenAPI file by OpenRouter;
- the vendor's own SDK or samples;
- for the Claude endpoints on clouds, Anthropic's own SDK.

A claim backed only by LiteLLM's source, about a cloud other than LiteLLM, is marked **[unverified: docs site not read]**.

### AWS Bedrock

| Aspect | Finding [n] |
|---|---|
| Formats spoken | Vendor-native on `bedrock-runtime`: `Converse`, `ConverseStream`, `InvokeModel`, `InvokeModelWithResponseStream`, `InvokeModelWithBidirectionalStream`, `CountTokens` [1]. InvokeModel takes a model-native body. For Claude that is the Anthropic Messages body with `"anthropic_version":"bedrock-2023-05-31"`; this is seen in LiteLLM's source, and AWS's own page was not read: **[unverified: docs site not read]** [5]. OpenAI Responses at `https://bedrock-runtime.{region}.{dns_suffix}/openai/v1/responses`, for OpenAI models: **[unverified: docs site not read]** (LiteLLM source only) [6]. "Bedrock Mantle" at `https://bedrock-mantle.{region}.api.aws/v1` serves OpenAI Chat Completions and Anthropic Messages: **[unverified: docs site not read]** (LiteLLM source only; it cites `docs.aws.amazon.com/bedrock/latest/userguide/bedrock-mantle.html`, not read) [7][8]. |
| Base URL | `https://bedrock-runtime.{Region}.{dnsSuffix}`. Variants: FIPS `bedrock-runtime-fips.…` and dual-stack [2]. Control plane: endpointPrefix `bedrock`, i.e. `bedrock.{Region}.…` [3]. |
| Model id goes in | URL path, for example `POST /model/{modelId}/converse`, `/converse-stream`, `/invoke`, `/invoke-with-response-stream` [1]. `modelId` takes a base model ID, an inference-profile ID, or an ARN (inference profile, provisioned throughput, prompt) [1]. |
| Auth / key header | Service auth is `aws.auth#sigv4` or `smithy.api#httpBearerAuth`, with signingName `bedrock` [1]. botocore takes a bearer token from env `AWS_BEARER_TOKEN_{signing name}`, which for Bedrock is `AWS_BEARER_TOKEN_BEDROCK`, and prefers bearer when one is present [4]. The header form is `Authorization: Bearer …` per the standard; AWS's page was not read: **[unverified: docs site not read]**. |
| Model list | Control plane `GET /foundation-models` (ListFoundationModels), `GET /foundation-models/{modelIdentifier}`, and `GET /inference-profiles` (ListInferenceProfiles) [3]. No pricing in these responses [3]. |
| Usage fields | Converse response `usage` and the ConverseStream `metadata` event `usage` (TokenUsage): `inputTokens`, `outputTokens`, `totalTokens`, `cacheReadInputTokens`, `cacheWriteInputTokens`, `cacheDetails` [1]. Response headers `X-Amzn-Bedrock-Input-Token-Count` (and Output) are **not** in the InvokeModel response model [1]. LiteLLM reads `x-amzn-bedrock-input-token-count` from InvokeModel responses: **[unverified: docs site not read]** from AWS [9]. For Claude over InvokeModel, the body carries Anthropic `usage` (**[unverified: docs site not read]**, not read). |
| Streaming quirks | The stream outputs are modelled as `eventstream` unions, not SSE [1]. ConverseStream events: `messageStart`, `contentBlockStart`, `contentBlockDelta`, `contentBlockStop`, `messageStop`, `metadata`, plus exception events (`internalServerException`, `modelStreamErrorException`, `validationException`, `throttlingException`, `serviceUnavailableException`) [1]. InvokeModelWithResponseStream yields `chunk.bytes` (base64 model-native JSON, here Anthropic SSE event objects) plus exceptions including `modelTimeoutException` [1]. The content type `application/vnd.amazon.eventstream` is named in LiteLLM's source; AWS's page was not read: **[unverified: docs site not read]** [10]. Errors can arrive inside a 200 stream as exception events [1]. |
| Anything a data declaration could NOT capture | SigV4 request signing over the final body (service `bedrock`). Credential-chain resolution: profiles, SSO, STS AssumeRole, session-token refresh. The binary AWS event-stream framing: prelude, headers, CRCs, then base64 `chunk.bytes` holding a second, model-native format. Region-dependent hosts, with FIPS and dual-stack variants. Inference-profile and ARN model ids that need URL-encoding in the path. Bearer is a simpler path, but the token is still minted in AWS (**[unverified: docs site not read]** how). |

[1] https://github.com/boto/botocore/blob/develop/botocore/data/bedrock-runtime/2023-09-30/service-2.json (last changed 2026-07-31) — read 2026-11-14
[2] https://github.com/boto/botocore/blob/develop/botocore/data/bedrock-runtime/2023-09-30/endpoint-rule-set-1.json — read 2026-11-14
[3] https://github.com/boto/botocore/blob/develop/botocore/data/bedrock/2023-04-20/service-2.json — read 2026-11-14
[4] https://github.com/boto/botocore/blob/develop/botocore/utils.py (`AWS_BEARER_TOKEN_{bearer_name}`) and https://github.com/boto/botocore/blob/develop/botocore/handlers.py (`_should_prefer_bearer_auth`) — read 2026-11-14
[5] https://github.com/BerriAI/litellm/blob/main/litellm/llms/bedrock/count_tokens/transformation.py — read 2026-11-14
[6] https://github.com/BerriAI/litellm/blob/main/litellm/llms/bedrock/responses/transformation.py — read 2026-11-14
[7] https://github.com/BerriAI/litellm/blob/main/litellm/llms/bedrock_mantle/chat/transformation.py — read 2026-11-14
[8] https://github.com/BerriAI/litellm/blob/main/litellm/llms/bedrock_mantle/messages/transformation.py — read 2026-11-14
[9] https://github.com/BerriAI/litellm/blob/main/litellm/llms/bedrock/chat/invoke_transformations/base_invoke_transformation.py — read 2026-11-14
[10] https://github.com/BerriAI/litellm/blob/main/litellm/llms/bedrock/passthrough/guardrail_translation/handler.py — read 2026-11-14

### Google Vertex AI

| Aspect | Finding [n] |
|---|---|
| Formats spoken | Gemini `generateContent` / `streamGenerateContent` [1]. `rawPredict` / `streamRawPredict`, which carry the partner's native body: for Claude, the Anthropic Messages body with `"anthropic_version":"vertex-2023-10-16"` [1][2][3]. OpenAI Chat Completions at `…/endpoints/openapi/chat/completions` [4]; the proto names `ChatCompletions` on v1beta1 `{endpoint=projects/*/locations/*/endpoints/*}/chat/completions` [5]. OpenAI Responses: none found [1][5] (**[unverified: docs site not read]** absence). |
| Base URL | Regional `https://{location}-aiplatform.googleapis.com`, or `https://aiplatform.googleapis.com` for `global` [6][2]. Anthropic's SDK also uses `aiplatform.us.rep.googleapis.com` / `aiplatform.eu.rep.googleapis.com` for multi-region [2]. Paths: `/v1/projects/{p}/locations/{l}/publishers/{publisher}/models/{model}:{method}` [1]. Express mode (API key) adds project-less bindings `/v1/publishers/*/models/*:generateContent` [1][6]. OpenAI-compatible base: `https://aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/endpoints/openapi` [4]. |
| Model id goes in | URL path (`publishers/{publisher}/models/{model}:method`) for Gemini and rawPredict [1]. Body `model` (e.g. `google/gemini-…`) on the OpenAI-compatible endpoint [4]. |
| Auth / key header | OAuth2 access token from ADC (scope `https://www.googleapis.com/auth/cloud-platform`) as `Authorization: Bearer …` [1][4][6]. The SDKs refresh the token on expiry (default 3600 s) [6][2]. Express mode: API key in `x-goog-api-key`, global host [6]. Whether express mode covers Claude/rawPredict: **[unverified: docs site not read]**. |
| Model list | `ListPublisherModels`: `GET /v1beta1/{parent=publishers/*}/models` (e.g. `publishers/anthropic/models`) [7]. Pricing and context fields: **[unverified: docs site not read]** (not read). |
| Usage fields | Gemini `usageMetadata` (proto `usage_metadata`): `promptTokenCount`, `candidatesTokenCount`, `thoughtsTokenCount`, `totalTokenCount`, `cachedContentTokenCount`, `promptTokensDetails`, `cacheTokensDetails`, `candidatesTokensDetails`, `toolUsePromptTokensDetails`, `trafficType` [1]. Claude via rawPredict: Anthropic `usage` in the body (**[unverified: docs site not read]**, not read). OpenAI endpoint: OpenAI `usage` (**[unverified: docs site not read]**). |
| Streaming quirks | `streamGenerateContent` without `?alt=sse` returns a JSON array stream. The Google GenAI SDK always appends `?alt=sse` to get SSE [8]. `streamRawPredict` relays the partner's stream, which for Claude is Anthropic SSE (**[unverified: docs site not read]**, not read). |
| Anything a data declaration could NOT capture | Minting the OAuth token from ADC (service account JWT, gcloud user, workload identity) and refreshing it. Project and location in every URL. A different host for `global`, regional and multi-region. Express mode switches both host and header. `anthropic_version` sits in the body and `model` moves to the path for Claude. |

[1] https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1/prediction_service.proto (last changed 2026-03-17) — read 2026-11-14
[2] https://github.com/anthropics/anthropic-sdk-python/blob/main/src/anthropic/lib/vertex/_client.py — read 2026-11-14
[3] https://github.com/anthropics/anthropic-sdk-python/blob/main/tests/lib/test_vertex.py — read 2026-11-14
[4] https://github.com/GoogleCloudPlatform/python-docs-samples/blob/main/generative_ai/chat_completions/chat_completions_non_streaming_text.py — read 2026-11-14
[5] https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1beta1/prediction_service.proto — read 2026-11-14
[6] https://github.com/googleapis/python-genai/blob/main/google/genai/_api_client.py — read 2026-11-14
[7] https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1beta1/model_garden_service.proto — read 2026-11-14
[8] https://github.com/googleapis/python-genai/blob/main/google/genai/models.py — read 2026-11-14

### Azure (Azure OpenAI / AI Foundry, incl. Foundry Claude)

| Aspect | Finding [n] |
|---|---|
| Formats spoken | **v1 API:** OpenAI Chat Completions (`/chat/completions`), OpenAI Responses (`/responses`, plus retrieve/cancel/input_items), `/models` [1]. **Dated API:** OpenAI Chat Completions per deployment [2]. **Foundry Claude:** Anthropic Messages under `https://{resource}.services.ai.azure.com/anthropic/` (i.e. `/anthropic/v1/messages`) [3]. |
| Base URL | v1: `{endpoint}/openai/v1`; `api-version` is optional and defaults to `v1` [1]. Dated: `https://{resource}.openai.azure.com/openai/deployments/{deployment-id}/chat/completions?api-version=YYYY-MM-DD[-preview]` (latest spec in repo: `2025-04-01-preview`) [2]. Claude: `https://{resource}.services.ai.azure.com/anthropic/` [3]. |
| Model id goes in | v1: body `model`, required [1]. That it must be the deployment name: **[unverified: docs site not read]** (MS Learn not read). Dated: URL path `deployments/{deployment-id}` [2]. Claude: body `model` (Anthropic format) [3]. Whether that is the deployment name: **[unverified: docs site not read]**. |
| Auth / key header | v1 spec security schemes: header `api-key`; or header `authorization`; or OAuth2 Entra ID with scope `https://cognitiveservices.azure.com/.default` [1]. Foundry Claude: `x-api-key` (the SDK also sends `api-key`) or `Authorization: Bearer {Entra token}` [3]. |
| Model list | v1 `GET /models`, `GET /models/{model}` [1]. Deployments are listed on ARM: `GET https://management.azure.com/subscriptions/{subscriptionId}/resourceGroups/{rg}/providers/Microsoft.CognitiveServices/accounts/{accountName}/deployments` (api-version e.g. `2026-11-01`), which needs an ARM token [4]. A model list on the Claude endpoint: **[unverified: docs site not read]**. |
| Usage fields | OpenAI `usage` (`CompletionUsage`) on chat completions [1]. Azure additions: top-level `prompt_filter_results`, per-choice `content_filter_results`, and for Responses `content_filter_results` / `content_filter_offsets` [1]. Claude: Anthropic `usage` (**[unverified: docs site not read]**). |
| Streaming quirks | SSE per the OpenAI spec [1]. The first chunk carrying `prompt_filter_results` with empty `choices`, and content-filter-only chunks: **[unverified: docs site not read]** (MS Learn content-filter page not read; the spec only shows the fields exist). |
| Anything a data declaration could NOT capture | A URL per resource and per deployment on the dated API (the deployment name sits in the path). An `api-version` query that changes the schema. Minting and refreshing Entra ID tokens (two audiences: `cognitiveservices.azure.com` for data, ARM for listing deployments). Model listing that needs subscription, resource group and account names. Two hosts per resource (`*.openai.azure.com`, `*.services.ai.azure.com`). |

[1] https://github.com/Azure/azure-rest-api-specs/blob/main/specification/ai/data-plane/OpenAI.v1/azure-v1-v1-generated.json (last changed 2026-06-01) — read 2026-11-14
[2] https://github.com/Azure/azure-rest-api-specs/blob/main/specification/cognitiveservices/data-plane/OpenAIInference/preview/2025-04-01-preview/inference.json — read 2026-11-14
[3] https://github.com/anthropics/anthropic-sdk-python/blob/main/src/anthropic/lib/foundry.py — read 2026-11-14
[4] https://github.com/Azure/azure-rest-api-specs/blob/main/specification/cognitiveservices/resource-manager/Microsoft.CognitiveServices/CognitiveServices/stable/2026-11-01/cognitiveservices.json — read 2026-11-14

### OpenRouter

| Aspect | Finding [n] |
|---|---|
| Formats spoken | OpenAI Chat Completions `/chat/completions`, OpenAI Responses `/responses`, Anthropic Messages `/messages` ("Creates a message using the Anthropic Messages API format") [1][2]. |
| Base URL | `https://openrouter.ai/api/v1` [1]. |
| Model id goes in | Body `model` (e.g. `openai/gpt-4o`) [2]. |
| Auth / key header | `Authorization: Bearer <key>`. Anthropic-SDK clients may instead send an API key (not an access token) in `x-api-key` [3]. Optional attribution headers: `HTTP-Referer` and `X-OpenRouter-Title` [3]. Legacy `X-Title`: not in the current docs: **[unverified: docs site not read]**. |
| Model list | `GET /api/v1/models` (also `/models/{author}/{slug}/endpoints`, `/models/count`, `/models/user`) [1][4]. Model fields: `id`, `canonical_slug`, `name`, `context_length`, `pricing`, `top_provider`, `architecture`, `supported_parameters`, `per_request_limits`, `default_parameters`, `knowledge_cutoff`, `expiration_date` and others [1]. `pricing` fields (strings): `prompt`, `completion`, `request`, `image`, `input_cache_read`, `input_cache_write`, `internal_reasoning`, `web_search`, `discount` and others [1]. Sentinels: `"-1"` for dynamic routers, `"0"` for free [4]. |
| Usage fields | Always returned; `usage: {include: true}` and `stream_options.include_usage` are deprecated and have no effect [5]. Responses: `input_tokens`, `input_tokens_details.{cached_tokens, cache_write_tokens}`, `output_tokens`, `output_tokens_details.reasoning_tokens`, `total_tokens`, `cost`, `is_byok`, `cost_details.{upstream_inference_cost, upstream_inference_input_cost, upstream_inference_output_cost}` [5]. Chat Completions: the `prompt_tokens…` names, plus `cost` and `cost_details` [5]. Response header `X-Generation-Id` [6]. |
| Streaming quirks | SSE. Keep-alive comment lines `: OPENROUTER PROCESSING` [6]. Errors before commit come as a JSON body with an HTTP status. After commit the status stays 200, and the error arrives as a Chat chunk with top-level `error` and `finish_reason:"error"`, or a Responses `response.failed` event (possibly the only event) [6]. The Chat final usage chunk has one choice repeating `finish_reason` (not empty `choices`, unlike OpenAI's spec) [6]. Responses usage comes in `response.completed`; Messages usage in `message_delta` [6]. |
| Anything a data declaration could NOT capture | Little: static host and bearer key. Hard to declare: the mid-stream error shapes inside a 200; SSE comment lines; and the duplicated `finish_reason` on the usage chunk. |

[1] https://github.com/OpenRouterTeam/docs/blob/main/openapi/openapi.yaml (repo HEAD 2026-11-13) — read 2026-11-14
[2] https://github.com/OpenRouterTeam/docs/blob/main/api_reference/streaming.mdx (examples, `model`) — read 2026-11-14
[3] https://github.com/OpenRouterTeam/docs/blob/main/api_reference/authentication.mdx — read 2026-11-14
[4] https://github.com/OpenRouterTeam/docs/blob/main/api_reference/models.mdx — read 2026-11-14
[5] https://github.com/OpenRouterTeam/docs/blob/main/cookbook/administration/usage-accounting.mdx — read 2026-11-14
[6] https://github.com/OpenRouterTeam/docs/blob/main/api_reference/streaming.mdx — read 2026-11-14

### LiteLLM proxy

| Aspect | Finding [n] |
|---|---|
| Formats spoken | OpenAI Chat Completions `/chat/completions`, `/v1/chat/completions`, `/openai/deployments/{model}/chat/completions` (Azure-style). OpenAI Responses `/responses`, `/v1/responses`. Anthropic Messages `/v1/messages`. Gemini `/v1beta/models/{model}:generateContent`, `:streamGenerateContent`, `:countTokens` [1]. Provider pass-through prefixes include `/bedrock`, `/vertex_ai` (`/vertex-ai`), `/anthropic`, `/azure`, `/azure_ai`, `/openai`, `/gemini`, `/cohere`, `/vllm` [1]. |
| Base URL | Operator's own host, e.g. `http://{proxy}:4000` (port **[unverified: docs site not read]**). |
| Model id goes in | Body `model` = the operator's model alias (model group). Path on the Gemini and Azure-style routes [1]. |
| Auth / key header | Virtual key accepted in any of: `Authorization: Bearer`, `API-Key` (Azure), `x-api-key` (Anthropic), `x-goog-api-key` (Gemini), `Ocp-Apim-Subscription-Key`, `x-litellm-api-key` (overrides Authorization), or an operator-set `litellm_key_header_name` [2][3]. |
| Model list | `GET /models`, `/v1/models` [1][4]. `GET /model/info`, `/v1/model/info`, `/v2/model/info`, `/model_group/info` with `model_info.{input_cost_per_token, output_cost_per_token, max_tokens, max_input_tokens…}` [4]. |
| Usage fields | Body: the upstream format's `usage`. Response headers: `x-litellm-response-cost`, `-cost-original`, `-cost-input`, `-cost-output`, `-cost-cache-read`, `-cost-cache-creation`, `-cost-reasoning`, `-cost-tool-usage`, `-cost-discount-amount`, `-cost-margin-amount`, `-cost-margin-percent`. Also `x-litellm-model-id`, `x-litellm-model-group`, `x-litellm-call-id`, `x-litellm-key-spend`, `x-litellm-response-duration-ms` [5]. Whether cost headers are set on streamed responses: **[unverified: docs site not read]**. |
| Streaming quirks | SSE in the route's format. Errors during a stream are sent as `data: {"error": {...}}` events in OpenAI error shape [5]. |
| Anything a data declaration could NOT capture | Per-deployment host and aliases (the model list is the operator's config). Header precedence among the key headers. Pass-through routes inherit the upstream's signing and framing (e.g. Bedrock event-stream through `/bedrock`). |

[1] https://github.com/BerriAI/litellm/blob/main/litellm/proxy/_types.py (main at ac3277dce4, 2026-11-13) — read 2026-11-14
[2] https://github.com/BerriAI/litellm/blob/main/litellm/proxy/auth/user_api_key_auth.py — read 2026-11-14
[3] https://github.com/BerriAI/litellm/blob/main/litellm/types/proxy/auth/special_headers.py — read 2026-11-14
[4] https://github.com/BerriAI/litellm/blob/main/litellm/proxy/proxy_server.py — read 2026-11-14
[5] https://github.com/BerriAI/litellm/blob/main/litellm/proxy/common_request_processing.py — read 2026-11-14

## Local model servers

### LM Studio

The LM Studio server is closed source. Evidence is the docs source repo `lmstudio-ai/docs` (what lmstudio.ai/docs renders), pinned at `8c7159be3ffe`, and `lmstudio-ai/lmstudio-js` at `2025945b3600`.

| Aspect | Finding [n] |
|---|---|
| Formats spoken | OpenAI-compatible: `GET /v1/models`, `POST /v1/responses`, `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`, `/v1/decisions` [1]. Anthropic-compatible `POST /v1/messages` (since 0.4.1); the SSE events are `message_start`, `content_block_*`, `message_delta`, `message_stop` [2][3][4]. Native: REST v1 `/api/v1/chat`, `/api/v1/models`, `/api/v1/models/load`, `/unload`, `/download`, `/download/status` (since 0.4.0) [5]. Legacy REST v0 `/api/v0/models`, `/api/v0/models/{model}`, `/api/v0/chat/completions`, `/api/v0/completions`, `/api/v0/embeddings` [6]. The SDKs use a WebSocket RPC (`ws://…`) [12]. What `/v1/messages` does not support (`count_tokens`, caching, etc.) is not documented: **[unverified: docs site not read]**. |
| Base URL | Every doc example uses `http://localhost:1234/v1`. The docs say "examples assume the server port is 1234" [1]. lmstudio-js calls 1234 "the default port" [12]. `lms server start --port N` falls back to "the last used port" [7]. Network: the "Serve on Local Network" toggle, or `lms server start --bind 0.0.0.0` (default `127.0.0.1`; env `LMS_SERVER_HOST`) [7][8]. |
| Key header | No auth by default. "Require Authentication" (0.4.0+) needs an API token made in the app, with per-token permissions [9][10]. The header is `Authorization: Bearer <token>`. `/v1/messages` also accepts `x-api-key` [2]. That `x-api-key` also works on the OpenAI or REST paths: **[unverified: docs site not read]**. |
| Model list | `/v1/models` lists "models visible to the server"; with JIT on, this "may include all downloaded models" [11]. `/api/v0/models` gives `state` (`loaded`/`not-loaded`), `max_context_length`, `type`, `quantization`, and `capabilities` (since 0.3.16) [6][4]. `/api/v1/models` gives `loaded_instances[].config.context_length`, `max_context_length`, `capabilities.{vision,trained_for_tool_use,reasoning}` [13]. JIT loading is on by default: the first request loads the model. The `ttl` request field (default 60 min for JIT loads) and Auto-Evict (on, at most one JIT model loaded) apply [14]. |
| Usage fields | OpenAI-compat: `usage.{prompt_tokens,completion_tokens,total_tokens}` [6]. `stream_options.include_usage` has been honoured since 0.3.18 [4]. v0 adds `stats.{tokens_per_second,time_to_first_token,generation_time,stop_reason}` and `model_info.context_length` [6]. v1 `/api/v1/chat` has `stats.{input_tokens,total_output_tokens,reasoning_output_tokens,tokens_per_second,time_to_first_token_seconds}` [15]. No cached-token field is documented in any format: **[unverified: docs site not read]**. Anthropic `usage` field names are not documented: **[unverified: docs site not read]**. |
| Streaming quirks | SSE on every surface [2][3][16]. `[DONE]` on chat completions is assumed from OpenAI parity: **[unverified: docs site not read]**. Reasoning appears as `choices.message.reasoning` / `choices.delta.reasoning` (gpt-oss, 0.3.23). There is an older opt-in `reasoning_content` (app setting, 0.3.9) [4]. Tool-call arguments stream as generated (0.3.17) [4]. v1 chat uses named events such as `chat.start`, `reasoning.delta` and `chat.end` [16]. `model` must be an LM Studio model key or identifier; the docs give no fallback for an unknown name [1][14]. |
| Anything a data declaration could NOT capture | Discovery: lmstudio-js probes `GET http://127.0.0.1:<port>/lmstudio-greeting` and expects JSON `{"lmstudio": true}`. It races the internal ports `41343, 52993, 16141, 39414, 22931`, not 1234 [12][17]. The port is user-chosen and "last used" persists [7]. Whether a model is loaded is runtime state: JIT, TTL and Auto-Evict settings decide it [14]. Context length is fixed at load time (`/api/v1/models/load` `context_length`, `lms load --context-length`). Only `/api/v1/chat` accepts `context_length` per request. OpenAI and Anthropic endpoints cannot set it [5][18]. Auth on/off and token permissions live in app settings [9]. |

[1] lmstudio-ai/docs:1_developer/3_openai-compat/index.mdx@8c7159be3ffe364af10aa8ae6f5a7570e9402089 — read 2026-11-14
[2] lmstudio-ai/docs:1_developer/4_anthropic-compat/index.mdx@8c7159be3ffe — read 2026-11-14
[3] lmstudio-ai/docs:1_developer/4_anthropic-compat/messages.md@8c7159be3ffe — read 2026-11-14
[4] lmstudio-ai/docs:1_developer/api-changelog.md@8c7159be3ffe — read 2026-11-14
[5] lmstudio-ai/docs:1_developer/2_rest/index.mdx@8c7159be3ffe — read 2026-11-14
[6] lmstudio-ai/docs:1_developer/2_rest/endpoints.mdx@8c7159be3ffe (REST API v0) — read 2026-11-14
[7] lmstudio-ai/docs:3_cli/1_serve/server-start.mdx@8c7159be3ffe — read 2026-11-14
[8] lmstudio-ai/docs:1_developer/0_core/0_server/serve-on-network.mdx@8c7159be3ffe — read 2026-11-14
[9] lmstudio-ai/docs:1_developer/0_core/authentication.mdx@8c7159be3ffe — read 2026-11-14
[10] lmstudio-ai/docs:1_developer/0_core/0_server/settings.md@8c7159be3ffe — read 2026-11-14
[11] lmstudio-ai/docs:1_developer/3_openai-compat/models.md@8c7159be3ffe — read 2026-11-14
[12] lmstudio-ai/lmstudio-js:packages/lms-client/src/LMStudioClient.ts@2025945b3600cbb740eb85b52a499e7e14ed35c0 (guessBaseUrl, isLocalhostWithGivenPortLMStudioServer) — read 2026-11-14
[13] lmstudio-ai/docs:1_developer/2_rest/list.md@8c7159be3ffe — read 2026-11-14
[14] lmstudio-ai/docs:1_developer/0_core/ttl-and-auto-evict.md@8c7159be3ffe — read 2026-11-14
[15] lmstudio-ai/docs:1_developer/2_rest/chat.md@8c7159be3ffe — read 2026-11-14
[16] lmstudio-ai/docs:1_developer/2_rest/streaming-events.md@8c7159be3ffe; 1_developer/3_openai-compat/responses.md@8c7159be3ffe — read 2026-11-14
[17] lmstudio-ai/lmstudio-js:packages/lms-common/src/apiServerPorts.ts@2025945b3600 — read 2026-11-14
[18] lmstudio-ai/docs:1_developer/2_rest/load.md@8c7159be3ffe — read 2026-11-14

### Ollama

The source is `ollama/ollama` at `5632619189ab493c70bd3f9435214b8788cc1c80` (main).

| Aspect | Finding [n] |
|---|---|
| Formats spoken | Native: `/api/chat`, `/api/generate`, `/api/embed`, `/api/tags`, `/api/show`, `/api/ps`, `/api/pull`, `/api/version` and others. OpenAI: `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`, `/v1/models`, `/v1/models/:model`, `/v1/responses` (stateless), `/v1/responses/compact`, `/v1/audio/*`. Anthropic: `/v1/messages` [1]. Anthropic unsupported: `/v1/messages/count_tokens`, `tool_choice`, `metadata`, prompt caching (`cache_control`), batches, citations, PDF, mid-stream `error` events, URL images. `budget_tokens` is accepted but not enforced [2]. OpenAI unsupported: `tool_choice`, `logit_bias`, `n`, logprobs. Responses do not support `previous_response_id` or `conversation` [3]. |
| Base URL | `http://127.0.0.1:11434`, set with `OLLAMA_HOST` (default port 11434; 80/443 if a scheme is given without a port) [4]. Network: `OLLAMA_HOST=0.0.0.0:11434` [5]. On a loopback bind, `allowedHostsMiddleware` returns 403 for a `Host` header that is not local/private or allow-listed (DNS-rebinding guard) [1]. |
| Key header | The local server has no auth and does not validate keys. Clients may send any `api_key` ("required but ignored") [2][3][6]. `OLLAMA_AUTH` only affects the client's own signing; the server never reads it [4]. ollama.com cloud (`https://ollama.com/api`, `/v1`) needs `Authorization: Bearer $OLLAMA_API_KEY`; `x-api-key` alone is not accepted there [6]. |
| Model list | `GET /api/tags` lists local (pulled) models. `POST /api/show` returns `model_info["<arch>.context_length"]`, `capabilities[]` and `thinking` [7][8]. `GET /api/ps` lists loaded models with `context_length` [8]. `/v1/models` is the OpenAI shape, where `created` is the last-modified time [3]. A model must be pulled first ("Pull a local model before use"); an unpulled name is not fetched [2][3]. A pulled model loads on first request and stays for `keep_alive` (default `5m`, `OLLAMA_KEEP_ALIVE`). An empty prompt or messages list loads it; `keep_alive: 0` unloads it [7][4]. |
| Usage fields | Native: `prompt_eval_count`, `prompt_eval_cached_count`, `eval_count`, and `total_duration`, `load_duration`, `prompt_eval_duration`, `eval_duration` in nanoseconds [8][7]. OpenAI-compat: `usage.{prompt_tokens,completion_tokens,total_tokens}` plus `prompt_tokens_details.cached_tokens` when the cached count is known [9]. `stream_options.include_usage` is honoured [3][10]. Responses: `usage.input_tokens_details.cached_tokens` [10]. Anthropic: `usage.{input_tokens,output_tokens,cache_read_input_tokens}`, where `input_tokens` = total − cached [11]. The docs list only `input_tokens` and `output_tokens`, and call the counts approximations [2]. |
| Streaming quirks | Native streams by default as NDJSON (`application/x-ndjson`), ending at `"done": true` with `done_reason`. `stream:false` turns it off [12]. `/v1/*` streams SSE and ends with `data: [DONE]` [10]. Reasoning: native `message.thinking` [8]. OpenAI-compat `message.reasoning` / `delta.reasoning`, not `reasoning_content` [9]. Anthropic `thinking_delta` [2]. Reasoning is controlled by `think`, `reasoning_effort` / `reasoning.effort`, and `thinking` / `output_config.effort`, resolved per model from `/api/show` [2][3]. Whether tool calls stream incrementally or arrive whole per chunk: **[unverified: docs site not read]**. `model` is `name:tag` (tag defaults to `latest`) and must match a pulled model. Aliases come from `ollama cp` (e.g. to `claude-3-5-sonnet`) [7][2]. |
| Anything a data declaration could NOT capture | Whether a model is pulled, and whether it is loaded (`/api/ps`), are runtime state [8]. Default context length depends on VRAM: 4k below 24 GiB, 32k at 24–48 GiB, 256k at 48 GiB and above, or `OLLAMA_CONTEXT_LENGTH`. It is fixed per load and checked with `ollama ps` [13][4]. A per-request `options.num_ctx` exists (native) [7]; whether changing it reloads the model: **[unverified: docs site not read]**. Discovery: `GET /` returns "Ollama is running" and `GET /api/version` returns `{version}` [1]. `*:cloud` models proxy to ollama.com through a signed-in local server [2]. |

[1] ollama/ollama:server/routes.go@5632619189ab493c70bd3f9435214b8788cc1c80 (GenerateRoutes ~L2394–2452; allowedHostsMiddleware L2315) — read 2026-11-14
[2] ollama/ollama:docs/api/anthropic-compatibility.mdx@5632619189ab — read 2026-11-14
[3] ollama/ollama:docs/api/openai-compatibility.mdx@5632619189ab — read 2026-11-14
[4] ollama/ollama:envconfig/config.go@5632619189ab (Host L20–52, KeepAlive, UseAuth L231, ContextLength L230) — read 2026-11-14
[5] ollama/ollama:docs/faq.mdx@5632619189ab (L189 "binds 127.0.0.1 port 11434 by default") — read 2026-11-14
[6] ollama/ollama:docs/api/authentication.mdx@5632619189ab — read 2026-11-14
[7] ollama/ollama:docs/api.md@5632619189ab (conventions, load/unload, show) — read 2026-11-14
[8] ollama/ollama:api/types.go@5632619189ab (Metrics L562; Message.Thinking L207; ShowResponse L806; ProcessModelResponse.ContextLength L953) — read 2026-11-14
[9] ollama/ollama:openai/openai.go@5632619189ab (Message/Delta.Reasoning L36/L47; Usage L76–84; usage mapping L266–271) — read 2026-11-14
[10] ollama/ollama:middleware/openai.go@5632619189ab (include_usage L145/L197; `data: [DONE]` L159/L226; Responses cached_tokens L1085) — read 2026-11-14
[11] ollama/ollama:anthropic/anthropic.go@5632619189ab (Usage L223–239) — read 2026-11-14
[12] ollama/ollama:docs/api/streaming.mdx@5632619189ab — read 2026-11-14
[13] ollama/ollama:docs/context-length.mdx@5632619189ab — read 2026-11-14

### vLLM

The source is `vllm-project/vllm` at `7f855f605892656ebf7265642e5d22a34a6ae731` (main).

| Aspect | Finding [n] |
|---|---|
| Formats spoken | OpenAI: `/v1/completions` (no `suffix`), `/v1/chat/completions`, `/v1/chat/completions/batch`, `/v1/responses` (plus `/{id}` and `/{id}/cancel`, so stateful), `/v1/embeddings`, `/v1/audio/transcriptions`, `/v1/audio/translations` [1]. Anthropic: `/v1/messages`, `/v1/messages/count_tokens` [2]. Anthropic rejects with 400: server tools such as `web_search_*`, and `max_tokens: 0` (cache pre-warm) [3]. There is no Ollama-style native chat API. `/invocations` mirrors `/v1` inference [1]. |
| Base URL | `vllm serve` uses `--port` default `8000` [4]. `--host` defaults to `None`, which binds `("", port)`, that is all interfaces (`0.0.0.0`), not loopback [4][5]. `--uds` takes a Unix socket and ignores host and port [4]. |
| Key header | None by default. `--api-key` (repeatable) or `VLLM_API_KEY`; the CLI takes precedence [4][6]. Only `Authorization: Bearer <key>` is checked; there is no `x-api-key`, even for `/v1/messages`. This is true of both the Python middleware and the Rust frontend [7][8]. Only paths under `/v1`, `/v2`, `/inference` and `/cohere` are guarded. `/invocations`, `/health` and others are open, and `OPTIONS` is skipped [7][1]. vLLM's Claude Code guide sets both `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN`, the latter "required" [3]. |
| Model list | `GET /v1/models` returns ModelCards with `max_model_len` [9][10]. The model is loaded at server start, one base model per process, with LoRA adapters added at runtime [10]. `model` must equal a `--served-model-name` or LoRA name, or be empty or omitted. Anything else gets 404 "The model `X` does not exist." [10]. The Claude Code guide warns that names with `/` cause trouble with Claude Code, so it uses `--served-model-name` [3]. |
| Usage fields | `usage.{prompt_tokens,completion_tokens,total_tokens}` and `completion_tokens_details.reasoning_tokens` [11]. `prompt_tokens_details.{cached_tokens,created_cache_tokens,multimodal_tokens}` appear only with `--enable-prompt-tokens-details` (default off) [11][4]. `stream_options.{include_usage,continuous_usage_stats}` are honoured. `--enable-force-include-usage` adds usage to every request [12][13][4]. Anthropic: `usage.{input_tokens,output_tokens,cache_read_input_tokens,cache_creation_input_tokens}`. The cache fields are omitted when details are off; `input_tokens` = prompt − cached − created [14]. |
| Streaming quirks | SSE, ending with `data: [DONE]` on chat completions [13]. Reasoning (with `--reasoning-parser`) appears as `message.reasoning` / `delta.reasoning`. The deprecated input `reasoning_content` is renamed to `reasoning` [15][12]. Anthropic carries `thinking` / `redacted_thinking` blocks [16]. Tool calls need `--enable-auto-tool-choice --tool-call-parser <p>` and stream as `delta.tool_calls` [12][3]. Optional SSE keep-alive comments come with `--sse-keep-alive-interval` (default off) [4]. |
| Anything a data declaration could NOT capture | The served name is whatever `--served-model-name` says, so it must be read from `/v1/models` [10]. Context length is fixed at launch (`--max-model-len`) and visible only as `max_model_len` [9]. No load-before-use and no JIT: one model per process [10]. The listen address may be all interfaces by default [5]. Whether a key is required can only be told by probing a guarded path for 401 [7]. |

[1] vllm-project/vllm:docs/serving/online_serving/openai_compatible_server.md@7f855f605892656ebf7265642e5d22a34a6ae731 — read 2026-11-14
[2] vllm-project/vllm:vllm/entrypoints/anthropic/api_router.py@7f855f605892 — read 2026-11-14
[3] vllm-project/vllm:docs/serving/integrations/claude_code.md@7f855f605892 — read 2026-11-14
[4] vllm-project/vllm:vllm/entrypoints/common/cli_args.py@7f855f605892 (host/port L295–298; api_key L337; enable_prompt_tokens_details L156; enable_force_include_usage L162) — read 2026-11-14
[5] vllm-project/vllm:vllm/entrypoints/launchers/launcher.py@7f855f605892 (L386 `sock_addr = (args.host or "", args.port)`; L398) — read 2026-11-14
[6] vllm-project/vllm:vllm/entrypoints/serve/middleware/register.py@7f855f605892 (L32–33) — read 2026-11-14
[7] vllm-project/vllm:vllm/entrypoints/serve/middleware/authenticate.py@7f855f605892 — read 2026-11-14
[8] vllm-project/vllm:rust/src/server/src/middleware/auth.rs@7f855f605892 — read 2026-11-14
[9] vllm-project/vllm:vllm/entrypoints/serve/engine/protocol.py@7f855f605892 (ModelCard.max_model_len L112; UsageInfo L121–142) — read 2026-11-14
[10] vllm-project/vllm:vllm/entrypoints/openai/models/serving.py@7f855f605892 (check_model L53–62) — read 2026-11-14
[11] = [9] (PromptTokenUsageInfo, CompletionTokenUsageInfo, UsageInfo)
[12] vllm-project/vllm:vllm/entrypoints/generate/base/protocol.py@7f855f605892 (StreamOptions L300; DeltaMessage.reasoning L366) — read 2026-11-14
[13] vllm-project/vllm:vllm/entrypoints/openai/chat_completion/serving.py@7f855f605892 (`[DONE]` L519; should_include_usage L583) — read 2026-11-14
[14] vllm-project/vllm:vllm/entrypoints/anthropic/serving.py@7f855f605892 (usage mapping ~L100–127) — read 2026-11-14
[15] vllm-project/vllm:vllm/entrypoints/openai/chat_completion/protocol.py@7f855f605892 (ChatMessage.reasoning L90; reasoning_content rename L588–604) — read 2026-11-14
[16] vllm-project/vllm:vllm/entrypoints/anthropic/protocol.py@7f855f605892 (thinking L74–92; AnthropicUsage L59–62) — read 2026-11-14

### llama.cpp server (`llama-server`)

The source is `ggml-org/llama.cpp` at `ded522f8304554317e7ff8f88698dbbbca5425aa` (master). The latest release is `v1.0.0` (2026-11-13).

| Aspect | Finding [n] |
|---|---|
| Formats spoken | OpenAI: `/v1/chat/completions` (and `/chat/completions`), `/v1/completions`, `/v1/responses` (and `/responses`), `/v1/embeddings`, `/v1/models`, `/v1/audio/*`, `/v1/realtime` (WebSocket), and `/v1/chat/completions/input_tokens` and `/v1/responses/input_tokens` for counting. Anthropic: `/v1/messages`, `/v1/messages/count_tokens`. Native: `/completion` (legacy, non-OAI), `/infill`, `/embedding`, `/tokenize`, `/detokenize`, `/apply-template`, `/props`, `/slots`, `/health`, `/metrics`, `/rerank` [1]. Anthropic accepts `tool_choice` (`auto`/`any`/`tool`); tools need `--jinja` [2]. What the Anthropic endpoint does not support is not documented: **[unverified: docs site not read]**. |
| Base URL | **The default port is now 9931, not 8080.** It changed on 2026-10-09 in commit `b42b7e6d3059` (#30159) and is in v1.0.0 [3][4]. Doc examples use `http://localhost:9931/v1` [2]. `--host` defaults to `127.0.0.1`; it takes a comma list or `*.sock`, env `LLAMA_ARG_HOST`. `--port` takes env `LLAMA_ARG_PORT` [2]. Docker examples still pass `--host 0.0.0.0 --port 8080` explicitly [2]. Port 8080 survives only as the GCP `AIP_HTTP_PORT` default [5]. |
| Key header | None by default. `--api-key K[,K2]` (env `LLAMA_API_KEY`) or `--api-key-file` [2]. Checked in `Authorization` first, then `X-Api-Key`, with an optional `Bearer ` prefix stripped [5]. `/health`, `/v1/health` and the web UI assets are public. `OPTIONS` is skipped [5]. |
| Model list | Single-model mode: `/v1/models` "always has one single element". `id` = `-m` path or `--alias`; `meta.n_ctx_train`; `meta` is `null` while loading [2]. `GET /props` has `default_generation_settings.n_ctx` (the actual context) [2]. Router mode (`--models-dir`, `-hf` cache, presets): `GET /models` lists all models with load status and memory, plus `POST /models/load` and `/models/unload`, and `/models/sse` [1][2]. It autoloads on request by default (`--no-models-autoload`, `?autoload=`) and caps loaded models with `--models-max` (default 4, LRU) [2]. |
| Usage fields | OpenAI: `usage.{prompt_tokens,completion_tokens,total_tokens,prompt_tokens_details.cached_tokens}` [6][2]. There is a `timings` object `{cache_n,prompt_n,prompt_ms,prompt_per_token_ms,prompt_per_second,predicted_n,predicted_ms,predicted_per_token_ms,predicted_per_second}`; context used = `prompt_n + cache_n + predicted_n` [2]. `stream_options.include_usage` is honoured (default false). It sends a final chunk with empty `choices` and `usage` [7][6]. Responses: `usage.{input_tokens,output_tokens,input_tokens_details.cached_tokens}`. Anthropic: `usage.{input_tokens (= prompt − cached),cache_read_input_tokens,output_tokens}` [6]. Native `/completion`: `tokens_evaluated`, `tokens_cached`, `tokens_predicted`, `truncated` [2]. Cached tokens are reported in every format. |
| Streaming quirks | SSE everywhere. `/completion` uses SSE over POST, so browser `EventSource` cannot read it [2]. `data: [DONE]` is sent for chat and completions but **not** for Responses or Anthropic streams [8]. Reasoning is `message.reasoning_content` / delta `reasoning_content` (DeepSeek style). `--reasoning-format none|deepseek|deepseek-legacy` (default auto). Anthropic gets `thinking` blocks [2][6]. Tool calls need `--jinja` and are streamed as diffs [2][6]. `model`: single-model mode does not route on it; the docs say "Ignored in single model mode" for the control endpoint, and examples send `gpt-3.5-turbo` [2]. Router mode routes on body `model` or query `?model=` [2]. That chat completions never rejects an unknown name in single-model mode: **[unverified: docs site not read]** in code. |
| Anything a data declaration could NOT capture | The 9931/8080 split by build version, so a probe is needed: `/health` is unauthenticated [3][5]. The context size is fixed at load (`-c`, where 0 = from the model; router `--fit` picks the largest that fits at load time). It can be read only from `/props` `n_ctx` or router `memory.effective.n_ctx` [2]. Single-model or router mode, the model id (a file path or alias), and whether `--jinja` (tools) is enabled are launch flags, not discoverable as data except through `/props` and `/models` [2]. |

[1] ggml-org/llama.cpp:tools/server/server.cpp@ded522f8304554317e7ff8f88698dbbbca5425aa (routes L174–233; router routes L555–559) — read 2026-11-14
[2] ggml-org/llama.cpp:tools/server/README.md@ded522f83045 (args table L194–239; /v1/models L1353; chat L1445–1577; /v1/messages L1821; router L2049–2232) — read 2026-11-14
[3] ggml-org/llama.cpp:common/common.h@ded522f83045 (L645 `int32_t port = 9931`; L667 hostnames `127.0.0.1`); blame → commit b42b7e6d3059503dd4bed6ca5dcf342bb6379f67 "server: use port 9931 by default (#30159)", 2026-10-09 — read 2026-11-14
[4] ggml-org/llama.cpp:common/common.h@v1.0.0 (L645, same value) — read 2026-11-14
[5] ggml-org/llama.cpp:tools/server/server-http.cpp@ded522f83045 (gcp_params L170–182; public endpoints L338–345; middleware_validate_api_key L347–375) — read 2026-11-14
[6] ggml-org/llama.cpp:tools/server/server-task.cpp@ded522f83045 (usage_json_oaicompat L357–364; include_usage chunk L494–505; Responses usage L580–584; Anthropic usage L790–793; thinking L751–754) — read 2026-11-14
[7] ggml-org/llama.cpp:tools/server/server-schema.cpp@ded522f83045 (L28–30); tools/server/server-task.h@ded522f83045 (L79 `include_usage = false`) — read 2026-11-14
[8] ggml-org/llama.cpp:tools/server/server-context.cpp@ded522f83045 (L5540–5549) — read 2026-11-14
