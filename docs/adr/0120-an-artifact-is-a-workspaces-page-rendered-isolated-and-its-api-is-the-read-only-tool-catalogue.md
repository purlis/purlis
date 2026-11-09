# An artifact is a workspace's page, rendered isolated, and its API is the read-only tool catalogue

**Accepted 2026-11-09**, from the operator's grill of the same day, once the isolation prototype
(#5949, `tools/artifact-isolation-probe`) had chosen between the two renderings in §3: **a
separate native webview with an empty grant**. Its five tests are green for it on macOS, run by
hand on 2026-11-09; the sandboxed frame failed two of them there. Linux and Windows run the same
tests in CI (`.github/workflows/artifact-isolation.yml`), and until they are green on a platform,
an artifact with script is refused there and renders script-free (§10).

Until now nothing a person, a chat, a skill or an extension made could be *rendered* in the
window. ADR 0041 kept code out of the tree purlis draws its consent prompts into; ADR 0043 made
a panel data, never markup, and refused a declared row a verb; ADR 0052 set `frame-src 'none'`
("the app renders no frames"). ADR 0081's E6 already foresaw "a capability-gated webview
(sandboxed iframe, strict CSP, message API)", and ADR 0101 §6 deferred webview panels to
"PE-39's isolation", which no record defined. The operator wants a page made by anyone, such as
the backlog-sync skill's state report, behind an icon the whole team sees, showing live values,
and in time able to use any purlis feature and any extension's data. This record makes that
change to the boundary openly, and keeps ADR 0041's and ADR 0043's properties by moving them to
a new edge rather than giving them up.

## The decision

### 1. What an artifact is

**An artifact is an HTML template in `workspaces/<ws>/artifacts/<name>/` that someone pinned to
the workspace.** It is a part of **Workspace** (ADR 0072). Pinning it to the workspace makes it
an artifact and unpinning ends it; the files stay. The folder is a new committed store, added to
the live-workspace block of the plane's ignore list, with a `**Tier:**` line in
`docs/plane-format.md`: *Plane* when the workspace is LIVE, *Clone state* when it is LOCAL. In a
LOCAL workspace an artifact works and the pin dialog says it stays on this machine.

**The template is shared; the data is each person's.** A template may read data files beside
it (`data.json` and the like). Those are inert JSON, gitignored, and written on each machine by
that person's **Regenerate** (§6). Nothing a page shows is written to the plane by showing it.
A machine with no data yet shows "Not generated on this machine" with Regenerate.

**The pins file** (`workspaces/<ws>/artifacts/pins.toml`, versioned) holds one entry per pin:
the folder, a name, an icon from purlis's icon set (never the page's own image), the order, and
the producing command if any. A chat or skill only proposes a pin; a person confirms it, and the
plane's save shares it. Pin to workspace confirms "visible to everyone in <plane>". Any change
to a pin's folder or command is a **new pin**: it is not trusted on any machine until confirmed
there, and the bar names who changed it, from git history.

### 2. Where it shows

**Each pinned artifact has its own icon on its workspace's left activity bar, in a group of its
own below purlis's icons, after a divider.** Membership and order are shared; purlis's own icons
never move. At most eight are drawn; any more that a merge produced go to the bar's show-more,
never dropped. A press opens the artifact in a **private tab**: a view tab,
`ViewRef { from: null, view: "artifact", key: <folder> }`, through `open_view`. Tabs, and a
person's own **Pin**, stay machine state (ADR 0040). It is never the file tab's preview
(ADR 0081:126). A restored artifact tab runs nothing and asks nothing until pressed (ADR 0043,
amended 2026-09-23).

### 3. How it is rendered: the isolation PE-39 named

**An artifact never runs in the window's own document.** It runs in **a separate native webview
whose label matches no capability glob, so it holds an empty grant**. The prototype weighed it
against a sandboxed iframe (`sandbox="allow-scripts"`, no `allow-same-origin`, opaque origin) and
chose it on evidence (below). It must pass, on each platform, tests that go red when:

1. the page reaches any Tauri command, the vault-values commands among them;
2. an endless loop in the page stops the window responding;
3. a second artifact or a nested frame can speak on the page's message channel;
4. a script-free page loads or navigates to anything off its folder, through `<meta refresh>`,
   `<base>`, a form, CSS `@import` or `url()`, SVG, prefetch or `file:`;
5. a request walks out of the folder (`..`, `%2e%2e`, backslashes, case folding, a symlink
   planted after the pin).

**Every artifact has its own origin**, so no two share storage. **purlis serves only the bytes
it hashed** (§4), from memory, under a strict CSP on every response: `default-src` its own
scheme, `connect-src 'none'`, `form-action 'none'`, `base-uri 'none'`, `frame-src 'none'`.
**There is no network, ever, approved or not.** The page talks to purlis only on one channel
per load, revoked on reload, which purlis attributes by the label of the webview it came from,
never by anything the page says: a second artifact is another webview, and a nested frame cannot
load under `frame-src 'none'`. A page navigates only within its own folder: the webview's
navigation handler refuses anything else before a request is made, `<meta refresh>` included.

**The evidence, macOS, 2026-11-09** (`tools/artifact-isolation-probe/results/macos-2026-11-09.txt`).
The native webview passed all five. With its guards taken off one by one (the label granted the
window's commands, no CSP, no navigation guard, files read from disk at request time), tests 1,
3, 4 and 5 each failed for their own reason, so each tests its guard. The sandboxed frame passed
1, 3 and 5 and failed two:

- **2:** the window's own script stopped answering while the frame looped. WebKit runs the frame in
  the window's process and thread, so an artifact could freeze purlis.
- **4:** a `<meta refresh>` took the frame to a sibling artifact's file. The window's `frame-src`
  must admit the artifact scheme for the frame to load at all, and a CSP source cannot tell one
  artifact's origin from another's.

**A page cannot be mistaken for purlis.** Every artifact tab carries a bar purlis draws outside
the page: who pinned it, approved or not, freshness, errors, and Regenerate. Approval is given
only there, and the frame's border changes with its state, which the page cannot draw.

### 4. Who approves what

**A page with no executable script opens without asking.** (`<script type="application/json">`
is data, not script.) **A page with script, or one that uses the API, is approved by each
person on their own machine**, and that approval is over **the whole template folder**: every
path in it and every byte. The data files are not covered; they are inert. A change to the
folder tears down the running page and its port first, renders it again with scripts off, and
asks again. Approvals are machine state, and every window hears when one changes. A teammate's
approval never counts as yours. A fork of the workspace asks again.

This amends ADR 0041's "a plugin never travels in a plane" for artifacts only, and only under
this per-person, per-version approval.

### 5. Links and Discuss

Links open the system browser for `https:` only, after showing the host and the whole URL, with
a per-host "always open" for each person; `purlis://` links navigate inside purlis and never act.
A link a script built is refused. **Discuss** opens a chat with the artifact attached as quoted
third-party content, with its pinner and commit, and sends nothing by itself.

### 6. Regenerate

The pin may name the command that produces its data and its working folder. **Regenerate runs
in charterd, in the sandbox a chat's commands run in, never in the app.** The approval covers
what the command resolves to (its skill or script folder, fingerprinted), not the command's
text. It writes to a temporary file and renames it into place, and the bar shows progress,
its error output and "last good: <date>". Freshness comes from a date the data declares, else
the file's commit or write time.

### 7. The API is the read-only tool catalogue

**A page query is the same thing as a read-only MCP tool.** There is one catalogue; MCP, the CLI
and pages are three ways to reach it, and an extension contributes into that same catalogue.
"Any purlis feature" from a page means any read-only tool, written once.

- Every query is `<source>.<name>`: `purlis.*` for the core, `<extension-id>.*` for an
  extension (whose id is never a core word, ADR 0053). Ids are permanent once released; a rename
  is a new id declaring what it `replaces`. A query has `since` and may be `deprecated`, still
  answering for at least one release and named in the bar.
- A page declares `api: N` and the queries it uses (`name@N`) in a block covered by its
  approval. The prompt lists them; an undeclared call is refused. purlis keeps answering every
  `N` it has shipped.
- Results are described in the JSON Schema subset MCP tools already use, never per query
  freeform, and are data, never markup. Every answer is
  `{ data | error: { kind: needs-extension | not-approved | timeout | too-large | refused | failed }, at, cursor? }`,
  under fixed size caps.
- Parameters are declared with closed types and checked by purlis. A parameter that widens what
  a page reads (a channel, a repo) is named in the approval. A query is `workspace` or `plane`
  in scope, and plane scope is a grant line of its own.
- **Live:** a page subscribes. purlis pushes a new answer on core events or when an extension's
  facts file changes. A live query must be cacheable: answers are shared across pages for the
  same query and parameters, at most one run is in flight, and an extension's query declares
  `fresh_seconds`, which purlis asks again on while someone subscribes. The facts file gains a
  per-workspace key. Extensions are still never long-running (ADR 0053).
- **A missing extension** answers `needs-extension`; the page still renders and the bar says so.
  **The page is shared; each viewer's own purlis, extensions and logins answer.**
- Authors get `purlis queries` (and its MCP twin) listing every query with its shape, a
  generated `purlis-page.d.ts`, a fixture-backed mock, and `purlis artifact check <folder>`.

### 8. Actions are offers

**A page never performs an action.** It may *offer* one: a `Does` id from `actions.ts`'s
catalogue and a subject. purlis draws the offer in its own bar and the person presses it; a
delete always asks. This is ADR 0043's rule that nothing in the window acts on another's say-so,
kept.

### 9. Extensions contribute artifacts and queries

Under ADR 0053, two new capability words, each with its own amendment there: **`queries`**
(declared ids, shapes, parameters, scope, freshness) and **`artifacts`** (an extension ships
templates that a person may pin to a workspace like any other). Both need the protocol version
that introduces them.

### 10. Windows and the budget

Extensions do not run on Windows (ADR 0031, ADR 0053): there, `queries` and extension artifacts
are refused, not answered `needs-extension`. Artifacts with script render on Windows only once
the §3 tests pass on WebView2; until then they render script-free. ADR 0086 gains a row for the
memory of an open artifact tab, with the job that measures it.

## What this changes in other records

Each change is an amendment file of its own, written when this record is accepted:

- **ADR 0038**: the left activity bar gains a second kind of item, an artifact icon that opens a
  tab, in its own group that never moves purlis's icons.
- **ADR 0040**: a pin to workspace is a deliberate team act saved like any change, not an
  arrangement; the unqualified Pin is unchanged.
- **ADR 0041**: an artifact may travel in a plane under §4; decision 2's DOM row holds because the
  page never reaches the window's document.
- **ADR 0043**: the panel vocabulary is unchanged; an artifact is not a panel.
- **ADR 0052**: the window's `frame-src` stays `'none'`; the artifact's webview is a label no
  capability names, held so by a test.
- **ADR 0081 E6**: realised by this record, and approved per page, not per extension.
- **ADR 0101 §6**: PE-39 is §3 here.

## Considered options

- **Widen the panel vocabulary instead.** Rejected: an artifact is made by people and chats in
  plain HTML, and the operator's case needs layouts no closed block set will keep up with.
- **Approve the page's exact bytes.** Rejected: a page regenerated daily would ask daily, and a
  prompt clicked by reflex is worse than none (ADR 0041). Hence template and data apart.
- **Commit each person's data.** Rejected: whoever regenerated last would decide everyone's view,
  and two people regenerating would conflict on the shared plane.
- **One icon for an Artifacts list.** Rejected by the operator: each pinned page is a destination.
- **A separate page API beside MCP and the CLI.** Rejected: three surfaces would drift, the
  failure `actions.ts` exists to prevent.
- **A page that performs actions with a grant.** Rejected for offers, which keep the person's
  press on every act.
- **A sandboxed iframe in the window's document.** Rejected on the prototype's evidence (§3): on
  WebKit an endless loop in it froze the window, and the window's `frame-src` let it navigate to
  another artifact. It would also have needed `frame-src` opened in the window's own CSP.
