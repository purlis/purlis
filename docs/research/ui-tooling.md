# UI tooling for purlis: skills, primitives, tokens, verification

Researched 2026-10-20. The question: which agent skills, tools and libraries let an AI coding agent build world-class, accessible desktop UI for purlis/purlis, and *verify* it? purlis is a Tauri 2 app with a React + TypeScript + Tailwind v4 webview and a Rust core.

## How this was sourced (read first)

- Primary sources only: official docs, specs, vendor repos, the npm registry and the GitHub API. Every claim carries its URL.
- **The sandbox blocked many doc sites**, and no WebFetch was available, so all reading went through `curl`.
  - Reachable: GitHub (repos and raw files), npm, designtokens.org, developer.apple.com, code.visualstudio.com, zed.dev, storybook.js.org, base-ui.com, ui.shadcn.com, radix-ui.com, code.claude.com.
  - Where a doc site was blocked, the same page was read from the project's own docs source on GitHub, and that GitHub URL is cited.
  - Claims that could not be checked are marked **unverified (host blocked)**.
  - The main gaps: gzip bundle sizes (bundlephobia blocked), the Fluent 2 design site's usage prose, and crates.io.
- **Not checked:** which UI libraries the purlis app already depends on. The app repo was not reachable from this workspace (no local clone; `gh` to GitHub timed out). Before adopting anything below, diff the recommendation against the app's `package.json`.
- Installed-skill findings come from the local SKILL.md files under `/Users/aharon/.claude/plugins/cache/claude-plugins-official/`.

## Contents

1. Agent skills and plugins (installed, then candidates)
2. Accessible headless primitives
3. Design tokens and theming architecture
4. Verification
5. Recommendation

---

# 1. Agent skills and plugins

## 1a. Installed skills: what each actually gives (verified from local SKILL.md files)

All paths are under `/Users/aharon/.claude/plugins/cache/claude-plugins-official/`.

| Skill | What it actually gives | UI build | UI verify | a11y |
|---|---|---|---|---|
| `frontend-design` (`frontend-design/<hash>/skills/frontend-design/SKILL.md`) | Aesthetic direction prompt: "design lead at a studio", ground the design in the subject, a list of AI-default looks to avoid (cream + terracotta, SaaS-card kit, ALL-CAPS eyebrows, `→` on buttons), a two-pass plan (4-6 hex palette, type, layout, principles) then build, UI-copy rules (active voice, sentence case, errors that say how to fix). One-line "quality floor": responsive, visible keyboard focus, reduced motion, accessible colour; "take screenshots to review if your environment supports it". | Strong for marketing/hero pages; weak for dense desktop tool UI (it talks about heroes, landing pages) | Only "take screenshots" | One sentence, no criteria, no tooling |
| `figma:figma-design-to-code` (`figma/2.2.157/skills/figma-design-to-code/SKILL.md`, 28 lines) | Mandatory rules before `get_design_context`: request a screenshot as the visual target, reuse project components/tokens and Code Connect mappings instead of raw markup, translate absolute positioning into native layout, download every asset, verify each asset's geometry. | Yes, from a Figma frame | Visual match to the Figma screenshot | None |
| `figma:figma-generate-library` (343 lines) | Workflow for building a design system *in Figma* from code: scope contract (tokens only / one component / full library / reconciliation), variables with modes, scopes and code syntax, variant sets, bindings, one screenshot per phase. Its QA phase says "audit accessibility, naming, and bindings" but gives no criteria; accessibility sweeps are "follow-up work unless the scope requires them". | Figma side only | Figma screenshots | Mentioned, not specified |
| `figma:figma-use` (380 lines) | Plugin API rules for `use_figma` (return IDs, async page switching, font loading, 0-1 colours, auto-layout sizing enums, variable scopes). Mechanics, not design. | Figma side only | Structural evidence + screenshots | None |
| `figma:figma-code-connect` (528 lines) | Writing parserless `.figma.ts` Code Connect templates that map published Figma components to code snippets. Requires a published library and an Organization/Enterprise plan. | Mapping only | None | None |
| `superpowers` 7.0.0 (obra/superpowers, MIT) | Process skills: brainstorming, writing-plans, executing-plans, subagent-driven-development, TDD, systematic-debugging, requesting/receiving code review, git worktrees, **verification-before-completion** ("no completion claims without fresh verification evidence"). No UI-specific skill. | No | Generic evidence gate (useful as the rule that forces running the a11y/visual tests) | None |
| `mattpocock-skills` 1.3.1 | Engineering process: tdd, code-review (standards + spec axes), diagnosing-bugs, codebase-design, domain-modeling, research, implement, **prototype** (UI.md: 3-5 radically different variants on the real route behind `?variant=`, floating switcher bar). | `prototype` helps explore layouts | No | None |

**Gap.** None of the installed skills tells the agent how to make a component accessible (APG patterns, focus order, names), none runs an accessibility check, and none gives desktop-app layout rules (dialogs, forms, control sizes). Verification is either "match a Figma screenshot" or the generic evidence gate. That gap is what the rest of this report fills.

## 1b. Candidate skills, plugins and MCP servers

Researched 2026-10-20. No WebFetch/WebSearch in this session, so everything was fetched with `curl` from primary sources (GitHub repos and raw files, npm registry, official docs sites). Star counts and `pushed_at` come from the GitHub REST API on 2026-10-20. Nothing was installed.

#### The fact that shapes every verdict

purlis ships in **WKWebView on macOS**, not Chromium. Playwright MCP and Chrome DevTools MCP both drive a *browser* pointed at the Vite dev server. They test the React UI with Tauri IPC mocked, not the shipped webview. Tauri's own docs say macOS "has no WKWebView driver tool available" for `tauri-driver`. The supported route on macOS is WebdriverIO's `@wdio/tauri-service`, which runs an embedded WebDriver server inside the app. Source: https://raw.githubusercontent.com/tauri-apps/tauri-docs/v2/src/content/docs/develop/Tests/WebDriver/index.mdx

So the stack needs two layers:
1. Chrome-based agent tools for fast inner-loop UI and a11y checks against the dev server.
2. Real-webview E2E (WDIO tauri-service) to catch WebKit-specific breakage.

purlis also already has its own `browser_*` MCP tools and `theme_preview`. Anything below overlaps with those and should be compared against them, not just added.

#### Install syntax (official Claude Code docs)

- Official marketplace: `/plugin install <name>@claude-plugins-official`. Claude Code registers this marketplace on first interactive start. The in-session command opens a review panel and you pick the scope there. From a shell, use `claude plugin install`. Sources: https://code.claude.com/docs/en/discover-plugins.md and https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/README.md
- Other marketplaces: `/plugin marketplace add <owner/repo>` first, then `/plugin install <plugin>@<marketplace-name>`. Source: https://code.claude.com/docs/en/discover-plugins.md
- Remote MCP: `claude mcp add --transport http <name> <url>`. Local stdio MCP: `claude mcp add [options] <name> -- <command> [args...]`. Use `--scope user|project|local`. Source: https://code.claude.com/docs/en/mcp.md
- Security warning from Anthropic's marketplace README: "Make sure you trust a plugin before installing... Anthropic does not control what MCP servers, files, or other software are included in plugins." Being listed in the official marketplace is **not** a security audit of third-party plugins. https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/README.md
- `npx skills add ...` is the **Vercel `skills` CLI** (vercel-labs/skills, 35.8k stars). It is not part of Claude Code. Several vendors below use it. It is a third-party installer that writes skill files, so review what it writes. https://github.com/vercel-labs/skills

---

#### Candidates

##### 1. Chrome DevTools MCP + skills (Google): INSTALL (top pick for verifying)
- Owner: ChromeDevTools (Google). https://github.com/ChromeDevTools/chrome-devtools-mcp. 53.9k stars, pushed 2026-10-20. Also listed in claude-plugins-official as `chrome-devtools-mcp`.
- What it gives:
  - Puppeteer-driven Chrome.
  - `take_snapshot`, which returns the accessibility tree.
  - `lighthouse_audit` for accessibility, SEO and best-practices scores, including the DOM nodes each failing audit flags.
  - Performance traces and insights, network and console logs with source-mapped stacks.
  - `emulate`, which covers `colorScheme` dark/light, `forcedColors`, `prefersContrast`, `prefersReducedMotion`, CPU and network throttling, and viewport. This is exactly the matrix a world-class a11y desktop UI needs to check.
  - Memory heap-snapshot tools.
  - Tool reference: https://raw.githubusercontent.com/ChromeDevTools/chrome-devtools-mcp/main/docs/tool-reference.md
- The plugin ships these skills: `a11y-debugging`, `chrome-devtools`, `chrome-devtools-cli`, `debug-optimize-lcp`, `memory-leak-debugging`, `troubleshooting`, `cookie-debugging`. The `a11y-debugging` skill runs Lighthouse, then checks the a11y tree, focus, keyboard, tap targets and contrast. https://github.com/ChromeDevTools/chrome-devtools-mcp/tree/main/skills
- Caveats, from the README:
  - Google usage statistics are **on by default**. Opt out with `--no-usage-statistics`, or set `CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS`. It is also disabled when `CI` is set.
  - Performance tools send trace URLs to the CrUX API. Turn this off with `--no-performance-crux`.
  - It checks npm for updates.
  - It is Chrome-only, so it does not exercise WKWebView.
  - Source: https://raw.githubusercontent.com/ChromeDevTools/chrome-devtools-mcp/main/README.md
- Install, MCP plus skills. Pick one of these routes, not both. The README says to remove any earlier MCP install first. https://raw.githubusercontent.com/ChromeDevTools/chrome-devtools-mcp/main/docs/client-configurations.md
  ```
  /plugin install chrome-devtools-mcp@claude-plugins-official
  # or the vendor marketplace:
  /plugin marketplace add ChromeDevTools/chrome-devtools-mcp
  /plugin install chrome-devtools-mcp@chrome-devtools-plugins
  # MCP only, with telemetry off:
  claude mcp add chrome-devtools --scope user -- npx chrome-devtools-mcp@latest --no-usage-statistics --no-performance-crux
  ```

##### 2. Microsoft Playwright MCP / Playwright CLI + skills: MAYBE (pick one browser driver)
- Owner: Microsoft. MCP: https://github.com/microsoft/playwright-mcp (38.4k stars, pushed 2026-10-19). CLI: https://github.com/microsoft/playwright-cli (14.4k stars, pushed 2026-10-19). The MCP is also listed in claude-plugins-official as `playwright`; its `.mcp.json` runs `npx @playwright/mcp@latest`. https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/playwright
- What it gives:
  - Browser control driven by accessibility snapshots (`browser_snapshot`, a find-in-snapshot tool), with no vision model needed.
  - `--caps vision,pdf,devtools`, `--snapshot-boxes`, `--cdp-endpoint`.
  - Source: https://raw.githubusercontent.com/microsoft/playwright-mcp/main/README.md
- Microsoft itself now recommends **the CLI with skills over the MCP for coding agents**, because it is more token-efficient. https://raw.githubusercontent.com/microsoft/playwright-cli/main/README.md
- Overlap: this duplicates Chrome DevTools MCP and purlis's own `browser_*` tools. Its edge is cross-engine, since Playwright can run **WebKit**, the closest thing to WKWebView in a plain browser. That also makes it the natural home for `@axe-core/playwright` checks in real tests.
- Install:
  ```
  claude mcp add playwright -- npx @playwright/mcp@latest      # README form: claude mcp add playwright npx @playwright/mcp@latest
  # or the CLI with skills (recommended by Microsoft for coding agents):
  npm install -g @playwright/cli@latest && playwright-cli install --skills
  ```
- Verdict: if a second driver is wanted, take the **CLI with skills** and use it for WebKit-engine checks. Skip the MCP if Chrome DevTools MCP is installed.

##### 3. anthropics/skills (`example-skills`): MAYBE, only `webapp-testing`
- Owner: Anthropic. https://github.com/anthropics/skills. 182k stars, pushed 2026-10-09. The README calls these "for demonstration and educational purposes only".
- `webapp-testing`: native **Python** Playwright scripts plus a `with_server.py` helper. https://raw.githubusercontent.com/anthropics/skills/main/skills/webapp-testing/SKILL.md. It conflicts with the operator rule "no language mixing" (Python only as a test oracle), and Chrome DevTools MCP or Playwright CLI cover the same ground. **Skip.**
- `web-artifacts-builder` targets claude.ai artifacts, not an app. `theme-factory` and `canvas-design` target artifacts, slides and posters. None fit purlis. **Skip.**
- `frontend-design` is also here, and it is the one already installed from claude-plugins-official.
- Install (not recommended): `/plugin marketplace add anthropics/skills`, then `/plugin install example-skills@anthropic-agent-skills`. https://raw.githubusercontent.com/anthropics/skills/main/README.md

##### 4. Modern Web Guidance (Google Chrome team, also backed by Edge): INSTALL
- Owner: GoogleChrome. https://github.com/GoogleChrome/modern-web-guidance. 2.5k stars, pushed 2026-10-19, **preview release**. Listed in claude-plugins-official as `modern-web-guidance`.
- What it gives: a skill plus a search/retrieve CLI with curated, token-efficient guides on modern CSS and HTML. Topics include View Transitions, container queries, oklch, an accessibility section (error announcement, OS text scale), native switch, slider and checkbox, spatial navigation, and INP. It is **Baseline-aware**, which matters because the macOS webview is Safari/WebKit. https://raw.githubusercontent.com/GoogleChrome/modern-web-guidance/main/README.md
- Install: `/plugin install modern-web-guidance@claude-plugins-official`, or `npx modern-web-guidance@latest install`.

##### 5. React Aria MCP + Agent Skill (Adobe): INSTALL if purlis uses React Aria or shadcn's `aria` base; otherwise MAYBE
- Owner: Adobe. https://github.com/adobe/react-spectrum. 16k stars, pushed 2026-10-20.
- What it gives: `@react-aria/mcp` (docs MCP), an agent skill, and plain-markdown docs at https://react-aria.adobe.com/llms.txt. Sources: https://react-aria.adobe.com/ai.md and https://react-aria.adobe.com/llms.txt
- Install: `claude mcp add react-aria -- npx @react-aria/mcp@latest`. The page shows `claude mcp add react-aria npx @react-aria/mcp@latest`. Skill: `npx skills add https://react-aria.adobe.com`, which is the third-party Vercel CLI.
- Not checked: which headless library the purlis app actually uses. The app repo was not in reach from this workspace.

##### 6. shadcn/ui MCP + skill: MAYBE (only if purlis uses shadcn and has `components.json`)
- Owner: shadcn (shadcn-ui/ui, 126k stars, pushed 2026-10-19).
- MCP: browses, searches and installs registry items. https://ui.shadcn.com/docs/mcp.md
- Skill: runs `shadcn info --json` and enforces composition rules. It knows the base library (`base`, `radix` or `aria`) and Tailwind v4 theming with OKLCH. https://ui.shadcn.com/docs/skills.md
- Install: `npx shadcn@latest mcp init --client claude`, and for the skill, `npx skills add shadcn/ui`.
- Security: third-party registries pull remote code into the repo. Keep the registry list limited to `@shadcn` or vetted ones.

##### 7. Storybook MCP (`@storybook/addon-mcp`): MAYBE, high value only if purlis adopts Storybook
- Owner: Storybook. The old repo `storybookjs/mcp` is **archived** and has "moved to storybookjs/storybook as of Storybook v10.6.0". https://raw.githubusercontent.com/storybookjs/mcp/main/README.md. Current npm version is `@storybook/addon-mcp` 10.6.3, from `code/addons/mcp`. https://registry.npmjs.org/@storybook/addon-mcp/latest
- What it gives:
  - An MCP server at `http://localhost:6006/mcp`, in three toolsets.
  - Docs: `docs-list` and `docs-show`, built from the components manifest; React is supported.
  - Development: story-writing instructions and previews.
  - Testing: `test-run` with interaction **and a11y checks**.
  - Together these give a "self-healing loop" where the agent writes UI, writes stories, tests them and fixes them. Marked **preview**: "The API may change."
  - Source: https://raw.githubusercontent.com/storybookjs/storybook/next/docs/ai/mcp/overview.mdx
- Install: `npx storybook add @storybook/addon-mcp`, then `claude mcp add --transport http purlis-sb http://localhost:6006/mcp --scope project`. The docs suggest `npx mcp-add ...` instead, but that is a third-party helper (paoloricciuti/mcp-add); use the native command. There is also a Claude Code plugin, documented in the archived repo as `claude plugin marketplace add storybookjs/mcp@main` then `claude plugin install storybook@storybook`. Since the repo is archived, **unverified whether that marketplace is still current**.

##### 8. Tauri-specific agent tooling
- **`@wdio/tauri-service` (WebdriverIO), INSTALL as the real-webview E2E layer.** This is the officially recommended Tauri route and works on Windows, Linux **and macOS** through `tauri-plugin-wdio-webdriver` (embedded driver). It offers IPC mocking, log capture, and a browser mode against Vite. npm 1.5.0, repo `webdriverio/desktop-mobile`. https://raw.githubusercontent.com/tauri-apps/tauri-docs/v2/src/content/docs/develop/Tests/WebDriver/index.mdx and https://registry.npmjs.org/@wdio/tauri-service/latest. It is a test framework, not an agent skill. The agent runs it from the shell, and `@axe-core/webdriverio` (Deque) can run axe inside the real webview. Install command for the agent to run, not run here: `npm create wdio@latest ./`, then choose Desktop Testing and Tauri. Star count not verified: the GitHub API was rate-limited.
- **hypothesi/mcp-server-tauri, SKIP for now (third-party, unvetted).** 325 stars, pushed 2026-10-13, MIT. It provides screenshots, DOM, IPC monitoring and logs from the *running* Tauri app, through a Rust plugin `tauri-plugin-mcp-bridge` that you add to the app. Its README says: "This MCP was developed using agentic coding tools. It may contain bugs." It recommends a third-party installer (`npx @a1st/aix`). Adding an agent bridge crate to a security-sensitive app is a real attack-surface change, and this needs a security review before any trial. https://raw.githubusercontent.com/hypothesi/mcp-server-tauri/main/README.md
- `tauri-apps/tauri-plugin-mcp` does **not exist** (GitHub 404). No official Tauri MCP was found.

##### 9. Vercel agent-skills: MAYBE for `web-design-guidelines` (vendor it pinned); SKIP the rest
- Owner: vercel-labs. https://github.com/vercel-labs/agent-skills. 32.7k stars, pushed 2026-10-14. Skills include `web-design-guidelines`, `react-best-practices`, `composition-patterns`, `react-view-transitions`, `writing-guidelines`, and deploy-to-vercel skills.
- `web-design-guidelines` reviews UI against Vercel's Web Interface Guidelines (a11y, focus, forms, motion and so on). **Security flag:** on every run it tells the agent to fetch fresh instructions from `https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md`. That makes it remote, unpinned instructions, a prompt-injection and supply-chain path. https://raw.githubusercontent.com/vercel-labs/agent-skills/main/skills/web-design-guidelines/SKILL.md. If adopted, vendor a pinned copy of `command.md` (repo: https://github.com/vercel-labs/web-interface-guidelines, 1k stars).
- `react-best-practices` has 70 rules and is aimed at React/**Next.js** (server, waterfalls, bundles). Much of it does not apply to a Tauri SPA.
- No Claude plugin marketplace was found in this repo (`.claude-plugin` returns 404). Install: `npx skills add vercel-labs/agent-skills` (Vercel CLI).

##### 10. Deque axe MCP Server: SKIP (paid); use free axe-core libraries instead
- Owner: Deque (official). https://github.com/dequelabs/axe-mcp-server-public. npm `axe-mcp-server` 1.7.0. It provides two tools, `analyze` and `remediate`. Its requirements are "Docker", a "Subscription to Axe DevTools for Web" and an API key. The README notes "Requires a paid Axe DevTools for Web subscription." It is proprietary. https://raw.githubusercontent.com/dequelabs/axe-mcp-server-public/main/README.md
- The free route is Deque's own MPL-licensed `axe-core` 4.14 with `@axe-core/playwright` or `@axe-core/webdriverio`, run inside tests (npm search: https://registry.npmjs.org/-/v1/search?text=axe%20mcp). Lighthouse a11y in Chrome DevTools MCP is built on axe as well.

##### 11. Context7 (Upstash): MAYBE
- Owner: Upstash. https://github.com/upstash/context7. 63k stars, pushed 2026-10-20. Listed in claude-plugins-official as `context7`, pointing at the remote `https://mcp.context7.com/mcp`.
- What it gives: version-specific library docs on demand. It works anonymously and an API key raises the rate limits. It also offers a "CLI + Skills" mode (`npx ctx7 setup`). https://raw.githubusercontent.com/upstash/context7/master/README.md
- Caveats:
  - Every query goes to a third-party hosted service.
  - The content is community-indexed, so it is not primary-source.
  - For the libraries purlis actually uses, the vendor `llms.txt` and MCPs (React Aria, Base UI, shadcn) are more authoritative.
- Install: `/plugin install context7@claude-plugins-official`.

##### 12. Base UI / Radix `llms.txt`: Base UI USE (no install); Radix unverified
- Base UI (mui/base-ui, 11.2k stars, pushed 2026-10-20) publishes https://base-ui.com/llms.txt. It is per-component `.md` docs written for **Tailwind v4**, which is a direct fit. No MCP or skill is mentioned in it. Use: point the agent at the `.md` URLs, or add them to CLAUDE.md.
- Radix: `https://www.radix-ui.com/llms.txt` returned an HTML page, not llms.txt, so it is **unverified / likely absent**. Radix primitives were not checked further; the repo API call was rate-limited.

##### 13. Other UI-relevant entries in claude-plugins-official (from its marketplace.json)
Source: https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json (315 plugins).
- `typescript-lsp` and `rust-analyzer-lsp` (Anthropic): LSP code intelligence for the TS webview and the Rust side. **MAYBE.** They need `typescript-language-server` and `rust-analyzer` on PATH.
- `superdesign` (Superdesign dev, Inc., third party): canvas-based UI drafts through their hosted service. **SKIP**, since it overlaps Figma and is a third-party SaaS.
- `browser-use` (third party, can use their cloud): **SKIP**, because it overlaps purlis browser tools.
- `figma`, `frontend-design` and `superpowers` are already installed.

---

#### Recommended set

| Pick | Why | Install |
|---|---|---|
| Chrome DevTools MCP + skills | Lighthouse a11y, a11y tree, emulation of contrast, forced colors, reduced motion and dark mode, perf traces | `/plugin install chrome-devtools-mcp@claude-plugins-official` (turn off telemetry with flags) |
| Modern Web Guidance | Modern CSS/a11y patterns with Baseline awareness (WebKit webview) | `/plugin install modern-web-guidance@claude-plugins-official` |
| `@wdio/tauri-service` + `@axe-core/webdriverio` | The only real-WKWebView E2E path on macOS; axe in the shipped engine | `npm create wdio@latest ./` (a repo change, made through a ticket) |
| React Aria MCP / Base UI llms.txt / shadcn skill | Whichever headless library purlis actually uses; vendor docs beat Context7 | see sections 5, 6, 12 |
| Storybook addon-mcp | Only if purlis adopts Storybook; gives the best build, test and a11y loop for components | section 7 |

Skip: anthropics `webapp-testing` (Python), Deque axe MCP (paid), hypothesi Tauri MCP (unvetted bridge crate), superdesign and browser-use. Skip Playwright MCP if Chrome DevTools MCP is installed; Playwright CLI is the better pick for WebKit-engine checks.

#### Unverified / blocked
- The purlis app's actual UI dependencies (shadcn? React Aria? Base UI? Storybook?). The app repo was not reachable from this workspace.
- Star counts for webdriverio/desktop-mobile, dequelabs/axe-mcp-server-public and radix-ui/primitives: GitHub API rate limit.
- Whether the Storybook Claude Code plugin marketplace (`storybookjs/mcp`) still works after the repo was archived.
- Radix `llms.txt`: returned HTML.

---

# 2. Accessible headless primitives

Researched 2026-10-20. The target is one accessible headless library under every component of a Tauri 2 + React + TS + Tailwind v4 webview themed with CSS variables.

### How this was sourced (read first)

- This run had no WebFetch/WebSearch. Everything below came from `curl` against hosts the sandbox allowed: `registry.npmjs.org`, `api.github.com`, `github.com` / `raw.githubusercontent.com` (each project's own repo, docs sources included) and, late in the run, `base-ui.com`.
- **Blocked hosts:** react-spectrum.adobe.com / react-aria.adobe.com, radix-ui.com, ark-ui.com, zagjs.com, headlessui.com, bundlephobia.com, pkg-size.dev, unpkg, jsdelivr. Where a doc site was blocked I read the same page's source file in the project's GitHub repo and cite the GitHub URL. Facts that exist only on a blocked site are marked **unverified (host blocked)**.
- GitHub stars and issue counts come from the GitHub REST API, read 2026-10-20 (`https://api.github.com/repos/<owner>/<repo>`, plus `search/issues?q=repo:<r>+type:issue+state:open` for issues alone).
- Version dates come from the `time` field of each npm packument (`https://registry.npmjs.org/<pkg>`).

---

### 1. Comparison table

| | React Aria Components (RAC) | Radix Primitives | Ark UI (Zag.js) | Headless UI | Base UI |
|---|---|---|---|---|---|
| npm package | `react-aria-components` | `radix-ui` (+ `@radix-ui/react-*`) | `@ark-ui/react` | `@headlessui/react` | `@base-ui/react` |
| Latest stable (date) | 1.22.1 (2026-10-09) | radix-ui 1.8.1 (2026-10-12) | 5.39.3 (2026-10-05) | 2.2.10 (2026-04-07) | 1.9.0 (2026-10-09) |
| Stable releases in 2026 | 11 | 13 (all since 2026-06-06) | 19 | **1** | 9 (monthly minors) |
| GitHub stars | 15,967 (whole `adobe/react-spectrum` monorepo) | 19,411 | 5,426 (ark) / 5,235 (zag) | 28,783 | 11,185 |
| Open issues (excl. PRs) | 524 (monorepo, includes Spectrum) | 124 | 0 (ark) | 89 | 300 |
| Last push to default branch | 2026-10-20 | 2026-10-14 | 2026-10-19 | 2026-04-13 | 2026-10-20 |
| Backing org | Adobe | WorkOS (copyright holder) | Chakra UI org (`chakra-ui`) | Tailwind Labs | MUI (`mui/base-ui`); team from Radix, MUI and Floating UI |
| Licence | Apache-2.0 | MIT | MIT | MIT | MIT |
| Tree view | **Yes**: `Tree` (role `treegrid`) + `NavigationTree` (alpha) | No | **Yes**: `TreeView` (role `tree`) | No | No (open request) |
| Grid / table | **Yes**: `Table`, `GridList` | No | No | No | No |
| Toolbar | Yes | Yes | No | No | Yes |
| Menubar | No component | Yes | No | No | Yes |
| Combobox | Yes (`ComboBox`, `Autocomplete`) | **No** | Yes | Yes | Yes (`Combobox`, `Autocomplete`) |
| Date / time pickers | Yes (DatePicker, DateRangePicker, Calendar, DateField, TimeField) | No | Yes (DatePicker, DateInput) | No | No |
| Drag and drop | **Yes** (ListBox, GridList, Table, Tree; mouse, touch, keyboard, screen reader) | No | No (none found in TreeView docs) | No | No (Tree/DnD proof of concept PR only) |
| Virtualization | **Built in** (`Virtualizer` with List, Grid, Waterfall and Table layouts) | None | Via external virtualizer (TreeView docs example) | Combobox `virtual` prop only (TanStack Virtual) | Via TanStack Virtual (Combobox example) |
| i18n / RTL | 30+ locales built in, `I18nProvider`, RTL keyboard handling, date and number formatting | `DirectionProvider` | `LocaleProvider` (locale + direction) | Nothing found in repo sources; unverified | `DirectionProvider` |
| Styling hooks | `className`/`style` as functions of state, `data-*` states, render-prop children, CSS vars, **official Tailwind v4 plugin** | `className`, `data-state` etc., `asChild` | `data-scope` / `data-part` / `data-state`, `className`, `asChild` | `data-*` states (v2), render props, CSS vars | `className`/`style` as functions of state, `data-*` states, CSS vars, `render` prop |
| npm unpacked size (not a bundle size) | 6.94 MB | 105 KB (meta) / 107 KB (dialog) | 3.34 MB | 1.02 MB | 10.38 MB |

Sources for every cell are in the sections below.

---

### 2. APG pattern coverage, by library

#### React Aria Components
Component sources (`packages/react-aria-components/src`, https://github.com/adobe/react-spectrum/tree/main/packages/react-aria-components/src) include: Autocomplete, Breadcrumbs, Button, Calendar, Checkbox, Color* (Area, Field, Picker, Slider, Swatch, SwatchPicker, Wheel), ComboBox, DateField, DatePicker, Dialog, Disclosure, DragAndDrop, DropZone, FileTrigger, Form, GridList, Link, ListBox, Menu, Meter, Modal, NavigationTree, NumberField, Popover, ProgressBar, RadioGroup, SearchField, Select, Separator, Sheet, Slider, Switch, Table, Tabs, TagGroup, TextField, Toast, ToggleButton, ToggleButtonGroup, TokenField, Toolbar, Tooltip, Tree, Virtualizer.
- Docs pages (source) include CheckboxGroup, DateRangePicker, DisclosureGroup, RangeCalendar and TimeField: https://github.com/adobe/react-spectrum/tree/main/packages/dev/s2-docs/pages/react-aria
- **APG patterns covered:** dialog and alert dialog, menu (menu button, submenus), listbox, combobox, tabs, tree (as a single-column **treegrid**), grid (Table, GridList), toolbar, slider, radio group, checkbox, switch, tooltip, disclosure and accordion, breadcrumb, link, meter, button and toggle button, spinbutton (NumberField). Beyond APG it covers date and time pickers, color pickers, drag and drop, toasts and a drop zone.
- `useTree` sets `gridProps.role = 'treegrid'` and calls itself "a single column treegrid component": https://github.com/adobe/react-spectrum/blob/main/packages/react-aria/src/tree/useTree.ts
- **Gaps:** no menubar component (no `Menubar.tsx` in src). The file-tree component is a treegrid, not the APG `tree` role. That is a legitimate APG pattern and is what lets rows hold interactive content (see `keyboardNavigationBehavior="tab"` in the Tree docs), but it is a different role from Ark's.
- Docs say it follows WAI-ARIA and APG and is tested with VoiceOver (macOS, iOS), JAWS, NVDA and TalkBack: https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/quality.mdx

#### Radix Primitives
Packages (https://github.com/radix-ui/primitives/tree/main/packages/react) and documented components (https://github.com/radix-ui/website/tree/main/data/primitives/docs/components): accordion, alert-dialog, aspect-ratio, avatar, checkbox, collapsible, context-menu, dialog, dropdown-menu, form, hover-card, label, menubar, navigation-menu, one-time-password-field, password-toggle-field, popover, progress, radio-group, scroll-area, select, separator, slider, switch, tabs, toast, toggle-group, toggle, toolbar, tooltip.
- **APG patterns covered:** dialog and alert dialog, menu button and menubar, listbox (only inside `Select`), tabs, toolbar, slider, radio group, checkbox, switch (button-based), tooltip, accordion and disclosure.
- **Gaps:** no combobox. The `[New Primitive] Combobox` request has 995 reactions and is closed with no primitive shipped: https://github.com/radix-ui/primitives/issues/1342. Also missing: tree (`[New Primitive] Tree` is closed, https://github.com/radix-ui/primitives/issues/1456), grid or table, date pickers, drag and drop, virtualization (https://github.com/radix-ui/primitives/issues/894 is closed).
- Docs say "follow the WAI-ARIA authoring practices guidelines" and that "basic keyboard support" is provided: https://github.com/radix-ui/website/blob/main/data/primitives/docs/overview/accessibility.mdx

#### Ark UI (Zag.js)
Components (https://github.com/chakra-ui/ark/tree/main/packages/react/src/components): accordion, angle-slider, avatar, carousel, checkbox, clipboard, collapsible, color-picker, combobox, date-input, date-picker, dialog, drawer, editable, field, fieldset, file-upload, floating-panel, focus-trap, hover-card, image-cropper, json-tree-view, listbox, marquee, menu, navigation-menu, number-input, pagination, password-input, pin-input, popover, progress, qr-code, radio-group, rating-group, scroll-area, segment-group, select, signature-pad, slider, splitter, steps, switch, tabs, tags-input, timer, toast, toc, toggle, toggle-group, tooltip, tour, tree-view.
- **APG patterns covered:** dialog, menu (including context menu), listbox, combobox, tabs, **tree view**, slider, radio group, checkbox, switch, tooltip, accordion and disclosure, spinbutton (number-input), carousel.
- Tree View docs: "Complies with the Tree View WAI-ARIA design pattern". It supports lazy loading, lazy mount, virtualization, a checkbox tree and inline rename on F2: https://github.com/chakra-ui/ark/blob/main/website/src/content/pages/components/tree-view.mdx
- The Zag connect code sets `role: "tree"`, `"treeitem"` and `aria-level`: https://github.com/chakra-ui/zag/blob/main/packages/machines/tree-view/src/tree-view.connect.ts
- **Gaps:** no toolbar, no menubar, no grid or table, no drag and drop. The Tree View docs page has no mention of drag, reorder or DnD.

#### Headless UI
Exports (https://github.com/tailwindlabs/headlessui/blob/main/packages/@headlessui-react/src/index.ts): Button, Checkbox, CloseButton, Combobox, DataInteractive, Description, Dialog, Disclosure, Field, Fieldset, FocusTrap, Input, Label, Legend, Listbox, Menu, Popover, Portal, RadioGroup, Select (native), Switch, Tabs, Textarea, Transition.
- Tooltip exists in source but its export is commented out: `// TODO: Enable when ready`.
- **APG patterns covered:** dialog, menu button, listbox, combobox, tabs, radio group, checkbox, switch, disclosure.
- **Gaps:** no tooltip, slider, toolbar, menubar, tree, grid, date picker or drag and drop. Virtualization is limited to the Combobox `virtual` prop added in 2.0.0: https://github.com/tailwindlabs/headlessui/blob/main/packages/@headlessui-react/CHANGELOG.md

#### Base UI
Exports (https://github.com/mui/base-ui/blob/master/packages/react/src/index.ts, and the component list at https://base-ui.com/llms.txt): accordion, alert-dialog, autocomplete, avatar, button, checkbox, checkbox-group, collapsible, combobox, context-menu, dialog, drawer, field, fieldset, form, input, menu, menubar, meter, navigation-menu, number-field, otp-field, popover, preview-card, progress, radio, radio-group, scroll-area, select, separator, slider, switch, tabs, toast, toggle, toggle-group, toolbar, tooltip.
- **APG patterns covered:** dialog and alert dialog, menu, menubar, context menu, listbox (Select), combobox, tabs, toolbar, slider, radio group, checkbox, switch, tooltip, accordion and disclosure, meter, spinbutton (number-field).
- Docs: "adhere to WAI-ARIA design patterns": https://base-ui.com/react/overview/about.md
- **Gaps:** no tree. "Add Tree component" is open (https://github.com/mui/base-ui/issues/3327), and so is a "Tree Select" discussion (https://github.com/mui/base-ui/issues/4790). An open PR is marked "[DO NOT MERGE] Virtualizer + drag engine build for the Tree proof of concept" (https://github.com/mui/base-ui/pull/5559), so a tree and DnD are being explored but have not shipped. Also missing: grid or table and date pickers.

---

### 3. Keyboard, focus, interactions, i18n

**React Aria Components (strongest here)**
- `FocusScope` provides `contain` (trap), `restoreFocus` and `autoFocus`: https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/FocusScope.mdx
- There are docs pages for `FocusRing`, `useFocusRing`, `useFocusVisible`, `useFocusWithin`, `usePress`, `useLongPress`, `useHover`, `useMove`, `useKeyboard`, `useLandmark` and `useClipboard` (file list at https://github.com/adobe/react-spectrum/tree/main/packages/dev/s2-docs/pages/react-aria).
- From the quality guide (https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/quality.mdx):
  - `data-pressed` is "like :active, but removed when the pointer is dragged off".
  - `data-hovered` is "not applied on touch devices".
  - `data-focus-visible` is "like :focus-visible, but not on input click or programmatic focus".
  - i18n: "localized strings for 30+ languages, handles dates and numbers in many calendar and numbering systems, and supports right-to-left interactions (e.g. keyboard navigation)"; `I18nProvider` and `useLocale` give `{locale, direction}`.
- A bundler plugin can strip the locale strings you don't need: https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/frameworks.mdx
- Tree keyboard navigation: arrow keys by default, or `keyboardNavigationBehavior="tab"` for rows that hold inputs: https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/Tree.mdx
- I did not find RAC typeahead documented on a page I read. Typeahead in ListBox, Menu and Tree is **unverified (docs host blocked)**.

**Radix**
- Has `focus-scope`, `focus-guards`, `roving-focus`, `dismissable-layer` and `direction` packages: https://github.com/radix-ui/primitives/tree/main/packages/react
- Menu implements typeahead (`typeahead` appears in https://github.com/radix-ui/primitives/blob/main/packages/react/menu/src/menu.tsx).
- `DirectionProvider` provides global reading direction: https://github.com/radix-ui/website/blob/main/data/primitives/docs/utilities/direction-provider.mdx
- Focus-visible styling is left to CSS. The June 2026 release added `focusVisible` support for slider thumbs: https://github.com/radix-ui/website/blob/main/data/primitives/docs/overview/releases.mdx
- No press-normalization layer comparable to `usePress` was found.

**Ark UI**
- Ships `focus-trap` and `@zag-js/focus-visible` (dependency list of `@ark-ui/react` 5.39.3, https://registry.npmjs.org/@ark-ui/react).
- `LocaleProvider` sets locale and direction, defaulting to `en-US` / `ltr`: https://github.com/chakra-ui/ark/blob/main/website/src/content/pages/utilities/locale.mdx
- Pulls in `@internationalized/date` (Adobe's) for the date pickers (same npm dependency list).

**Headless UI**
- v2 depends on `@react-aria/focus` and `@react-aria/interactions` (Adobe) plus `@floating-ui/react` and `@tanstack/react-virtual` (npm dependencies of 2.2.10, https://registry.npmjs.org/@headlessui/react). Its focus and hover states therefore rest on React Aria's hooks.
- Exports a `FocusTrap`.
- RTL support: **unverified (headlessui.com blocked)**.

**Base UI**
- Docs say components "manage focus automatically", with `initialFocus` / `finalFocus` props; visible focus is left to the developer through `:focus` / `:focus-visible`; arrow keys, alphanumeric keys, Home, End, Enter and Esc are supported per APG: https://base-ui.com/react/overview/accessibility.md
- `DirectionProvider` enables RTL; `CSPProvider` applies a nonce to inline style and script tags: https://base-ui.com/llms.txt
- The CSP nonce is relevant to a Tauri webview with a strict CSP.

---

### 4. Styling under Tailwind v4 and CSS variables

All five are unstyled.

- **RAC.** `className` and `style` accept functions of state. `data-*` states work as Tailwind `data-[selected]:` modifiers. The official `tailwindcss-react-aria-components` plugin supports v4 (`@plugin "tailwindcss-react-aria-components";`) and gives short modifiers such as `selected:` and `pressed:`. CSS variables such as `--trigger-width` can be used as `w-(--trigger-width)`. Default class names follow `react-aria-*`. Source: https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/styling.mdx
- **Base UI.** `className` and `style` accept functions of state. Data attributes such as `[data-checked]` and `data-starting-style` / `data-ending-style` handle animation. CSS variables include `--available-height` and `--transform-origin`. Tailwind examples are written for v4. Sources: https://base-ui.com/react/handbook/styling.md, https://base-ui.com/llms.txt
- **Radix.** `className` on every part, state in `data-state`, composition through `asChild`. Source: https://github.com/radix-ui/website/blob/main/data/primitives/docs/guides/styling.mdx
- **Ark.** Parts are identified by `data-scope` / `data-part`, state by `data-state`, and `className` is accepted. Some components expose CSS variables. Source: https://github.com/chakra-ui/ark/blob/main/website/src/content/pages/guides/styling.mdx
- **Headless UI.** v2 added `data-*` attributes as an alternative to `data-headlessui-state`, and the CSS variables `--button-width` / `--input-width`. Source: https://github.com/tailwindlabs/headlessui/blob/main/packages/@headlessui-react/CHANGELOG.md

All five work fine with token-driven CSS variables, since none ships CSS that would fight the tokens. RAC and Base UI are the most ergonomic for Tailwind, because state-function `className` and `data-*` modifiers cover every state. RAC is the only one with a first-party Tailwind v4 plugin.

---

### 5. Maintenance

- **RAC.** Monthly-ish minors: 1.17.0 (2026-04-14), 1.18.0 (05-28), 1.19.0 (06-18), 1.20.0 (07-31), 1.21.0 (09-01), 1.22.0 (10-08), 1.22.1 (10-09). Source: https://registry.npmjs.org/react-aria-components. The repo is Adobe's monorepo, also used for React Spectrum, and was pushed 2026-10-20 (https://api.github.com/repos/adobe/react-spectrum).
- **Radix.**
  - Repo copyright is "WorkOS": https://github.com/radix-ui/primitives/blob/main/README.md
  - On npm, `radix-ui` 1.4.3 shipped on 2025-08-13 and the next release was 1.5.0 on **2026-06-06**. That is a ~10-month gap with no stable release.
  - Since then the cadence has been dense: 1.6.x in June and July, 1.7.0 on 2026-10-05, 1.8.0 / 1.8.1 on 2026-10-12. Source: https://registry.npmjs.org/radix-ui. The website changelog shows the same gap, jumping from "August 13, 2025" to "June 6, 2026": https://github.com/radix-ui/website/blob/main/data/primitives/docs/overview/releases.mdx
  - Recent work is mostly fixes plus tree-shaking and subpath entry points (`radix-ui/accordion`): https://github.com/radix-ui/primitives/blob/main/packages/react/radix-ui/CHANGELOG.md
  - I found **no official WorkOS or Radix statement** about reduced maintenance in the repo, README or changelog. I did not find one via issue search either; the one "not maintained?" issue is about an outdated changelog page: https://github.com/radix-ui/primitives/issues/2897.
  - Base UI's About page says it comes "From the creators of Radix, Material UI, and Floating UI" and lists Colm Tuite among the team: https://base-ui.com/react/overview/about.md. So some of Radix's original people now work on Base UI. That is my inference from the team list, not a statement either project makes.
  - Claims that Radix is "less maintained" are therefore backed by the 2025–26 release gap, not by any official statement.
- **Ark / Zag.** Highest release frequency: 19 stable `@ark-ui/react` releases in 2026; 5.39.3 on 2026-10-05; a `6.0.0-next.0` dist-tag exists. Source: https://registry.npmjs.org/@ark-ui/react. `@zag-js/tree-view` 1.46.0 shipped 2026-10-11 (https://registry.npmjs.org/@zag-js/tree-view). The organization is `chakra-ui`; a funding entity behind it is **unverified**.
- **Headless UI.** One release in 2026 (2.2.10 on 2026-04-07; previous was 2.2.9 on 2025-09-25), and the last push to the repo was 2026-04-13. Sources: https://registry.npmjs.org/@headlessui/react, https://api.github.com/repos/tailwindlabs/headlessui. Maintenance is in a slow phase.
- **Base UI.**
  - **v1.0.0 stable shipped 2025-12-11.** That is the date of the v1.0.0 release notes (https://base-ui.com/react/overview/releases/v1-0-0.md, listed in https://base-ui.com/llms.txt) and of the npm `time` entry for `@base-ui/react@1.0.0`.
  - The package was renamed from `@base-ui-components/react`, which npm marks as deprecated with "Package was renamed to @base-ui/react".
  - Since then there has been a minor every month: 1.1.0 (Jan 15) through 1.9.0 (Oct 9, 2026). Source: https://registry.npmjs.org/@base-ui/react.

---

### 6. Licence

- RAC: **Apache-2.0** (permissive, with a patent grant; NOTICE-file obligations apply when redistributing). Sources: https://registry.npmjs.org/react-aria-components, https://api.github.com/repos/adobe/react-spectrum
- Radix, Ark, Zag, Headless UI and Base UI: **MIT**. Sources: the npm `license` field and the GitHub API `license.spdx_id` for each repo.

---

### 7. Bundle size

- **No primary-source gzip or tree-shaken numbers could be obtained.** bundlephobia, pkg-size, unpkg and jsdelivr were all blocked, and none of the projects' docs that I could read publish per-component kB figures.
- **Unverified (host blocked):** https://bundlephobia.com/package/react-aria-components, https://bundlephobia.com/package/@base-ui/react, https://bundlephobia.com/package/radix-ui, https://bundlephobia.com/package/@ark-ui/react, https://bundlephobia.com/package/@headlessui/react
- What *is* verified:
  - npm `dist.unpackedSize` (install footprint, **not** shipped bytes): RAC 6,935,582 B; `@base-ui/react` 10,377,319 B; `@ark-ui/react` 3,339,249 B; `@headlessui/react` 1,017,854 B; `radix-ui` 105,530 B (the meta package only; each primitive is a separate package, e.g. `@radix-ui/react-dialog` 107,250 B). Source: https://registry.npmjs.org/<pkg>
  - Every library supports per-component imports, so only the parts you import are bundled:
    - RAC subpaths such as `react-aria-components/TextField` (quality.mdx above)
    - `@base-ui/react/menu`; Base UI's `package.json` has `sideEffects: false` (npm metadata)
    - `@ark-ui/react/dialog` (https://github.com/chakra-ui/ark/blob/main/README.md)
    - Radix per-primitive subpaths and `/* @__PURE__ */` tree-shaking fixes, July 2026 (Radix releases.mdx above)
  - RAC's locale strings for 30+ languages are by default part of the bundle. Its bundler plugin trims them to the locales you support (frameworks.mdx above).
- Expect RAC to cost the most per component, because it is the most feature-complete. This is **not measured here**. Measure it in purlis's own Vite build with `rollup-plugin-visualizer` before committing, if bytes matter. In a desktop Tauri app loading local assets, a few tens of kB are unlikely to be felt.

---

### 8. Tree and virtualization: the deciding requirement for a dev tool

| | File-tree component | Virtualized tree | DnD in tree | Virtualized list/table |
|---|---|---|---|---|
| RAC | `Tree` (treegrid, selection, sections, links, `TreeLoadMoreItem`) | Yes: Tree is virtualization-aware (`isVirtualized` via `CollectionRendererContext`, https://github.com/adobe/react-spectrum/blob/main/packages/react-aria-components/src/Tree.tsx), using the `Virtualizer` + `ListLayout` pair | **Yes**: `dragAndDropHooks` with "mouse, touch, keyboard, and screen reader" (Tree.mdx) | `Virtualizer` with ListLayout, GridLayout, WaterfallLayout and TableLayout (https://github.com/adobe/react-spectrum/blob/main/packages/dev/s2-docs/pages/react-aria/Virtualizer.mdx) |
| Ark | `TreeView` (role tree, lazy loading, checkbox tree, F2 rename) | Yes, with an external virtualizer: "Pass `scrollToIndexFn` to enable keyboard navigation within the virtualized list" (tree-view.mdx) | Not documented | Not built in; an external library is expected |
| Base UI | None (issue #3327 open; PoC PR #5559) | n/a | n/a | Combobox with `@tanstack/react-virtual` (https://base-ui.com/react/components/combobox.md, section "Virtualized") |
| Headless UI | None | n/a | n/a | Combobox `virtual` prop only |
| Radix | None | n/a | n/a | None |

---

### 9. Recommendation: React Aria Components

Choose **React Aria Components** as the single primitives layer.

Reasons:
1. **It is the only option that covers what a dev tool needs from one library.**
   - A file tree with keyboard navigation, multi-select, drag and drop, **and** virtualization.
   - Tables and grid lists for logs, diffs and lists of sessions.
   - A toolbar, combobox and autocomplete for a command palette, plus tabs, dialogs and menus.
   - Date and time fields.

   Every other candidate would force a second library for the tree or the grid (Base UI, Radix, Headless UI) or for DnD, toolbar and table (Ark). That defeats "one library every component sits on".
2. **Interaction quality.** Press, hover and focus-visible are normalized across mouse, touch and keyboard (`usePress`, `data-focus-visible` that is not set on programmatic focus). `FocusScope` handles contain and restore. Assistive-technology testing across the five major screen readers is documented. Headless UI itself builds on React Aria's focus and interaction packages.
3. **Fit with Tailwind v4 and tokens.** State-function `className`, `data-*` states, a first-party Tailwind v4 plugin and CSS-variable outputs (`--trigger-width`) map directly onto a token-driven theme.
4. **Maintenance and backing.** Adobe-backed, a minor roughly monthly through 2026, and Adobe's own Spectrum 2 design system is built on the same code.
5. **i18n and RTL** are first-class (30+ locales, RTL keyboard handling), and the locale plugin keeps bundles lean.

Costs to accept, or to watch:
- **Apache-2.0, not MIT.** It is permissive, but keep the NOTICE/licence text in the app's third-party notices.
- **The API is heavier.** Collections, `Key`/`Selection` sets, `id`-keyed items and layouts take more learning than Radix-style parts. Mitigation: wrap each primitive once in purlis's own component layer, which you want anyway for tokens.
- **Probably the largest per-component JS** (not measured here; see §7). Measure in the Vite build; in a local Tauri webview this is unlikely to be felt.
- **Tree is a treegrid.** That is correct for rows with inline controls, but it announces differently from an APG `tree`. `NavigationTree` (alpha) exists for pure navigation.
- No menubar component. A Toolbar of MenuTriggers covers app menus, and Tauri provides the native menubar anyway.

Runner-up: **Base UI.** It has the cleanest modern API, very similar Tailwind ergonomics, MIT licence, a strong team and monthly releases since v1.0.0 (2025-12-11). Pick it only if purlis drops the virtualized, drag-and-drop file tree and table from the primitives layer, or is willing to wait for its Tree (issue #3327, PoC #5559).

Not recommended:
- **Ark UI.** It has a good TreeView, but no toolbar, grid, table or DnD, and its `data-scope`/`data-part` styling is less direct with Tailwind.
- **Radix.** No combobox, tree or grid, and the 10-month release gap in 2025–26.
- **Headless UI.** The smallest component set, and only one release in 2026.

---

# 3. Design tokens and theming architecture

Researched 2026-10-20. Primary sources only; every claim has its URL. npm facts come from the registry JSON (`https://registry.npmjs.org/<pkg>`), read on 2026-10-20. When a docs site was blocked, I read the same docs in the project's GitHub repo (raw.githubusercontent.com / `gh api`). VS Code and Zed are out of scope (already done).

---

### 1. W3C Design Tokens Community Group (DTCG) format

#### Status
- **2025.10 is the first stable version.** The Technical Reports index lists "2025.10 — Stable — 2025-10-28". The earlier editors' drafts are dated 2025-07-21, 2022-06-14 and 2021-09-23. https://www.designtokens.org/technical-reports/
- Announcement: "Design tokens specification reaches first stable version", W3C CG blog, 2025-10-28. https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/
- Document type: **"Final Community Group Report, 28 October 2025"**, published under the W3C Community Final Specification Agreement (FSA). "It is not a W3C Standard nor is it on the W3C Standards Track." It was "published by the DTCG as a Candidate Recommendation following the definitions provided by the W3C process … This specification is considered stable. Further updates will be provided in superseding specifications." https://www.designtokens.org/TR/2025.10/
- 2025.10 has three modules (https://www.designtokens.org/TR/2025.10/):
  - **Format**: https://www.designtokens.org/TR/2025.10/format/
  - **Color**: https://www.designtokens.org/TR/2025.10/color/
  - **Resolver** (themes/modes): https://www.designtokens.org/TR/2025.10/resolver/. It has the **same status** as the others: a Final CG Report of 28 October 2025, "considered stable". So the resolver is part of the stable 2025.10 release, not a separate draft.
- The living draft at https://www.designtokens.org/TR/drafts/ is labelled "Draft Community Group Report 08 September 2026 … This is a preview. Do not attempt to implement this version."
- JSON Schemas are published at `https://www.designtokens.org/schemas/2025.10/format.json` and `…/resolver.json`. The resolver examples cite them in `$schema`; Terrazzo's docs cite them too. https://www.designtokens.org/TR/2025.10/resolver/ , https://github.com/terrazzoapp/terrazzo/blob/main/www/src/pages/docs/guides/resolvers.md

#### What the Format module defines (https://www.designtokens.org/TR/2025.10/format/)
- **File**: JSON. MIME type `application/design-tokens+json` (SHOULD); tools MUST also accept `application/json`. Extensions `.tokens` / `.tokens.json` (§4).
- **Token**: an object with `$value` (required), plus optional `$type`, `$description`, `$extensions` and `$deprecated` (§5). Names MUST NOT start with `$` or contain `{`, `}` or `.` (§5.1.1).
- **`$type` is mandatory in effect.** A token gets its type directly, from a parent group's `$type`, or by aliasing a typed token. "If no explicit type has been set for a token, tools MUST consider the token invalid and not attempt to infer" one (§8).
- **Types** (§8): `color`, `dimension`, `fontFamily`, `fontWeight`, `duration`, `cubicBezier`, `number`. **Composite types** (§9): `strokeStyle`, `border`, `transition`, `shadow`, `gradient`, `typography`.
- **Groups** (§6): any object without `$value`. Groups can carry `$type`, `$description`, `$deprecated` and `$extensions`. They have a reserved **`$root`** token, so `{color.accent.$root}` is valid and `{color.accent}` (a group) is not (§6.2). **`$extends`** inherits from another group and is defined as sugar for JSON Schema `$ref` (§6.4). The spec says groups "SHOULD NOT" be used to infer a token's type or purpose (§6).
- **Aliases / references** (§7): curly braces `"{colors.blue}"` resolve to the target's whole `$value` and may target only tokens. **JSON Pointer `$ref` support is required** (RFC 6901) for property-level references such as one colour component (§7.1.2, §7.3). Chained references are allowed; circular references are errors (§7.2).
- **`$extensions`** (§5.2.3): a vendor-keyed object, with reverse-DNS keys recommended. Tools **MUST preserve** extension data they don't understand. Extensions SHOULD hold only optional metadata "not crucial to understanding that token's value".
- **`$deprecated`** (§5.2.4): `true`, an explanation string (which may contain aliases such as "use {button.activeBorder}"), or `false` to override a group default.

#### Color module (https://www.designtokens.org/TR/2025.10/color/)
- `$value` is an object: `colorSpace` (required), `components` (required; numbers or the `"none"` keyword), `alpha` (optional, 0–1, default 1) and `hex` (an optional 6-digit fallback) (§4.1).
- 14 colour spaces: `srgb`, `srgb-linear`, `hsl`, `hwb`, `lab` (CIELAB), `lch`, `oklab`, `oklch`, `display-p3`, `a98-rgb`, `prophoto-rgb`, `rec2020`, `xyz-d65`, `xyz-d50` (§4.2.1–4.2.14). There are also sections on gamut mapping, interpolation and token naming (base / alias / **component** categories, §7.1.3).

#### Resolver module (https://www.designtokens.org/TR/2025.10/resolver/)
- Purpose: "work with design tokens in multiple contexts (such as 'light mode' and 'dark mode' color themes)". It avoids the combinatorial explosion of themes × sizes × accessibility modes (§1).
- File: `.resolver.json`, plain JSON (JSONC/JSON5 MAY be accepted) (§3).
- Root properties: `version` (required, MUST be `"2025.10"`), `name`, `description`, `sets`, `modifiers`, `resolutionOrder` (required), `$schema`, `$defs` and `$extensions` (§4).
- **Set**: an ordered `sources` array of `{ "$ref": "file.json" }` and/or inline tokens. When a token repeats, the later one wins (§4.1.4).
- **Modifier**: a `contexts` map (for example `light`, `dark`, `darkHighContrast`), each holding an ordered list of sources, plus an optional `default`. A modifier SHOULD have ≥2 contexts, MUST have ≥1, and MUST NOT reference another modifier (§4.1.5).
- **Input**: a JSON object that selects one context per modifier, e.g. `{ "theme": "dark" }`. Resolving an input produces one **permutation**, a flat token set (§2.2, §5, §6). There is also a bundling section that covers inlining files and using `$defs` (§7).
- Fit for purlis: one modifier `appearance: {light, dark}` plus further modifiers per theme "option" maps directly onto purlis's appearances and options.

---

### 2. Build tools

#### Style Dictionary
- **Current major: v5.** npm `style-dictionary` `latest` = **5.7.0** (published 2026-10-16); 5.0.0 was published 2025-05-16 and 4.0.0 on 2024-06-28. **Licence Apache-2.0.** https://registry.npmjs.org/style-dictionary
- v5 is a "drop-in replacement for the majority of users". It is stricter about references (they must target a token), removes customisable reference syntax and requires Node ≥ 22. https://github.com/style-dictionary/style-dictionary/blob/main/docs/src/content/docs/versions/v5/migration.md
- DTCG: "As of version 4, Style Dictionary has first-class support for the DTCG format". The docs also warn: "**the latest format 2025.10 does not have full support yet** … This is a work in progress in v5". https://github.com/style-dictionary/style-dictionary/blob/main/docs/src/content/docs/info/DTCG.mdx
- The tracking issue #1590 "Support for DTCG v2025.10" is **OPEN** (last updated 2026-10-12). Done: colour module, border, shadow, dimension. In progress: gradient, duration. **The resolver module has no tick.** https://github.com/style-dictionary/style-dictionary/issues/1590
- CSS output: the built-in `css/variables` format emits `--kebab-name: value;` inside a selector. `options.selector` takes a string or an array of nested wrappers, and `outputReferences` keeps `var(--x)` chains. https://github.com/style-dictionary/style-dictionary/blob/main/docs/src/content/docs/reference/Hooks/Formats/predefined.md
- The implementation emits `${selector} {` verbatim (https://github.com/style-dictionary/style-dictionary/blob/main/lib/common/formats.js, around line 190). So `selector: '@theme inline'` produces a Tailwind v4 `@theme inline { … }` block. That is my inference from the source; there is no documented Tailwind v4 format. The only Tailwind example in the repo is a v3-style "tailwind-preset". https://github.com/style-dictionary/style-dictionary/blob/main/docs/src/content/docs/getting-started/examples.md

#### Tailwind v4 `@theme` (what the generated CSS must look like)
- Theme variables live in **namespaces**, and each namespace generates utilities: `--color-*` → `bg-*`/`text-*`, `--font-*`, `--text-*`, `--radius-*`, `--spacing-*`, and so on. https://github.com/tailwindlabs/tailwindcss.com/blob/main/src/docs/theme.mdx (§"Theme variable namespaces")
- **`@theme inline`**: use it when a theme variable references another variable. The utility then inlines `var(--x)` instead of pointing at the theme variable, which avoids resolution surprises across the tree. **`@theme static`** emits every variable, even unused ones. (Same file, §"Referencing other variables", §"Generating all CSS variables".)
- `@utility` registers custom utilities and `@custom-variant dark (&:where([data-theme=dark], [data-theme=dark] *))` defines attribute-driven dark mode. https://github.com/tailwindlabs/tailwindcss.com/blob/main/src/docs/adding-custom-styles.mdx , https://github.com/tailwindlabs/tailwindcss.com/blob/main/src/docs/dark-mode.mdx

#### Terrazzo (formerly Cobalt UI)
- `terrazzoapp/terrazzo`, **MIT**. It describes itself as "generate code from DTCG tokens (formerly known as 'Cobalt UI')". https://github.com/terrazzoapp/terrazzo
- npm `@terrazzo/cli`, `@terrazzo/parser`, `@terrazzo/plugin-css` and `@terrazzo/plugin-tailwind` are all at `latest` **2.9.0** (2026-10-14). https://registry.npmjs.org/@terrazzo/cli
- **It supports the DTCG resolver natively.** "A resolver is the new way to describe your whole tokens system … Resolvers are a DTCG standard, and the successor to legacy modes." https://github.com/terrazzoapp/terrazzo/blob/main/www/src/pages/docs/guides/resolvers.md
- **It has a first-party Tailwind v4 plugin** that generates "a Tailwind v4 Theme from your design tokens". The plugin maps token globs to theme namespaces (`color: ["color.*"]`, `radius: ["borderRadius.*"]`) through a CSS template. The CSS plugin's `permutations` take a resolver `input` and a `prepare` wrapper, e.g. `[data-theme="dark"] { … }`. https://github.com/terrazzoapp/terrazzo/blob/main/www/src/pages/docs/integrations/tailwind.md , https://github.com/terrazzoapp/terrazzo/blob/main/www/src/pages/docs/integrations/css.md
- Terrazzo's editor, Drew Powers, is a 2025.10 editor (https://www.designtokens.org/TR/2025.10/). Terrazzo is listed as an adopter on https://www.designtokens.org/.

#### Tokens Studio sd-transforms
- npm `@tokens-studio/sd-transforms` `latest` **2.0.3** (2025-12-10), MIT, `peerDependencies: { "style-dictionary": "^5.0.0" }`. Its description: "Custom transforms for Style-Dictionary, to work with Design Tokens that are exported from Tokens Studio". https://registry.npmjs.org/@tokens-studio/sd-transforms
- It only matters if designers author in the Tokens Studio Figma plugin. Otherwise skip it.

#### Rust crates that parse DTCG
- crates.io is blocked here. I probed the sparse index directly (https://index.crates.io/…), which returned **NoSuchKey** for `design-tokens`, `design_tokens`, `design-token`, `designtokens`, `dtcg-rs`, `dtcg-parser`, `dtcg-tokens`, `dtcg_tokens`, `tokens-dtcg`, `design-tokens-parser`, `design-data-dtcg`, `paintgun-dtcg`, `dtcg_runtime`, `themosis-tokens` and `pandacss_tokens`. The crate `terrazzo` does exist, but it is an unrelated web crate (depends on axum).
- A GitHub code search for `dtcg filename:Cargo.toml` finds only unpublished, 0–3-star projects (e.g. `workingdoge/paintgun` crates/paintgun-dtcg, `rayx-ui/RayX.Dtcg`, `QuadnucYard/themosis`). It also finds Adobe's `adobe/spectrum-design-data` `sdk/plugins/dtcg` (crate `design-data-dtcg` 0.1.0, "not owned/maintained by Adobe", and not on the index). https://github.com/adobe/spectrum-design-data/blob/main/sdk/plugins/dtcg/Cargo.toml
- Panda CSS is moving its token dictionary to Rust (`crates/pandacss_*` in the v2 branch). A benchmark note shows the Rust `pandacss_tokens::TokenDictionary` running 7–14× faster than JS, but the crates are **not published**. https://github.com/chakra-ui/panda/blob/v2/design-notes/bench/2026-05-17-token-dictionary-js-vs-rust-design.mdx
- **Conclusion: no maintained, published DTCG crate exists.** The format is small: serde JSON, `$type`/`$value`, curly-brace aliases plus RFC 6901 pointers, and a resolver with ordered merges. purlis should own a parser in its Rust theme engine and test it against the official JSON Schemas (§1). This is unverified on crates.io itself (host blocked: https://crates.io/search?q=dtcg).

---

### 3. Per-part ("slot / anatomy") theming in component libraries

| Library | Per-part model | Theme hook | Version (npm latest, 2026-10-20) |
|---|---|---|---|
| Radix Themes | none; components consume global scale tokens | CSS vars `--accent-1..12`, `--gray-*`, `--color-background/panel-solid/surface/overlay`; `data-accent-color` remaps `--accent-*` | `@radix-ui/themes` 3.3.0, MIT |
| Chakra UI v3 | `defineSlotRecipe` with `slots`, per-slot `base`/`variants`/`compoundVariants` | semantic tokens `{colors.red}` + conditions | `@chakra-ui/react` 3.37.0, MIT |
| Panda CSS | `sva` / `defineSlotRecipe`, per-slot class map; `className` → `card__root`; `data-slot` if hashed | semantic tokens with `_dark`; `themes` + `data-panda-theme` | `@pandacss/dev` 2.3.0, MIT |
| Tailwind Variants | `tv({ slots, variants, compoundVariants, compoundSlots })`, returns a function per slot | plain Tailwind classes (so `@theme` vars) | `tailwind-variants` 3.3.1, MIT |
| CVA | **no slots** (won't fix) | plain classes | `class-variance-authority` 0.7.1 stable; `cva` 1.0.0-beta.12 |
| Ark UI / Zag | anatomy: `data-scope` + `data-part` (+ `data-state`) | any CSS | `@ark-ui/react` 5.39.3, `@zag-js/anatomy` 1.46.0, MIT |

Details and sources:

- **Radix Themes.** Built on vanilla CSS; "components … are relatively closed". You customise through props, the theme configuration and "the same CSS variables that power the Radix Themes components", and "Changes to the token system are treated as breaking". https://www.radix-ui.com/themes/docs/overview/styling . The `<Theme accentColor grayColor panelBackground scaling radius>` component sets the look. https://www.radix-ui.com/themes/docs/theme/overview . In the source, `.rt-BaseButton:where(.rt-variant-classic) { background-color: var(--accent-9); color: var(--accent-contrast) }`, so parts are themed by **global scale steps, not per-component keys** (https://github.com/radix-ui/themes/blob/main/packages/radix-ui-themes/src/components/_internal/base-button.css). `[data-accent-color='indigo'] { --accent-1: var(--indigo-1); … }` and the semantic `--color-*` swap under `.dark` (https://github.com/radix-ui/themes/blob/main/packages/radix-ui-themes/src/styles/tokens/color.css).
- **Chakra UI v3 slot recipes**: `className`, `slots`, `base`, `variants`, `defaultVariants`, `compoundVariants`, all keyed per slot (e.g. `slots: ["root","control","label"]`), and used through `useSlotRecipe` or `createSlotRecipeContext`. https://github.com/chakra-ui/chakra-ui/blob/main/apps/www/content/docs/theming/slot-recipes.mdx . Semantic tokens reference other tokens with `{}` syntax. https://github.com/chakra-ui/chakra-ui/blob/main/apps/www/content/docs/theming/semantic-tokens.mdx . Chakra's token syntax is close to, but not the same as, DTCG (`value` rather than `$value`).
- **Panda CSS slot recipes**: `sva({ slots, base, variants, compoundVariants })` returns "a map of class strings, one per slot". The optional `className` gives each slot a stable `${className}__${slot}` class, and the docs say to "Target `data-slot` instead if you hash". https://github.com/chakra-ui/panda/blob/v2/website/content/docs/recipes/slot-recipes.mdx . Multiple themes: components read roles (`bg: 'surface'`), "One attribute picks the theme, one class picks the mode" via `[data-panda-theme='gothic']`, and themes can be loaded on demand as JSON. https://github.com/chakra-ui/panda/blob/v2/website/content/docs/theming/multiple-themes.mdx
- **Tailwind Variants**: its features include "Slots support" and "Tailwind CSS v4 support". The README notes that "Tailwind CSS v4 no longer supports `config.content.transform`, so responsive variants were removed". https://github.com/heroui-inc/tailwind-variants/blob/main/README.md . `compoundSlots` ("apply classes to multiple slots at once") is in the public types. https://github.com/heroui-inc/tailwind-variants/blob/main/src/types.ts . v2 made `tailwind-merge` an optional peer; v3 added a `/lite` build without it. https://github.com/heroui-inc/tailwind-variants/blob/main/.docs/migrations/v1-to-v2.md , https://github.com/heroui-inc/tailwind-variants/blob/main/.docs/migrations/v2-to-v3.md . npm `tailwind-variants` 3.3.1 (2026-08-03), with peers `tailwindcss: *` and `tailwind-merge >=3.0.0`, both optional. https://registry.npmjs.org/tailwind-variants . (The docs site tailwind-variants.org is blocked here.)
- **CVA**: stable `class-variance-authority` is 0.7.1 (2024-11-26, Apache-2.0). Version 1 ships as the npm package **`cva`**, which is still beta: `1.0.0-beta.1` came out 2023-08-25 and `1.0.0-beta.12` on 2026-09-17; `latest` is still 0.0.0. https://registry.npmjs.org/cva , https://registry.npmjs.org/class-variance-authority . Slots: issues #189 and #287 were **closed as not planned** (2024-04-28). https://github.com/joe-bell/cva/issues/287 , https://github.com/joe-bell/cva/issues/189 . The beta docs instead recommend building compound components "with CSS: leverage the cascade, custom properties, `:has()` selectors". https://github.com/joe-bell/cva/blob/main/docs/src/content/docs/beta/getting-started/compound-components.mdx
- **Zag / Ark UI anatomy**: `createAnatomy(name).parts(...)` builds, for each part, `attrs: { "data-scope": kebab(name), "data-part": kebab(part) }` and the selector `&[data-scope=…][data-part=…], & [data-scope=…][data-part=…]`. https://github.com/chakra-ui/zag/blob/main/packages/anatomy/src/create-anatomy.ts . Ark's styling guide: "use `data-scope` and `data-part` attributes to target specific elements within a component", with state in `data-state`, e.g. `[data-scope='accordion'][data-part='item'][data-state='open']`. https://github.com/chakra-ui/ark/blob/main/website/src/content/pages/guides/styling.mdx

---

### 4. JetBrains UI themes (`*.theme.json`)

#### Structure
- Root keys in the official schema (draft-07): `name`, `dark`, `author`, `editorScheme`, `parentTheme`, `ui`, `background`, `emptyFrameBackground`, `icons` (incl. `ColorPalette`), `colors` ("Define common colors here and use color names in 'ui' section instead of #rrggbb values") and `iconColorsOnSelection`. https://github.com/JetBrains/intellij-community/blob/master/plugins/devkit/intellij.devkit.themes/resources/schemes/theme.schema.json
- `dark` picks Darcula or Light as the base. `editorScheme` points at the editor colour XML. The theme is registered through `<themeProvider id=… path="/x.theme.json"/>` in plugin.xml. https://github.com/JetBrains/intellij-sdk-docs/blob/main/topics/basics/getting_started/theme/theme_structure.md
- **Inheritance**: `"parentTheme": "<themeProvider id>"`. https://github.com/JetBrains/intellij-sdk-docs/blob/main/topics/basics/getting_started/theme/themes_customize.md (§Extending Themes)
- **`ui` keys** take the form `Element.property`, sometimes deeper (`Popup.Advertiser.foreground`, `ScrollBar.Mac.Transparent.thumbColor`). A wildcard `"*": { "background": … }` sets every control's `background`, and a specific key such as `Label.background` overrides it. Keys can also be nested objects: `"Button": { "startBackground": … }` is `Button.startBackground`. (Same file, §Customizing UI Controls; nesting is visible in expUI_dark.theme.json below.)
- **Naming scheme**: `Object[.SubObject].[state][Part]Property`. Properties are `foreground`, `background` and `<part>Color`. States are `inactive`, `focused`, `selected`, `hover`, `pressed`, `error`/`warning`/`success` and `disabled` (the default state is unnamed). Common parts are `Border`, `Caret`, `Focus`, `Selection`, `Separator`, `Shadow`, `Icon`, `Info` and `Accelerator`/`Shortcut`. SubObjects mark variants (`Button.Default.background`) or inner components (`ToolWindow.HeaderTab.*`). Gradients use `start`/`end` (`Button.startBorderColor`/`Button.endBorderColor`). Swing legacy keys are exempt. https://github.com/JetBrains/intellij-sdk-docs/blob/main/topics/reference_guide/themes_metadata.md
- **This is the closest prior art for a per-component *and* per-state *and* per-part key grammar.** It is more structured than VS Code's flat `button.background`.

#### New UI and Islands
- The New UI ("expUI") themes live in `platform/platform-resources/src/themes/expUI/`: `expUI_dark.theme.json`, `expUI_light.theme.json` and `expUI_light_with_light_header.theme.json`. Islands themes live in `themes/islands/`: `ManyIslandsDark/Light/Darcula.theme.json`. https://github.com/JetBrains/intellij-community/tree/master/platform/platform-resources/src/themes
- `expUI_dark.theme.json` has `parentTheme: "Darcula"`, a **110-entry `colors` palette** (named colours such as `Gray2` and `Blue6`, which `ui` values reference by name), and `ui` with 99 top-level objects that **flatten to 659 keys**. The `Button` object holds `arc: 8`, `startBackground: "Gray2"`, `default.startBackground: "Blue6"`, `minimumSize: "72,28"`, and so on, so non-colour values (arc, sizes, border classes) share the namespace. I counted by flattening nested objects with `-I` Python. https://github.com/JetBrains/intellij-community/blob/master/platform/platform-resources/src/themes/expUI/expUI_dark.theme.json
- Islands (2025.3): set `"parentTheme": "Islands Dark"` or `"Islands Light"`. The keys are `Islands`, `Island.arc`/`.compact`, `Island.borderWidth`/`.compact`, `Island.borderColor` (the only one not inherited) and `Island.inactiveAlpha`. https://github.com/JetBrains/intellij-sdk-docs/blob/main/topics/basics/getting_started/theme/supporting_islands_theme.md

#### Key count (derived; JetBrains documents no total)
- Source: the two metadata files the SDK names as authoritative ("all keys from IntelliJ Platform and custom UI components" and "all keys from Swing components"). https://github.com/JetBrains/intellij-sdk-docs/blob/main/topics/reference_guide/themes_metadata.md
  - `IntelliJPlatform.themeMetadata.json`: **868 keys** (unique) across 121 top-level objects; 48 marked deprecated; 672 with `description`, 438 with `since`. https://github.com/JetBrains/intellij-community/blob/master/platform/platform-resources/src/themes/metadata/IntelliJPlatform.themeMetadata.json
  - `JDK.themeMetadata.json`: **402 keys** across 81 objects; 102 deprecated. https://github.com/JetBrains/intellij-community/blob/master/platform/platform-resources/src/themes/metadata/JDK.themeMetadata.json
  - **Union: 1,269 documented `ui` keys** (1 overlap), of which about 1,119 are not deprecated.
- Method: I downloaded both files from `master` on 2026-10-20 and counted `len(set(e["key"] for e in d["ui"]))` per file and across the union. Plugins may ship extra local `*.themeMetadata.json` files that are not counted here.

---

### 5. Recommended token pipeline for purlis

**Decision: DTCG 2025.10 as the only source format. The DTCG resolver models appearances and options. The Rust theme engine is the resolver. CSS custom properties are the single seam into the webview, and Tailwind v4 `@theme inline` maps them to utilities.**

1. **Source format: DTCG 2025.10 JSON, one resolver per theme folder.**
   ```
   themes/<id>/
     theme.resolver.json        # version "2025.10"
     tokens/base.tokens.json    # palette primitives (colorSpace: oklch)
     tokens/semantic.tokens.json# roles: surface, fg, accent, border…
     tokens/components/*.tokens.json  # button.tokens.json, tab.tokens.json …
     appearance/light.tokens.json, appearance/dark.tokens.json
     options/<option>-<value>.tokens.json
   ```
   - `sets` holds base, semantic and component tokens. `modifiers.appearance` = `{light, dark}`, with `default` taken from the OS. Each purlis theme **option** is a modifier, e.g. `density: {compact, comfortable}` or `contrast: {normal, high}`. The resolver **input** `{appearance, density, …}` is exactly the state the app already has (§1 Resolver).
   - Store colours as `oklch` with a `hex` fallback (Color module), so the Rust engine can do contrast checks and derive variants.
   - purlis-only metadata goes under `$extensions["app.purlis"]`: option labels, the part's `data-scope`/`data-part` and a preview hint. Tools must preserve it (§5.2.3), and Figma or other DTCG tools still read the files.

2. **Per-component part keys: the JetBrains grammar, expressed as DTCG groups.**
   - Path: `<component>.<part>.<state>.<property>`, borrowing JetBrains' `Object[.SubObject].[state][Part]Property` vocabulary. Default state is `$root`; states are `hover`, `pressed`, `focused`, `selected`, `disabled` and `inactive`; properties are `bg`, `fg`, `border`, `radius` and so on. Example: `button.root.hover.bg = "{color.accent.hover}"`, `tab.indicator.selected.bg`. DTCG forbids `.` in names, so the hierarchy has to be groups, which suits the format.
   - Component tokens **must alias semantic tokens** and never carry raw values. A theme author can override one part (as `Label.background` beats `"*"` in JetBrains) without forking the palette. Groups' `$extends` handles "inherit from the parent theme" (§6.4). The resolver's last-source-wins ordering handles a theme folder that layers over a built-in parent, the way JetBrains' `parentTheme` does.
   - **Keep the key set closed and versioned.** Publish a generated `themeMetadata`-style manifest (key, `$description`, `since`, `$deprecated`) from the Rust engine, as JetBrains does with 868 + 402 keys. Then theme authors get completion and a "Laf defaults"-style inspector. The `theme_preview` specimen is the natural consumer.

3. **Mapping to the React/Tailwind v4 webview.**
   - Every token becomes a CSS custom property named by its path: `--purlis-button-root-hover-bg`. Each permutation is emitted under `[data-theme="<id>"][data-appearance="dark"]` (or `:root` for the default), following Terrazzo's `permutations`/`prepare` pattern.
   - One generated `theme.css` registers the **semantic** layer in `@theme inline { --color-surface: var(--purlis-surface); --radius-control: var(--purlis-radius-control); … }`, so `bg-surface` and `rounded-control` utilities exist and `var()` resolves at the element, not at `:root` (Tailwind §"Referencing other variables"). Dark mode becomes `@custom-variant dark (&:where([data-appearance=dark], [data-appearance=dark] *))`.
   - Component parts **don't** become Tailwind namespaces; that would put about a thousand utilities in the generated namespace. Two routes instead:
     - (a) Recommended: **anatomy attributes plus CSS**. Every purlis component part renders `data-scope="button" data-part="root"` (the Zag/Ark convention, which also gives a stable hook for user CSS and tests). A generated `components.css` has `[data-scope=button][data-part=root]:hover { background: var(--purlis-button-root-hover-bg) }`.
     - (b) Where a component is written with Tailwind classes, use **tailwind-variants `tv({ slots })`** (3.3.1, Tailwind v4 support) with arbitrary-property classes such as `bg-(--purlis-button-root-bg) hover:bg-(--purlis-button-root-hover-bg)`. Slot names equal DTCG part names. CVA is ruled out because it has no slots (closed as not planned). Chakra and Panda are ruled out because each brings its own token system and runtime or codegen.
   - Switching appearance or options at runtime only changes `data-*` attributes on `<html>` (or swaps one injected `<style>` holding the resolved permutation). There is no rebuild and no React re-render.

4. **The Rust engine reads DTCG itself. Use JS tools at build time only, if at all.**
   - No published, maintained DTCG crate exists (§2), so write a small `serde_json` model: token or group, inherited `$type`, the 13 types, `{alias}` plus RFC 6901 `$ref` resolution with cycle detection, `$extends`, and the resolver's `sets`/`modifiers`/`resolutionOrder` with ordered merge. Validate in CI against `https://www.designtokens.org/schemas/2025.10/format.json` and `resolver.json`. Panda's Rust design note is useful prior art for the data model: "parse config once into compact token records, resolve references once, materialize derived records, then build read indexes".
   - The engine owns resolve → validate (types, contrast, closed key set) → emit CSS for a permutation. It serves that CSS to the webview over a Tauri command or event. The Rust core and any native surfaces (terminal ANSI palette, window chrome, tray) read the **same** resolved map, so there is one source of truth and no Node in the shipped app, consistent with the no-language-mixing rule.
   - Optional build-time oracle: Terrazzo 2.9.0 (MIT) is the only JS tool here with native resolver and Tailwind v4 support. Use it as a **test oracle** that diffs its CSS against the Rust engine's for the built-in themes; don't put it in the runtime path. Style Dictionary v5.7.0 (Apache-2.0) is the more popular choice, but its 2025.10 support is incomplete and resolver support has not been delivered (issue #1590 open), so it is the weaker fit today.

#### Open points to settle in the theme spec (#2313)
- Exact state vocabulary and ordering. JetBrains puts `Selection` before state words "for historical reasons", and purlis should pick one ordering and enforce it.
- Whether user theme folders may add new component keys (JetBrains allows plugin-local metadata) or only override the closed set.
- Prefix choice: `--purlis-*` versus unprefixed. A prefix avoids clashes with Tailwind's own `--color-*` theme namespace.

## 3b. Editor theming surfaces
#### How VS Code, Zed and JetBrains expose UI theming

| Editor | Keys for theme authors | Granularity | Source |
|---|---|---|---|
| VS Code | **770** workbench colour keys in 60 sections (counted from the reference page's `<li><code>` entries on 2026-10-20), plus syntax via `tokenColors` (TextMate) and `semanticTokenColors` | Per component and per state, flat: `button.background`, `button.hoverBackground`, `button.secondaryBorder`, `checkbox.selectBorder`, `editorWidget.border` | https://code.visualstudio.com/api/references/theme-color |
| Zed | **142** style keys in `ThemeStyleContent` (schema v0.2.0), plus `players` and `syntax` | **Semantic, not per component**: `element.{background,hover,active,selected,disabled}`, `ghost_element.*`, `surface.background`, `elevated_surface.background`, `border.{focused,selected,variant,disabled}`, `text.{muted,placeholder,disabled,accent}`; only a few component keys (`tab.*`, `title_bar.*`, `status_bar.background`, `panel.*`) and 29 `terminal.*` ANSI keys | https://zed.dev/schema/themes/v0.2.0.json |
| JetBrains | **1,269** documented `ui` keys (868 platform + 402 Swing); New UI dark uses 659 of them | Per component, per state, per part, with a stated grammar `Object[.SubObject].[state][Part]Property`, a `"*"` wildcard and `parentTheme` inheritance | See the JetBrains section above |

VS Code details (all from https://code.visualstudio.com/api/references/theme-color):
- **Base colours fall back.** `focusBorder`, `foreground`, `disabledForeground`, `widget.border` and `widget.shadow` are "only used if not overridden by a component". That is a two-layer model: global semantic keys, then per-component overrides.
- **High-contrast hooks**: `contrastBorder` and `contrastActiveBorder` add "an extra border around elements … for greater contrast". Without these, HC themes are not possible.
- **Colour format**: `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` only. There are no colour spaces and no references between keys.
- A recent "Modern UI colors" section adds surface keys (`surface.background`, `surface.border`, `modernPanel.border`, `modernTab.*`). Even VS Code is moving towards Zed-style semantic surfaces.
- Workbench colours vs syntax colours are separate: "Workbench colors used in views and editors … Syntax colors and styles used for source code", the latter from TextMate grammars plus semantic tokens. https://code.visualstudio.com/api/extension-guides/color-theme

Zed details:
- A theme file holds `name`, `appearance` (light/dark) and `style` (schema `ThemeContent`). Users override any key per theme through `theme_overrides` in settings, and pick a theme per OS mode via `"theme": { "mode": "system", "light": …, "dark": … }`. https://zed.dev/docs/themes
- Zed has an official Theme Builder for previewing "every surface". https://zed.dev/docs/themes
- Zed's contributor **UI/UX checklist** is a good agent rubric for purlis:
  - keyboard-only use, discoverable shortcuts, clear focus indicators;
  - light and dark;
  - narrow and short panes; dialogs stay inside the viewport;
  - respect system font/scaling;
  - instant feedback, frames ≤ 8 ms;
  - actionable errors, undo.

  https://zed.dev/docs/development/ui-checklist

**Takeaway for purlis.** The three editors sit on a spectrum.
- Zed: about 140 semantic keys. Easy to author, but limited per-component control.
- VS Code: about 770 flat per-component keys with global fallbacks.
- JetBrains: about 1,270 keys under a formal grammar, with a wildcard and parent inheritance.

The pipeline recommended below takes Zed's semantic layer as the part every theme must set. It then adds JetBrains-grammar component keys as optional overrides that alias semantic tokens. This also follows VS Code's "only used if not overridden by a component" fallback rule. So a theme author can ship a complete theme by setting about 40 to 140 semantic tokens, and touch component keys only to diverge.

---

# 4. Verification

Researched 2026-10-20 for purlis (Tauri 2, React + TS + Tailwind v4 + Vite, Rust core, vitest).

### How these sources were read

The sandbox's network filter shaped what could be checked, so each claim below carries one of these tags:

- **verified**: read from the primary source during this session.
- **verified (source repo)**: www.w3.org was blocked, so W3C text was read from the W3C's own source repository (`github.com/w3c/wcag`, `main` branch), fetched through the jsDelivr GitHub mirror. The same goes for the axe-core repo docs (`github.com/dequelabs/axe-core`, `develop` branch). The canonical URL is the one cited.
- **verified (package)**: read from the published npm package: the registry metadata at `registry.npmjs.org`, or type declarations inside the package tarball.
- **unverified (host blocked)**: the canonical host could not be reached (fluent2.microsoft.design, react.fluentui.dev, v2.tauri.app, playwright.dev, vitest.dev, chromatic.com, deque.com). The claim comes from prior knowledge and has to be checked before anyone relies on it.

Reachable hosts: developer.apple.com (HIG JSON API), storybook.js.org, registry.npmjs.org.

---

### A. Component catalogue: Storybook vs Ladle

#### Storybook (verified)

- **Version.** `storybook` latest is **10.6.3**, published 2026-10-20. The `next` tag is 11.0.0-alpha.7. `@storybook/react-vite`, `@storybook/addon-vitest`, `@storybook/addon-a11y` and `@storybook/addon-mcp` are all at 10.6.3. Licence MIT. https://registry.npmjs.org/storybook
- **Vite builder.** The framework package is `@storybook/react-vite`. https://storybook.js.org/docs/builders/vite
- **Vitest addon (`@storybook/addon-vitest`).** "transforms your stories into component tests, which test the rendering and behavior of your components in a real browser environment". https://storybook.js.org/docs/writing-tests/integrations/vitest-addon
  - Requirements: a Vite-based framework, **Vitest ≥ 3.0**, and MSW ≥ 2.0 if MSW is used.
  - `npx storybook add @storybook/addon-vitest` installs Vitest if needed and "set[s] up browser mode using Playwright's Chromium browser".
  - For Vitest ≥ 4 the plugin is `storybookTest({ configDir, storybookScript })` from `@storybook/addon-vitest/vitest-plugin`, inside a `test.projects` entry. That entry has `browser: { enabled: true, provider: playwright({}), headless: true, instances: [{ browser: 'chromium' }] }`, with `playwright` imported from `@vitest/browser-playwright`.
  - CLI: `vitest --project=storybook`. Failures can link to the story through `storybookUrl`. The same page notes that a play function makes a story an interaction test.
- **A11y addon (`@storybook/addon-a11y`).** "built on top of Deque's axe-core library, which automatically catches up to 57% of WCAG issues". https://storybook.js.org/docs/writing-tests/accessibility-testing
  - The panel has three tabs: Violations, Passes, and Incomplete ("confirm manually").
  - `parameters.a11y.{context, config, options, test}`. **`test: 'off' | 'todo' | 'error'`**: `'error'` fails the Vitest/CI run, and `'todo'` only warns.
  - The `region` rule is disabled by default.
  - **The default ruleset is WCAG 2.0 A/AA, WCAG 2.1 A/AA and best practices. It does not include WCAG 2.2.** Use `options.runOnly` to add `wcag22aa`.
- **Storybook MCP server (it exists, and it is "in preview"; the API may change).** https://storybook.js.org/docs/ai/mcp/overview
  - Install with `npx storybook add @storybook/addon-mcp`. The dev server then serves MCP at `http://localhost:6006/mcp`.
  - Register it with an agent using `npx mcp-add --type http --url "http://localhost:6006/mcp" --scope project`.
  - **Development toolset:** `stories-changed`, `get-storybook-story-instructions`, `stories-preview` (rendered inline when the agent supports MCP Apps, otherwise as links), `stories-find-by-component` and `review-create`.
  - **Docs toolset:** `docs-list`, `docs-show` and `docs-show-story`. It needs `features: { componentsManifest: true }` in `.storybook/main.ts`; every React framework supports this.
  - **Testing toolset:** `test-run`. It "runs tests for specific stories and returns results, including any accessibility issues (if configured)". It needs the Vitest addon.
  - The page proposes an AGENTS.md/CLAUDE.md snippet: query `docs-list`/`docs-show` before using props, and "Check your work by running `test-run`".
  - `@storybook/mcp` (10.6.3) is the reusable library behind it: `createStorybookMcpHandler`, with the tools `docs-list`, `docs-show` and `docs-show-story`. Its README says "This package is a library … no bin … use the dev server instead". https://registry.npmjs.org/@storybook/mcp
  - `@storybook/addon-mcp`'s README says the endpoint is configurable (`options.endpoint`). https://registry.npmjs.org/@storybook/addon-mcp
- `@chromatic-com/storybook` latest is 5.6.0, and `chromatic` latest is 18.13.0 (verified (package)). That Chromatic is a paid SaaS for visual and cross-browser snapshots is **unverified (host blocked)**: https://www.chromatic.com/pricing

#### Ladle (verified (package))

- `@ladle/react` latest is **5.1.1, published 2025-11-04**, and the registry's `modified` timestamp is the same day. That means **no release for roughly 11.5 months** as of 2026-10-20. Repo: github.com/tajo/ladle. https://registry.npmjs.org/@ladle/react
- Ladle has no equivalent of the Vitest addon, the a11y test runner or the MCP server in what I could verify. I could not reach ladle.dev or its GitHub page to check its feature list (**unverified**).

#### Recommendation

**Use Storybook 10 with `@storybook/react-vite`, `@storybook/addon-vitest`, `@storybook/addon-a11y` and `@storybook/addon-mcp`.** It is the only option here that gives an agent all three:

1. a test loop it can run headless (`vitest --project=storybook`);
2. axe checks per story that fail CI (`a11y.test: 'error'`);
3. an MCP server whose `test-run` tool returns component and a11y failures straight into the agent loop.

Ladle is lighter, but its releases have stalled and it has none of these integrations.

---

### B. Accessibility checks

- **axe-core 4.14.0**, published 2026-10-05, licence **MPL-2.0** (verified (package), https://registry.npmjs.org/axe-core). The README confirms the licence: "distributed under the Mozilla Public License, version 2.0". https://github.com/dequelabs/axe-core/blob/develop/README.md
- **Deque's own coverage claim:** "With axe-core, you can find **on average 57% of WCAG issues automatically**. Additionally, axe-core will return elements as 'incomplete' where axe-core could not be certain, and manual review is needed." (verified (source repo), same README). Storybook repeats the 57% figure. Deque's separate study page (deque.com, "automated testing coverage report") is **unverified (host blocked)**.
- **Rules and tags.** https://github.com/dequelabs/axe-core/blob/develop/doc/rule-descriptions.md and https://github.com/dequelabs/axe-core/blob/develop/doc/API.md (verified (source repo))
  - Tags include `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, **`wcag22aa`**, `wcag2aaa`, `best-practice`, `section508`, `TTv5`, `EN-301-549` and `experimental`. Per-criterion tags look like `wcag258`.
  - **The only WCAG 2.2 A/AA rule is `target-size` (SC 2.5.8). The docs say "These rules are disabled by default, until WCAG 2.2 is more widely adopted and required."** Experimental rules such as `focus-order-semantics` are also off by default.
  - `axe.run` runs every rule except experimental ones by default. `runOnly` "limit[s] which rules are executed, based on names or tags". The docs do not say outright that `runOnly: ['wcag22aa']` switches on a disabled-by-default rule, so to be safe, **also pass `rules: { 'target-size': { enabled: true } }`**.
- **`@axe-core/playwright` 4.14.0** (MPL-2.0). Its version tracks axe-core's major.minor. It provides `new AxeBuilder({ page })` with `.include()`, `.exclude()`, `.withTags()`, `.withRules()`, `.disableRules()`, `.options()` and `.analyze()`, and `.options()` overrides `withTags`/`withRules`. (verified (package) README, https://registry.npmjs.org/@axe-core/playwright)

  ```ts
  const r = await new AxeBuilder({ page })
    .withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa'])
    .analyze();
  expect(r.violations).toEqual([]);
  ```

- **vitest-axe 0.1.0 was last published 2022-10-21**. It is a fork of jest-axe exposing `expect.extend(matchers)` from `vitest-axe/matchers` (verified (package), https://registry.npmjs.org/vitest-axe). Because it is stale, prefer calling `axe-core` directly (`await axe.run(el)`), or rely on Storybook's addon. **jest-axe 11.0.0** was published 2026-07-26 and is maintained, but targets Jest. https://registry.npmjs.org/jest-axe
- **jsdom caveat (from prior knowledge, not checked this session):** `color-contrast` needs real layout, so run axe in a real browser (Vitest browser mode or Playwright), not in jsdom.
- **What axe cannot judge.** These need a manual check or a scripted keyboard/visual test:
  - whether focus order is logical;
  - whether the focus indicator is visible (2.4.7) and not obscured (2.4.11);
  - keyboard operability and traps (2.1.1, 2.1.2);
  - reflow at 320 px (1.4.10);
  - text spacing (1.4.12) and resize (1.4.4);
  - hover/focus content behaviour (1.4.13);
  - dragging alternatives (2.5.7);
  - whether labels make sense (3.3.2);
  - non-text contrast of custom states (1.4.11; axe checks text contrast only);
  - whether accessible names are correct, as opposed to merely present.

  Axe reports uncertain items as "incomplete".

---

### C. Visual regression

- **Playwright `toHaveScreenshot`.** `@playwright/test` is at 1.64.1 (Apache-2.0). The following is verified (package) from `playwright/types/test.d.ts` in the 1.64.1 tarball:
  - options `threshold` (YIQ color difference, default `0.2`), `maxDiffPixels`, `maxDiffPixelRatio`, `animations` (default `"disabled"`), `caret`, `mask` and `scale`;
  - `snapshotPathTemplate` tokens include **`{platform}`** ("The value of `process.platform`") and `{projectName}`, which **means baselines are stored per OS**.
  - Canonical docs (unverified (host blocked)): https://playwright.dev/docs/test-snapshots. Not re-checked this session: that page warns screenshots differ across OS/browser because of rendering and fonts, and recommends generating baselines in the same environment as CI, for example the official Docker image `mcr.microsoft.com/playwright:v1.64.1-noble`.
  - Playwright also has `toMatchAriaSnapshot` (ARIA tree as YAML), from prior knowledge.
- **Vitest browser mode.** `vitest` latest is **5.0.5** (2026-10-15), and `@vitest/browser` and `@vitest/browser-playwright` are at 5.0.5 (verified (package)).
  - `await expect.element(locator).toMatchScreenshot(name?, { comparatorName: 'pixelmatch', comparatorOptions: { allowedMismatchedPixelRatio } })` writes reference, actual and diff images (from `@vitest/browser/jest-dom.d.ts`).
  - Default screenshot paths go in `__screenshots__/<testfile>/<name>-<browserName>-<platform>.png` (from `dist/index.js` `resolveScreenshotPath`), so these are **per-OS baselines too**.
  - New and **@experimental**: `toMatchAriaSnapshot()`, which snapshots roles, names and states. This is a cheap structural check for agents.
  - Docs: https://vitest.dev/api/browser/assertions#tomatchscreenshot (host blocked; the API was read from the package's own types).
- **Storybook visual tests** are Chromatic-backed (`@chromatic-com/storybook`), and Chromatic is SaaS. The Vitest addon page mentions "If you have the Visual tests addon installed, you'll see an option to run Visual tests alongside Component tests" (verified). An alternative is to call `toMatchScreenshot` inside story tests run by the Vitest addon. That this composes cleanly is my inference, not something I tested.
- **Lost Pixel**: `lost-pixel` 3.22.0, last published **2024-11-14**, MIT. It is an "open-source alternative to Percy, Chromatic, Applitools" with Storybook, Ladle, Pages and custom-shot modes. Its approval UI, review UI and monorepo support are "Lost Pixel Platform only" (verified (package) README, https://registry.npmjs.org/lost-pixel). It is stale.
- **reg-suit**: 0.14.5, published 2025-08-26, MIT. It compares directories of images, publishes snapshots to S3 or GCS through plugins, and notifies GitHub, GitLab or Slack. Bring your own screenshots. https://registry.npmjs.org/reg-suit (verified (package))
- **Fonts and platforms.** Both Playwright and Vitest put the platform in the baseline filename (verified above). A macOS developer machine and Linux CI will never match. **Generate and compare baselines only inside one pinned Linux Docker image.** Bundle the app's fonts (no system fonts) and disable animations and the caret. The Docker-image recommendation is standard Playwright guidance, **unverified (host blocked)** this session.
- **Testing inside Tauri itself (WebDriver)**: **unverified (host blocked)**, canonical https://v2.tauri.app/develop/tests/webdriver/. From prior knowledge:
  - `tauri-driver` is a cargo-installed wrapper. It drives `WebKitWebDriver` on Linux and Microsoft Edge Driver (`msedgedriver`) on Windows.
  - **On macOS it is not supported, because WKWebView has no WebDriver tool.**
  - Tests use WebdriverIO or Selenium against the built debug binary.

  **Correction:** the tauri-docs source (https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/develop/Tests/WebDriver/index.mdx) confirms that `tauri-driver` does not support macOS. It also names `@wdio/tauri-service`, which embeds a WebDriver server in the app, as working on macOS. So real-webview E2E *is* possible on macOS. Locally, the agent verifies against the Vite dev server in Chromium (Playwright), with Tauri IPC mocked, for example through `@tauri-apps/api/mocks` `mockIPC` (from prior knowledge, unverified). `@tauri-apps/cli` latest is 2.12.3 (verified (package)).

---

### D. WCAG 2.2 AA criteria most relevant to a desktop dev tool

Normative text was read from the W3C source repo (verified (source repo)). The links are the canonical Understanding docs.

| SC | Level | Requirement (abridged) | Agent-checkable how |
|---|---|---|---|
| [2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html) | AA | At least **24×24 CSS px**, except: **Spacing** (a 24 px circle centred on each undersized target does not intersect another target or circle), **Equivalent** control on the same page, **Inline** (in a sentence or line-height constrained), **User-agent control**, **Essential**. Sliders and other spatial targets count as one target. Understanding: if a 24×24 square cannot fit inside (e.g. rounded corners), the target is undersized, but it can still pass on spacing. | axe `target-size` (enable it explicitly); also bounding-box asserts |
| [2.4.7 Focus Visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html) | AA | Keyboard-operable UI has a mode where the focus indicator is visible. | Tab through, then screenshot; check that `:focus-visible` styles exist |
| [2.4.11 Focus Not Obscured (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html) | AA | A focused component is **not entirely hidden** by author-created content. Understanding names **sticky footers/headers and non-modal dialogs** and suggests **scroll padding** as the fix. | Tab through a scroll region under a sticky dialog footer; check that `getBoundingClientRect` intersects the visible area |
| [2.4.13 Focus Appearance](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html) | **AAA** (guidance) | Indicator area at least that of a **2 CSS px perimeter** of the component, with **3:1** contrast between focused and unfocused pixels. | Use as the design target: `outline: 2px solid` with a 3:1 colour |
| [1.4.10 Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html) | AA | No 2-D scrolling at **320 CSS px** width (vertical content) or **256 CSS px** height (horizontal content). This equals 1280 px at 400% zoom. Exception: content that needs a 2-D layout, e.g. data tables, diagrams, and "interfaces where it is necessary to keep toolbars in view while manipulating content". Editors and terminals plausibly fall under this exception. | Set viewport to 320 wide and assert `scrollWidth <= clientWidth` |
| [1.4.11 Non-text Contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html) | AA | **3:1** against adjacent colours for UI component boundaries and states, and for graphics needed for understanding. Inactive components and unmodified user-agent controls are exempt. | Not covered by axe; needs a custom token-pair contrast check |
| [1.4.3 Contrast (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) | AA | **4.5:1** for text; **3:1** for large text. Inactive, decorative and logo text is exempt. | axe `color-contrast` (needs a real browser) |
| [1.4.12 Text Spacing](https://www.w3.org/WAI/WCAG22/Understanding/text-spacing.html) | AA | No loss when line-height is 1.5×, paragraph spacing 2×, letter spacing 0.12× and word spacing 0.16× the font size. | Inject that CSS, then screenshot and check for overflow or clipping |
| [1.4.4 Resize Text](https://www.w3.org/WAI/WCAG22/Understanding/resize-text.html) | AA | Text resizes to **200%** without loss of content or function. | Zoom or set root font-size to 200%, then check for overflow |
| [2.1.1 Keyboard](https://www.w3.org/WAI/WCAG22/Understanding/keyboard.html) | A | Everything works by keyboard except freehand path-dependent input. | Play functions using `userEvent.keyboard` |
| [2.1.2 No Keyboard Trap](https://www.w3.org/WAI/WCAG22/Understanding/no-keyboard-trap.html) | A | Focus can always leave by keyboard; a non-standard exit must be explained. This matters for **embedded terminals and code editors**, which capture Tab. | A test that presses the documented exit key |
| [4.1.2 Name, Role, Value](https://www.w3.org/WAI/WCAG22/Understanding/name-role-value.html) | A | Components expose a name, role, state and value to assistive technology. Custom controls need ARIA. | axe (button-name, aria-*), `getByRole(...,{name})`, aria snapshots |
| [3.3.2 Labels or Instructions](https://www.w3.org/WAI/WCAG22/Understanding/labels-or-instructions.html) | A | Labels or instructions when content requires user input. The source fetch failed; this wording is from prior knowledge. | axe `label`; review |
| [2.5.7 Dragging Movements](https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html) | AA | Every drag operation also works with a single pointer without dragging, unless dragging is essential. This applies to tab reordering, split panes and sliders. | Test the click or keyboard alternative |
| [1.4.13 Content on Hover or Focus](https://www.w3.org/WAI/WCAG22/Understanding/content-on-hover-or-focus.html) | AA | Tooltips and popovers must be **dismissible** (Esc) without moving pointer or focus, **hoverable**, and **persistent**. Native `title` tooltips are exempt. | Play: hover, press Esc, assert hidden; hover the tooltip and assert it stays |

---

### E. Platform guidance: macOS HIG and Fluent 2

#### macOS HIG (verified; read from the developer.apple.com HIG JSON)

- **Alerts** (https://developer.apple.com/design/human-interface-guidelines/alerts):
  - title, optional informative text, **up to three buttons**; on macOS also icon, accessory view, suppression checkbox and Help button;
  - "**Always place the default button on the trailing side of a row** or at the top of a stack. **Cancel buttons are typically on the leading side**";
  - "Always use the title 'Cancel'"; "you don't want to make a Cancel button the default button";
  - with a destructive action, include Cancel; avoid "OK" except in informational alerts;
  - Esc and Cmd-. cancel;
  - titles should not wrap past two lines.
- **Buttons** (https://developer.apple.com/design/human-interface-guidelines/buttons):
  - the primary role is the default button, which "responds to the Return key", and in a sheet or alert it lets the view close on Return;
  - "use style — not size — to distinguish the preferred choice";
  - keep one or two prominent buttons per view;
  - add a trailing ellipsis when a button opens another window or view;
  - "a hit region of at least 44x44 pt — in visionOS, 60x60 pt".
  - The explicit **mini 28 / small 32 / regular 44 / large 52 / extra large 64 pt** sizes on that page are **for visionOS only**.
- **Accessibility, control sizes** (https://developer.apple.com/design/human-interface-guidelines/accessibility):
  - **macOS: default control size 28×28 pt, minimum 20×20 pt**;
  - about 12 pt of padding around bezeled elements, about 24 pt around unbezeled ones;
  - macOS text: default 13 pt, minimum 10 pt;
  - contrast 4.5:1 up to 17 pt, 3:1 at 18 pt or for bold.
- **AppKit `NSControl.ControlSize`** has `mini`, `small`, `regular` (default), `large` and `extraLarge`. **The docs give no pixel heights** (https://developer.apple.com/documentation/appkit/nscontrol/controlsize-swift.enum). Commonly quoted heights such as regular push button about 20–22 pt and small about 17 pt are **not in Apple's current docs (unverified)**.
- **Sheets** (https://developer.apple.com/design/human-interface-guidelines/sheets):
  - on macOS a sheet is always modal: a card attached to its parent window, with the parent dimmed;
  - "**Present a sheet in a reasonable default size.** People don't generally expect to resize sheets", but resizing helps sometimes;
  - one sheet at a time;
  - use a panel when people need to provide input repeatedly;
  - pair Done with Cancel; never show Cancel, Done and Back together.
  - **No maximum height and no rule for sticky footers or scroll regions is given.**
- **Forms and label alignment.** The current HIG Layout page says only "Align elements … Group related items … negative space, container shapes, or separator lines" (https://developer.apple.com/design/human-interface-guidelines/layout). **The well-known macOS convention of right-aligned labels ending in a colon is not in the current HIG** (the Layout and Text fields pages say nothing about it). It came from the legacy OS X HIG. Treat it as convention, not current guidance.

#### Fluent 2 and Fluent UI React v9

The Fluent 2 design site (fluent2.microsoft.design) and react.fluentui.dev were blocked. The Fluent UI React v9 **source code** on GitHub was readable, and it is the implementation those pages describe. It settles the numbers. Design-site usage prose (for example primary-vs-secondary button order) is **unverified (host blocked)**: https://fluent2.microsoft.design/components/web/react/core/dialog/usage

- **Dialog constants** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-dialog/library/src/contexts/constants.ts): `SURFACE_PADDING = 24px`, `DIALOG_GAP = 8px`, `SURFACE_BORDER_WIDTH = 1px`, narrow breakpoint `max-width: 480px`, short-screen breakpoint `max-height: 359px`.
- **DialogSurface** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-dialog/library/src/components/DialogSurface/useDialogSurfaceStyles.styles.ts): `position: fixed`, `height: fit-content`, **`maxWidth: 600px`**, `maxHeight: 100vh / 100dvh`. Under 480 px wide it goes `maxWidth: 100vw`. On a short screen the whole surface scrolls instead.
- **DialogBody is the layout grid** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-dialog/library/src/components/DialogBody/useDialogBodyStyles.styles.ts): `display: grid`, `gap: 8px`, **`maxHeight: calc(100vh - 2 * 24px)`**, `gridTemplateRows: auto 1fr`, `gridTemplateColumns: 1fr 1fr auto`.
- **DialogContent is the only scroll region** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-dialog/library/src/components/DialogContent/useDialogContentStyles.styles.ts): `overflowY: auto`, `minHeight: 32px`, grid row 2. Title (row 1) and actions (row 3) therefore stay in view. This is the **sticky action footer** pattern: the footer is not `position: sticky`, it sits in its own grid row beside a scrolling middle.
- **DialogActions** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-dialog/library/src/components/DialogActions/useDialogActionsStyles.styles.ts): flex with an 8 px gap, `position="end"` puts the group at `justifySelf: end`, `position="start"` at the start (both may be used at once, for example a "Learn more" on the left). Under 480 px the actions stack and stretch full width (`fluid`).
- **Best practices** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-dialog/stories/src/Dialog/DialogBestPractices.md): header `DialogTitle`, content `DialogContent` and footer `DialogActions` inside a `DialogBody`; validate inline before closing; use modal dialogs sparingly; `aria-describedby` on short confirmations; **no more than three buttons in `DialogActions`**; **no nested dialogs**; never a dialog without a focusable element.
- **Button sizes** (verified, https://github.com/microsoft/fluentui/blob/master/packages/react-components/react-button/library/src/components/Button/useButtonStyles.styles.ts): icon-only buttons are square at **24 / 32 / 40 px** for `small` / `medium` / `large`. Text buttons have `minWidth` **64 px** (small) and **96 px** (medium, large). Medium is the default size. Icons are 20 px (small, medium) and 24 px (large).
- `@fluentui/react-dialog` 9.18.6 was published 2026-10-19 (verified (package)).

**What this means for purlis dialogs.** Fluent gives exact, copyable numbers and the HIG gives the ordering rule:
- a 3-row grid (title / scrolling content / actions);
- max height = viewport minus 2×24 px;
- max width 600 px for a standard dialog;
- 8 px gaps;
- at most three actions;
- on macOS, the default button trailing and Cancel leading (HIG Alerts above);
- `scroll-padding-bottom` on the content region, so a focused field is never hidden under the footer (WCAG 2.4.11).

---

### Recommended verification stack for the agent

Nothing below was installed. The versions are the npm `latest` tags as of 2026-10-20.

**Packages (devDependencies):**

```
storybook@10.6.3 @storybook/react-vite@10.6.3
@storybook/addon-vitest@10.6.3 @storybook/addon-a11y@10.6.3 @storybook/addon-mcp@10.6.3
vitest@5.0.5 @vitest/browser-playwright@5.0.5
@playwright/test@1.64.1 @axe-core/playwright@4.14.0 axe-core@4.14.0
```

Skip Ladle, vitest-axe and lost-pixel (stale). Leave Chromatic and reg-suit out unless hosted review becomes a requirement.

**Setup commands:**

```
npx storybook@10.6.3 init --builder vite    # or: npx storybook add for each addon below
npx storybook add @storybook/addon-vitest
npx storybook add @storybook/addon-a11y
npx storybook add @storybook/addon-mcp
npx playwright install chromium
npx mcp-add --type http --url "http://localhost:6006/mcp" --scope project
```

**Config essentials:**

- `.storybook/main.ts`: `features: { componentsManifest: true }`.
- `.storybook/preview.ts`:

  ```ts
  parameters: { a11y: {
    test: 'error',
    options: {
      runOnly: ['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa','best-practice'],
      rules: { 'target-size': { enabled: true } }
    }
  } }
  ```

- Give every interactive component a story with a `play` function covering:
  - keyboard paths (2.1.1/2.1.2);
  - Esc dismissing hover content (1.4.13);
  - focus not obscured under a sticky dialog footer (2.4.11).
- Add an `expect.element(...).toMatchAriaSnapshot()` (experimental) for structure, and `toMatchScreenshot()` for pixels.
- Write a small custom check for 1.4.11 that runs a contrast function over theme token pairs (border vs surface, focus ring vs surface, at least 3:1), plus 320-px reflow and 200% text-size overflow checks.

**Commands the agent runs to verify UI work:**

```
npx vitest run --project=storybook          # component + interaction + axe (fails on violations)
npx playwright test e2e/a11y.spec.ts        # AxeBuilder.withTags([... 'wcag22aa']) on the dev-server app with mocked Tauri IPC
docker run --rm -v "$PWD":/w -w /w mcr.microsoft.com/playwright:v1.64.1-noble \
  npx vitest run --project=storybook --update=false   # visual baselines only in this pinned Linux image
```

In an agent session, start `npx storybook dev -p 6006 --no-open` and use the MCP tools: `stories-find-by-component`, then `test-run`, then `stories-preview`.

**Real-webview E2E (corrected; see the skills section §8):** Tauri's own docs, read from the tauri-docs repo, say macOS "has no WKWebView driver tool available" for `tauri-driver`. They also name WebdriverIO's `@wdio/tauri-service` as the supported route on Windows, Linux **and macOS**, through an embedded WebDriver plugin. Use it, with `@axe-core/webdriverio`, for the real-webview layer. Source: https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/develop/Tests/WebDriver/index.mdx

**Manual or human items axe cannot cover** (axe automates about 57% of issues by Deque's own claim): logical focus order, quality of names and labels, screen-reader behaviour in VoiceOver on WKWebView, and the macOS-vs-Fluent button-order decision.

---

# 5. Recommendation

## Primitives: React Aria Components

`react-aria-components` (Adobe, Apache-2.0, 1.22.1). It is the only candidate that covers, in one library, what a dev tool needs:
- a file tree with keyboard navigation, drag and drop and virtualization;
- tables and grid lists;
- toolbar, combobox and autocomplete, menus, dialogs and tabs.

It also has the strongest focus and press handling (`FocusScope`, `usePress`, `data-focus-visible` that skips programmatic focus), 30+ locales with RTL, and a first-party Tailwind v4 plugin (§2.9).
- **How to adopt it:** wrap each primitive once in a purlis component layer. That layer adds the anatomy attributes (`data-scope`/`data-part`) the token pipeline styles.
- **Costs to accept:** the Apache NOTICE, a heavier collections API, and per-component bytes that have not been measured (measure them in the Vite build).
- **Runner-up:** Base UI, if the tree and table move out of the primitives layer.

## Token pipeline: DTCG 2025.10 + resolver, resolved by the Rust theme engine

1. **Source.** Each theme folder holds DTCG 2025.10 JSON (stable, 2025-10-28) and one `theme.resolver.json`.
   - Appearance is one resolver modifier (`light`/`dark`), and each theme option is another modifier.
   - Colours are stored as `oklch` with a `hex` fallback.
2. **Layers.**
   - Primitives.
   - A **required semantic layer** of roughly 40 to 140 tokens, Zed-style: surface, elevated surface, element and ghost-element states, border, text roles, focus.
   - **Optional component-part tokens** named `<component>.<part>.<state>.<property>`, after JetBrains' grammar. They may only alias semantic tokens, and they fall back to the semantic layer as VS Code's base colours do.
   - The key set is closed and versioned, and a manifest is published for theme authors.
3. **Engine.** The Rust theme engine owns parsing, resolving and validation (types, closed key set, 3:1 and 4.5:1 contrast pairs).
   - It emits CSS custom properties per permutation under `[data-theme][data-appearance]`.
   - No published DTCG crate exists, so the parser is purlis's own. It is validated in CI against the official 2025.10 JSON Schemas.
   - The shipped app contains no Node, which keeps to the no-language-mixing rule.
4. **Webview.** Only the semantic layer goes into Tailwind v4 `@theme inline`.
   - Component parts are styled by a generated `components.css` keyed on `[data-scope][data-part]` plus RAC's `data-*` states.
   - Where a component is written in Tailwind classes, use `tailwind-variants` slots with `bg-(--purlis-…)`.
   - CVA is ruled out because it has no slots. Chakra and Panda are ruled out because each brings its own token system.
5. **Oracle.** Terrazzo 2.9.0 (MIT; native resolver and Tailwind v4) runs only as a build-time test oracle, diffing its CSS against the engine's output for the built-in themes. Style Dictionary 5.7 is the weaker fit, because its 2025.10 resolver support is still open (#1590).

## Verification stack

| Layer | What runs | Catches |
|---|---|---|
| Component catalogue + unit a11y | Storybook 10.6 (`@storybook/react-vite`) + `@storybook/addon-vitest` (Vitest 5 browser mode, Chromium) + `@storybook/addon-a11y` with `test: 'error'`, the `wcag22aa` tag **and `target-size` enabled explicitly** (it is off by default in axe) | Name/role/value, contrast, labels, 2.5.8 target size; keyboard paths and 1.4.13 dismissal through `play` functions; `toMatchAriaSnapshot` for structure |
| Agent loop | `@storybook/addon-mcp` (`test-run`, `stories-preview`, `docs-show`) + Chrome DevTools MCP (Lighthouse a11y, a11y tree, emulation of forced colours, contrast, reduced motion and dark mode, perf traces) | Lets the agent verify its own work before claiming done |
| App-level a11y | `@playwright/test` + `@axe-core/playwright` (`withTags([... 'wcag22aa'])`) against the Vite dev server with Tauri IPC mocked; run on **WebKit** as well as Chromium, since WebKit is the closest engine to WKWebView | Whole-screen issues; 1.4.10 reflow at 320 px and 1.4.4 200% text checks as custom asserts |
| Real webview | `@wdio/tauri-service` + `@axe-core/webdriverio` (embedded WebDriver; works on macOS, unlike `tauri-driver`) | WebKit-only breakage inside the shipped app |
| Visual regression | `toMatchScreenshot` (Vitest) or `toHaveScreenshot` (Playwright), baselines generated **only** inside a pinned `mcr.microsoft.com/playwright:v1.64.1-noble` image, bundled fonts, animations off | Pixel drift. Baselines are per-OS by file name, so macOS and Linux never match |
| Token lint | Rust engine test: every theme × appearance passes 4.5:1 text, 3:1 non-text (1.4.11) and focus ring 3:1 (2.4.13 target) | What axe cannot check: non-text contrast of custom states |

**Design rules to encode** in a purlis UI skill or the component layer:
- **Targets:** 24×24 CSS px minimum (2.5.8); the macOS HIG uses 28 pt default and 20 pt minimum for controls. Fluent sizes are 24, 32 and 40 px.
- **Focus:** a 2 px outline at 3:1 (2.4.13 as the target). Use `scroll-padding` so focus never hides under sticky footers (2.4.11).
- **Dialogs:**
  - a 3-row grid: title, a scrolling content region, actions;
  - max height = viewport minus 48 px; max width 600 px;
  - 8 px gaps; at most three actions; no nested dialogs;
  - on macOS, the default button trailing and Cancel leading. The default button responds to Return; Esc and Cmd-. cancel.
- **Forms:** group related fields. Right-aligned "Label:" columns are a legacy OS X convention, not current HIG guidance, so choose one deliberately.
- Exit keys for embedded terminals and editors that capture Tab (2.1.2).

## Agent skills to install

Commands only; nothing was installed. Each one is a decision for the operator or a ticket.

```sh
# 1. Verify UI in a real browser: a11y tree, Lighthouse a11y, emulation, perf (Google, official marketplace)
/plugin install chrome-devtools-mcp@claude-plugins-official
#    The plugin route has no flags, so turn off Google usage stats with an env var in your shell profile:
export CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS=1
#    Or skip the plugin and install the MCP alone with both opt-outs (no skills):
#    claude mcp add chrome-devtools --scope user -- npx chrome-devtools-mcp@latest --no-usage-statistics --no-performance-crux

# 2. Modern CSS/HTML and a11y guidance, Baseline-aware (matters for WKWebView) (Google Chrome team; preview)
/plugin install modern-web-guidance@claude-plugins-official

# 3. Docs for the chosen primitives library (Adobe)
claude mcp add react-aria --scope project -- npx @react-aria/mcp@latest

# 4. Once Storybook is in the repo (made through a ticket): the component-aware MCP
npx storybook@10.6.3 add @storybook/addon-mcp
claude mcp add --transport http storybook http://localhost:6006/mcp --scope project

# 5. Optional: code intelligence for both halves (Anthropic)
/plugin install typescript-lsp@claude-plugins-official
/plugin install rust-analyzer-lsp@claude-plugins-official
```

**Keep from what is installed:**
- `superpowers:verification-before-completion`: the evidence gate that forces the a11y and visual runs.
- `mattpocock-skills:prototype` (UI.md): variants on the real route.
- `figma:*`: only when there is a Figma source. Code Connect needs an Org/Enterprise plan.
- `frontend-design`: for aesthetic direction only. It is written for marketing pages, not dense desktop UI, and its accessibility guidance is one line.

**Skip:**
- `anthropics/skills` `webapp-testing`: Python, which breaks the no-language-mixing rule.
- Deque axe MCP: paid; the free `axe-core` covers the same rules.
- `hypothesi/mcp-server-tauri`: third-party, unvetted, and it adds a bridge crate to the app.
- Playwright MCP: duplicates Chrome DevTools MCP. Use `@playwright/test` for WebKit runs.
- Vercel `web-design-guidelines` as shipped: it fetches unpinned remote instructions on every run. Vendor a pinned copy if you want its rules.
- Context7: a third-party index, less authoritative than vendor `llms.txt` and MCPs.
- Ladle: no release in 11.5 months.
- `vitest-axe` (2022) and `lost-pixel` (2024): stale.

**The real gap is a purlis UI skill.** None of the installed or third-party skills combines APG patterns, the purlis component layer, the dialog/form/target rules above and the verification commands. Write one, `purlis-ui`, that:
- points at the RAC and Storybook MCPs;
- requires a story with a `play` function for every interactive component;
- requires `npx vitest run --project=storybook` and the Playwright a11y spec to pass before "done".

This is the highest-leverage item, and it is an in-repo change: a ticket, not an install.
