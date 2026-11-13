# Model routing terms: your own keys and sign-ins, on open-source harnesses, with no purlis cloud

Research note for issue #6579 (map #6578, "Any model on any harness"). First drafted 2026-11-14
without web access; **re-verified against primary sources on 2026-11-14**. It answers one
question: under today's terms, what may a person do with their **own** API keys and subscription
sign-ins on open-source harnesses (and Claude Code), started by an open-source launcher (purlis)
that has no cloud of its own in the path? It refreshes research 09 §3 and §6.3
(`purlis-plane/docs/program/research/09-model-gateway.md`, read on 2026-09-29), which was written
for a purlis *cloud* gateway. This effort has none.

**This is a cited reading of the terms, not legal advice. A qualified review comes before shipping
anything this note gates.**

## How to read this note

Each claim is tagged:

- **[cited]**: read on 2026-11-14 in the primary source linked (vendor terms, vendor docs,
  licence file, or a vendor's own statement in its own repository). Quotes are verbatim.
- **[cited, not re-read]**: a primary source research 09 read on 2026-09-29 that could not be
  opened on 2026-11-14 (named in [Sources](#sources) with the reason). The quote is research
  09's. No cell rests on one of these alone.
- **[secondary]**: press, blogs, forum posts. Never settles a cell on its own.
- **[repo]**: what purlis already decided, in this repository's ADRs.
- **[inference]**: my reasoning from the above.

Verdicts use three words:

- **allowed**: a clause or official doc grants or documents it.
- **forbidden**: a clause prohibits it.
- **unclear**: the terms are silent, the texts pull both ways, or the vendor only calls it
  unsupported. **Unsupported is not forbidden.** Where the texts lean one way, the cell says so.

One status sits outside the three: **ended**, where the vendor no longer serves the route at all.

Source numbers like [S3] point at the [Sources](#sources) list at the end.

## The answer

- **Your own API key in any harness is allowed by every vendor read, as long as the usage is
  billed to you and the key is not shared, sold or transferred.** Anthropic says so for Claude
  Code ("configuring an API key in a development environment … provided the resulting usage is
  billed to the key owner") [S1] [cited]; DeepSeek grants integration "into various downstream
  systems, applications" [S25] [cited]; Mistral bars only buying, selling or transferring keys
  [S26] [cited]; OpenRouter bars only resale [S15] [cited].
- **A subscription is allowed inside its vendor's own harness, signed in through that harness's
  own flow**: a Claude plan in Claude Code [S1], a ChatGPT plan in Codex [S8], a paid Gemini Code
  Assist Standard or Enterprise licence in Gemini CLI [S12b] [all cited].
- **New since research 09: OpenAI now grants ChatGPT plan use in open-source harnesses.** Its
  "Sign in with ChatGPT" programme says "ChatGPT plan usage is available to all open-source
  partners", and lists **OpenCode** under "Open-source integrations" [S23, S24] [cited]. The
  cell moves from *unclear (tolerated)* to **allowed**, for opencode's own Sign in with ChatGPT
  flow. Its "legacy Codex sign-in methods" stay unclear.
- **A Claude subscription in a third-party harness through an OAuth plugin is forbidden** [S1,
  S2, S4, S28] [cited]. The new Usage Policy (effective 2026-11-12) adds, among prohibited
  conduct: "Resell, proxy, or otherwise provide access to Claude through unauthorized means,
  including services that route requests through consumer subscriptions or misrepresent the
  product or client being used" [S28] [cited].
- **The new case, a third-party harness that drives the unmodified Claude Code binary as its model
  client on the person's own plan (opencode V2's Claude Agent, Goose's `claude-acp`), is
  unclear and leans forbidden.** The legal page's carve-out for "signing in to the unmodified
  Claude Code binary with their own Claude subscription" pulls one way; the Agent SDK note
  ("Anthropic does not allow third party developers to offer claude.ai login or rate limits for
  their products, including agents built on the Claude Agent SDK"), whose SDK is itself "a
  library that runs the Claude Code binary", and the Usage Policy's "misrepresent the product or
  client being used" pull the other. See [The Claude Code binary as a model client](#the-claude-code-binary-as-a-model-client).
- **A Google-account sign-in borrowed from Gemini CLI by another tool is forbidden** by Gemini
  CLI's own terms page [S12c] [cited]. Google-account sign-in in Gemini CLI itself **ended** on
  2026-06-18 for Google AI Pro, Ultra and free Code Assist users, and continues for Code Assist
  Standard and Enterprise licences [S12b] [cited].
- **Claude Code on a non-Claude model is unsupported, not forbidden.** Anthropic "doesn't support
  routing Claude Code to non-Claude models through any gateway" [S3] [cited]. No clause read
  forbids it. purlis chose never to offer it (ADR 0087) [repo]; that is a product choice, not a
  line the terms draw. See [Findings against ADR 0087](#findings-against-adr-0087).
- **Codex with a local model server is allowed**: Codex documents `--oss` with Ollama or LM Studio
  and a "No authentication" provider "for a local model" [S9, S8] [cited].
- **Licences:** Claude Code is proprietary ("All rights reserved. Use is subject to Anthropic's
  Commercial Terms of Service") [S29]; Codex, Gemini CLI, Goose and `claude-agent-acp` are
  Apache-2.0; opencode is MIT; LiteLLM is MIT outside `enterprise/` [S29] [all cited].

## Anthropic

### Claude Code with your own key, plan or cloud credential

Claude Code's legal page, read 2026-11-14 [S1] [cited]. It says Claude Code's use is subject to
the Commercial Terms "for Team, Enterprise, and Claude API users" and the Consumer Terms "for
Free, Pro, Max, and Ultra users". On products:

> "Unless we've mutually agreed otherwise, preinstalling or running Claude Code in your products
> or services (e.g. in hosted sandboxes or other agent infrastructure) requires agreeing to our
> Commercial Terms of Service and complying with the conditions below:
> **The Claude Code binary must not be modified.** Claude Code must be installed and run as
> published by Anthropic, and customers may not remove, disable, or restrict any authentication
> method built into it (including methods that permit signing in with a Claude account or the
> user's own API key).
> **Customers may not pay for, resell, or intermediate Claude usage on their end users' behalf.**
> Each end user must authenticate with their own Anthropic API key, Claude subscription plan
> credentials, or 3P inference provider credential (Amazon Bedrock, Google Cloud's Agent
> Platform, Microsoft Foundry)."

On credentials:

> "This does not restrict how customers provision and manage their own API keys or third-party
> inference provider credentials — for example, configuring an API key in a development
> environment, secrets manager, or machine image for use by the customer's own authorized users —
> provided the resulting usage is billed to the key owner under their agreement with Anthropic (or
> the applicable provider) and is not resold or intermediated as described above. Nor does it
> prevent an end user from signing in to the unmodified Claude Code binary with their own Claude
> subscription, including where a platform hosts Claude Code as described under *Can customers
> offer Claude Code in their products?* above."

And: "Advertised usage limits for Pro, Max, and Ultra plans assume ordinary, individual usage of
Claude Code and the Agent SDK."

- **Own Anthropic key: allowed** [S1].
- **Own Bedrock, Agent Platform (Vertex) or Foundry credential: allowed** [S1].
- **Own Claude subscription, signed in to the unmodified binary: allowed** [S1]. Long unattended
  runs are a usage-limit matter, not a prohibition [inference]; purlis asks once before a plan
  runs unattended (ADR 0087 §11) [repo].
- **Whether a local, open-source launcher is "your products or services"** is still not said
  [S1]. purlis meets the conditions either way: it runs the binary as installed, removes no sign-in
  method, and pays for nothing (ADR 0087 §5, ADR 0095) [repo]. A key profile does not "remove,
  disable, or restrict" the binary's sign-in methods; the person can still `/login` [inference].
- **Changed since research 09:** the page now names the Ultra plan, and its licence section
  splits Commercial Terms (Team, Enterprise, API) from Consumer Terms (Free, Pro, Max, Ultra)
  [S1] [cited]. The clauses research 09 quoted are otherwise unchanged in substance.

### Claude subscription in a third-party harness

[S1] [cited]:

> "**OAuth authentication** is intended exclusively for purchasers of Claude Free, Pro, Max,
> Ultra, Team, and Enterprise subscription plans and is designed to support ordinary use of
> Claude Code and other native Anthropic applications."

> "Anthropic does not permit third-party developers to offer Claude.ai login into their own
> applications, or to route requests through Free, Pro, Max, or Ultra plan credentials on behalf
> of their users. Moreover, developers may not collect, store, or intermediate Claude.ai
> credentials or session tokens — sign-in to a Claude account must complete through Anthropic's
> own flow."

> "Anthropic reserves the right to take measures to enforce these restrictions and may do so
> without prior notice."

Agent SDK overview [S2] [cited]: "Unless previously approved, Anthropic does not allow third party
developers to offer claude.ai login or rate limits for their products, including agents built on
the Claude Agent SDK. Use the API key authentication methods described in the Quickstart
instead." The same page describes the SDK as "A library that runs the Claude Code binary".

Consumer Terms (effective 2025-10-08) [S4] [cited]: users may not, "Except when you are accessing
our Services via an Anthropic API Key or where we otherwise explicitly permit it, … access the
Services through automated or non-human means, whether through a bot, script, or otherwise", and
"You may not share your Account login information, Anthropic API key, or Account credentials with
anyone else."

Usage Policy (effective 2026-11-12) [S28] [cited], under "Do Not Abuse Our Platform": "Resell,
proxy, or otherwise provide access to Claude through unauthorized means, including services that
route requests through consumer subscriptions or misrepresent the product or client being used",
and "Intentionally bypass or interfere with capabilities, restrictions, or guardrails established
within our products, including by circumvention of the account sign-up or log-in process … without
prior authorization from Anthropic".

- **Claude plan via an OAuth plugin in opencode or any other harness: forbidden** [S1, S2, S4,
  S28]. The Usage Policy line is new since research 09 and names subscription routing directly.
- Enforcement history: server-side rejection of third-party OAuth from 2026-01-09; opencode
  removed its Claude Pro/Max plugin "per legal requests" (PR #18186, 2026-03-19) [S20]
  [secondary].
- purlis never offers a Claude plan in opencode (ADR 0095 §3, #2039) [repo].

### The Claude Code binary as a model client

The new case. Two harnesses now run the **unmodified Claude Code binary** as the model client,
signed in to the person's own Claude plan through Anthropic's own `claude auth login` flow:

- **opencode V2, "Claude Agent"** [S19b] [cited]: "Use your Claude plan or another Claude Code
  login. Claude Agent runs Claude Code on your machine, so install Claude Code 2.1.248 or later
  and sign in first." Under "How it works": "Claude Code calls the model, and OpenCode does
  everything else"; "Claude uses OpenCode's tools and permissions instead of Claude Code's
  built-in tools"; "Your Claude Code settings, CLAUDE.md files, hooks, MCP servers, skills, and
  memory are not loaded." Cost for a Claude subscription: "None, your plan covers it".
- **Goose, `claude-acp`** [S27] [cited]: "Wraps claude-agent-acp, an ACP adapter for Anthropic's
  Claude Code" and "Uses the same Claude subscription as the deprecated `claude-code` CLI
  provider". The adapter's README: it "implements an ACP agent by using the official Claude Agent
  SDK", and its standalone archives hold "the Claude CLI (`claude`) it runs" [S27b] [cited].

Does that fall inside the carve-out "signing in to the unmodified Claude Code binary with their own
Claude subscription"?

What supports it [cited, S1]:

- The binary is unmodified, its sign-in is Anthropic's own flow, and neither harness sees the
  token. Those are the binary-and-flow conditions S1 states.
- The carve-out reaches "where a platform hosts Claude Code", so a third party being involved does
  not by itself take the case out.

What weighs against it [cited]:

- The carve-out's own words tie it to hosting Claude Code "as described under *Can customers offer
  Claude Code in their products?*", which means running Claude Code as Claude Code. In both
  harnesses the binary is only a transport: opencode swaps in its own tools, permissions and
  compaction and drops the person's Claude Code settings [S19b] [inference from S1].
- The developer clause is aimed at the harness, not the binary: "Anthropic does not permit
  third-party developers to offer Claude.ai login into their own applications, or to route
  requests through … plan credentials on behalf of their users" [S1]. "Use your Claude plan" is
  exactly that offer [S19b].
- The Agent SDK note forbids third-party products offering claude.ai login "including agents built
  on the Claude Agent SDK", "unless previously approved" [S2], and the SDK *is* "a library that
  runs the Claude Code binary" [S2]. So running the unmodified binary was not, in Anthropic's own
  reading, enough. `claude-agent-acp` is built on that SDK [S27b], so for Goose this text applies
  directly.
- The Usage Policy forbids "services that route requests through consumer subscriptions or
  misrepresent the product or client being used" [S28]. Here the client Anthropic sees is Claude
  Code, while the product is opencode or Goose [inference].
- "OAuth authentication … is designed to support ordinary use of Claude Code and other native
  Anthropic applications" [S1].

Verdict: **unclear, leaning forbidden.** The carve-out is the only text for it, and its wording
fits hosting Claude Code, not using it as a model transport. Three texts point against it, one of
which (S2) names the SDK route outright. Whether opencode, Zed or the ACP project hold the
"previous approval" S2 allows is not public. purlis should not offer a Claude plan through either
route unless Anthropic confirms in writing (S1 points to "contact sales" for "questions about
permitted authentication methods") [inference]. Running the same harnesses with the person's own
**Anthropic API key or cloud credential** in Claude Code is allowed [S1].

### Claude Code on a non-Claude model

[S3] [cited], on both the gateway overview and the "Other LLM gateways" page: "Anthropic doesn't
endorse, maintain, or audit third-party gateway products, and doesn't support routing Claude Code
to non-Claude models through any gateway." Model discovery behind a gateway: "Claude Code keeps an
entry when its `id` contains `claude` or `anthropic` anywhere in the string, matched
case-insensitively, and ignores the rest" [S3] [cited].

- **Unclear: unsupported, not forbidden.** No clause in the legal page [S1], Commercial Terms
  [S5], Consumer Terms [S4] or Usage Policy [S28] mentions non-Claude models. "Unsupported" is
  about who fixes it when it breaks.
- The Commercial Terms bar access "to build a competing product or service, including to train
  competing AI models" (D.4) [S5] [cited]. Running Anthropic's harness on another vendor's model is
  not obviously "building" anything; no text read applies D.4 to it [inference].
- Claude Code's licence is the Commercial Terms [S29], and nothing in them limits which model
  endpoint the binary is pointed at [S5] [inference].
- OpenRouter's own Claude Code page says "Claude Code is optimized for Anthropic models and may
  not work correctly with other providers", and its picker shows "Some picker entries route to
  non-Anthropic providers" [S17] [cited]. That is OpenRouter's statement, not Anthropic's.

### Subscriptions and gateways

Anthropic now documents a subscription session sent through a gateway [S3] [cited]: "Setting only
that variable [`ANTHROPIC_BASE_URL`], without a gateway credential, doesn't replace the
subscription. Requests still route through the gateway, but a saved claude.ai login remains the
active credential … Gateways that pass this traffic on to Anthropic must forward `Authorization`
and the OAuth capability in `anthropic-beta`." The page frames gateways as ones "your
organization already runs".

- For an organization's own gateway, Anthropic documents it, so **allowed as documented** [S3].
- For a launcher's own relay, the legal page still forbids developers to "collect, store, or
  intermediate Claude.ai credentials or session tokens" [S1], and the Usage Policy forbids a
  "proxy … through consumer subscriptions" [S28]. **Forbidden** for purlis [inference from S1,
  S28]. ADR 0087 §5 and §7 already refuse it [repo].

### Claude models in Codex, opencode and other harnesses by API key

The Commercial Terms: "Anthropic gives Customer permission to use the Services, including to power
products and services Customer makes available to its own customers and end users" (A.1), and
bar resale "except as expressly approved by Anthropic" (D.4) [S5] [cited]. No clause limits which
client calls the API with the key owner's key.

- **Allowed** [S5, S1]. opencode documents the Anthropic provider and a custom `baseURL` for it
  [S19] [cited]. Codex needs a Responses-format translator for a Claude model; that is a technical
  limit, not a term [S9] [cited].

## OpenAI

### Codex with your own key or ChatGPT sign-in

Codex authentication docs, read 2026-11-14 [S8] [cited]: "Sign in with ChatGPT for subscription
access", "Sign in with an API key for usage-based access"; "OpenAI bills API key usage through
your OpenAI Platform account at standard API rates". Codex caches credentials "in auth.json under
CODEX_HOME (defaults to ~/.codex)", and says "treat ~/.codex/auth.json like a password … Don't
commit it, paste it into tickets, or share it in chat". The same page documents copying
`auth.json` to the person's own headless machine or container.

- **OpenAI API key in Codex: allowed** [S8].
- **ChatGPT sign-in in Codex: allowed** [S8].
- Custom providers may set `requires_openai_auth = true`: "You can then sign in with ChatGPT or an
  API key. This is useful when you access OpenAI models through an LLM proxy server" [S8] [cited].
  OpenAI therefore documents a ChatGPT session through a proxy, for OpenAI models. purlis's rule
  that a login chat is never rerouted (ADR 0087 §5) is stricter than OpenAI's docs [repo].
- The OpenAI Services Agreement's key clauses, as research 09 read them: 3.3(g), the customer
  will not "buy, sell, or transfer API keys from, to, or with a third party", and 3.1, "Customer
  may not resell or lease access to its Account" [S6] [cited, not re-read].

### Codex with other providers, routers and local models

Codex advanced config [S9] [cited]: "A model provider defines how Codex connects to a model (base
URL, wire API, authentication, and optional HTTP headers)." Its own example defines
`[model_providers.mistral]` with `base_url = "https://api.mistral.ai/v1"` and a
`[model_providers.local_ollama]`. "If you just need to point the built-in OpenAI provider at an
LLM proxy, router, or data-residency enabled project, set openai_base_url". "Codex can run against
a local 'open source' provider such as Ollama or LM Studio when you pass --oss". The auth page adds
"No authentication: For a local model that doesn't require credentials, leave
requires_openai_auth unset" [S8] [cited].

- **Other vendor's key via `model_providers`: allowed** [S9], subject to that vendor's terms.
- **Router: allowed** [S9, S15].
- **Local model server: allowed** [S9, S8]. This settles the open cell.

### ChatGPT sign-in inside opencode and other harnesses

OpenAI's "Sign in with ChatGPT" (SIWC) docs, read 2026-11-14 [S24] [cited]:

> "**For open-source developers:** Let users run AI workloads in your tools with their ChatGPT
> plan, without requiring them to provide an API key. The open-source sign-in flow registers your
> client and issues OAuth credentials for eligible Responses API requests … ChatGPT plan usage is
> available to all open-source partners and selected private clients."

> "These docs explain ChatGPT plan usage for open-source and locally hosted apps. If you're
> interested in offering it in a paid or remotely hosted app, complete the interest form."

OpenAI's user-facing page lists, under "Open-source integrations": "OpenClaw OpenCode Pi T3"
[S23] [cited]. opencode's providers page: "Use a ChatGPT Plus or Pro plan instead of an OpenAI API
key. Connect OpenAI, then choose Sign in with ChatGPT or one of the legacy Codex sign-in methods"
[S19] [cited]. OpenAI's help article on the same programme could not be opened (HTTP 403); a
search excerpt says it covers "participating commercial apps" and that "supported open-source
tools remain available to all ChatGPT users" [S23b] [secondary: search excerpt only].

- **ChatGPT sign-in in opencode through its Sign in with ChatGPT flow: allowed (granted)** [S23,
  S24]. This replaces research 09's *tolerated* reading.
- **Through opencode's "legacy Codex sign-in methods": unclear.** That path borrows Codex's client
  rather than registering opencode's own, and no page read grants it [inference from S19, S24].
- **Other open-source harnesses:** allowed when they use their own SIWC registration and run
  locally or self-hosted [S24]; a harness not doing so is unclear. A paid or remotely hosted
  product needs OpenAI's approval [S24].
- OpenAI has ended a named partner's direct model access by vendor decision before (Cursor) [S11]
  [cited, not re-read; the draft's date for it could not be checked]: grants can be withdrawn.

## Google

- **Gemini API key, in Gemini CLI or any harness: allowed.** Gemini API Additional Terms
  (effective 2026-03-23) [S14] [cited]: "You may only access the Services (or make API Clients
  available to users) within an available region. **You may use only Paid Services when making
  API Clients available to users in the European Economic Area, Switzerland, or the United
  Kingdom.**" Also: "Use of Google AI Studio and Gemini API is for developers building with Google
  AI models for professional or business purposes, not for consumer use." A coding harness used
  for work fits that; a person in the EEA, Switzerland or the UK needs a paid key [inference].
  Google's transition post says "Gemini CLI will remain accessible via paid Gemini and Gemini
  Enterprise Agent Platform API keys" [S12b] [cited]; whether an unpaid key still works in Gemini
  CLI is not said there, though Gemini CLI's terms page still lists "Gemini API - Unpaid Services"
  [S12c] [cited].
- **Vertex in Gemini CLI: allowed.** Documented with `GOOGLE_CLOUD_PROJECT` and
  `GOOGLE_CLOUD_LOCATION` [S12] [cited]. Google Cloud's terms bar resale: Customer will not "(c)
  sell, resell, sublicense, transfer, or distribute any or all of the Services" (§3.3) [S16]
  [cited].
- **Google-account sign-in in Gemini CLI after 2026-06-18.** Google's post of 2026-05-19 [S12b]
  [cited]: "On June 18, 2026, Gemini CLI and Gemini Code Assist IDE extensions will stop serving
  requests for Google AI Pro and Ultra, as well as those using it free of charge using Gemini
  Code Assist for individuals." And: "If your organization uses Gemini CLI or our IDE extensions
  via a Gemini Code Assist Standard or Enterprise license … your access remains unchanged." The
  auth page's banner agrees ("Unpaid tier and Google One users: Gemini CLI was replaced by
  Antigravity CLI on June 18th, 2026") although its body still tells Google AI Pro and Ultra
  subscribers to sign in [S12] [cited]. Terms for that sign-in: Google Cloud Platform Terms for
  Standard and Enterprise editions [S12c] [cited].
  - **Code Assist Standard or Enterprise licence: allowed** [S12b, S12c].
  - **Google AI Pro, Ultra, free Code Assist: ended** [S12b].
- **A Google sign-in borrowed by another tool: forbidden.** Gemini CLI's terms page [S12c]
  [cited]: "Directly accessing the services powering Gemini CLI (for example, the Gemini Code
  Assist service) using third-party software, tools, or services (for example, using OpenClaw with
  Gemini CLI OAuth) is a violation of applicable terms and policies. Such actions may be grounds
  for suspension or termination of your account." The maintainer statement of 2026-02-27 says the
  same [S13] [cited]. A question in that thread about "wrappers which utilize the ACP
  functionality" got no maintainer answer [S13] [cited], so a harness driving Gemini CLI over ACP
  on a Google sign-in is **unclear**.
- **Gemini CLI on another model** (`GOOGLE_GEMINI_BASE_URL` and a translator): not documented on
  any Google page read on 2026-11-14 [S12]. **Unclear**: unsupported, no clause read forbids it.

## DeepSeek, Mistral, xAI

- **DeepSeek** Open Platform Terms (effective 2026-04-29) [S25] [cited]: 1.1 "You can integrate
  the capabilities of the DeepSeek models into various downstream systems, applications, or
  functionalities for intended purposes and specific scenarios, providing services to both
  internal and external end users." 2.2 "Please keep your created API key securely, prevent any
  form of leakage, do not share or publicly disclose your API key with others, and do not expose
  it in browser or other client-side code." Nothing names coding agents, relays or who sets the
  base URL. **Own DeepSeek key in a harness: allowed** [S25]. A harness on the owner's own machine
  is not "client-side code" shipped to others [inference]; purlis's loopback proxy keeps the key
  out of the harness's process anyway (ADR 0087 §7) [repo].
- **Mistral** Commercial Terms (effective 2026-09-25) [S26] [cited], §2.2 Use Restrictions:
  Customer may not "(h) buy, sell, or transfer API keys or any type of Mistral AI account from, to,
  or with a third party; or (i) integrate or combine Vibe with your own products you offer to
  third-parties including any other software, hardware, or other technology, nor grant any third
  party access to the Mistral AI Products without our prior written authorization". Codex's own
  docs show a Mistral provider [S9]. **Own Mistral key in a harness: allowed** [S26]. Clause (i)
  bars bundling Mistral's own coding agent, Vibe, into a product for others; purlis does not ship
  Vibe [inference].
- **xAI**: the terms pages (x.ai/legal, terms-of-service-enterprise) returned HTTP 403 to both
  fetch tools on 2026-11-14. xAI's developer FAQ says "Do not share keys between teammates to avoid
  unauthorized access" [S30] [cited]. **Every xAI cell is unclear** until the terms are read.

## Routers: OpenRouter and LiteLLM

- **OpenRouter** Terms (last updated 2026-08-31) [S15] [cited], §7: you agree not to "access the
  Site or Service for purposes of reselling API access to Models or otherwise developing a
  competing service", nor to "create multiple accounts as a single user, for purposes of bypassing
  or circumventing use limits". §5.1: "By accessing or using any Model through the Service, you
  agree … to comply with the applicable terms for each Model". §3.2 makes you responsible for
  "all activity and charges under its account or API Credentials".
  - **Own OpenRouter key in any harness that takes a base URL: allowed**, for your own use
    [S15].
  - **OpenRouter as Claude Code's backend, Claude models.** OpenRouter documents it
    (`ANTHROPIC_BASE_URL=https://openrouter.ai/api`, key in `ANTHROPIC_AUTH_TOKEN`) [S17] [cited].
    Anthropic says "Any gateway that exposes a supported API format works" but endorses none [S3]
    [cited]. S1's "billed to the key owner under their agreement with Anthropic (or the applicable
    provider)" names Anthropic and three clouds, not routers; S1's conditions bind products that
    run Claude Code, and the person's own use is governed by the Consumer or Commercial Terms,
    which say nothing about routers [inference]. **Unclear**, with nothing read forbidding it.
- **LiteLLM** is software you run; its licence is MIT outside `litellm/enterprise/`, which has its
  own licence [S29] [cited]. The upstream's terms govern your key [inference].
  - **LiteLLM in front of Claude Code with your own Anthropic or cloud credential: allowed**, as a
    gateway Anthropic documents and billed to the key owner [S3, S1].
  - LiteLLM's PyPI releases 1.82.7 and 1.82.8 carried a credential stealer on 2026-03-24 [S18]
    [cited, not re-read]: pin and check what the person installs.
- **A launcher setting the base URL** for a key chat is configuration each harness documents:
  Claude Code [S3], Codex [S9], opencode [S19], and opencode's Claude Agent maps its `baseURL` to
  `ANTHROPIC_BASE_URL` [S19b] [all cited]. No clause read addresses who sets it [inference]. For a
  subscription chat, see [Subscriptions and gateways](#subscriptions-and-gateways).

## Licences of each harness

All read 2026-11-14 [S29] [cited].

| Harness | Licence file | Licence | What a launcher may do [inference] |
|---|---|---|---|
| Claude Code | anthropics/claude-code `LICENSE.md` | "© Anthropic PBC. All rights reserved. Use is subject to Anthropic's Commercial Terms of Service." Not open source | Run it as installed and configure it through its documented flags, settings and variables. Not modify, patch or redistribute it. Wrapping its process is what [S1] describes for products; its documented gateway path is [S3]. Bundling it would need Anthropic's agreement |
| Codex | openai/codex `LICENSE` | Apache-2.0 | Configure, wrap, proxy, fork and redistribute, keeping the licence and NOTICE. The licence grants no right to OpenAI's service; [S6, S8, S24] govern the traffic |
| opencode | anomalyco/opencode `LICENSE` | MIT ("Copyright (c) 2025 opencode") | Configure, wrap, proxy, fork and redistribute, keeping the notice. Each provider's terms govern its traffic |
| Gemini CLI | google-gemini/gemini-cli `LICENSE` | Apache-2.0; its terms page adds that "When you use Gemini CLI to access or use Google's services, the Terms of Service and Privacy Notices applicable to those services apply" [S12c] | As Codex. Google's terms govern the traffic, and a fork or wrapper using Gemini CLI's OAuth is the forbidden case [S12c] |
| Goose | block/goose `LICENSE` | Apache-2.0 | As Codex. Each provider's terms govern its traffic |
| claude-agent-acp (Goose's `claude-acp`) | agentclientprotocol/claude-agent-acp `LICENSE` | Apache-2.0 for the adapter; the Claude Code binary it runs stays under [S5] | The adapter is free to use; the binary and the plan sign-in are not, see [The Claude Code binary as a model client](#the-claude-code-binary-as-a-model-client) |
| LiteLLM | BerriAI/litellm `LICENSE` | MIT, except `litellm/enterprise/` under its own licence | Use the open part freely; check whether a feature lives under `enterprise/` |

A software licence settles what may be done with the program, never what may be done with a
vendor's service through it [inference]. purlis runs each harness as an installed program and
never ships or patches one (ADR 0095) [repo].

## What an open-source launcher must never do

1. **Collect, store or intermediate a Claude sign-in.** "developers may not collect, store, or
   intermediate Claude.ai credentials or session tokens — sign-in to a Claude account must complete
   through Anthropic's own flow" [S1] [cited]. purlis: the harness's own flow, the harness's own
   folder, never read or moved (ADR 0095 §3) [repo].
2. **Offer a Claude plan outside Anthropic's own apps, or route requests through plan
   credentials** [S1, S2, S28] [cited]. purlis: no Claude plan in opencode (ADR 0095 §3) [repo].
   Until Anthropic says otherwise, that includes opencode's Claude Agent and Goose's `claude-acp`
   [inference].
3. **Relay a subscription session through its own proxy.** Anthropic and OpenAI both document an
   organization's gateway in front of a subscription session [S3, S8], but a launcher's relay is a
   developer intermediating the credential [S1] and a "proxy … through consumer subscriptions"
   [S28] [cited]. purlis: a `login` chat starts with no routing variables and the key proxy refuses
   it (ADR 0087 §5, §7) [repo].
4. **Pay for, resell or intermediate usage.** Anthropic [S1], Commercial Terms D.4 [S5], Usage
   Policy [S28], Google Cloud §3.3(c) [S16], OpenRouter §7 [S15] [cited]; OpenAI 3.1, 3.3(g) [S6]
   [cited, not re-read]. With no purlis cloud there is nothing to bill through; keep it so.
5. **Share or move a credential between people.** Anthropic [S4]; DeepSeek 2.2 [S25]; Mistral
   (h) [S26]; Codex docs [S8] [cited]. purlis sends keys only to devices the person owns
   (ADR 0095 §3, §4) [repo]. Codex documents copying the person's own `auth.json` to their own
   headless machine [S8], which is moving between devices, not people.
6. **Borrow one harness's sign-in for another tool.** Google [S12c, S13]; Anthropic [S1, S2]
   [cited]. OpenAI's grant covers an open-source app's *own* SIWC registration [S24], not a
   borrowed Codex client [inference].
7. **Modify the Claude Code binary or remove its sign-in methods** [S1] [cited].
8. **Present a product as Claude Code.** S1: "you can't use the Claude Code or Anthropic names or
   logos as part of your own product, feature, or company name"; S2 forbids "'Claude Code' or
   'Claude Code Agent'" as branding for SDK-built agents [cited].

## Findings against ADR 0087

ADR 0087 says, in its context, "**Vendor terms draw hard lines** (research 09 §3, §6.3) … A
subscription session must not be pointed at a gateway. Claude Code is not to be routed to a
non-Claude model", and decides "Claude Code is never offered a non-Claude model" [repo].

1. **No primary source read on 2026-11-14 forbids Claude Code on a non-Claude model.** Anthropic's
   only text is that it "doesn't support" it [S3]; the legal page, Commercial Terms, Consumer Terms
   and the Usage Policy of 2026-11-12 are silent [S1, S4, S5, S28]. The decision stands as a product
   choice (the model filter [S3] and "may not work correctly" [S17] are good reasons), but the
   context's claim that *the terms* draw this line is not supported. The amendment should say
   "unsupported by Anthropic", not "not to be routed".
2. **"A subscription session must not be pointed at a gateway" is narrower in the vendors' own
   docs.** Anthropic documents a claude.ai login through an organization's gateway [S3]; Codex
   documents ChatGPT sign-in through "an LLM proxy server" for OpenAI models [S8]. What the terms
   forbid is a *third-party developer* intermediating the credential [S1] or proxying through
   consumer subscriptions [S28]. purlis's rule is still right for purlis's own relay; the reason
   should cite that clause, not a general ban.
3. **ChatGPT in opencode is now granted, not tolerated** [S23, S24], for opencode's own Sign in with
   ChatGPT flow. Wherever ADR 0087 or ADR 0095 treats it as a tolerance, that can be updated.

## Open questions

1. **Is the Claude Code binary as a model client (opencode Claude Agent, Goose `claude-acp`)
   inside S1's carve-out?** The texts pull both ways and no approval is public. Only Anthropic can
   settle it; S1 names "contact sales" for authentication questions.
2. **Does Claude Code's "products or services" condition cover a local, open-source launcher with
   no service?** Not said [S1]. purlis complies either way.
3. **Is OpenRouter a permitted backend for Claude Code on Claude models** under the person's own
   Consumer or Commercial Terms? Nothing read forbids or names it.
4. **xAI's terms** could not be fetched (HTTP 403).
5. **The OpenAI Services Agreement and Terms of Use** could not be re-read (403 on openai.com; the
   PDF's text did not extract). The OpenAI cells rest on the Codex and SIWC docs instead.
6. **Does an unpaid Gemini API key still work in Gemini CLI?** Google's post names only paid keys
   [S12b]; Gemini CLI's terms page still lists unpaid services [S12c].
7. **Does the key proxy (ADR 0087 §7) count as "intermediating"?** The reading here is no: it runs
   on the owner's device, swaps one header for the owner's own key, and the usage is billed to the
   owner, which is the carve-out S1 states [inference]. A qualified review should confirm it.

## The matrix

(harness, sign-in kind, model provider) → verdict, every cell cited. Read 2026-11-14 unless a
source says otherwise. "Own" means the person's own credential, used on their own devices.

### Claude Code

| Sign-in kind | Model provider | Verdict | Why |
|---|---|---|---|
| API key | Anthropic | **allowed** | Own key, billed to its owner [S1]. Through the loopback key proxy: open question 7 |
| Subscription (own `/login`) | Anthropic (Claude plan) | **allowed** | Unmodified binary, Anthropic's own flow [S1] |
| Subscription, through a launcher's relay | Anthropic | **forbidden** | Developer may not "intermediate" plan credentials [S1]; "proxy … through consumer subscriptions" [S28] |
| Subscription, through the person's organization's gateway | Anthropic | **allowed** (documented) | [S3] documents it; purlis does not offer it (ADR 0087 §5) [repo] |
| Cloud credential | Bedrock, Agent Platform (Vertex), Foundry | **allowed** | "3P inference provider credential" [S1] |
| Router key | LiteLLM (self-run) with own Anthropic or cloud credential | **allowed** | Documented gateway [S3], billed to the key owner [S1] |
| Router key | OpenRouter, Claude models | **unclear** | OpenRouter documents it [S17]; Anthropic: "Any gateway that exposes a supported API format works", endorses none [S3]; not named in [S1]. Open question 3 |
| Router key | OpenRouter, non-Claude models | **unclear** (unsupported) | "doesn't support routing Claude Code to non-Claude models through any gateway" [S3]; no clause forbids it [S1, S4, S5, S28] |
| None (local) | Local model server | **unclear** (unsupported) | No local Claude model, so a non-Claude model [S3]; discovery drops non-`claude` IDs [S3] |
| API key via base URL | Any non-Claude vendor (DeepSeek, Mistral, …) | **unclear** (unsupported, not forbidden) | [S3]; no clause forbids it [S1, S4, S5, S28]; vendor side allowed [S25, S26]. purlis never offers it (ADR 0087) [repo] |

### Codex

| Sign-in kind | Model provider | Verdict | Why |
|---|---|---|---|
| API key | OpenAI | **allowed** | [S8]; not to be transferred, OSA 3.3(g) [S6] (not re-read) |
| Subscription (own `codex login`) | OpenAI (ChatGPT plan) | **allowed** | [S8] |
| Subscription via `requires_openai_auth` through a proxy | OpenAI | **allowed** (documented) | "useful when you access OpenAI models through an LLM proxy server" [S8]; purlis does not relay it (ADR 0087 §5) [repo] |
| API key via `model_providers` | Anthropic, Mistral, DeepSeek, other vendors | **allowed** (degraded off OpenAI models) | Documented, Mistral is OpenAI's own example [S9]; vendor side [S5, S26, S25] |
| Router key | OpenRouter, LiteLLM | **allowed** | `openai_base_url` "LLM proxy, router" [S9]; own use, no resale [S15] |
| None (local) | Local model server (Ollama, LM Studio) | **allowed** | `--oss`, "No authentication … for a local model" [S9, S8] |
| API key | xAI | **unclear** | Codex side allowed [S9]; xAI terms unreadable (403) |

### opencode

| Sign-in kind | Model provider | Verdict | Why |
|---|---|---|---|
| API key | Anthropic | **allowed** | Commercial Terms A.1 [S5]; own key [S1]; documented provider [S19] |
| Subscription via OAuth plugin | Anthropic (Claude plan) | **forbidden** | [S1] developer clause; [S2]; [S4] automated means; [S28] |
| Claude Agent: Claude Code binary signed in to own Claude plan | Anthropic (Claude plan) | **unclear, leaning forbidden** | Carve-out [S1] vs developer clause [S1], SDK note [S2], Usage Policy [S28]; see [the section](#the-claude-code-binary-as-a-model-client). Open question 1 |
| Claude Agent: Claude Code binary with own API key or cloud credential | Anthropic, Bedrock, Vertex, Foundry | **allowed** | Own key or 3P credential [S1]; documented [S19b] |
| API key | OpenAI | **allowed** | [S8] (OpenAI's own key docs); OSA [S6] (not re-read) |
| Subscription: opencode's Sign in with ChatGPT | OpenAI (ChatGPT plan) | **allowed** (granted) | OpenCode listed as an open-source integration [S23]; "available to all open-source partners" [S24]; [S19] |
| Subscription: opencode's "legacy Codex sign-in methods" | OpenAI (ChatGPT plan) | **unclear** | Not the SIWC registration [S24]; no grant read [S19] |
| API key | Gemini API | **allowed** | API clients addressed; paid only for EEA/CH/UK users [S14] |
| Google-account sign-in (Gemini CLI's OAuth) | Google | **forbidden** | [S12c]; [S13] |
| API key | DeepSeek | **allowed** | Integration granted 1.1, key not shared 2.2 [S25] |
| API key | Mistral | **allowed** | Only buying, selling, transferring keys barred, (h) [S26] |
| API key | xAI | **unclear** | Terms unreadable (403); key-sharing advice only [S30] |
| Router key | OpenRouter, LiteLLM | **allowed** | Native OpenRouter provider and custom gateways documented [S19]; own use, no resale [S15] |
| None (local) | Local model server (Ollama, LM Studio, vLLM) | **allowed** | Documented [S19]; no provider terms apply [inference] |

### Gemini CLI

| Sign-in kind | Model provider | Verdict | Why |
|---|---|---|---|
| API key (paid) | Gemini API | **allowed** | Documented [S12, S12c]; "remain accessible via paid Gemini … API keys" [S12b]; [S14] |
| API key (unpaid) | Gemini API | **unclear** | Terms page lists unpaid services [S12c]; transition post names only paid keys [S12b]. Open question 6 |
| Google-account sign-in, Code Assist Standard or Enterprise licence | Google | **allowed** | "your access remains unchanged" [S12b]; GCP Terms apply [S12c] |
| Google-account sign-in, Google AI Pro, Ultra or free Code Assist | Google | **ended** (2026-06-18) | "will stop serving requests" [S12b]; banner [S12] |
| Google-account sign-in, driven by another tool over ACP | Google | **unclear** | Asked of the maintainer, unanswered [S13]; "third-party software" accessing the service is forbidden [S12c] |
| Cloud credential | Vertex | **allowed** | Documented [S12]; no resale §3.3(c) [S16] |
| API key via base URL | Non-Google model | **unclear** (undocumented) | Not on any Google page read [S12] |

### Goose

| Sign-in kind | Model provider | Verdict | Why |
|---|---|---|---|
| `claude-acp`: Claude Code binary (via Agent SDK) signed in to own Claude plan | Anthropic (Claude plan) | **unclear, leaning forbidden** | Adapter "using the official Claude Agent SDK" [S27b]; SDK note forbids claude.ai login for SDK-built agents "unless previously approved" [S2]; carve-out [S1]; [S28] |
| `claude-acp` with own Anthropic API key or cloud credential | Anthropic, Bedrock, Vertex, Foundry | **allowed** | [S1] |
| `codex-acp`: Codex signed in with own ChatGPT plan | OpenAI (ChatGPT plan) | **unclear** | Runs Codex with Codex's own login [S27]; Goose is not on OpenAI's integration list [S23]; no grant or bar read |
| `codex-acp` with own OpenAI API key | OpenAI | **allowed** | [S8], [S27] |
| API key, router key or local server | Any vendor | **allowed** by the vendor terms above [S14, S15, S25, S26, S5] | Goose's own provider docs beyond ACP were not read [inference] |

### Licence column

| Harness | Licence [S29] | Configure | Wrap (run as a child process) | Proxy its model traffic |
|---|---|---|---|---|
| Claude Code | Proprietary, Commercial Terms | **allowed**: documented flags, settings, `ANTHROPIC_*` [S3] | **allowed**, unmodified, sign-in methods intact [S1] | **allowed** for key and cloud credentials through a gateway [S3]; **forbidden** for a launcher relaying a plan [S1, S28] |
| Codex | Apache-2.0 | **allowed** [S9] | **allowed** [S29] | **allowed** [S9]; ChatGPT sign-in through a proxy documented [S8] |
| opencode | MIT | **allowed** [S19] | **allowed** [S29] | **allowed** [S19] |
| Gemini CLI | Apache-2.0 | **allowed** [S12] | **allowed** [S29]; but a third-party tool using its OAuth is **forbidden** [S12c] | **unclear**: base URL undocumented [S12] |
| Goose | Apache-2.0 | **allowed** [S27] | **allowed** [S29] | **allowed** by its licence [S29]; each provider's terms govern |

## Sources

All read 2026-11-14 unless marked otherwise. "403" means the site refused both fetch tools that
day.

- [S1] Anthropic, Claude Code, Legal and compliance:
  https://code.claude.com/docs/en/legal-and-compliance [cited].
- [S2] Anthropic, Agent SDK overview: https://code.claude.com/docs/en/agent-sdk/overview [cited].
- [S3] Anthropic, Claude Code gateways: https://code.claude.com/docs/en/gateways,
  https://code.claude.com/docs/en/llm-gateway, https://code.claude.com/docs/en/llm-gateway-connect,
  https://code.claude.com/docs/en/llm-gateway-protocol.md (model discovery) [cited]; and
  https://code.claude.com/docs/en/authentication.md [cited].
- [S4] Anthropic, Consumer Terms (effective 2025-10-08): https://www.anthropic.com/legal/consumer-terms [cited].
- [S5] Anthropic, Commercial Terms (effective 2025-06-17): https://www.anthropic.com/legal/commercial-terms [cited].
- [S6] OpenAI Services Agreement: https://cdn.openai.com/osa/openai-services-agreement.pdf. The
  PDF downloaded but its text did not extract [cited, not re-read; research 09, 2026-09-29].
- [S7] OpenAI Terms of Use: https://openai.com/policies/row-terms-of-use/. 403 [cited, not
  re-read]. No cell rests on it.
- [S8] OpenAI, Codex authentication: https://developers.openai.com/codex/auth (serves
  https://learn.chatgpt.com/docs/auth) [cited].
- [S9] OpenAI, Codex advanced config: https://learn.chatgpt.com/docs/config-file/config-advanced [cited].
- [S11] OpenAI, decision on Cursor:
  https://openai.com/index/our-decision-on-cursor-following-its-acquisition-by-spacex/. 403
  [cited, not re-read; research 09].
- [S12] Gemini CLI authentication: https://geminicli.com/docs/get-started/authentication/ [cited].
- [S12b] Google for Developers blog, "An important update: Transitioning Gemini CLI to Antigravity
  CLI" (2026-05-19):
  https://developers.googleblog.com/an-important-update-transitioning-gemini-cli-to-antigravity-cli [cited].
- [S12c] Gemini CLI, License, Terms of Service, and Privacy Notices:
  https://geminicli.com/docs/resources/tos-privacy/ (source:
  https://github.com/google-gemini/gemini-cli/blob/main/docs/resources/tos-privacy.md) [cited].
- [S13] Gemini CLI discussion #20632 (maintainer statement, 2026-02-27):
  https://github.com/google-gemini/gemini-cli/discussions/20632 [cited: vendor staff in the vendor's
  own repository].
- [S14] Gemini API Additional Terms (effective 2026-03-23): https://ai.google.dev/gemini-api/terms
  (redirects to https://aistudio.google.com/docs/terms) [cited].
- [S15] OpenRouter Terms (last updated 2026-08-31): https://openrouter.ai/terms [cited].
- [S16] Google Cloud Platform Terms: https://cloud.google.com/terms [cited].
- [S17] OpenRouter, Claude Code integration:
  https://openrouter.ai/docs/cookbook/coding-agents/claude-code-integration [cited].
- [S18] LiteLLM, security update March 2026: https://docs.litellm.ai/blog/security-update-march-2026
  [cited, not re-read; research 09].
- [S19] opencode V2 providers: https://opencode.ai/docs/providers/ [cited].
- [S19b] opencode V2, Claude Agent: https://opencode.ai/v2/docs/claude-agent [cited].
- [S20] [secondary] Enforcement coverage:
  https://venturebeat.com/technology/anthropic-cracks-down-on-unauthorized-claude-usage-by-third-party-harnesses,
  https://github.com/anomalyco/opencode/pull/18186. Read by research 09, 2026-09-29.
- [S23] OpenAI, Sign in with ChatGPT (user page, partner list):
  https://learn.chatgpt.com/docs/sign-in-with-chatgpt [cited].
- [S23b] OpenAI Help, "Using your ChatGPT plan in other apps and sites":
  https://help.openai.com/en/articles/20001542-using-your-chatgpt-plan-in-other-apps-and-sites.
  403; known only from a search-engine excerpt [secondary].
- [S24] OpenAI, Sign in with ChatGPT developer docs (quickstart, "ChatGPT plan usage" overview):
  https://developers.openai.com/siwc/llms-full.txt, https://developers.openai.com/siwc/quickstart,
  https://developers.openai.com/siwc/token-sharing-open-source [cited].
- [S25] DeepSeek Open Platform Terms of Service (effective 2026-04-29):
  https://cdn.deepseek.com/policies/en-US/deepseek-open-platform-terms-of-service.html [cited].
- [S26] Mistral AI Commercial Terms of Service (effective 2026-09-25):
  https://legal.mistral.ai/terms/commercial-terms-of-service [cited].
- [S27] Goose, ACP providers: https://goose-docs.ai/docs/guides/acp-providers (undated page) [cited].
- [S27b] claude-agent-acp README and LICENSE: https://github.com/agentclientprotocol/claude-agent-acp [cited].
- [S28] Anthropic, Usage Policy (effective 2026-11-12): https://www.anthropic.com/legal/aup [cited].
- [S29] Licence files [cited]: https://github.com/anthropics/claude-code/blob/main/LICENSE.md,
  https://github.com/openai/codex/blob/main/LICENSE,
  https://github.com/anomalyco/opencode/blob/dev/LICENSE,
  https://github.com/google-gemini/gemini-cli/blob/main/LICENSE,
  https://github.com/block/goose/blob/main/LICENSE,
  https://github.com/agentclientprotocol/claude-agent-acp/blob/main/LICENSE,
  https://github.com/BerriAI/litellm/blob/main/LICENSE.
- [S30] xAI, API security FAQ: https://docs.x.ai/developers/faq/security [cited]. xAI terms
  (https://x.ai/legal/terms-of-service-enterprise): 403.
- [repo] ADR 0087 and its 2026-10-01 ruling (V36); ADR 0095 §3 and §4 and its dispatcher decisions
  (#2039); research 09 (`purlis-plane/docs/program/research/09-model-gateway.md`).
