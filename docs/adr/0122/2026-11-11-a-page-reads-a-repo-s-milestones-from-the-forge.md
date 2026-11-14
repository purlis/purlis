# A page reads a repo's milestones from the forge

**2026-11-11, #6233.** This amends §7 of ADR 0122 with the first core query that reads the
network. It also amends the chat tool boundary of `chattools` (ADR 0077) and records the
operator's rulings of 2026-11-11 and 2026-11-12.

The operator wanted a pinned page that shows a repo's GitHub milestones live and updates by
itself. Their ruling was *"Live query only"*: a catalogue query, with no snapshot written for the
page to read.

- **`purlis.forge_milestones`, since 0.5.2.** It is a core id under `purlis.*`. `forge.*` would
  be an extension's source (§7): purlis would answer it `needs-extension`, and the word would no
  longer be free for an extension's id. It takes `repo`, which widens what it reads and is named
  in the approval, and `state`, which is `open` (the default) or `all`. Its data is `repo`, `path`,
  `state`, `cut` and `milestones`. Each milestone carries `number`, `title`, `description`,
  `due_on`, `state`, `open_issues`, `closed_issues`, `url` and `updated_at`. purlis reads three
  pages of milestones at most (300), and `cut` is `true` when the last page read was full, so
  GitHub may list more.
- **Only a repo of the asking workspace, and only the one the project records** (the
  operator's ruling of 2026-11-12, *"recorded repo only"*). The repo is a clone of that
  workspace, named by its folder, or the workspace's only clone when none is named. Its forge
  path is the one the project's inventory (`inventory/repos.json`, which no chat writes)
  records under the clone's name, on a GitHub host the project declares. The clone's live
  `origin` is not trusted on its own, because anything that writes the clone's `.git/config` can
  point it elsewhere: it must name the same forge, host and path, in any case, or the ask is
  `refused` with a sentence that names neither repo. A caller never writes an owner/repo. Any
  other name, a clone the inventory has no record for, a clone with no such origin, and a clone
  on GitLab are each `refused` before anything is asked.
- **A page asks as the person in the window, in the background** (the operator's ruling of
  2026-11-11). Its caller is `Caller::window().background()`. ADR 0070's route sends that on the
  person's sign-in when there is one, and otherwise on their `gh` login. **What is metered, on
  each route** (the operator's ruling of 2026-11-12, *"meter the gh route"*):
  - **On a sign-in**, each request is admitted and counted by that account's hourly budget
    (`<config>/forge-budget/<account>.json`, ADR 0070 §6), which holds a background request back
    once the hour is spent, and goes through the account's ETag store (ADR 0070 §3), so an
    unchanged answer is a `304` that GitHub does not count.
  - **On the `gh` login**, purlis asks `gh` which account it is signed in as (`user`, once an
    hour per forge host, itself counted), and sends the request as that account. The same
    hourly budget of that account admits and counts it, and holds it back once the hour is spent,
    as it would a sign-in of that login. The ETag store does not apply: it serves the native
    transport alone, and an answer a sign-in's token fetched is never handed to a CLI call, which
    may be logged in as someone else. Every request on this route is counted. Where `gh` does
    not say who it is, the request is sent unmetered, as before, and fails with the sentence for
    having no login if `gh` has none.

  The token stays in the transport. The page is handed data and never a credential, and no new
  store is made.
- **A chat asks as itself** (the operator's ruling of 2026-11-11). The MCP tool
  `forge_milestones` asks as `Caller::mcp(<chat>)`. ADR 0070's route sends that on the chat's own
  CLI login, and refuses it where that login was imported as the person's. Before this,
  `chattools` read no forge at all. Now it reads this one, and only as the chat. The answer is
  the same read-only, workspace-scoped one a page gets. A chat could already run `gh api` itself,
  so nothing is reached that the chat could not already reach.
- **Why it is safe for a page.** It only reads. It is scoped to the asking workspace's own clones.
  It asks with the person's own login, through the route that already decides every forge call.
  Its answer is data in the catalogue's envelope and is never markup. The page still has no
  network of its own: purlis reads GitHub, and the page reads purlis on its port, as before.
- **An error may carry a sentence**: `error: { kind, message? }`. This adds to api 1 and changes
  nothing a page already reads. Having no login, being offline, a spent limit, a repo GitHub does
  not show, and a login that may not read it each answer `failed` with a sentence that follows
  `docs/ui-copy.md`. A repo or state the query does not take is `refused` with one. The forge's
  own words stay out of the message: a page shows it, and those words could say more than the
  reader needs.
- **`fresh_seconds` on a core query.** A query may declare how long its answer stays good, and
  `purlis queries` lists it. `purlis.forge_milestones` declares 60. No change of the plane moves
  it, and a subscription to it is asked again once its freshness has passed. That is at most once
  a minute per feed, however many pages share the feed, and however often a page
  subscribes, lets go and subscribes again: a feed with a freshness is counted by its key, and
  one left with no subscriber is kept, with its last run and answer, until its freshness has
  passed, so a new subscription within it is handed that answer and starts no run.
